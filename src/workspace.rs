use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use time::format_description::well_known::Rfc3339;

use crate::model::{
    ChunkingConfig, FileRecipe, FileRecipeChunk, Manifest, ManifestEntry, ManifestEntryKind,
    ObjectId, SnapRecord, SnapStats, SuperpositionVariant, SuperpositionVariantKind,
    compute_snap_id,
};
use crate::store::LocalStore;
use crate::store::hash_bytes;

const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
const DEFAULT_CHUNK_THRESHOLD: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct ChunkingPolicy {
    chunk_size: usize,
    threshold: u64,
}

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
        let cfg = self.store.read_config()?;
        let policy = chunking_policy_from_config(cfg.chunking.as_ref())?;

        let mut stats = SnapStats::default();
        let root_manifest = self.build_manifest(&self.root, &mut stats, policy)?;
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
        let cfg = self.store.read_config()?;
        let policy = chunking_policy_from_config(cfg.chunking.as_ref())?;
        let mut stats = SnapStats::default();
        let mut manifests: HashMap<ObjectId, Manifest> = HashMap::new();
        let root_manifest =
            build_manifest_in_memory(&self.root, &mut stats, &mut manifests, policy)?;
        Ok((root_manifest, manifests, stats))
    }

    fn build_manifest(
        &self,
        dir: &Path,
        stats: &mut SnapStats,
        policy: ChunkingPolicy,
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
                let manifest = self.build_manifest(&path, stats, policy)?;
                ManifestEntryKind::Dir { manifest }
            } else if file_type.is_file() {
                let mode = file_mode(&path)?;
                let meta = fs::symlink_metadata(&path)
                    .with_context(|| format!("stat {}", path.display()))?;
                let size = meta.len();

                let kind = if size >= policy.threshold {
                    let recipe =
                        chunk_file_to_recipe_store(&self.store, &path, size, policy.chunk_size)?;
                    ManifestEntryKind::FileChunks { recipe, mode, size }
                } else {
                    let bytes =
                        fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
                    let blob = self.store.put_blob(&bytes)?;
                    ManifestEntryKind::File { blob, mode, size }
                };

                stats.files += 1;
                stats.bytes += size;
                kind
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

#[derive(Clone, Debug, Default)]
pub struct GcReport {
    pub kept_snaps: usize,
    pub pruned_snaps: usize,
    pub deleted_blobs: usize,
    pub deleted_manifests: usize,
    pub deleted_recipes: usize,
}

impl Workspace {
    pub fn gc_local(&self, dry_run: bool) -> Result<GcReport> {
        let cfg = self.store.read_config()?;
        let retention = cfg.retention.unwrap_or_default();

        let mut snaps = self.store.list_snaps()?;
        snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let head = self.store.get_head()?;
        let now = time::OffsetDateTime::now_utc();
        let keep_days = retention.keep_days;
        let keep_last = retention.keep_last;

        let mut keep = std::collections::HashSet::new();
        for s in &retention.pinned {
            keep.insert(s.clone());
        }
        if let Some(h) = head {
            keep.insert(h);
        }
        if let Some(n) = keep_last {
            for s in snaps.iter().take(n as usize) {
                keep.insert(s.id.clone());
            }
        }
        if let Some(days) = keep_days {
            let cutoff = now - time::Duration::days(days as i64);
            for s in &snaps {
                if let Ok(ts) = time::OffsetDateTime::parse(
                    &s.created_at,
                    &time::format_description::well_known::Rfc3339,
                ) && ts >= cutoff
                {
                    keep.insert(s.id.clone());
                }
            }
        }
        if keep.is_empty() {
            // Safety: keep the newest snap if nothing else matches.
            if let Some(s) = snaps.first() {
                keep.insert(s.id.clone());
            }
        }

        // Walk reachable objects.
        let mut keep_blobs = std::collections::HashSet::new();
        let mut keep_manifests = std::collections::HashSet::new();
        let mut keep_recipes = std::collections::HashSet::new();
        for s in &snaps {
            if !keep.contains(&s.id) {
                continue;
            }
            collect_reachable_objects(
                &self.store,
                &s.root_manifest,
                &mut keep_blobs,
                &mut keep_manifests,
                &mut keep_recipes,
            )?;
        }

        // Delete unreferenced objects.
        let mut report = GcReport {
            kept_snaps: keep.len(),
            pruned_snaps: snaps.len().saturating_sub(keep.len()),
            ..GcReport::default()
        };

        for id in self.store.list_blob_ids()? {
            if !keep_blobs.contains(id.as_str()) {
                report.deleted_blobs += 1;
                if !dry_run {
                    self.store.delete_blob(&id)?;
                }
            }
        }
        for id in self.store.list_manifest_ids()? {
            if !keep_manifests.contains(id.as_str()) {
                report.deleted_manifests += 1;
                if !dry_run {
                    self.store.delete_manifest(&id)?;
                }
            }
        }
        for id in self.store.list_recipe_ids()? {
            if !keep_recipes.contains(id.as_str()) {
                report.deleted_recipes += 1;
                if !dry_run {
                    self.store.delete_recipe(&id)?;
                }
            }
        }

        if retention.prune_snaps && !dry_run {
            for s in &snaps {
                if !keep.contains(&s.id) {
                    self.store.delete_snap(&s.id)?;
                }
            }
        }

        Ok(report)
    }
}

fn collect_reachable_objects(
    store: &LocalStore,
    root: &ObjectId,
    blobs: &mut std::collections::HashSet<String>,
    manifests: &mut std::collections::HashSet<String>,
    recipes: &mut std::collections::HashSet<String>,
) -> Result<()> {
    let mut stack = vec![root.clone()];
    while let Some(mid) = stack.pop() {
        if !manifests.insert(mid.as_str().to_string()) {
            continue;
        }
        let m = store.get_manifest(&mid)?;
        for e in m.entries {
            match e.kind {
                ManifestEntryKind::Dir { manifest } => {
                    stack.push(manifest);
                }
                ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                ManifestEntryKind::FileChunks { recipe, .. } => {
                    let rid = recipe.as_str().to_string();
                    if recipes.insert(rid) {
                        let r = store.get_recipe(&recipe)?;
                        for c in r.chunks {
                            blobs.insert(c.blob.as_str().to_string());
                        }
                    }
                }
                ManifestEntryKind::Symlink { .. } => {}
                ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        collect_variant_objects(store, &v, blobs, manifests, recipes, &mut stack)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn collect_variant_objects(
    store: &LocalStore,
    v: &SuperpositionVariant,
    blobs: &mut std::collections::HashSet<String>,
    _manifests: &mut std::collections::HashSet<String>,
    recipes: &mut std::collections::HashSet<String>,
    stack: &mut Vec<ObjectId>,
) -> Result<()> {
    match &v.kind {
        SuperpositionVariantKind::File { blob, .. } => {
            blobs.insert(blob.as_str().to_string());
        }
        SuperpositionVariantKind::FileChunks { recipe, .. } => {
            let rid = recipe.as_str().to_string();
            if recipes.insert(rid) {
                let r = store.get_recipe(recipe)?;
                for c in r.chunks {
                    blobs.insert(c.blob.as_str().to_string());
                }
            }
        }
        SuperpositionVariantKind::Dir { manifest } => {
            stack.push(manifest.clone());
        }
        SuperpositionVariantKind::Symlink { .. } => {}
        SuperpositionVariantKind::Tombstone => {}
    }
    Ok(())
}

fn chunk_file_to_recipe_store(
    store: &LocalStore,
    path: &Path,
    size: u64,
    chunk_size: usize,
) -> Result<ObjectId> {
    let f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut r = BufReader::new(f);
    let mut buf = vec![0u8; chunk_size];
    let mut chunks = Vec::new();
    let mut total: u64 = 0;

    loop {
        let n = r
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        let blob = store.put_blob(&buf[..n])?;
        chunks.push(FileRecipeChunk {
            blob,
            size: n as u32,
        });
    }

    if total != size {
        anyhow::bail!(
            "size mismatch while chunking {} (expected {}, got {})",
            path.display(),
            size,
            total
        );
    }

    let recipe = FileRecipe {
        version: 1,
        size,
        chunks,
    };
    store.put_recipe(&recipe)
}

fn build_manifest_in_memory(
    dir: &Path,
    stats: &mut SnapStats,
    manifests: &mut HashMap<ObjectId, Manifest>,
    policy: ChunkingPolicy,
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
            let manifest = build_manifest_in_memory(&path, stats, manifests, policy)?;
            ManifestEntryKind::Dir { manifest }
        } else if file_type.is_file() {
            let mode = file_mode(&path)?;
            let meta =
                fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
            let size = meta.len();

            let kind = if size >= policy.threshold {
                let recipe = chunk_file_to_recipe_id(&path, size, policy.chunk_size)?;
                ManifestEntryKind::FileChunks { recipe, mode, size }
            } else {
                let bytes =
                    fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
                let blob = hash_bytes(&bytes);
                ManifestEntryKind::File { blob, mode, size }
            };

            stats.files += 1;
            stats.bytes += size;
            kind
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

fn chunk_file_to_recipe_id(path: &Path, size: u64, chunk_size: usize) -> Result<ObjectId> {
    let f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut r = BufReader::new(f);
    let mut buf = vec![0u8; chunk_size];
    let mut chunks = Vec::new();
    let mut total: u64 = 0;

    loop {
        let n = r
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        let blob = hash_bytes(&buf[..n]);
        chunks.push(FileRecipeChunk {
            blob,
            size: n as u32,
        });
    }

    if total != size {
        anyhow::bail!(
            "size mismatch while chunking {} (expected {}, got {})",
            path.display(),
            size,
            total
        );
    }

    let recipe = FileRecipe {
        version: 1,
        size,
        chunks,
    };
    let bytes = serde_json::to_vec(&recipe).context("serialize recipe")?;
    Ok(hash_bytes(&bytes))
}

fn chunking_policy_from_config(cfg: Option<&ChunkingConfig>) -> Result<ChunkingPolicy> {
    let chunk_size = cfg
        .map(|c| c.chunk_size)
        .unwrap_or(DEFAULT_CHUNK_SIZE)
        .max(64 * 1024);
    let threshold = cfg.map(|c| c.threshold).unwrap_or(DEFAULT_CHUNK_THRESHOLD);

    let chunk_size_usize =
        usize::try_from(chunk_size).map_err(|_| anyhow!("chunk_size too large: {}", chunk_size))?;

    Ok(ChunkingPolicy {
        chunk_size: chunk_size_usize,
        threshold,
    })
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
            ManifestEntryKind::FileChunks { recipe, mode, size } => {
                let r = store.get_recipe(&recipe)?;
                if r.size != size {
                    return Err(anyhow!(
                        "recipe size mismatch for {} (recipe {}, entry {})",
                        path.display(),
                        r.size,
                        size
                    ));
                }

                let f = fs::File::create(&path)
                    .with_context(|| format!("create file {}", path.display()))?;
                let mut w = BufWriter::new(f);
                for c in r.chunks {
                    let bytes = store.get_blob(&c.blob)?;
                    if bytes.len() != c.size as usize {
                        return Err(anyhow!(
                            "chunk size mismatch for {} (chunk {} expected {}, got {})",
                            path.display(),
                            c.blob.as_str(),
                            c.size,
                            bytes.len()
                        ));
                    }
                    w.write_all(&bytes)
                        .with_context(|| format!("write {}", path.display()))?;
                }
                w.flush()
                    .with_context(|| format!("flush {}", path.display()))?;
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
                        SuperpositionVariantKind::FileChunks { .. } => {
                            format!("{}: chunked_file", v.source)
                        }
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
