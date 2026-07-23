use super::*;

impl LocalStore {
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

    pub fn update_snap_message(&self, snap_id: &str, message: Option<&str>) -> Result<()> {
        let mut snap = self.get_snap(snap_id)?;
        let msg = message
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        snap.message = msg;
        self.put_snap(&snap)
    }
}
