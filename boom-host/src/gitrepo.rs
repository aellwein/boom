//! Local bookmark git repository management.

use std::fs;
use std::path::Path;

use git2::{IndexAddOption, RepositoryInitOptions, Signature};

/// Root file that holds shared settings. It is not a bookmark.
pub const METADATA_FILE: &str = ".metadata";

const INITIAL_METADATA: &str = "[boom]\nformat_version = 1\n";

/// Creates a new local repo at `path` with an initial commit containing
/// the root `.metadata` file. Refuses to use a non-empty directory.
pub fn create(path: &Path, user_name: &str, user_email: &str) -> Result<(), String> {
    if path.exists() {
        let non_empty = fs::read_dir(path)
            .map_err(|err| err.to_string())?
            .next()
            .is_some();
        if non_empty {
            return Err(format!("{} is not empty", path.display()));
        }
    } else {
        fs::create_dir_all(path).map_err(|err| err.to_string())?;
    }

    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = git2::Repository::init_opts(path, &options).map_err(|err| err.to_string())?;

    fs::write(path.join(METADATA_FILE), INITIAL_METADATA).map_err(|err| err.to_string())?;

    let mut index = repo.index().map_err(|err| err.to_string())?;
    index
        .add_all([METADATA_FILE], IndexAddOption::DEFAULT, None)
        .map_err(|err| err.to_string())?;
    index.write().map_err(|err| err.to_string())?;
    let tree_id = index.write_tree().map_err(|err| err.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|err| err.to_string())?;

    let signature = Signature::now(user_name, user_email).map_err(|err| err.to_string())?;
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "boom: initialize bookmark repo",
        &tree,
        &[],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("boom-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn creates_repo_with_metadata_commit() {
        let dir = temp_dir("create");
        create(&dir, "John", "john@example.com").unwrap();

        let repo = git2::Repository::open(&dir).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(head.shorthand().unwrap(), "main");
        let commit = head.peel_to_commit().unwrap();
        assert_eq!(commit.message().unwrap(), "boom: initialize bookmark repo");
        assert_eq!(commit.author().name().unwrap(), "John");
        assert!(commit.tree().unwrap().get_name(METADATA_FILE).is_some());
        assert_eq!(
            fs::read_to_string(dir.join(METADATA_FILE)).unwrap(),
            INITIAL_METADATA
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_non_empty_directory() {
        let dir = temp_dir("non_empty");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep.txt"), "x").unwrap();

        assert!(create(&dir, "John", "john@example.com").is_err());
        assert!(!dir.join(".git").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
