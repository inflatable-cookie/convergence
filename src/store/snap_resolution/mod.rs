use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use crate::model::{Resolution, SnapRecord};

use super::{LocalStore, write_atomic};

mod head;
mod resolutions;
mod snaps;
