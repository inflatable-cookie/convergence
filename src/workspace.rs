use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use time::format_description::well_known::Rfc3339;

use crate::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SnapRecord, SnapStats,
    SuperpositionVariantKind, compute_snap_id,
};
use crate::store::LocalStore;
use crate::store::hash_bytes;

#[derive(Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub store: LocalStore,
}

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

    pub fn create_snap(&self, message: Option<String>) -> Result<SnapRecord> {
        // Validate store format early.
        let _cfg = self.store.read_config()?;

        let mut stats = SnapStats::default();
        let root_manifest = self.build_manifest(&self.root, &mut stats)?;
        let created_at = time::OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .context("format created_at")?;

        let id = compute_snap_id(&created_at, &root_manifest);
        let snap = SnapRecord {
            version: 1,
            id,
            created_at,
            root_manifest,
            message,
            stats,
        };
        self.store.put_snap(&snap)?;
        self.store.set_head(Some(&snap.id))?;
        Ok(snap)
    }

    pub fn list_snaps(&self) -> Result<Vec<SnapRecord>> {
        let mut snaps = self.store.list_snaps()?;
        snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snaps)
    }

    pub fn show_snap(&self, snap_id: &str) -> Result<SnapRecord> {
        self.store.get_snap(snap_id)
    }

    pub fn restore_snap(&self, snap_id: &str, force: bool) -> Result<()> {
        let snap = self.store.get_snap(snap_id)?;

        if !force {
            let (cur_root, _cur_manifests, _stats) = self.current_manifest_tree()?;

            if let Some(head_id) = self.store.get_head()? {
                let head_snap = self.store.get_snap(&head_id)?;
                if cur_root != head_snap.root_manifest {
                    let short = head_id.chars().take(8).collect::<String>();
                    return Err(anyhow!(
                        "Refusing to restore: workspace has changes since {} (use --force)",
                        short
                    ));
                }
            } else if is_empty_except_converge_and_git(&self.root)? {
                // Empty workspace: allow restore.
            } else {
                // No HEAD: try to infer one from the current workspace state.
                let snaps = self.list_snaps()?;
                let matching = snaps
                    .into_iter()
                    .find(|s| s.root_manifest == cur_root)
                    .map(|s| s.id);
                let Some(head_id) = matching else {
                    return Err(anyhow!(
                        "No HEAD snap and workspace doesn't match any known snap (use --force)"
                    ));
                };
                self.store.set_head(Some(&head_id))?;
            }
        }

        clear_workspace_except_converge_and_git(&self.root)?;

        materialize_manifest(&self.store, &snap.root_manifest, &self.root)?;
        self.store.set_head(Some(&snap.id))?;
        Ok(())
    }

    /// Compute a manifest tree for the current working directory without writing a snap.
    ///
    /// Note: this still reads file contents to compute stable blob ids.
    pub fn current_manifest_tree(
        &self,
    ) -> Result<(ObjectId, HashMap<ObjectId, Manifest>, SnapStats)> {
        let mut stats = SnapStats::default();
        let mut manifests: HashMap<ObjectId, Manifest> = HashMap::new();
        let root_manifest = build_manifest_in_memory(&self.root, &mut stats, &mut manifests)?;
        Ok((root_manifest, manifests, stats))
    }

    fn build_manifest(&self, dir: &Path, stats: &mut SnapStats) -> Result<ObjectId> {
        let mut entries = Vec::new();
        let children = read_dir_sorted(dir)?;

        for child in children {
            let file_name = child
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("non-utf8 filename in {}", dir.display()))?;

            if should_ignore_name(&file_name) {
                continue;
            }

            let path = child.path();
            let file_type = child.file_type().context("read file type")?;

            let kind = if file_type.is_dir() {
                stats.dirs += 1;
                let manifest = self.build_manifest(&path, stats)?;
                ManifestEntryKind::Dir { manifest }
            } else if file_type.is_file() {
                let bytes =
                    fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
                let size = bytes.len() as u64;
                let blob = self.store.put_blob(&bytes)?;
                let mode = file_mode(&path)?;
                stats.files += 1;
                stats.bytes += size;
                ManifestEntryKind::File { blob, mode, size }
            } else if file_type.is_symlink() {
                let target = fs::read_link(&path)
                    .with_context(|| format!("read symlink {}", path.display()))?;
                let target = target
                    .to_str()
                    .ok_or_else(|| anyhow!("non-utf8 symlink target for {}", path.display()))?
                    .to_string();
                stats.symlinks += 1;
                ManifestEntryKind::Symlink { target }
            } else {
                continue;
            };

            entries.push(ManifestEntry {
                name: file_name,
                kind,
            });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let manifest = Manifest {
            version: 1,
            entries,
        };
        self.store.put_manifest(&manifest)
    }
}

