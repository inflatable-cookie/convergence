mod diff_ops;
mod signatures;
mod tree_build;
mod walk;

pub use diff_ops::{DiffLine, diff_trees};
pub use signatures::EntrySig;
pub use tree_build::{tree_from_memory, tree_from_store};
