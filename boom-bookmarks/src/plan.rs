//! Desired-state diff between two bookmark trees.
//!
//! Both trees are flattened to paths. The planner compares them path by path
//! and returns the changes that turn `current` into `desired`. It does not
//! touch git or the browser, so the same plan can be applied in either
//! direction.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{Bookmark, Node};
use crate::names::{BOOKMARK_EXTENSION, sibling_names};

/// What lives at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Folder,
    Bookmark(Bookmark),
}

/// One step that turns the current tree into the desired one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    RemoveBookmark(PathBuf),
    RemoveFolder(PathBuf),
    CreateFolder(PathBuf),
    WriteBookmark { path: PathBuf, bookmark: Bookmark },
}

/// Flattens a tree into `path -> entry`, where paths are relative to the root.
pub fn flatten(nodes: &[Node]) -> BTreeMap<PathBuf, Entry> {
    let mut out = BTreeMap::new();
    flatten_into(nodes, Path::new(""), &mut out);
    out
}

fn flatten_into(nodes: &[Node], parent: &Path, out: &mut BTreeMap<PathBuf, Entry>) {
    for (node, name) in nodes.iter().zip(sibling_names(nodes)) {
        match node {
            Node::Folder { children, .. } => {
                let path = parent.join(&name);
                out.insert(path.clone(), Entry::Folder);
                flatten_into(children, &path, out);
            }
            Node::Bookmark(bookmark) => {
                let path = parent.join(format!("{name}.{BOOKMARK_EXTENSION}"));
                out.insert(path, Entry::Bookmark(bookmark.clone()));
            }
        }
    }
}

/// Changes that turn `current` into `desired`.
///
/// Removals come first, bookmark files before folders, and deeper folders
/// before shallower ones. Creates and writes follow, with folders before
/// bookmarks and shallower paths first, so parents exist before children.
pub fn plan(current: &[Node], desired: &[Node]) -> Vec<Change> {
    let current = flatten(current);
    let desired = flatten(desired);

    let mut removes = Vec::new();
    let mut creates = Vec::new();

    for (path, entry) in &current {
        match (entry, desired.get(path)) {
            (_, None) => removes.push(remove(path, entry)),
            (Entry::Folder, Some(Entry::Folder)) => {}
            (Entry::Bookmark(old), Some(Entry::Bookmark(new))) => {
                if old != new {
                    creates.push(write(path, new));
                }
            }
            (_, Some(new)) => {
                removes.push(remove(path, entry));
                creates.push(create(path, new));
            }
        }
    }

    for (path, entry) in &desired {
        if !current.contains_key(path) {
            creates.push(create(path, entry));
        }
    }

    removes.sort_by_key(|change| match change {
        Change::RemoveBookmark(_) => (0, 0),
        Change::RemoveFolder(path) => (1, usize::MAX - depth(path)),
        _ => (2, 0),
    });
    creates.sort_by_key(|change| match change {
        Change::CreateFolder(path) => (0, depth(path)),
        Change::WriteBookmark { path, .. } => (1, depth(path)),
        _ => (2, 0),
    });

    removes.into_iter().chain(creates).collect()
}

fn depth(path: &Path) -> usize {
    path.components().count()
}

fn remove(path: &Path, entry: &Entry) -> Change {
    match entry {
        Entry::Folder => Change::RemoveFolder(path.to_path_buf()),
        Entry::Bookmark(_) => Change::RemoveBookmark(path.to_path_buf()),
    }
}

fn create(path: &Path, entry: &Entry) -> Change {
    match entry {
        Entry::Folder => Change::CreateFolder(path.to_path_buf()),
        Entry::Bookmark(bookmark) => write(path, bookmark),
    }
}

fn write(path: &Path, bookmark: &Bookmark) -> Change {
    Change::WriteBookmark {
        path: path.to_path_buf(),
        bookmark: bookmark.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn flatten_maps_folders_and_bookmarks_to_paths() {
        let tree = vec![folder(
            "Work",
            vec![bookmark("Docs", "https://docs.example")],
        )];
        let flat = flatten(&tree);
        assert_eq!(flat.get(Path::new("0010_Work")), Some(&Entry::Folder));
        assert!(matches!(
            flat.get(Path::new("0010_Work/0010_Docs.toml")),
            Some(Entry::Bookmark(_))
        ));
    }

    #[test]
    fn identical_trees_have_no_changes() {
        let tree = vec![bookmark("A", "https://a.example")];
        assert!(plan(&tree, &tree).is_empty());
    }

    #[test]
    fn new_bookmark_is_written() {
        let current = vec![];
        let desired = vec![bookmark("A", "https://a.example")];
        assert_eq!(
            plan(&current, &desired),
            vec![Change::WriteBookmark {
                path: PathBuf::from("0010_A.toml"),
                bookmark: Bookmark {
                    title: "A".into(),
                    url: "https://a.example".into(),
                },
            }]
        );
    }

    #[test]
    fn removed_folder_removes_children_first() {
        let current = vec![folder("Work", vec![bookmark("Docs", "https://d.example")])];
        let desired = vec![];
        let changes = plan(&current, &desired);
        assert_eq!(
            changes,
            vec![
                Change::RemoveBookmark(PathBuf::from("0010_Work/0010_Docs.toml")),
                Change::RemoveFolder(PathBuf::from("0010_Work")),
            ]
        );
    }

    #[test]
    fn changed_url_is_rewritten() {
        let current = vec![bookmark("A", "https://old.example")];
        let desired = vec![bookmark("A", "https://new.example")];
        let changes = plan(&current, &desired);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            &changes[0],
            Change::WriteBookmark { bookmark, .. } if bookmark.url == "https://new.example"
        ));
    }

    #[test]
    fn reorder_renames_as_remove_and_create() {
        let current = vec![
            bookmark("A", "https://a.example"),
            bookmark("B", "https://b.example"),
        ];
        let desired = vec![
            bookmark("B", "https://b.example"),
            bookmark("A", "https://a.example"),
        ];
        let changes = plan(&current, &desired);
        assert!(changes.contains(&Change::RemoveBookmark(PathBuf::from("0010_A.toml"))));
        assert!(changes.contains(&Change::WriteBookmark {
            path: PathBuf::from("0020_A.toml"),
            bookmark: Bookmark {
                title: "A".into(),
                url: "https://a.example".into(),
            },
        }));
    }

    #[test]
    fn folder_created_before_its_children() {
        let desired = vec![folder("Work", vec![bookmark("Docs", "https://d.example")])];
        let changes = plan(&[], &desired);
        assert!(matches!(changes[0], Change::CreateFolder(_)));
        assert!(matches!(changes[1], Change::WriteBookmark { .. }));
    }
}
