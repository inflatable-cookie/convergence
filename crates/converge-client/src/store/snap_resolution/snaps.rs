use super::*;

impl LocalStore {
    /// Store a snap record, preserving an existing one (batch 13.4,
    /// audit C3). Snap ids cover tree + lineage only, so two records can
    /// share an id while carrying different messages, triggers, or
    /// timestamps; blind overwrite silently discards the first writer's
    /// metadata. Deliberate edits go through [`Self::overwrite_snap`].
    pub fn put_snap(&self, snap: &SnapRecord) -> Result<()> {
        if self.has_snap(&snap.id) {
            return Ok(());
        }
        self.overwrite_snap(snap)
    }

    /// Replace a snap record outright — for edits to an existing record,
    /// never for storing a newly captured one.
    pub fn overwrite_snap(&self, snap: &SnapRecord) -> Result<()> {
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

    /// Fetch a snap by id, or by a prefix long enough to be unique.
    ///
    /// Batch 22.4 fixed the same thing server-side for candidates and left
    /// this half live: `converge show <12-char snap id>` still answered
    /// "neither a local snap nor a reachable candidate", because the local
    /// lookup is a filename and the prefix was not one. Snap ids are
    /// printed shortened in the TUI and in messages people write, so the
    /// short form is what comes back.
    pub fn get_snap(&self, snap_id: &str) -> Result<SnapRecord> {
        let snap_id = &self.resolve_snap_prefix(snap_id)?;
        let path = self.root.join("snaps").join(format!("{}.json", snap_id));
        let bytes = fs::read(&path).with_context(|| format!("read snap {}", snap_id))?;
        let s: SnapRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse snap {}", snap_id))?;
        Ok(s)
    }

    /// Expand a unique snap-id prefix. Exact ids are returned untouched,
    /// so the lineage walks that pass full ids never read the directory.
    /// Ambiguity is an error rather than a guess: `restore` and `unsnap`
    /// take these, and the wrong one is somebody's work.
    fn resolve_snap_prefix(&self, given: &str) -> Result<String> {
        const SHORTEST: usize = 8;
        if given.len() >= 64
            || given.len() < SHORTEST
            || !given.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Ok(given.to_string());
        }
        let dir = self.root.join("snaps");
        let Ok(entries) = fs::read_dir(&dir) else {
            return Ok(given.to_string());
        };
        let mut found: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if id.starts_with(given) {
                found.push(id.to_string());
            }
        }
        match found.as_slice() {
            [only] => Ok(only.clone()),
            // Leave the caller's own "no such snap" error to fire.
            [] => Ok(given.to_string()),
            _ => {
                found.sort();
                anyhow::bail!(
                    "snap id {given} is ambiguous: it matches {}, use more characters",
                    found
                        .iter()
                        .map(|id| id[..12.min(id.len())].to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
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
        self.overwrite_snap(&snap)
    }
}
