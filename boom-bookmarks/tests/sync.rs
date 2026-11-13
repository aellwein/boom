//! Pull, merge and push against a local bare remote.

use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use boom_bookmarks::git::{self, Integration};
use boom_bookmarks::{Bookmark, Node, apply, plan, read_tree};
use git2::{Repository, RepositoryInitOptions};
use tempfile::TempDir;

const NAME: &str = "Test";
const EMAIL: &str = "test@example.invalid";

fn bookmark(title: &str, url: &str) -> Node {
    Node::Bookmark(Bookmark {
        title: title.into(),
        url: url.into(),
    })
}

fn init_bare(dir: &Path) {
    Repository::init_opts(
        dir,
        RepositoryInitOptions::new().bare(true).initial_head("main"),
    )
    .unwrap();
}

fn init_repo(dir: &Path) {
    Repository::init_opts(dir, RepositoryInitOptions::new().initial_head("main")).unwrap();
    fs::write(dir.join(".metadata"), "[boom]\nformat_version = 1\n").unwrap();
}

/// Makes the working tree at `dir` match `desired`, without touching `.metadata`.
fn set_tree(dir: &Path, desired: &[Node]) {
    let current = read_tree(dir).unwrap();
    apply(dir, &plan(&current, desired)).unwrap();
}

fn commit(dir: &Path, message: &str) {
    git::commit_all(dir, message, NAME, EMAIL).unwrap();
}

/// Creates a bare remote with one pushed commit, and returns a clone of it.
fn setup(initial: &[Node]) -> (TempDir, TempDir, TempDir) {
    let remote = TempDir::new().unwrap();
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();
    init_bare(remote.path());

    init_repo(a.path());
    set_tree(a.path(), initial);
    commit(a.path(), "sync: initial");
    Repository::open(a.path())
        .unwrap()
        .remote("origin", remote.path().to_str().unwrap())
        .unwrap();
    git::push(a.path()).unwrap();

    Repository::clone(remote.path().to_str().unwrap(), b.path()).unwrap();
    (remote, a, b)
}

/// Waits so the next commit has a later timestamp (git stores whole seconds).
fn tick() {
    sleep(Duration::from_millis(1100));
}

#[test]
fn clean_clone_is_up_to_date() {
    let (_remote, _a, b) = setup(&[bookmark("A", "https://a.example")]);
    let outcome = git::integrate(b.path(), NAME, EMAIL).unwrap();
    assert_eq!(outcome, Integration::UpToDate);
}

#[test]
fn remote_commit_is_fast_forwarded() {
    let initial = [bookmark("A", "https://a.example")];
    let (_remote, a, b) = setup(&initial);

    let mut desired = initial.to_vec();
    desired.push(bookmark("B", "https://b.example"));
    set_tree(a.path(), &desired);
    commit(a.path(), "sync: add B");
    git::sync(a.path(), NAME, EMAIL).unwrap();

    let outcome = git::sync(b.path(), NAME, EMAIL).unwrap();
    assert_eq!(outcome, Integration::FastForward);
    assert_eq!(read_tree(b.path()).unwrap(), desired);
}

#[test]
fn diverged_changes_to_different_files_merge_cleanly() {
    let initial = [bookmark("A", "https://a.example")];
    let (_remote, a, b) = setup(&initial);

    let mut from_a = initial.to_vec();
    from_a.push(bookmark("FromA", "https://from-a.example"));
    set_tree(a.path(), &from_a);
    commit(a.path(), "sync: add from A");
    git::sync(a.path(), NAME, EMAIL).unwrap();

    let mut from_b = initial.to_vec();
    from_b.push(bookmark("FromB", "https://from-b.example"));
    set_tree(b.path(), &from_b);
    commit(b.path(), "sync: add from B");

    let outcome = git::sync(b.path(), NAME, EMAIL).unwrap();
    assert_eq!(outcome, Integration::Merged { conflicts: 0 });

    let merged = read_tree(b.path()).unwrap();
    assert_eq!(merged.len(), 3);
    assert!(merged.contains(&bookmark("FromA", "https://from-a.example")));
    assert!(merged.contains(&bookmark("FromB", "https://from-b.example")));

    git::sync(a.path(), NAME, EMAIL).unwrap();
    assert_eq!(read_tree(a.path()).unwrap(), merged);
}

#[test]
fn conflict_takes_remote_version_when_remote_is_newer() {
    let initial = [bookmark("F", "https://old.example")];
    let (_remote, a, b) = setup(&initial);

    set_tree(b.path(), &[bookmark("F", "https://b.example")]);
    commit(b.path(), "sync: B edits F");

    tick();
    set_tree(a.path(), &[bookmark("F", "https://a.example")]);
    commit(a.path(), "sync: A edits F");
    git::sync(a.path(), NAME, EMAIL).unwrap();

    let outcome = git::sync(b.path(), NAME, EMAIL).unwrap();
    assert_eq!(outcome, Integration::Merged { conflicts: 1 });
    assert_eq!(
        read_tree(b.path()).unwrap(),
        vec![bookmark("F", "https://a.example")]
    );
}

#[test]
fn conflict_takes_local_version_when_local_is_newer() {
    let initial = [bookmark("F", "https://old.example")];
    let (_remote, a, b) = setup(&initial);

    set_tree(a.path(), &[bookmark("F", "https://a.example")]);
    commit(a.path(), "sync: A edits F");
    git::sync(a.path(), NAME, EMAIL).unwrap();

    tick();
    set_tree(b.path(), &[bookmark("F", "https://b.example")]);
    commit(b.path(), "sync: B edits F");

    let outcome = git::sync(b.path(), NAME, EMAIL).unwrap();
    assert_eq!(outcome, Integration::Merged { conflicts: 1 });

    git::sync(a.path(), NAME, EMAIL).unwrap();
    assert_eq!(
        read_tree(a.path()).unwrap(),
        vec![bookmark("F", "https://b.example")]
    );
}

#[test]
fn merge_applies_remote_deletion_to_working_tree() {
    let initial = [
        bookmark("F", "https://f.example"),
        bookmark("G", "https://g.example"),
    ];
    let (_remote, a, b) = setup(&initial);

    set_tree(a.path(), &[bookmark("G", "https://g.example")]);
    commit(a.path(), "sync: A removes F");
    git::sync(a.path(), NAME, EMAIL).unwrap();

    let mut from_b = initial.to_vec();
    from_b.push(bookmark("H", "https://h.example"));
    set_tree(b.path(), &from_b);
    commit(b.path(), "sync: B adds H");

    let outcome = git::sync(b.path(), NAME, EMAIL).unwrap();
    assert_eq!(outcome, Integration::Merged { conflicts: 0 });
    assert!(!b.path().join("0010_F.toml").exists());
    assert_eq!(
        read_tree(b.path()).unwrap(),
        vec![
            bookmark("G", "https://g.example"),
            bookmark("H", "https://h.example"),
        ]
    );
}

#[test]
fn set_origin_adds_then_updates_remote() {
    let dir = TempDir::new().unwrap();
    init_repo(dir.path());
    assert!(!git::has_origin(dir.path()).unwrap());

    git::set_origin(dir.path(), "https://example.invalid/one.git").unwrap();
    assert!(git::has_origin(dir.path()).unwrap());

    git::set_origin(dir.path(), "https://example.invalid/two.git").unwrap();
    let repo = Repository::open(dir.path()).unwrap();
    let remote = repo.find_remote("origin").unwrap();
    assert_eq!(remote.url().unwrap(), "https://example.invalid/two.git");
}
