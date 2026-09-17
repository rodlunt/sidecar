use std::collections::HashSet;

use git2::{Repository, Sort};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct GraphCommit {
    id: String,
    summary: String,
    author: String,
    date: i64,
    /// Parent commit ids that are present in this graph's commit set.
    parents: Vec<String>,
    /// Parent ids referenced by this commit that were NOT walked (shallow
    /// clone boundary, or truncated by `limit`), rendered as a break, not
    /// a continuous line, so history isn't implied where none is known.
    truncated_parents: Vec<String>,
    refs: Vec<String>,
    /// Present only for a recognised "Merge pull request #N from <branch>"
    /// commit (GitHub's standard regular-merge shape): the PR number and
    /// title, pulled from the message body rather than a GitHub API call,
    /// since the data's already in local history.
    pr_number: Option<u64>,
    pr_title: Option<String>,
}

#[derive(Serialize)]
pub struct GraphData {
    commits: Vec<GraphCommit>,
    is_shallow: bool,
}

#[tauri::command]
pub fn git_log(path: String, limit: usize) -> Result<GraphData, String> {
    let repo = Repository::open(&path).map_err(|e| e.to_string())?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk
        .set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(|e| e.to_string())?;
    revwalk.push_glob("refs/heads/*").map_err(|e| e.to_string())?;
    if let Ok(head) = repo.head() {
        if let Some(target) = head.target() {
            let _ = revwalk.push(target);
        }
    }

    let oids: Vec<git2::Oid> = revwalk
        .filter_map(|oid| oid.ok())
        .take(limit)
        .collect();
    let walked: HashSet<git2::Oid> = oids.iter().copied().collect();

    // Map each oid to the ref names (branches/tags/HEAD) that point at it.
    let mut refs_by_oid: std::collections::HashMap<git2::Oid, Vec<String>> =
        std::collections::HashMap::new();
    for reference in repo.references().map_err(|e| e.to_string())?.flatten() {
        if let Some(target) = reference.target() {
            let name = reference
                .shorthand()
                .ok()
                .or_else(|| reference.name().ok())
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                refs_by_oid.entry(target).or_default().push(name);
            }
        }
    }

    let mut commits = Vec::with_capacity(oids.len());
    for oid in &oids {
        let commit = repo.find_commit(*oid).map_err(|e| e.to_string())?;
        let mut parents = Vec::new();
        let mut truncated_parents = Vec::new();
        for parent_id in commit.parent_ids() {
            if walked.contains(&parent_id) {
                parents.push(parent_id.to_string());
            } else {
                truncated_parents.push(parent_id.to_string());
            }
        }

        let message = commit.message().unwrap_or("").to_string();
        let pr = parse_merge_pr(&message);

        commits.push(GraphCommit {
            id: oid.to_string(),
            summary: commit.summary().ok().flatten().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date: commit.time().seconds(),
            parents,
            truncated_parents,
            refs: refs_by_oid.remove(oid).unwrap_or_default(),
            pr_number: pr.as_ref().map(|(n, _)| *n),
            pr_title: pr.map(|(_, t)| t),
        });
    }

    Ok(GraphData {
        commits,
        is_shallow: repo.is_shallow(),
    })
}

#[tauri::command]
pub fn git_remote_owner_repo(path: String) -> Result<Option<(String, String)>, String> {
    let repo = Repository::open(&path).map_err(|e| e.to_string())?;
    let remote = match repo.find_remote("origin") {
        Ok(remote) => remote,
        Err(_) => return Ok(None),
    };
    let url = match remote.url() {
        Ok(url) => url,
        Err(_) => return Ok(None),
    };

    Ok(parse_github_owner_repo(url))
}

/// Recognises GitHub's standard regular-merge commit message shape:
/// "Merge pull request #N from <branch>" as the first line, with the PR
/// title as the first non-empty line after it. Returns `None` for anything
/// else (a non-merge commit, a squash/rebase merge, or a hand-written merge
/// message): this is deliberately narrow rather than guessing.
fn parse_merge_pr(message: &str) -> Option<(u64, String)> {
    let mut lines = message.lines();
    let first = lines.next()?.trim();
    let rest = first.strip_prefix("Merge pull request #")?;
    let number_str = rest.split_whitespace().next()?;
    let number: u64 = number_str.parse().ok()?;
    if !rest.starts_with(&format!("{number} from ")) {
        return None;
    }

    let title = lines.find(|l| !l.trim().is_empty())?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some((number, title))
}

