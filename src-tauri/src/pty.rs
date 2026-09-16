use std::io::{Read, Write};
use std::sync::Mutex;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter, State};

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
pub fn pty_spawn(app: AppHandle, state: State<PtyState>, cols: u16, rows: u16) -> Result<(), String> {
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

    let emit_handle = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
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
