use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

const SETTINGS_FILE: &str = "settings.json";
const TRANSCRIPTS_DIR: &str = "transcripts";

/// Shared, mutable "should we be writing a transcript right now" flag.
///
/// It is an `Arc<AtomicBool>` rather than a plain bool behind a `Mutex` so a
/// clone can be moved into the PTY reader thread: toggling the setting from
/// the UI flips the same atomic the reader thread checks on every read, so
/// turning transcript saving off takes effect immediately, mid-session, with
/// no PTY restart required. Turning it back on resumes appending to the same
/// session file rather than starting a new one.
pub struct TranscriptState {
    pub enabled: Arc<AtomicBool>,
}

impl TranscriptState {
    pub fn new(enabled: bool) -> Self {
        TranscriptState {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }
}

impl Default for TranscriptState {
    // Fail open to "on": if `.setup()` somehow never ran (it always should),
    // the managed default still matches the required default-on behaviour
    // rather than silently disabling transcript capture.
    fn default() -> Self {
        TranscriptState::new(true)
    }
}

#[derive(Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_transcript_enabled")]
    transcript_enabled: bool,
}

fn default_transcript_enabled() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            transcript_enabled: true,
        }
    }
}

fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(SETTINGS_FILE)
}

/// Loads the persisted toggle state. Any failure to read or parse the
/// settings file (missing on first run, corrupt, permissions issue) falls
/// back to the default of `true`. That is a deliberate fail-open: this
/// toggle guards a convenience feature, not a security boundary, and
/// "capture the session" is the safer default to fall back to than
/// silently going quiet.
fn load_settings(app_data_dir: &Path) -> Settings {
    match std::fs::read_to_string(settings_path(app_data_dir)) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

fn save_settings(app_data_dir: &Path, settings: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(settings_path(app_data_dir), json)
}

/// Reads the persisted transcript-enabled setting (defaulting to `true`).
/// Called once, from `.setup()`, to seed the managed `TranscriptState`.
pub fn load_transcript_enabled(app_data_dir: &Path) -> bool {
    load_settings(app_data_dir).transcript_enabled
}

/// A timestamped, per-session transcript path so relaunching sidecar never
/// clobbers the previous session's transcript. One file per PTY session is
/// enough: this issue is about having a record at all, not about splitting
/// output per command.
pub fn session_transcript_path(app_data_dir: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    app_data_dir
        .join(TRANSCRIPTS_DIR)
        .join(format!("session-{millis}.log"))
}

/// Appends raw bytes to a transcript file, opening (and creating the parent
/// directory) lazily on first write. Lazy open means a session that starts
/// disabled and gets toggled on mid-session still produces a file, without
/// creating an empty transcript for a session that never wanted one.
pub struct TranscriptWriter {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl TranscriptWriter {
    pub fn new(path: PathBuf) -> Self {
        TranscriptWriter { path, file: None }
    }

    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if self.file.is_none() {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
        }
        if let Some(file) = self.file.as_mut() {
            file.write_all(bytes)?;
        }
        Ok(())
    }
}

/// Writes `bytes` to `writer` only while `enabled` is true, and logs (never
/// silently swallows) a write failure instead of letting the terminal keep
/// running as though the transcript were fine. This is the single place the
/// PTY reader loop calls into, so the enabled-check and the write are never
/// duplicated or allowed to drift apart.
pub fn maybe_write(writer: &mut TranscriptWriter, enabled: &AtomicBool, bytes: &[u8]) {
    if !enabled.load(Ordering::Relaxed) {
        return;
    }
    if let Err(e) = writer.write(bytes) {
        eprintln!("sidecar: failed to write terminal transcript: {e}");
    }
}

#[tauri::command]
pub fn get_transcript_enabled(state: State<TranscriptState>) -> bool {
    state.enabled.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_transcript_enabled(
    app: AppHandle,
    state: State<TranscriptState>,
    enabled: bool,
) -> Result<(), String> {
    state.enabled.store(enabled, Ordering::Relaxed);
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    save_settings(
        &dir,
        &Settings {
            transcript_enabled: enabled,
        },
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sidecar-transcript-test-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_bytes_to_file_when_enabled() {
        let dir = unique_temp_dir("writes-enabled");
        let path = dir.join("session.log");
        let mut writer = TranscriptWriter::new(path.clone());
        let enabled = AtomicBool::new(true);

        maybe_write(&mut writer, &enabled, b"hello ");
        maybe_write(&mut writer, &enabled, b"world");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "hello world");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn suppresses_writes_when_disabled() {
        let dir = unique_temp_dir("writes-disabled");
        let path = dir.join("session.log");
        let mut writer = TranscriptWriter::new(path.clone());
        let enabled = AtomicBool::new(false);

        maybe_write(&mut writer, &enabled, b"should not appear");

        assert!(
            !path.exists(),
            "transcript file should not be created while disabled"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resumes_writing_to_same_file_after_being_reenabled() {
        let dir = unique_temp_dir("writes-resume");
        let path = dir.join("session.log");
        let mut writer = TranscriptWriter::new(path.clone());
        let enabled = AtomicBool::new(true);

        maybe_write(&mut writer, &enabled, b"before-off ");
        enabled.store(false, Ordering::Relaxed);
        maybe_write(&mut writer, &enabled, b"during-off ");
        enabled.store(true, Ordering::Relaxed);
        maybe_write(&mut writer, &enabled, b"after-on");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "before-off after-on");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn defaults_transcript_enabled_true_when_no_settings_file() {
        let dir = unique_temp_dir("default-no-file");

        assert!(load_transcript_enabled(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_settings_round_trip() {
        let dir = unique_temp_dir("round-trip");

        save_settings(
            &dir,
            &Settings {
                transcript_enabled: false,
            },
        )
        .unwrap();
        assert!(!load_transcript_enabled(&dir));

        save_settings(
            &dir,
            &Settings {
                transcript_enabled: true,
            },
        )
        .unwrap();
        assert!(load_transcript_enabled(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_settings_file_falls_back_to_enabled_default() {
        let dir = unique_temp_dir("corrupt");
        std::fs::write(settings_path(&dir), "not valid json").unwrap();

        assert!(load_transcript_enabled(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_transcript_paths_are_unique_and_under_transcripts_dir() {
        let dir = unique_temp_dir("path-shape");

        let a = session_transcript_path(&dir);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = session_transcript_path(&dir);

        assert_ne!(a, b, "relaunching should not reuse the previous filename");
        assert_eq!(a.parent().unwrap(), dir.join(TRANSCRIPTS_DIR));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
