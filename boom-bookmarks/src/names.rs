//! Filename rules: readable names, stable order prefixes, and duplicate handling.
//!
//! Folders become directories and bookmarks become `<name>.toml` files.
//! Each name gets a position prefix such as `0010_` so that `ls` shows the
//! browser order.

use std::collections::{HashMap, HashSet};

use crate::model::{Bookmark, Node};

/// Spacing between position prefixes, so a new item can be inserted
/// without renaming its neighbours.
const PREFIX_STEP: usize = 10;

/// Longest base name in characters, before the prefix and extension.
const MAX_NAME_CHARS: usize = 80;

/// Used when a title and a URL both give an empty name.
const FALLBACK_NAME: &str = "untitled";

/// File extension for bookmark files.
pub const BOOKMARK_EXTENSION: &str = "toml";

/// Returns the order prefix for the item at `position` (0-based), e.g. `0010_`.
pub fn prefix(position: usize) -> String {
    format!("{:04}_", (position + 1) * PREFIX_STEP)
}

/// Makes a title safe to use as a file or directory name.
///
/// Characters that are invalid in paths are replaced with `-`, whitespace is
/// collapsed, leading dots and trailing spaces are removed, and the result is
/// capped at `MAX_NAME_CHARS` characters.
pub fn sanitize(title: &str) -> String {
    let mapped: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let collapsed = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|c: char| c == '.' || c.is_whitespace());
    trimmed
        .chars()
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Name used when a bookmark has no usable title: its URL without scheme and query.
pub fn fallback_from_url(url: &str) -> String {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let without_query = without_scheme
        .split(['?', '#'])
        .next()
        .unwrap_or(without_scheme);
    sanitize(without_query)
}

/// Short, stable hash of `input`, 6 hex digits. Used to tell apart bookmarks
/// that share a title. FNV-1a is used because it does not change between
/// Rust versions, unlike the standard library hasher.
pub fn short_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:06x}", hash & 0x00ff_ffff)
}

fn base_for_bookmark(bookmark: &Bookmark) -> String {
    let title = sanitize(&bookmark.title);
    if !title.is_empty() {
        return title;
    }
    let from_url = fallback_from_url(&bookmark.url);
    if from_url.is_empty() {
        FALLBACK_NAME.to_string()
    } else {
        from_url
    }
}

fn base_for_folder(title: &str) -> String {
    let title = sanitize(title);
    if title.is_empty() {
        FALLBACK_NAME.to_string()
    } else {
        title
    }
}

/// Names for the nodes of one folder, in order, without file extension.
///
/// Duplicate bookmark names get the URL hash appended. Any remaining
/// duplicate gets `-2`, `-3`, and so on. The result is always unique within
/// the folder.
pub fn sibling_names(nodes: &[Node]) -> Vec<String> {
    let mut bases: Vec<String> = nodes
        .iter()
        .map(|node| match node {
            Node::Folder { title, .. } => base_for_folder(title),
            Node::Bookmark(bookmark) => base_for_bookmark(bookmark),
        })
        .collect();

    let mut counts: HashMap<String, usize> = HashMap::new();
    for base in &bases {
        *counts.entry(base.clone()).or_insert(0) += 1;
    }
    for (base, node) in bases.iter_mut().zip(nodes) {
        if let Node::Bookmark(bookmark) = node
            && counts[base.as_str()] > 1
        {
            *base = format!("{base}-{}", short_hash(&bookmark.url));
        }
    }

    let mut seen: HashSet<String> = HashSet::new();
    bases
        .into_iter()
        .enumerate()
        .map(|(position, base)| {
            let mut candidate = base.clone();
            let mut suffix = 2;
            while !seen.insert(candidate.clone()) {
                candidate = format!("{base}-{suffix}");
                suffix += 1;
            }
            format!("{}{}", prefix(position), candidate)
        })
        .collect()
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

    #[test]
    fn prefix_has_gaps_and_fixed_width() {
        assert_eq!(prefix(0), "0010_");
        assert_eq!(prefix(1), "0020_");
        assert_eq!(prefix(99), "1000_");
    }

    #[test]
    fn sanitize_replaces_path_characters() {
        assert_eq!(sanitize("a/b:c*d?"), "a-b-c-d-");
        assert_eq!(sanitize("  Rust   Book  "), "Rust Book");
        assert_eq!(sanitize("..hidden"), "hidden");
    }

    #[test]
    fn sanitize_keeps_unicode_and_caps_length() {
        assert_eq!(sanitize("Über Straße"), "Über Straße");
        let long = "x".repeat(200);
        assert_eq!(sanitize(&long).chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn fallback_uses_url_without_scheme_and_query() {
        assert_eq!(
            fallback_from_url("https://example.com/docs/page?ref=1"),
            "example.com-docs-page"
        );
    }

    #[test]
    fn short_hash_is_stable_and_six_digits() {
        assert_eq!(short_hash("https://example.com").len(), 6);
        assert_eq!(short_hash("a"), short_hash("a"));
        assert_ne!(short_hash("a"), short_hash("b"));
    }

    #[test]
    fn names_follow_browser_order() {
        let nodes = vec![
            bookmark("First", "https://one.example"),
            bookmark("Second", "https://two.example"),
        ];
        assert_eq!(sibling_names(&nodes), vec!["0010_First", "0020_Second"]);
    }

    #[test]
    fn same_title_gets_url_hash() {
        let nodes = vec![
            bookmark("Docs", "https://a.example"),
            bookmark("Docs", "https://b.example"),
        ];
        let names = sibling_names(&nodes);
        assert_eq!(
            names,
            vec![
                format!("0010_Docs-{}", short_hash("https://a.example")),
                format!("0020_Docs-{}", short_hash("https://b.example")),
            ]
        );
    }

    #[test]
    fn identical_bookmarks_get_counter() {
        let nodes = vec![
            bookmark("Docs", "https://a.example"),
            bookmark("Docs", "https://a.example"),
        ];
        let names = sibling_names(&nodes);
        let hash = short_hash("https://a.example");
        assert_eq!(names[0], format!("0010_Docs-{hash}"));
        assert_eq!(names[1], format!("0020_Docs-{hash}-2"));
    }

    #[test]
    fn empty_title_uses_url_fallback() {
        let nodes = vec![bookmark("  ", "https://example.com/a")];
        assert_eq!(sibling_names(&nodes), vec!["0010_example.com-a"]);
    }

    #[test]
    fn duplicate_folder_names_get_counter() {
        let nodes = vec![
            Node::Folder {
                title: "Work".into(),
                children: vec![],
            },
            Node::Folder {
                title: "Work".into(),
                children: vec![],
            },
        ];
        assert_eq!(sibling_names(&nodes), vec!["0010_Work", "0020_Work-2"]);
    }
}
