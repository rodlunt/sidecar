use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::Mutex;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

pub struct WatcherState(Mutex<Option<notify::RecommendedWatcher>>);

impl Default for WatcherState {
    fn default() -> Self {
        WatcherState(Mutex::new(None))
    }
}

#[derive(Serialize, Clone)]
pub struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
}

#[tauri::command]
pub fn read_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let mut entries: Vec<FileEntry> = std::fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with('.')
        })
        .map(|entry| {
            let file_type = entry.file_type().ok();
            FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
                is_dir: file_type.map(|t| t.is_dir()).unwrap_or(false),
            }
        })
        .collect();

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

#[tauri::command]
pub fn watch_root(
    app: AppHandle,
    state: State<WatcherState>,
    path: String,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(tx).map_err(|e| e.to_string())?;
    watcher
        .watch(Path::new(&path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    *guard = Some(watcher);

    std::thread::spawn(move || {
        let mut last_emit = std::time::Instant::now() - Duration::from_secs(1);
        for res in rx {
            if res.is_err() {
                continue;
            }
            // Debounce: filesystem watchers fire multiple events per change
            // (e.g. a save = modify + create + modify). Coalesce into one
            // "something changed" signal at most every 200ms.
            if last_emit.elapsed() < Duration::from_millis(200) {
                continue;
            }
            last_emit = std::time::Instant::now();
            if app.emit("fs-changed", ()).is_err() {
                break;
            }
        }
    });

    Ok(())
}
