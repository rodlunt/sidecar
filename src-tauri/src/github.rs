use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use http::header::ACCEPT;
use keyring::Entry;
use octocrab::auth::{DeviceCodes, OAuth};
use octocrab::Octocrab;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::git::git_remote_owner_repo;

// Not a secret: this is a public-client (device flow) OAuth App ID, safe to
// embed in a distributed desktop app. See github.com/settings/developers.
const CLIENT_ID: &str = "Ov23lieWy9cPVCPE7FVr";
const SCOPES: [&str; 2] = ["repo", "read:org"];

const KEYRING_SERVICE: &str = "sidecar";
const KEYRING_ACCOUNT: &str = "github";

/// Holds the device codes between `github_device_login_start` and
/// `github_device_login_poll` (two Tauri commands, one in-progress login).
#[derive(Default)]
pub struct GithubAuthState(Mutex<Option<DeviceCodes>>);

#[derive(Serialize, Deserialize, Clone)]
struct StoredTokens {
    login: String,
    access_token: String,
    access_expires_at: i64,
    refresh_token: Option<String>,
    refresh_expires_at: Option<i64>,
}

#[derive(Serialize)]
pub struct DeviceLoginInfo {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RelativeAge {
    OlderThanUsual,
    NewerThanUsual,
    Typical,
}

#[derive(Serialize)]
pub struct IssueSummary {
    number: u64,
    title: String,
    html_url: String,
    age_days: i64,
    days_since_update: i64,
    comments: u32,
    relative_to_median: RelativeAge,
}

#[derive(Serialize)]
pub struct IssuesSummary {
    issues: Vec<IssueSummary>,
    median_age_days: i64,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn keyring_entry() -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|e| e.to_string())
}

