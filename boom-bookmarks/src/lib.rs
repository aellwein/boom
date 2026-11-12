//! Git-backed bookmark sync library for boom.
//!
//! The library is pure logic for now: the bookmark tree model, the filename
//! rules for the repo, and a planner that computes the changes between two
//! trees. The store reads and writes a working tree, and the git module
//! commits it.

pub mod git;
pub mod model;
pub mod names;
pub mod plan;
pub mod store;

pub use model::{Bookmark, Node};
pub use plan::{flatten, plan, Change, Entry};
pub use store::{apply, read_tree};
