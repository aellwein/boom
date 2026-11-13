//! Round trip through the working tree and git commits.

use std::fs;
use std::path::Path;

use boom_bookmarks::{apply, git, plan, read_tree, Bookmark, Node};
use git2::Repository;
use git2::RepositoryInitOptions;

fn bookmark(title: &str, url: &str) -> Node {
    Node::Bookmark(Bookmark {
        title: title.into(),
        url: url.into(),
    })
}

fn folder(title: &str, children: Vec<Node>) -> Node {
    Node::Folder {
        title: title.into(),
        children,
    }
}

fn sample_tree() -> Vec<Node> {
    vec![
        bookmark("Rust Book", "https://doc.rust-lang.org/book/"),
        folder(
            "Work",
            vec![
                bookmark("Docs", "https://docs.example"),
                folder("Deep", vec![bookmark("Notes", "https://notes.example")]),
            ],
        ),
    ]
}

#[test]
fn applied_tree_reads_back_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let desired = sample_tree();

    apply(dir.path(), &plan(&[], &desired)).unwrap();

    let read = read_tree(dir.path()).unwrap();
    assert_eq!(read, desired);
    assert!(plan(&read, &desired).is_empty());
}

#[test]
fn metadata_and_git_dir_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".metadata"), "[boom]\nformat_version = 1\n").unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join("notes.txt"), "not a bookmark").unwrap();

    assert!(read_tree(dir.path()).unwrap().is_empty());
}

#[test]
fn update_removes_and_rewrites_files() {
    let dir = tempfile::tempdir().unwrap();
    let before = sample_tree();
    apply(dir.path(), &plan(&[], &before)).unwrap();

    let after = vec![folder(
        "Work",
        vec![bookmark("Docs", "https://new.example")],
    )];
    apply(dir.path(), &plan(&before, &after)).unwrap();

    assert_eq!(read_tree(dir.path()).unwrap(), after);
    assert!(!dir.path().join("0010_Rust Book.toml").exists());
}

#[test]
fn commit_all_commits_once_and_skips_unchanged_tree() {
    let dir = tempfile::tempdir().unwrap();
    Repository::init_opts(
        dir.path(),
        RepositoryInitOptions::new().initial_head("main"),
    )
    .unwrap();
    fs::write(dir.path().join(".metadata"), "[boom]\nformat_version = 1\n").unwrap();
    apply(dir.path(), &plan(&[], &sample_tree())).unwrap();

    let first = git::commit_all(dir.path(), "sync: first", "Test", "test@example.invalid").unwrap();
    assert!(first.is_some());

    let second =
        git::commit_all(dir.path(), "sync: again", "Test", "test@example.invalid").unwrap();
    assert!(second.is_none());

    fs::remove_file(dir.path().join("0010_Rust Book.toml")).unwrap();
    let third =
        git::commit_all(dir.path(), "sync: remove", "Test", "test@example.invalid").unwrap();
    assert!(third.is_some());

    let repo = Repository::open(dir.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), "sync: remove");
    assert!(head
        .tree()
        .unwrap()
        .get_name("0010_Rust Book.toml")
        .is_none());
    assert!(Path::new(dir.path()).join(".metadata").exists());
}
