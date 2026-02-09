use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::model::{ObjectId, SnapStats};
use crate::store::LocalStore;

mod chunk_io;
mod chunking;
mod gc;
mod manifest_query;
mod manifest_scan;
mod materialize_fs;
mod path_ops;
mod restore_materialize;
mod root_lifecycle;
mod snap_ops;

#[derive(Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub store: LocalStore,
}
