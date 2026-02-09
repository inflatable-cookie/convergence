use super::*;

impl LocalStore {
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
