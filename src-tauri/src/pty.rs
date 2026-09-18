use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct PtyState {
    session: Mutex<Option<PtySession>>,
    next_id: AtomicU64,
}

struct PtySession {
    /// Identifies which spawn this is, so the reader thread below can tell a
    /// live session from one that has since been killed and replaced.
    id: u64,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Default for PtyState {
    fn default() -> Self {
        PtyState {
            session: Mutex::new(None),
            next_id: AtomicU64::new(0),
        }
    }
}

/// Builds the shell command for a PTY session. When `cwd` is given, the
/// child process actually starts there. This is the fix for issue #13:
/// previously `CommandBuilder` never had `.cwd()` called on it at all, so the
/// shell silently inherited whatever directory the Tauri process itself
/// happened to launch from, with no relationship to a folder opened in the
/// app. `SIDECAR_REPO` is exported alongside it so a tool that reads env
/// rather than the process's actual cwd (an LLM CLI asked "which repo am I
/// in") has the same answer available.
fn configure_command(shell: String, cwd: Option<&str>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    if let Some(dir) = cwd {
        cmd.cwd(dir);
        cmd.env("SIDECAR_REPO", dir);
    }
    cmd
}

/// True if `id` still names the session live in `state`. The background
/// reader thread checks this before emitting anything, so a thread left over
/// from a killed/replaced session (see `pty_restart`) can never emit stale
/// output or a spurious exit notice into the new session's stream.
fn is_current(state: &PtyState, id: u64) -> bool {
    matches!(state.session.lock(), Ok(guard) if guard.as_ref().map(|s| s.id) == Some(id))
}

fn spawn_session(
    app: &AppHandle,
    state: &PtyState,
    cwd: Option<&str>,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let cmd = configure_command(shell, cwd);
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| e.to_string())?;

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let id = state.next_id.fetch_add(1, Ordering::SeqCst) + 1;

    let emit_handle = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let pty_state = emit_handle.state::<PtyState>();
                    if !is_current(&pty_state, id) {
                        break;
                    }
                    let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                    if emit_handle.emit("pty-output", chunk).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let pty_state = emit_handle.state::<PtyState>();
        if is_current(&pty_state, id) {
            let _ = emit_handle.emit("pty-exit", ());
        }
    });

    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    *guard = Some(PtySession {
        id,
        master: pair.master,
        writer,
        child,
    });

    Ok(())
}

#[tauri::command]
pub fn pty_spawn(app: AppHandle, state: State<PtyState>, cols: u16, rows: u16) -> Result<(), String> {
    {
        let guard = state.session.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Ok(());
        }
    }
    spawn_session(&app, &state, None, cols, rows)
}

/// Kills whatever session is currently running (if any) and starts a fresh
/// one rooted at `path`. Called when a folder is opened, so the terminal
/// (and anything launched inside it, such as an LLM CLI) always reports the
/// repo actually open in the app, not wherever sidecar itself started from.
#[tauri::command]
pub fn pty_restart(
    app: AppHandle,
    state: State<PtyState>,
    path: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    {
        let mut guard = state.session.lock().map_err(|e| e.to_string())?;
        if let Some(mut session) = guard.take() {
            let _ = session.child.kill();
        }
    }
    spawn_session(&app, &state, Some(&path), cols, rows)
}

#[tauri::command]
pub fn pty_write(state: State<PtyState>, data: String) -> Result<(), String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    if let Some(session) = guard.as_mut() {
        session
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| e.to_string())?;
        session.writer.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<PtyState>, cols: u16, rows: u16) -> Result<(), String> {
    let guard = state.session.lock().map_err(|e| e.to_string())?;
    if let Some(session) = guard.as_ref() {
        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn pty_kill(state: State<PtyState>) -> Result<(), String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    if let Some(mut session) = guard.take() {
        let _ = session.child.kill();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Control: with no folder open yet, the command must not force a
    /// directory (the shell keeps inheriting the process's own cwd, which is
    /// the existing pre-#13 startup behaviour and must not regress).
    #[test]
    fn no_cwd_leaves_command_directory_unset() {
        let cmd = configure_command("/bin/bash".to_string(), None);
        assert_eq!(cmd.get_cwd(), None);
        assert!(cmd.iter_extra_env_as_str().next().is_none());
    }

    /// This is the control that must fail on the pre-fix code: issue #13's
    /// root cause was exactly that `.cwd()` was never called, so this
    /// assertion would see `None` against the old `CommandBuilder::new(shell)`
    /// with no further configuration.
    #[test]
    fn cwd_sets_directory_and_exports_env_var() {
        let cmd = configure_command("/bin/bash".to_string(), Some("/tmp/some-repo"));
        assert_eq!(
            cmd.get_cwd().map(|s| s.to_string_lossy().into_owned()),
            Some("/tmp/some-repo".to_string())
        );
        let envs: Vec<_> = cmd.iter_extra_env_as_str().collect();
        assert!(envs.contains(&("SIDECAR_REPO", "/tmp/some-repo")));
    }
}
