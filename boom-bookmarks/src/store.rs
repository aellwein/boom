//! Reads and writes the bookmark tree in a working directory.
//!
//! A folder is a directory, a bookmark is a `.toml` file, and entries are
//! ordered by their position prefix. The root `.metadata` file and the `.git`
//! directory are never part of the tree.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Bookmark, Node};
use crate::names::BOOKMARK_EXTENSION;
use crate::plan::Change;

const METADATA_FILE: &str = ".metadata";

/// Error from reading or writing the working tree.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Parse { path: PathBuf, message: String },
    Encode(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{err}"),
            Error::Parse { path, message } => {
                write!(f, "cannot parse {}: {message}", path.display())
            }
            Error::Encode(message) => write!(f, "cannot encode bookmark: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

/// On-disk form of a bookmark file.
#[derive(Debug, Serialize, Deserialize)]
struct BookmarkFile {
    title: String,
    url: String,
}

/// Reads the bookmark tree below `root`, in browser order.
pub fn read_tree(root: &Path) -> Result<Vec<Node>, Error> {
    read_dir(root)
}

fn read_dir(dir: &Path) -> Result<Vec<Node>, Error> {
    let mut entries: Vec<(u64, String, PathBuf, bool)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == METADATA_FILE || name == ".git" {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            entries.push((order_key(&name), name, entry.path(), true));
        } else if file_type.is_file() && has_bookmark_extension(&name) {
            entries.push((order_key(&name), name, entry.path(), false));
        }
    }
    // Sort by the numeric prefix, so positions above 9999 still order correctly.
    entries.sort();

    let mut nodes = Vec::with_capacity(entries.len());
    for (_, name, path, is_dir) in entries {
        if is_dir {
            nodes.push(Node::Folder {
                title: strip_prefix(&name).to_string(),
                children: read_dir(&path)?,
            });
        } else {
            nodes.push(Node::Bookmark(read_bookmark(&path)?));
        }
    }
    Ok(nodes)
}

fn has_bookmark_extension(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext == BOOKMARK_EXTENSION)
}

/// Numeric position prefix of an entry name, or `u64::MAX` when there is none.
fn order_key(name: &str) -> u64 {
    name.split_once('_')
        .and_then(|(digits, _)| digits.parse().ok())
        .unwrap_or(u64::MAX)
}

/// Removes the `NNNN_` position prefix from an entry name.
fn strip_prefix(name: &str) -> &str {
    match name.split_once('_') {
        Some((digits, rest)) if digits.parse::<u64>().is_ok() => rest,
        _ => name,
    }
}

fn read_bookmark(path: &Path) -> Result<Bookmark, Error> {
    let text = fs::read_to_string(path)?;
    let file: BookmarkFile = toml::from_str(&text).map_err(|err| Error::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    Ok(Bookmark {
        title: file.title,
        url: file.url,
    })
}

/// Applies planned changes to the working tree below `root`, in order.
///
/// The planner already orders removals children-first and creates
/// parents-first, so the steps can run as given.
pub fn apply(root: &Path, changes: &[Change]) -> Result<(), Error> {
    for change in changes {
        match change {
            Change::RemoveBookmark(path) => fs::remove_file(root.join(path))?,
            Change::RemoveFolder(path) => fs::remove_dir(root.join(path))?,
            Change::CreateFolder(path) => fs::create_dir_all(root.join(path))?,
            Change::WriteBookmark { path, bookmark } => {
                let file = BookmarkFile {
                    title: bookmark.title.clone(),
                    url: bookmark.url.clone(),
                };
                let text = toml::to_string(&file).map_err(|err| Error::Encode(err.to_string()))?;
                fs::write(root.join(path), text)?;
            }
        }
    }
    Ok(())
}
