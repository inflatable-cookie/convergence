use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{WorkflowProfile, WorkspaceConfig, WorkspaceState};

use super::{LocalStore, STORE_DIR, write_atomic};

impl LocalStore {
    pub fn converge_dir(root: &Path) -> std::path::PathBuf {
        root.join(STORE_DIR)
    }

    pub fn open(workspace_root: &Path) -> Result<Self> {
        let root = Self::converge_dir(workspace_root);
        if !root.is_dir() {
            return Err(anyhow!(
                "No {} directory found at {} (run `converge init`)",
                STORE_DIR,
                root.display()
            ));
        }
        // Checked on *open*, not on first bad read (batch 22.2). By the
        // time a read looks wrong, something has already been written
        // against an assumption that did not hold.
        crate::model::format::check_compatible(&root, crate::model::format::StoreKind::Workspace)?;
        Ok(Self { root })
    }

    pub fn init(workspace_root: &Path, force: bool) -> Result<Self> {
        let root = Self::converge_dir(workspace_root);
        if root.exists() && !force {
            return Err(anyhow!(
                "{} already exists at {} (use --force to re-init)",
                STORE_DIR,
                root.display()
            ));
        }
        // `--force` means "re-initialise over my own store". It does not
        // mean "destroy a store I cannot read" (batch 22.2).
        //
        // Found by driving it: every verb refused a format-7 workspace,
        // and then `init --force` cheerfully reset it to format 1 —
        // destroying exactly the history the refusal existed to protect.
        // Deleting the directory by hand is an unmistakable act; a flag
        // people reach for casually is not.
        if root.exists() {
            crate::model::format::check_compatible(
                &root,
                crate::model::format::StoreKind::Workspace,
            )
            .map_err(|err| {
                anyhow!(
                    "{err}\n\n\
                     `--force` will not re-initialise a store this build cannot read.\n\
                     If you are certain you want to discard it, remove {} yourself.",
                    root.display()
                )
            })?;
        }

        fs::create_dir_all(root.join("objects/blobs")).context("create blobs dir")?;
        fs::create_dir_all(root.join("objects/manifests")).context("create manifests dir")?;
        fs::create_dir_all(root.join("objects/recipes")).context("create recipes dir")?;
        fs::create_dir_all(root.join("snaps")).context("create snaps dir")?;
        fs::create_dir_all(root.join("resolutions")).context("create resolutions dir")?;

        let cfg = WorkspaceConfig {
            version: 1,
            remote: None,
            chunking: None,
            retention: None,
            workflow_profile: WorkflowProfile::default(),
        };
        let cfg_bytes = serde_json::to_vec_pretty(&cfg).context("serialize workspace config")?;
        write_atomic(&root.join("config.json"), &cfg_bytes).context("write config.json")?;

        let state = WorkspaceState {
            version: 1,
            lane_sync: std::collections::HashMap::new(),
            remote_tokens: std::collections::HashMap::new(),
            last_published: std::collections::HashMap::new(),
            last_seen_bundle: std::collections::HashMap::new(),
        };
        let state_bytes = serde_json::to_vec_pretty(&state).context("serialize workspace state")?;
        write_atomic(&root.join("state.json"), &state_bytes).context("write state.json")?;

        crate::model::format::write_version(&root, crate::model::format::StoreKind::Workspace)?;

        Ok(Self { root })
    }

    /// Pure read (audit R2): no writes on this hot path. Tokens live in
    /// state.json only; a legacy in-config token is ignored — `converge
    /// login` stores it properly.
    pub fn read_config(&self) -> Result<WorkspaceConfig> {
        let bytes = fs::read(self.root.join("config.json")).context("read config.json")?;
        serde_json::from_slice(&bytes).context("parse config.json")
    }

    pub fn write_config(&self, cfg: &WorkspaceConfig) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(cfg).context("serialize config")?;
        write_atomic(&self.root.join("config.json"), &bytes).context("write config.json")?;
        Ok(())
    }
}