fn load_tokens() -> Result<Option<StoredTokens>, String> {
    let entry = keyring_entry()?;
    match entry.get_password() {
        Ok(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn save_tokens(tokens: &StoredTokens) -> Result<(), String> {
    let entry = keyring_entry()?;
    let json = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    entry.set_password(&json).map_err(|e| e.to_string())
}

/// A device-flow client must hit github.com (not api.github.com) and accept
/// JSON responses; see octocrab's `authenticate_as_device` docs.
fn device_flow_client() -> Result<Octocrab, String> {
    Octocrab::builder()
        .base_uri("https://github.com")
        .map_err(|e| e.to_string())?
        .add_header(ACCEPT, "application/json".to_string())
        .build()
        .map_err(|e| e.to_string())
}

fn tokens_from_oauth(oauth: &OAuth, login: String) -> StoredTokens {
    let t = now();
    StoredTokens {
        login,
        access_token: oauth.access_token.expose_secret().to_string(),
        access_expires_at: oauth
            .expires_in
            .map(|secs| t + secs as i64)
            .unwrap_or(i64::MAX),
        refresh_token: oauth
            .refresh_token
            .as_ref()
            .map(|s| s.expose_secret().to_string()),
        refresh_expires_at: oauth.refresh_token_expires_in.map(|secs| t + secs as i64),
    }
}

#[derive(Serialize)]
struct RefreshRequest<'a> {
    client_id: &'a str,
    grant_type: &'a str,
    refresh_token: &'a str,
}

async fn refresh_access_token(current: &StoredTokens) -> Result<StoredTokens, String> {
    let refresh_token = current
        .refresh_token
        .as_deref()
        .ok_or_else(|| "no refresh token stored".to_string())?;

    let client = device_flow_client()?;
    let oauth: OAuth = client
        .post(
            "/login/oauth/access_token",
            Some(&RefreshRequest {
                client_id: CLIENT_ID,
                grant_type: "refresh_token",
                refresh_token,
            }),
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(tokens_from_oauth(&oauth, current.login.clone()))
}

/// A token expired (or expiring within a minute) needs a refresh before use.
/// Pulled out as a pure function so the expiry boundary is a unit-testable
/// decision, not something buried inside async keyring/network I/O.
fn needs_refresh(access_expires_at: i64, now: i64) -> bool {
    now >= access_expires_at.saturating_sub(60)
}

/// Loads the stored tokens, refreshing them first if the access token is
/// expired (or about to expire within a minute). Returns an error the
/// frontend should treat the same as "no token stored" (fall back to the
/// device-flow prompt) if there's nothing usable.
async fn valid_tokens() -> Result<StoredTokens, String> {
    let stored = load_tokens()?.ok_or_else(|| "not authenticated".to_string())?;

    if !needs_refresh(stored.access_expires_at, now()) {
        return Ok(stored);
    }

    let refreshed = refresh_access_token(&stored).await?;
    save_tokens(&refreshed)?;
    Ok(refreshed)
}

#[tauri::command]
pub fn github_auth_status() -> Result<Option<String>, String> {
    Ok(load_tokens()?.map(|t| t.login))
}

#[tauri::command]
pub fn github_logout() -> Result<(), String> {
    let entry = keyring_entry()?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn github_device_login_start(
    state: tauri::State<'_, GithubAuthState>,
) -> Result<DeviceLoginInfo, String> {
    let client = device_flow_client()?;
    let client_id = SecretString::from(CLIENT_ID.to_string());
    let codes = client
        .authenticate_as_device(&client_id, SCOPES)
        .await
        .map_err(|e| e.to_string())?;

    let info = DeviceLoginInfo {
        user_code: codes.user_code.clone(),
        verification_uri: codes.verification_uri.clone(),
        expires_in: codes.expires_in,
    };

    *state.0.lock().map_err(|e| e.to_string())? = Some(codes);
    Ok(info)
}

#[tauri::command]
pub async fn github_device_login_poll(
    state: tauri::State<'_, GithubAuthState>,
) -> Result<String, String> {
    let codes = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| "no device login in progress".to_string())?;

    let client = device_flow_client()?;
    let client_id = SecretString::from(CLIENT_ID.to_string());
    let oauth = codes
        .poll_until_available(&client, &client_id)
        .await
        .map_err(|e| e.to_string())?;

    let authed = Octocrab::builder()
        .user_access_token(oauth.access_token.expose_secret().to_string())
        .build()
        .map_err(|e| e.to_string())?;
    let user = authed.current().user().await.map_err(|e| e.to_string())?;

    let tokens = tokens_from_oauth(&oauth, user.login.clone());
    save_tokens(&tokens)?;
    Ok(tokens.login)
}

/// An issue's age relative to the fetched set's median, as a comparison a
/// reader can act on rather than a bare open/closed state. Pulled out as a
/// pure function so the thresholds are unit-testable directly.
fn classify_age(age_days: i64, median_age_days: i64) -> RelativeAge {
    if median_age_days == 0 {
        RelativeAge::Typical
    } else if age_days as f64 > median_age_days as f64 * 1.5 {
        RelativeAge::OlderThanUsual
    } else if (age_days as f64) < median_age_days as f64 * 0.5 {
        RelativeAge::NewerThanUsual
    } else {
        RelativeAge::Typical
    }
}

#[tauri::command]
pub async fn list_issues(path: String) -> Result<IssuesSummary, String> {
    let tokens = valid_tokens().await?;

    let (owner, repo) = git_remote_owner_repo(path)?
        .ok_or_else(|| "no GitHub remote found for this folder".to_string())?;

    let client = Octocrab::builder()
        .user_access_token(tokens.access_token)
        .build()
        .map_err(|e| e.to_string())?;

    let first_page = client
        .issues(&owner, &repo)
        .list()
        .state(octocrab::params::State::Open)
        .per_page(100)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let all = client
        .all_pages(first_page)
        .await
        .map_err(|e| e.to_string())?;

    // The issues API also returns pull requests; the Issues panel isn't
    // meant to duplicate PR listings, so filter those out.
    let issues: Vec<_> = all.into_iter().filter(|i| i.pull_request.is_none()).collect();

    let now_ts = now();
    let mut ages: Vec<i64> = issues
        .iter()
        .map(|i| (now_ts - i.created_at.timestamp()) / 86400)
        .collect();
    ages.sort_unstable();
    let median_age_days = if ages.is_empty() {
        0
    } else {
        ages[ages.len() / 2]
    };

    let summaries = issues
        .into_iter()
        .map(|i| {
            let age_days = (now_ts - i.created_at.timestamp()) / 86400;
            IssueSummary {
                number: i.number,
                title: i.title,
                html_url: i.html_url.to_string(),
                age_days,
                days_since_update: (now_ts - i.updated_at.timestamp()) / 86400,
                comments: i.comments,
                relative_to_median: classify_age(age_days, median_age_days),
            }
        })
        .collect();

    Ok(IssuesSummary {
        issues: summaries,
        median_age_days,
    })
}

#[derive(Serialize)]
pub struct IssueDetail {
    number: u64,
    title: String,
    html_url: String,
    /// Raw markdown, shown as preformatted text on the frontend rather than
    /// rendered to HTML: avoids a markdown-parser dependency and the
    /// sanitisation question rendering third-party markdown would raise,
    /// for a "read the description" need plain text already satisfies.
    body: Option<String>,
    author: String,
    created_at: i64,
    updated_at: i64,
    comments: u32,
    labels: Vec<String>,
}

#[tauri::command]
pub async fn get_issue_detail(path: String, number: u64) -> Result<IssueDetail, String> {
    let tokens = valid_tokens().await?;

    let (owner, repo) = git_remote_owner_repo(path)?
        .ok_or_else(|| "no GitHub remote found for this folder".to_string())?;

    let client = Octocrab::builder()
        .user_access_token(tokens.access_token)
        .build()
        .map_err(|e| e.to_string())?;

    let issue = client
        .issues(&owner, &repo)
        .get(number)
        .await
        .map_err(|e| e.to_string())?;

    Ok(IssueDetail {
        number: issue.number,
        title: issue.title,
        html_url: issue.html_url.to_string(),
        body: issue.body,
        author: issue.user.login,
        created_at: issue.created_at.timestamp(),
        updated_at: issue.updated_at.timestamp(),
        comments: issue.comments,
        labels: issue.labels.into_iter().map(|l| l.name).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_refresh_false_well_before_expiry() {
        assert!(!needs_refresh(1_000_000, 500_000));
    }

    #[test]
    fn needs_refresh_true_after_expiry() {
        assert!(needs_refresh(1_000_000, 1_000_001));
    }

    #[test]
    fn needs_refresh_true_within_margin() {
        // 30s before expiry: inside the 60s safety margin, must refresh now
        // rather than risk the token dying mid-request.
        assert!(needs_refresh(1_000_000, 999_970));
    }

    #[test]
    fn needs_refresh_false_just_outside_margin() {
        // 61s before expiry: outside the margin, no refresh needed yet.
        assert!(!needs_refresh(1_000_000, 999_939));
    }

    #[test]
    fn classify_age_older_than_usual() {
        assert_eq!(classify_age(20, 10), RelativeAge::OlderThanUsual);
    }

    #[test]
    fn classify_age_newer_than_usual() {
        assert_eq!(classify_age(2, 10), RelativeAge::NewerThanUsual);
    }

    #[test]
    fn classify_age_typical() {
        assert_eq!(classify_age(10, 10), RelativeAge::Typical);
    }

    #[test]
    fn classify_age_boundary_is_not_older() {
        // Exactly 1.5x median: the threshold is strictly-greater, so this
        // stays Typical rather than flipping at the boundary.
        assert_eq!(classify_age(15, 10), RelativeAge::Typical);
    }

    #[test]
    fn classify_age_empty_set_is_typical() {
        assert_eq!(classify_age(5, 0), RelativeAge::Typical);
    }

    #[test]
    fn stored_tokens_round_trip_through_json() {
        let tokens = StoredTokens {
            login: "rodlunt".to_string(),
            access_token: "access-abc".to_string(),
            access_expires_at: 1_700_000_000,
            refresh_token: Some("refresh-xyz".to_string()),
            refresh_expires_at: Some(1_800_000_000),
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let back: StoredTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(back.login, tokens.login);
        assert_eq!(back.access_token, tokens.access_token);
        assert_eq!(back.access_expires_at, tokens.access_expires_at);
        assert_eq!(back.refresh_token, tokens.refresh_token);
        assert_eq!(back.refresh_expires_at, tokens.refresh_expires_at);
    }
}
