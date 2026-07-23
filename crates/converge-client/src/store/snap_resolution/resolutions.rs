use super::*;

impl LocalStore {
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
}
