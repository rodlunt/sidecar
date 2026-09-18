mod fs;
mod git;
mod github;
mod pty;
mod transcript;

use fs::WatcherState;
use github::GithubAuthState;
use pty::PtyState;
use tauri::Manager;
use transcript::TranscriptState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(PtyState::default())
        .manage(WatcherState::default())
        .manage(GithubAuthState::default())
        .setup(|app| {
            // Transcript saving defaults to on: seed the managed state from
            // whatever was last persisted (or `true` on a fresh install)
            // before the frontend gets a chance to spawn a PTY session.
            let app_data_dir = app.path().app_data_dir()?;
            let enabled = transcript::load_transcript_enabled(&app_data_dir);
            app.manage(TranscriptState::new(enabled));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            fs::read_dir,
            fs::watch_root,
            git::git_log,
            git::git_remote_owner_repo,
            github::github_auth_status,
            github::github_logout,
            github::github_device_login_start,
            github::github_device_login_poll,
            github::list_issues,
            github::get_issue_detail,
            transcript::get_transcript_enabled,
            transcript::set_transcript_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
