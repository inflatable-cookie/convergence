use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use converge::workspace::Workspace;

#[test]
fn snap_and_restore_roundtrip() -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let root = tmp.path();

    fs::create_dir_all(root.join("sub")).context("create sub dir")?;
    fs::write(root.join("a.txt"), b"hello\n").context("write a.txt")?;
    fs::write(root.join("sub/b.bin"), b"\x00\x01\x02").context("write b.bin")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink("a.txt", root.join("link.txt")).context("create symlink")?;
    }

    let ws = Workspace::init(root, false)?;
    let expected = capture_tree(root)?;
    let snap = ws.create_snap(Some("test snap".to_string()))?;

    // Mutate working directory.
    fs::remove_file(root.join("a.txt")).context("remove a.txt")?;
    fs::write(root.join("new.txt"), b"new\n").context("write new.txt")?;

    ws.restore_snap(&snap.id, true)?;
    let actual = capture_tree(root)?;

    assert_eq!(expected, actual);
    Ok(())
}

#[test]
fn blob_integrity_check_detects_corruption() -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;

    let id = ws.store.put_blob(b"abc")?;
    let blob_path = root
        .join(".converge")
        .join("objects/blobs")
        .join(id.as_str());

    fs::write(&blob_path, b"not abc").context("corrupt blob")?;
    assert!(ws.store.get_blob(&id).is_err());
    Ok(())
}

#[test]
fn manifest_is_deterministic_for_same_tree() -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let root = tmp.path();

    fs::create_dir_all(root.join("sub")).context("create sub dir")?;
    fs::write(root.join("a.txt"), b"hello\n").context("write a.txt")?;
    fs::write(root.join("sub/b.txt"), b"world\n").context("write b.txt")?;

    let ws = Workspace::init(root, false)?;
    let s1 = ws.create_snap(Some("one".to_string()))?;
    let s2 = ws.create_snap(Some("two".to_string()))?;
    assert_eq!(s1.root_manifest, s2.root_manifest);
    assert_eq!(s1.stats.files, s2.stats.files);
    assert_eq!(s1.stats.dirs, s2.stats.dirs);
    Ok(())
}

#[test]
fn chunked_file_roundtrip() -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let root = tmp.path();

    let ws = Workspace::init(root, false)?;

    // Force a multi-chunk file (chunking threshold is >= 8MiB).
    let big_path = root.join("big.bin");
    {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(std::fs::File::create(&big_path)?);
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..9 {
            f.write_all(&chunk)?;
        }
        f.flush()?;
    }

    let expected_hash = blake3::hash(&fs::read(&big_path)?).to_hex().to_string();

    let snap = ws.create_snap(Some("big".to_string()))?;

    // Ensure it was stored as a chunked file entry.
    let m = ws.store.get_manifest(&snap.root_manifest)?;
    let e = m
        .entries
        .iter()
        .find(|e| e.name == "big.bin")
        .context("find big.bin entry")?;
    let recipe_id = match &e.kind {
        converge::model::ManifestEntryKind::FileChunks { recipe, .. } => recipe.clone(),
        other => anyhow::bail!("expected FileChunks, got {:?}", other),
    };
    let recipe = ws.store.get_recipe(&recipe_id)?;
    assert!(recipe.chunks.len() > 1);

    // Mutate, then restore.
    fs::write(&big_path, b"oops").context("mutate")?;
    ws.restore_snap(&snap.id, true)?;

    let got_hash = blake3::hash(&fs::read(&big_path)?).to_hex().to_string();
    assert_eq!(expected_hash, got_hash);
    Ok(())
}

#[test]
fn chunked_file_small_edit_reuses_most_chunks() -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let root = tmp.path();

    let ws = Workspace::init(root, false)?;

    // Force chunking (default threshold is 8MiB).
    let big_path = root.join("big.bin");
    {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(std::fs::File::create(&big_path)?);
        let chunk = vec![b'a'; 1024 * 1024];
        for _ in 0..9 {
            f.write_all(&chunk)?;
        }
        f.flush()?;
    }

    let blobs_dir = root.join(".converge/objects/blobs");
    let recipes_dir = root.join(".converge/objects/recipes");
    let manifests_dir = root.join(".converge/objects/manifests");

    let count = |p: &Path| -> Result<usize> { Ok(std::fs::read_dir(p)?.count()) };

    ws.create_snap(Some("one".to_string()))?;
    let blobs1 = count(&blobs_dir)?;
    let recipes1 = count(&recipes_dir)?;
    let manifests1 = count(&manifests_dir)?;

    // Small in-place edit: should only change one chunk blob.
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&big_path)?;
        f.seek(SeekFrom::Start(1234))?;
        f.write_all(b"Z")?;
        f.flush()?;
    }

    ws.create_snap(Some("two".to_string()))?;
    let blobs2 = count(&blobs_dir)?;
    let recipes2 = count(&recipes_dir)?;
    let manifests2 = count(&manifests_dir)?;

    // One changed chunk, new recipe, new manifest.
    assert_eq!(blobs2, blobs1 + 1);
    assert_eq!(recipes2, recipes1 + 1);
    assert_eq!(manifests2, manifests1 + 1);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    File { bytes: Vec<u8>, mode: u32 },
    Symlink { target: String },
}

fn capture_tree(root: &Path) -> Result<BTreeMap<PathBuf, Node>> {
    let mut out = BTreeMap::new();
    capture_dir(root, Path::new(""), &mut out)?;
    Ok(out)
}

fn capture_dir(root: &Path, rel: &Path, out: &mut BTreeMap<PathBuf, Node>) -> Result<()> {
    let dir = root.join(rel);
    for entry in fs::read_dir(&dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".converge" {
            continue;
        }
        let name = name
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-utf8 filename"))?;

        let child_rel = rel.join(&name);
        let path = root.join(&child_rel);
        let ft = entry.file_type()?;

        if ft.is_dir() {
            capture_dir(root, &child_rel, out)?;
            continue;
        }

        if ft.is_symlink() {
            let target = fs::read_link(&path)?;
            let target = target
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non-utf8 symlink target"))?
                .to_string();
            out.insert(child_rel, Node::Symlink { target });
            continue;
        }

        let bytes = fs::read(&path)?;
        let mode = file_mode(&path)?;
        out.insert(child_rel, Node::File { bytes, mode });
    }
    Ok(())
}

fn file_mode(path: &Path) -> Result<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::symlink_metadata(path)?;
        Ok(meta.permissions().mode())
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0)
    }
}
