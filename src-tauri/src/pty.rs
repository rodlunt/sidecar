use std::io::{Read, Write};
use std::sync::Mutex;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::transcript::{self, TranscriptState, TranscriptWriter};

pub struct PtyState(Mutex<Option<PtySession>>);

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Default for PtyState {
    fn default() -> Self {
        PtyState(Mutex::new(None))
    }
}

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    state: State<PtyState>,
    transcript_state: State<TranscriptState>,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Ok(());
    }

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
    let cmd = CommandBuilder::new(shell);
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| e.to_string())?;

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    // Resolved once per session, not per read: a transcript is one file for
    // the whole PTY session (see transcript::session_transcript_path). A
    // failure here (e.g. the app data dir can't be created) disables the
    // transcript for this session only, rather than failing the terminal
    // spawn itself, since a missing transcript is degraded, not fatal.
    let transcript_path = match app.path().app_data_dir() {
        Ok(dir) => Some(transcript::session_transcript_path(&dir)),
        Err(e) => {
            eprintln!("sidecar: could not resolve app data dir for transcript: {e}");
            None
        }
    };
    let transcript_enabled = transcript_state.enabled.clone();

    let emit_handle = app.clone();
    std::thread::spawn(move || {
        let mut transcript_writer = transcript_path.map(TranscriptWriter::new);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(writer) = transcript_writer.as_mut() {
                        transcript::maybe_write(writer, &transcript_enabled, &buf[..n]);
                    }
                    let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                    if emit_handle.emit("pty-output", chunk).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = emit_handle.emit("pty-exit", ());
    });

    *guard = Some(PtySession {
        master: pair.master,
        writer,
        child,
    });

    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<PtyState>, data: String) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
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
    let guard = state.0.lock().map_err(|e| e.to_string())?;
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
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut session) = guard.take() {
        let _ = session.child.kill();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// End-to-end check that mirrors `pty_spawn`'s reader loop against a
    /// real shell (no Tauri `AppHandle` involved, so it runs headless): a
    /// real PTY session's raw output must land in the transcript file, byte
    /// for byte, the same way it would be handed to xterm. This is the
    /// closest a unit test gets to acceptance criterion 2 without an actual
    /// GUI session.
    #[test]
    fn transcript_captures_real_pty_output_end_to_end() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();

        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg("echo transcript-marker-42");
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().unwrap();

        let dir = std::env::temp_dir().join(format!(
            "sidecar-pty-integration-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let path = dir.join("session.log");
        let mut writer = TranscriptWriter::new(path.clone());
        let enabled = AtomicBool::new(true);

        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => transcript::maybe_write(&mut writer, &enabled, &buf[..n]),
                Err(_) => break,
            }
        }
        let _ = child.wait();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(
            contents.contains("transcript-marker-42"),
            "transcript did not capture real PTY output: {contents:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
