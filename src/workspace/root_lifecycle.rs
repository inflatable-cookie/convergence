use super::*;

impl Workspace {
    pub fn init(root: &Path, force: bool) -> Result<Self> {
        let store = LocalStore::init(root, force)?;
        Ok(Self {
            root: root.to_path_buf(),
            store,
        })
    }

    pub fn discover(start: &Path) -> Result<Self> {
        let start = start
            .canonicalize()
            .with_context(|| format!("canonicalize {}", start.display()))?;
        for dir in start.ancestors() {
            let converge_dir = LocalStore::converge_dir(dir);
            if converge_dir.is_dir() {
                let store = LocalStore::open(dir)?;
                return Ok(Self {
                    root: dir.to_path_buf(),
                    store,
                });
            }
        }
        Err(anyhow!(
            "No .converge directory found (run `converge init`)"
        ))
    }
}