fn build_manifest_in_memory(
    dir: &Path,
    stats: &mut SnapStats,
    manifests: &mut HashMap<ObjectId, Manifest>,
) -> Result<ObjectId> {
    let mut entries = Vec::new();
    let children = read_dir_sorted(dir)?;

    for child in children {
        let file_name = child
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("non-utf8 filename in {}", dir.display()))?;

        if should_ignore_name(&file_name) {
            continue;
        }

        let path = child.path();
        let file_type = child.file_type().context("read file type")?;

        let kind = if file_type.is_dir() {
            stats.dirs += 1;
            let manifest = build_manifest_in_memory(&path, stats, manifests)?;
            ManifestEntryKind::Dir { manifest }
        } else if file_type.is_file() {
            let bytes = fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
            let size = bytes.len() as u64;
            let blob = hash_bytes(&bytes);
            let mode = file_mode(&path)?;
            stats.files += 1;
            stats.bytes += size;
            ManifestEntryKind::File { blob, mode, size }
        } else if file_type.is_symlink() {
            let target =
                fs::read_link(&path).with_context(|| format!("read symlink {}", path.display()))?;
            let target = target
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 symlink target for {}", path.display()))?
                .to_string();
            stats.symlinks += 1;
            ManifestEntryKind::Symlink { target }
        } else {
            continue;
        };

        entries.push(ManifestEntry {
            name: file_name,
            kind,
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = Manifest {
        version: 1,
        entries,
    };
    let bytes = serde_json::to_vec(&manifest).context("serialize manifest")?;
    let id = hash_bytes(&bytes);
    manifests.insert(id.clone(), manifest);
    Ok(id)
}

fn should_ignore_name(name: &str) -> bool {
    matches!(name, ".converge" | ".git")
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .with_context(|| format!("read dir {}", dir.display()))?
        .collect::<std::result::Result<_, _>>()
        .with_context(|| format!("collect dir entries for {}", dir.display()))?;

    entries.sort_by(|a, b| {
        let a = a.file_name();
        let b = b.file_name();
        os_str_bytes(&a).cmp(&os_str_bytes(&b))
    });
    Ok(entries)
}

#[cfg(unix)]
fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    s.to_string_lossy().as_bytes().to_vec()
}

fn file_mode(path: &Path) -> Result<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta =
            fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
        Ok(meta.permissions().mode())
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0)
    }
}

fn clear_workspace_except_converge_and_git(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".converge" || name == ".git" {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_dir() {
            fs::remove_dir_all(&path).with_context(|| format!("remove dir {}", path.display()))?;
        } else {
            fs::remove_file(&path).with_context(|| format!("remove file {}", path.display()))?;
        }
    }
    Ok(())
}

fn is_empty_except_converge_and_git(root: &Path) -> Result<bool> {
    for entry in fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".converge" || name == ".git" {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

fn materialize_manifest(store: &LocalStore, manifest_id: &ObjectId, out_dir: &Path) -> Result<()> {
    let manifest = store.get_manifest(manifest_id)?;
    for entry in manifest.entries {
        let path = out_dir.join(&entry.name);
        match entry.kind {
            ManifestEntryKind::Dir { manifest } => {
                fs::create_dir_all(&path)
                    .with_context(|| format!("create dir {}", path.display()))?;
                materialize_manifest(store, &manifest, &path)?;
            }
            ManifestEntryKind::File { blob, mode, .. } => {
                let bytes = store.get_blob(&blob)?;
                fs::write(&path, &bytes)
                    .with_context(|| format!("write file {}", path.display()))?;
                set_file_mode(&path, mode)?;
            }
            ManifestEntryKind::Symlink { target } => {
                create_symlink(&target, &path)?;
            }
            ManifestEntryKind::Superposition { variants } => {
                let mut sources = Vec::new();
                for v in variants {
                    sources.push(match v.kind {
                        SuperpositionVariantKind::Tombstone => format!("{}: tombstone", v.source),
                        SuperpositionVariantKind::File { .. } => format!("{}: file", v.source),
                        SuperpositionVariantKind::Dir { .. } => format!("{}: dir", v.source),
                        SuperpositionVariantKind::Symlink { .. } => {
                            format!("{}: symlink", v.source)
                        }
                    });
                }
                return Err(anyhow!(
                    "cannot materialize superposition at {} (variants: {})",
                    path.display(),
                    sources.join(", ")
                ));
            }
        }
    }
    Ok(())
}

fn set_file_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, perm)
            .with_context(|| format!("set permissions {}", path.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

fn create_symlink(target: &str, link_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, link_path)
            .with_context(|| format!("create symlink {} -> {}", link_path.display(), target))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = (target, link_path);
        Err(anyhow!("symlinks are not supported on this platform"))
    }
}
