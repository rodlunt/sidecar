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

/// A file's contents for the preview panel, or a marker that it isn't text.
/// Kept as a tagged result rather than a bare string so the frontend never
/// has to guess (e.g. by sniffing for control characters itself) whether
/// what came back is renderable.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileContent {
    Text { content: String },
    Binary,
}

/// How much of the file's start to inspect for binary tells before
/// committing to a full UTF-8 decode. 8KiB is enough to catch the NUL
/// byte or invalid-UTF-8 lead-in that a real text file would never have,
/// without scanning a large binary twice.
const SNIFF_LEN: usize = 8192;

#[tauri::command]
pub fn read_file(path: String) -> Result<FileContent, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;

    let sniff_len = bytes.len().min(SNIFF_LEN);
    if bytes[..sniff_len].contains(&0) {
        return Ok(FileContent::Binary);
    }

    match String::from_utf8(bytes) {
        Ok(content) => Ok(FileContent::Text { content }),
        Err(_) => Ok(FileContent::Binary),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sidecar-fs-test-{}", unique()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn unique() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    #[test]
    fn reads_text_file_contents() {
        let dir = temp_dir();
        let path = dir.join("hello.txt");
        std::fs::write(&path, "hello world\n").unwrap();

        let result = read_file(path.to_string_lossy().into_owned()).unwrap();
        match result {
            FileContent::Text { content } => assert_eq!(content, "hello world\n"),
            FileContent::Binary => panic!("expected text, got binary"),
        }
    }

    #[test]
    fn detects_binary_file_via_nul_byte() {
        let dir = temp_dir();
        let path = dir.join("blob.bin");
        std::fs::write(&path, [0x00u8, 0x01, 0x02, 0xff, 0xfe]).unwrap();

        let result = read_file(path.to_string_lossy().into_owned()).unwrap();
        assert!(matches!(result, FileContent::Binary));
    }

    #[test]
    fn detects_binary_file_via_invalid_utf8_without_nul() {
        let dir = temp_dir();
        let path = dir.join("invalid-utf8.bin");
        // 0xC3 0x28 is not a valid UTF-8 sequence, and contains no NUL byte,
        // so this exercises the from_utf8 fallback rather than the sniff.
        std::fs::write(&path, [0xC3u8, 0x28, 0x41, 0x42]).unwrap();

        let result = read_file(path.to_string_lossy().into_owned()).unwrap();
        assert!(matches!(result, FileContent::Binary));
    }

    #[test]
    fn read_error_for_missing_path() {
        let dir = temp_dir();
        let missing = dir.join("does-not-exist.txt");

        let result = read_file(missing.to_string_lossy().into_owned());
        assert!(result.is_err());
    }
}
