//! Git commit of the bookmark working tree.

use std::path::Path;

use git2::{IndexAddOption, Oid, Repository, Signature};

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
