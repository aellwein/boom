//! Bookmark tree as used by the sync logic.
//!
//! Browser-specific IDs are not part of this model. They stay machine-local
//! and are mapped by the host, never committed.

/// A single bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

/// One entry in a folder: a sub-folder or a bookmark.
/// The order of a list of nodes is the order shown in the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Folder { title: String, children: Vec<Node> },
    Bookmark(Bookmark),
}
