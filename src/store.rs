use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::model::{
    FileRecipe, LaneSyncRecord, Manifest, ObjectId, Resolution, SnapRecord, WorkspaceConfig,
    WorkspaceState,
};

const STORE_DIR: &str = ".converge";

#[derive(Clone)]
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    pub fn converge_dir(root: &Path) -> PathBuf {
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
        };
        let cfg_bytes = serde_json::to_vec_pretty(&cfg).context("serialize workspace config")?;
        write_atomic(&root.join("config.json"), &cfg_bytes).context("write config.json")?;

        let state = WorkspaceState {
            version: 1,
            lane_sync: std::collections::HashMap::new(),
            remote_tokens: std::collections::HashMap::new(),
            last_published: std::collections::HashMap::new(),
        };
        let state_bytes = serde_json::to_vec_pretty(&state).context("serialize workspace state")?;
        write_atomic(&root.join("state.json"), &state_bytes).context("write state.json")?;

        Ok(Self { root })
    }

    pub fn read_config(&self) -> Result<WorkspaceConfig> {
        let bytes = fs::read(self.root.join("config.json")).context("read config.json")?;
        let mut cfg: WorkspaceConfig =
            serde_json::from_slice(&bytes).context("parse config.json")?;

        // Migration: if an older config contains a token, move it into state.json.
        if let Some(remote) = cfg.remote.as_mut()
            && let Some(token) = remote.token.take()
        {
            self.set_remote_token(remote, &token)
                .context("migrate remote token to state")?;
            // Persist updated config without token.
            self.write_config(&cfg)
                .context("write config after token migration")?;
        }

        Ok(cfg)
    }

    pub fn write_config(&self, cfg: &WorkspaceConfig) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(cfg).context("serialize config")?;
        write_atomic(&self.root.join("config.json"), &bytes).context("write config.json")?;
        Ok(())
    }

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

    pub fn set_lane_sync(&self, lane_id: &str, snap_id: &str, synced_at: &str) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.lane_sync.insert(
            lane_id.to_string(),
            LaneSyncRecord {
                snap_id: snap_id.to_string(),
                synced_at: synced_at.to_string(),
            },
        );
        self.write_state(&st)
    }

    pub fn remote_token_key(&self, remote: &crate::model::RemoteConfig) -> String {
        format!("{}#{}", remote.base_url, remote.repo_id)
    }

    fn publish_key(&self, remote: &crate::model::RemoteConfig, scope: &str, gate: &str) -> String {
        format!("{}#{}#{}#{}", remote.base_url, remote.repo_id, scope, gate)
    }

    pub fn get_last_published(
        &self,
        remote: &crate::model::RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<Option<String>> {
        let st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        Ok(st
            .last_published
            .get(&self.publish_key(remote, scope, gate))
            .cloned())
    }

    pub fn set_last_published(
        &self,
        remote: &crate::model::RemoteConfig,
        scope: &str,
        gate: &str,
        snap_id: &str,
    ) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.last_published
            .insert(self.publish_key(remote, scope, gate), snap_id.to_string());
        self.write_state(&st)
    }

    pub fn get_remote_token(&self, remote: &crate::model::RemoteConfig) -> Result<Option<String>> {
        let st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        Ok(st
            .remote_tokens
            .get(&self.remote_token_key(remote))
            .cloned())
    }

    pub fn set_remote_token(&self, remote: &crate::model::RemoteConfig, token: &str) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.remote_tokens
            .insert(self.remote_token_key(remote), token.to_string());
        self.write_state(&st)
    }

    pub fn clear_remote_token(&self, remote: &crate::model::RemoteConfig) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.remote_tokens.remove(&self.remote_token_key(remote));
        self.write_state(&st)
    }

    pub fn put_blob(&self, bytes: &[u8]) -> Result<ObjectId> {
        let id = hash_bytes(bytes);
        let path = self.root.join("objects/blobs").join(id.as_str());
        write_if_absent(&path, bytes).context("store blob")?;
        Ok(id)
    }

    pub fn has_blob(&self, id: &ObjectId) -> bool {
        self.root.join("objects/blobs").join(id.as_str()).exists()
    }

    pub fn get_blob(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self.root.join("objects/blobs").join(id.as_str());
        let bytes = fs::read(&path).with_context(|| format!("read blob {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "blob integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        Ok(bytes)
    }

    pub fn put_manifest(&self, manifest: &Manifest) -> Result<ObjectId> {
        let bytes = serde_json::to_vec(manifest).context("serialize manifest")?;
        let id = hash_bytes(&bytes);
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, &bytes).context("store manifest")?;
        Ok(id)
    }

    pub fn put_manifest_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = blake3::hash(bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "manifest hash mismatch (expected {}, got {})",
                id.as_str(),
                actual
            ));
        }
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, bytes).context("store manifest bytes")?;
        Ok(())
    }

    pub fn has_manifest(&self, id: &ObjectId) -> bool {
        self.root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()))
            .exists()
    }

    pub fn put_recipe(&self, recipe: &FileRecipe) -> Result<ObjectId> {
        let bytes = serde_json::to_vec(recipe).context("serialize recipe")?;
        let id = hash_bytes(&bytes);
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, &bytes).context("store recipe")?;
        Ok(id)
    }

    pub fn put_recipe_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = blake3::hash(bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "recipe hash mismatch (expected {}, got {})",
                id.as_str(),
                actual
            ));
        }
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, bytes).context("store recipe bytes")?;
        Ok(())
    }

    pub fn has_recipe(&self, id: &ObjectId) -> bool {
        self.root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()))
            .exists()
    }

    pub fn get_recipe_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        let bytes = fs::read(&path).with_context(|| format!("read recipe {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "recipe integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        Ok(bytes)
    }

    pub fn get_recipe(&self, id: &ObjectId) -> Result<FileRecipe> {
        let bytes = self.get_recipe_bytes(id)?;
        let r: FileRecipe = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse recipe {}", id.as_str()))?;
        Ok(r)
    }

    pub fn get_manifest_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        let bytes = fs::read(&path).with_context(|| format!("read manifest {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "manifest integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        Ok(bytes)
    }

    pub fn get_manifest(&self, id: &ObjectId) -> Result<Manifest> {
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        let bytes = fs::read(&path).with_context(|| format!("read manifest {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "manifest integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        let m: Manifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse manifest {}", id.as_str()))?;
        Ok(m)
    }

    pub fn put_snap(&self, snap: &SnapRecord) -> Result<()> {
        let path = self.root.join("snaps").join(format!("{}.json", snap.id));
        let bytes = serde_json::to_vec_pretty(snap).context("serialize snap")?;
        write_atomic(&path, &bytes).context("write snap")?;
        Ok(())
    }

    pub fn has_snap(&self, snap_id: &str) -> bool {
        self.root
            .join("snaps")
            .join(format!("{}.json", snap_id))
            .exists()
    }

    pub fn get_snap(&self, snap_id: &str) -> Result<SnapRecord> {
        let path = self.root.join("snaps").join(format!("{}.json", snap_id));
        let bytes = fs::read(&path).with_context(|| format!("read snap {}", snap_id))?;
        let s: SnapRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse snap {}", snap_id))?;
        Ok(s)
    }

    pub fn list_snaps(&self) -> Result<Vec<SnapRecord>> {
        let mut out = Vec::new();
        let dir = self.root.join("snaps");
        if !dir.is_dir() {
            return Ok(out);
        }

        for entry in fs::read_dir(&dir).context("read snaps dir")? {
            let entry = entry.context("read snaps dir entry")?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes =
                fs::read(&path).with_context(|| format!("read snap file {}", path.display()))?;
            let snap: SnapRecord = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse snap file {}", path.display()))?;
            out.push(snap);
        }
        Ok(out)
    }

    pub fn delete_snap(&self, snap_id: &str) -> Result<()> {
        let path = self.root.join("snaps").join(format!("{}.json", snap_id));
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("remove snap file {}", path.display()))?;
        }
        Ok(())
    }

    pub fn list_blob_ids(&self) -> Result<Vec<ObjectId>> {
        let dir = self.root.join("objects/blobs");
        let mut out = Vec::new();
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(&dir).context("read blobs dir")? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(s) = name.to_str() else {
                continue;
            };
            if s.len() == 64 {
                out.push(ObjectId(s.to_string()));
            }
        }
        Ok(out)
    }

    pub fn list_manifest_ids(&self) -> Result<Vec<ObjectId>> {
        let dir = self.root.join("objects/manifests");
        let mut out = Vec::new();
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(&dir).context("read manifests dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem.len() == 64 {
                out.push(ObjectId(stem.to_string()));
            }
        }
        Ok(out)
    }

    pub fn list_recipe_ids(&self) -> Result<Vec<ObjectId>> {
        let dir = self.root.join("objects/recipes");
        let mut out = Vec::new();
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(&dir).context("read recipes dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem.len() == 64 {
                out.push(ObjectId(stem.to_string()));
            }
        }
        Ok(out)
    }

    pub fn delete_blob(&self, id: &ObjectId) -> Result<()> {
        let path = self.root.join("objects/blobs").join(id.as_str());
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove blob {}", path.display()))?;
        }
        Ok(())
    }

    pub fn delete_manifest(&self, id: &ObjectId) -> Result<()> {
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("remove manifest {}", path.display()))?;
        }
        Ok(())
    }

    pub fn delete_recipe(&self, id: &ObjectId) -> Result<()> {
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove recipe {}", path.display()))?;
        }
        Ok(())
    }

    pub fn put_resolution(&self, resolution: &Resolution) -> Result<()> {
        if resolution.version != 1 && resolution.version != 2 {
            return Err(anyhow!("unsupported resolution version"));
        }
        let bytes = serde_json::to_vec_pretty(resolution).context("serialize resolution")?;
        let path = self
            .root
            .join("resolutions")
            .join(format!("{}.json", resolution.bundle_id));
        write_atomic(&path, &bytes).context("write resolution")?;
        Ok(())
    }

    pub fn get_resolution(&self, bundle_id: &str) -> Result<Resolution> {
        let path = self
            .root
            .join("resolutions")
            .join(format!("{}.json", bundle_id));
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let r: Resolution = serde_json::from_slice(&bytes).context("parse resolution")?;
        if r.version != 1 && r.version != 2 {
            return Err(anyhow!("unsupported resolution version"));
        }
        if r.bundle_id != bundle_id {
            return Err(anyhow!("resolution bundle_id mismatch"));
        }
        Ok(r)
    }

    pub fn has_resolution(&self, bundle_id: &str) -> bool {
        self.root
            .join("resolutions")
            .join(format!("{}.json", bundle_id))
            .exists()
    }

    pub fn update_snap_message(&self, snap_id: &str, message: Option<&str>) -> Result<()> {
        let mut snap = self.get_snap(snap_id)?;
        let msg = message
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        snap.message = msg;
        self.put_snap(&snap)
    }

    fn head_path(&self) -> PathBuf {
        self.root.join("HEAD")
    }

    pub fn get_head(&self) -> Result<Option<String>> {
        let path = self.head_path();
        if !path.exists() {
            return Ok(None);
        }
        let s =
            fs::read_to_string(&path).with_context(|| format!("read head {}", path.display()))?;
        let s = s.trim().to_string();
        if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
    }

    pub fn set_head(&self, snap_id: Option<&str>) -> Result<()> {
        let path = self.head_path();
        match snap_id {
            None => {
                if path.exists() {
                    fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
                }
                Ok(())
            }
            Some(id) => {
                write_atomic(&path, id.as_bytes()).context("write head")?;
                Ok(())
            }
        }
    }
}

pub fn hash_bytes(bytes: &[u8]) -> ObjectId {
    ObjectId(blake3::hash(bytes).to_hex().to_string())
}

fn write_if_absent(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create parent directories")?;
    }
    write_atomic(path, bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create parent directories")?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, bytes).with_context(|| format!("write temp file {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}
