use std::path::{Path, PathBuf};
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

/// The folder opened via "Open folder", set by `watch_root`. `read_file`
/// checks every request against this so the frontend can only ever preview
/// a file inside the workspace the user actually opened, even if something
/// running in the webview (e.g. a compromised or malicious rendering of
/// GitHub issue content) tried to invoke `read_file` with an arbitrary path.
pub struct WorkspaceRootState(Mutex<Option<PathBuf>>);

impl Default for WorkspaceRootState {
    fn default() -> Self {
        WorkspaceRootState(Mutex::new(None))
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

/// A file's contents for the preview panel, or a marker that it isn't text
/// or wasn't safe/sensible to read in full. Kept as a tagged result rather
/// than a bare string so the frontend never has to guess (e.g. by sniffing
/// for control characters itself) whether what came back is renderable.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileContent {
    Text { content: String },
    Binary,
    TooLarge { size: u64 },
}

/// How much of the file's start to inspect for binary tells before
/// committing to a full UTF-8 decode. 8KiB is enough to catch the NUL
/// byte or invalid-UTF-8 lead-in that a real text file would never have,
/// without scanning a large binary twice.
const SNIFF_LEN: usize = 8192;

/// Preview is for skimming source, not for opening arbitrary large files.
/// Also bounds how much memory a single read_file call can pull in.
const MAX_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;

/// Resolves `path` and `root` to their real, symlink-free locations and
/// checks the former is inside the latter. Canonicalizing both (rather than
/// just prefix-comparing the raw strings) is what actually stops `..`
/// traversal and a symlink planted inside the workspace pointing outside it.
fn ensure_within_root(path: &str, root: &Path) -> Result<PathBuf, String> {
    let canonical_root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let canonical_path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    if canonical_path.starts_with(&canonical_root) {
        Ok(canonical_path)
    } else {
        Err("path is outside the open workspace".to_string())
    }
}

fn read_file_checked(path: &str, root: Option<&Path>) -> Result<FileContent, String> {
    let root = root.ok_or_else(|| "no workspace folder is open".to_string())?;
    let checked_path = ensure_within_root(path, root)?;

    let size = std::fs::metadata(&checked_path)
        .map_err(|e| e.to_string())?
        .len();
    if size > MAX_PREVIEW_BYTES {
        return Ok(FileContent::TooLarge { size });
    }

    let bytes = std::fs::read(&checked_path).map_err(|e| e.to_string())?;

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
pub fn read_file(path: String, state: State<WorkspaceRootState>) -> Result<FileContent, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    read_file_checked(&path, guard.as_deref())
}

#[tauri::command]
pub fn watch_root(
    app: AppHandle,
    state: State<WatcherState>,
    root_state: State<WorkspaceRootState>,
    path: String,
) -> Result<(), String> {
    let canonical_root = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    *root_state.0.lock().map_err(|e| e.to_string())? = Some(canonical_root);

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

        let result = read_file_checked(&path.to_string_lossy(), Some(&dir)).unwrap();
        match result {
            FileContent::Text { content } => assert_eq!(content, "hello world\n"),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn detects_binary_file_via_nul_byte() {
        let dir = temp_dir();
        let path = dir.join("blob.bin");
        std::fs::write(&path, [0x00u8, 0x01, 0x02, 0xff, 0xfe]).unwrap();

        let result = read_file_checked(&path.to_string_lossy(), Some(&dir)).unwrap();
        assert!(matches!(result, FileContent::Binary));
    }

    #[test]
    fn detects_binary_file_via_invalid_utf8_without_nul() {
        let dir = temp_dir();
        let path = dir.join("invalid-utf8.bin");
        // 0xC3 0x28 is not a valid UTF-8 sequence, and contains no NUL byte,
        // so this exercises the from_utf8 fallback rather than the sniff.
        std::fs::write(&path, [0xC3u8, 0x28, 0x41, 0x42]).unwrap();

        let result = read_file_checked(&path.to_string_lossy(), Some(&dir)).unwrap();
        assert!(matches!(result, FileContent::Binary));
    }

    #[test]
    fn read_error_for_missing_path() {
        let dir = temp_dir();
        let missing = dir.join("does-not-exist.txt");

        let result = read_file_checked(&missing.to_string_lossy(), Some(&dir));
        assert!(result.is_err());
    }

    #[test]
    fn rejects_path_outside_workspace_root() {
        let workspace = temp_dir();
        let outside = temp_dir();
        let secret = outside.join("secret.txt");
        std::fs::write(&secret, "not part of the opened folder").unwrap();

        // This is the control for the arbitrary-file-read finding: before
        // the root check existed, this call would happily return the
        // outside file's contents. It must now be rejected.
        let result = read_file_checked(&secret.to_string_lossy(), Some(&workspace));
        assert!(result.is_err(), "expected a path outside the workspace root to be rejected");
    }

    #[test]
    fn rejects_traversal_via_dot_dot() {
        let workspace = temp_dir();
        let outside = temp_dir();
        let secret = outside.join("secret.txt");
        std::fs::write(&secret, "not part of the opened folder").unwrap();

        let traversal_path = workspace
            .join("..")
            .join(outside.file_name().unwrap())
            .join("secret.txt");

        let result = read_file_checked(&traversal_path.to_string_lossy(), Some(&workspace));
        assert!(result.is_err(), "expected a ../ escape from the workspace root to be rejected");
    }

    #[test]
    fn rejects_read_when_no_workspace_open() {
        let dir = temp_dir();
        let path = dir.join("hello.txt");
        std::fs::write(&path, "hello world\n").unwrap();

        let result = read_file_checked(&path.to_string_lossy(), None);
        assert!(result.is_err());
    }

    #[test]
    fn reports_oversized_file_without_reading_it_fully() {
        let dir = temp_dir();
        let path = dir.join("huge.bin");
        // Sparse-ish large file: content doesn't matter, only that it
        // exceeds MAX_PREVIEW_BYTES and gets flagged rather than read.
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_PREVIEW_BYTES + 1).unwrap();

        let result = read_file_checked(&path.to_string_lossy(), Some(&dir)).unwrap();
        match result {
            FileContent::TooLarge { size } => assert_eq!(size, MAX_PREVIEW_BYTES + 1),
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }
}