/// Parses `git@github.com:owner/repo.git` and `https://github.com/owner/repo.git`
/// (with or without a trailing `.git`) into `(owner, repo)`.
fn parse_github_owner_repo(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim().trim_end_matches(".git");

    let path = if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        rest
    } else {
        return None;
    };

    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_url() {
        assert_eq!(
            parse_github_owner_repo("git@github.com:rodlunt/sidecar.git"),
            Some(("rodlunt".to_string(), "sidecar".to_string()))
        );
    }

    #[test]
    fn parses_https_url() {
        assert_eq!(
            parse_github_owner_repo("https://github.com/rodlunt/sidecar.git"),
            Some(("rodlunt".to_string(), "sidecar".to_string()))
        );
    }

    #[test]
    fn parses_https_url_without_git_suffix() {
        assert_eq!(
            parse_github_owner_repo("https://github.com/rodlunt/sidecar"),
            Some(("rodlunt".to_string(), "sidecar".to_string()))
        );
    }

    #[test]
    fn rejects_non_github_url() {
        assert_eq!(parse_github_owner_repo("https://gitlab.com/rodlunt/sidecar.git"), None);
    }

    #[test]
    fn parses_real_github_merge_message() {
        let message = "Merge pull request #4 from rodlunt/feat/git-graph-panel\n\nfeat: add git commit graph panel\n";
        assert_eq!(
            parse_merge_pr(message),
            Some((4, "feat: add git commit graph panel".to_string()))
        );
    }

    #[test]
    fn rejects_non_merge_commit_message() {
        assert_eq!(parse_merge_pr("fix(ci): declare pnpm version via packageManager field"), None);
    }

    #[test]
    fn rejects_hand_written_merge_message() {
        // A merge commit that isn't GitHub's standard regular-merge shape
        // (e.g. a manual `git merge` locally) must not be misparsed as a PR.
        assert_eq!(parse_merge_pr("Merge branch 'main' into feature-x"), None);
    }

    #[test]
    fn rejects_merge_with_no_title_line() {
        assert_eq!(parse_merge_pr("Merge pull request #4 from rodlunt/feat/x\n"), None);
    }

    fn commit(
        repo: &Repository,
        parents: &[&git2::Commit],
        message: &str,
    ) -> git2::Oid {
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent_refs: Vec<&git2::Commit> = parents.to_vec();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }

    /// A full (non-truncated) history should never report a truncated parent:
    /// this is the control that must PASS on correct input, proving the
    /// truncation test below is actually exercising the gap path, not just
    /// always-true logic.
    #[test]
    fn full_history_has_no_truncated_parents() {
        let dir = tempfile_dir();
        let repo = Repository::init(&dir).unwrap();
        let c1 = repo.find_commit(commit(&repo, &[], "first")).unwrap();
        let c2 = repo.find_commit(commit(&repo, &[&c1], "second")).unwrap();
        let _c3 = commit(&repo, &[&c2], "third");

        let data = git_log(dir.to_string_lossy().to_string(), 100).unwrap();
        assert_eq!(data.commits.len(), 3);
        assert!(data.commits.iter().all(|c| c.truncated_parents.is_empty()));
    }

    /// Fetching fewer commits than exist (the same shape a shallow clone
    /// produces: a parent oid referenced but not present locally) must mark
    /// the boundary commit's missing parent as truncated, not silently drop
    /// it as if it simply had no parent.
    #[test]
    fn limited_fetch_marks_boundary_as_truncated() {
        let dir = tempfile_dir();
        let repo = Repository::init(&dir).unwrap();
        let c1 = repo.find_commit(commit(&repo, &[], "first")).unwrap();
        let c2 = repo.find_commit(commit(&repo, &[&c1], "second")).unwrap();
        let _c3 = commit(&repo, &[&c2], "third");

        let data = git_log(dir.to_string_lossy().to_string(), 2).unwrap();
        assert_eq!(data.commits.len(), 2);
        let boundary = data.commits.last().unwrap();
        assert!(boundary.parents.is_empty());
        assert_eq!(boundary.truncated_parents.len(), 1);
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sidecar-git-test-{}", uuid_like()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid_like() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
