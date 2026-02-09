use std::fs;

use anyhow::{Context, Result};

use crate::model::WorkspaceState;

use super::{LocalStore, write_atomic};

mod lane_sync;
mod publishing;
mod remote_tokens;

impl LocalStore {
    pub fn read_state(&self) -> Result<WorkspaceState> {
        let path = self.root.join("state.json");
        if !path.exists() {
            return Ok(WorkspaceState {
                version: 1,
                lane_sync: std::collections::HashMap::new(),
                remote_tokens: std::collections::HashMap::new(),
                last_published: std::collections::HashMap::new(),
            });
        }
        let bytes = fs::read(&path).context("read state.json")?;
        let st: WorkspaceState = serde_json::from_slice(&bytes).context("parse state.json")?;
        Ok(st)
    }

    pub fn write_state(&self, st: &WorkspaceState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(st).context("serialize state")?;
        write_atomic(&self.root.join("state.json"), &bytes).context("write state.json")?;
        Ok(())
    }
}
