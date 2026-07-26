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
        // The personal identity directory is *also* called `.converge`
        // (`CONVERGE_HOME`, batch 19.1), and it sits in the home
        // directory — directly above most people's work.
        //
        // Batch 22.4 found what that costs on the first real session:
        // running a verb in a directory with no workspace walked up,
        // matched `~/.converge`, and reported the entire home directory
        // as the workspace. `converge snap` there would have tried to
        // capture everything the user owns.
        let identity_dir = crate::identity::converge_home().ok();
        for dir in start.ancestors() {
            let converge_dir = LocalStore::converge_dir(dir);
            if !converge_dir.is_dir() {
                continue;
            }
            if identity_dir.as_deref() == Some(converge_dir.as_path()) {
                // Keep walking: an ancestor above the home directory
                // could still hold a real workspace.
                continue;
            }
            // A workspace always has a config; the identity directory
            // never does. Belt and braces, because `CONVERGE_HOME` can
            // be moved anywhere and the name check alone would miss it.
            if !converge_dir.join("config.json").is_file() {
                continue;
            }
            let store = LocalStore::open(dir)?;
            return Ok(Self {
                root: dir.to_path_buf(),
                store,
            });
        }
        Err(anyhow!(
            "No .converge workspace found here or in any parent directory \
             (run `converge init`)"
        ))
    }
}
