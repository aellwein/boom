//! Git operations on the bookmark repository: commit, fetch, merge, push.
//!
//! Sync works on the current branch against the `origin` remote. Conflicts
//! use last writer wins per file: for each conflicting file, the side whose
//! latest commit is newer wins. Ties go to the local side.

use std::path::Path;

use git2::build::CheckoutBuilder;
use git2::{
    Cred, CredentialType, Delta, Direction, FetchOptions, IndexAddOption, Oid, PushOptions,
    RemoteCallbacks, Repository, Signature,
};

const REMOTE: &str = "origin";

/// Stage mask in `IndexEntry::flags`: conflict stages live in bits 12 and 13.
const STAGE_MASK: u16 = 0x3000;

/// What `integrate` did to the local branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integration {
    /// Local and remote already match, or the remote has no such branch yet.
    UpToDate,
    /// The local branch was moved forward to the remote commit.
    FastForward,
    /// Local and remote diverged and were merged into a new commit.
    Merged { conflicts: usize },
}

/// Stages all changes below the repository at `path` and commits them.
///
/// Returns `None` when the working tree matches HEAD, so no empty commit is
/// created. Removed files are staged as removals.
pub fn commit_all(
    path: &Path,
    message: &str,
    user_name: &str,
    user_email: &str,
) -> Result<Option<Oid>, git2::Error> {
    let repo = Repository::open(path)?;
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(err) if err.code() == git2::ErrorCode::UnbornBranch => None,
        Err(err) => return Err(err),
    };
    if let Some(parent) = &parent {
        if parent.tree_id() == tree_id {
            return Ok(None);
        }
    }

    let signature = Signature::now(user_name, user_email)?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    Ok(Some(oid))
}

/// Fetches the current branch from `origin` and brings it up to date.
///
/// The working tree must be clean: commit local changes first. A diverged
/// branch is merged, and conflicting files take the version from the side
/// with the newer commit. The merge commit is written, but not pushed.
pub fn integrate(
    path: &Path,
    user_name: &str,
    user_email: &str,
) -> Result<Integration, git2::Error> {
    let repo = Repository::open(path)?;
    let branch = current_branch(&repo)?;

    let Some(remote_oid) = fetch_branch(&repo, &branch)? else {
        return Ok(Integration::UpToDate);
    };

    let local_commit = repo.head()?.peel_to_commit()?;
    let remote_commit = repo.find_commit(remote_oid)?;
    let remote_annotated = repo.find_annotated_commit(remote_oid)?;
    let (analysis, _) = repo.merge_analysis(&[&remote_annotated])?;

    if analysis.is_up_to_date() {
        return Ok(Integration::UpToDate);
    }

    if analysis.is_fast_forward() {
        repo.find_reference(&format!("refs/heads/{branch}"))?
            .set_target(remote_oid, "boom: fast-forward")?;
        repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
        return Ok(Integration::FastForward);
    }

    // Diverged: merge into the index, then resolve every conflict.
    let local_tree = local_commit.tree()?;
    repo.merge(&[&remote_annotated], None, None)?;

    let remote_newer = remote_commit.time().seconds() > local_commit.time().seconds();
    let mut index = repo.index()?;
    let conflicts: Vec<_> = index.conflicts()?.collect::<Result<_, _>>()?;
    let count = conflicts.len();
    for conflict in conflicts {
        let Some(any) = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref())
        else {
            continue;
        };
        let conflict_path = path_to_str(&any.path)?;
        let winner = if remote_newer {
            conflict.their
        } else {
            conflict.our
        };
        index.conflict_remove(Path::new(&conflict_path))?;
        if let Some(mut entry) = winner {
            entry.flags &= !STAGE_MASK;
            index.add(&entry)?;
        }
    }
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = Signature::now(user_name, user_email)?;
    let message = format!("sync: merge {REMOTE}/{branch}");
    let merge_oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&local_commit, &remote_commit],
    )?;
    repo.cleanup_state()?;

    // Files that the merge removed are not written by checkout, so delete them first.
    let merged = repo.find_commit(merge_oid)?;
    let merged_tree = merged.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&local_tree), Some(&merged_tree), None)?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("repository has no working directory"))?
        .to_path_buf();
    for delta in diff.deltas() {
        if delta.status() == Delta::Deleted {
            if let Some(file) = delta.old_file().path() {
                remove_file_and_empty_parents(&workdir, file);
            }
        }
    }
    let mut checkout = CheckoutBuilder::new();
    checkout.force();
    repo.checkout_tree(merged.as_object(), Some(&mut checkout))?;

    Ok(Integration::Merged { conflicts: count })
}

/// Pushes the current branch to `origin` under the same name.
///
/// The push is rejected if the remote moved on, so call `integrate` first.
pub fn push(path: &Path) -> Result<(), git2::Error> {
    let repo = Repository::open(path)?;
    let branch = current_branch(&repo)?;
    let oid = repo.head()?.peel_to_commit()?.id();

    let mut remote = repo.find_remote(REMOTE)?;
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks());
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[refspec.as_str()], Some(&mut options))?;

    repo.reference(
        &format!("refs/remotes/{REMOTE}/{branch}"),
        oid,
        true,
        "boom: push",
    )?;
    Ok(())
}

/// Integrates with the remote, then pushes.
pub fn sync(path: &Path, user_name: &str, user_email: &str) -> Result<Integration, git2::Error> {
    let outcome = integrate(path, user_name, user_email)?;
    push(path)?;
    Ok(outcome)
}

fn current_branch(repo: &Repository) -> Result<String, git2::Error> {
    let head = repo.head()?;
    let name = head
        .shorthand()
        .map_err(|_| git2::Error::from_str("HEAD is not a valid branch name"))?;
    Ok(name.to_string())
}

/// Fetches `branch` from `origin`. Returns the remote commit, or `None` if
/// the remote has no such branch.
fn fetch_branch(repo: &Repository, branch: &str) -> Result<Option<Oid>, git2::Error> {
    let mut remote = repo.find_remote(REMOTE)?;
    let wanted = format!("refs/heads/{branch}");
    remote.connect_auth(Direction::Fetch, Some(callbacks()), None)?;
    let exists = remote.list()?.iter().any(|head| head.name() == wanted);
    remote.disconnect()?;
    if !exists {
        return Ok(None);
    }

    let mut options = FetchOptions::new();
    options.remote_callbacks(callbacks());
    remote.fetch(&[branch], Some(&mut options), None)?;

    let reference = repo.find_reference(&format!("refs/remotes/{REMOTE}/{branch}"))?;
    Ok(reference.target())
}

/// Removes a file below `workdir`, then its parent directories while they are empty.
fn remove_file_and_empty_parents(workdir: &Path, relative: &Path) {
    let _ = std::fs::remove_file(workdir.join(relative));
    let mut parent = relative.parent();
    while let Some(dir) = parent {
        if dir.as_os_str().is_empty() || std::fs::remove_dir(workdir.join(dir)).is_err() {
            break;
        }
        parent = dir.parent();
    }
}

fn path_to_str(bytes: &[u8]) -> Result<String, git2::Error> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|_| git2::Error::from_str("conflicting path is not UTF-8"))
}

fn callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, allowed| {
        if allowed.contains(CredentialType::SSH_KEY) {
            Cred::ssh_key_from_agent(username.unwrap_or("git"))
        } else {
            Cred::default()
        }
    });
    callbacks
}
