mod fs;
mod git;
mod github;
mod pty;

use fs::WatcherState;
use github::GithubAuthState;
use pty::PtyState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(PtyState::default())
        .manage(WatcherState::default())
        .manage(GithubAuthState::default())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
