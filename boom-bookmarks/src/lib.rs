//! Git-backed bookmark sync library for boom.
//!
//! The library is pure logic for now: the bookmark tree model, the filename
//! rules for the repo, and a planner that computes the changes between two
//! trees. Git reads and writes come in a later step.

pub mod model;
pub mod names;
pub mod plan;

pub use model::{Bookmark, Node};
pub use plan::{flatten, plan, Change, Entry};
