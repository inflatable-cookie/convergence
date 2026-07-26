//! On-disk format versioning (g02.022 batch 22.2).
//!
//! The failure this prevents is not a crash. A crash would be fine. It is
//! a **newer binary silently misreading an older store**: a field that
//! gained a meaning, an id whose domain tag changed (batch 18.3 moved
//! `converge-snap-v3` to `v4`), an enum that gained a variant an old
//! reader skips. Those corrupt quietly, and the corruption is discovered
//! long after the thing that caused it.
//!
//! ## Why a separate file
//!
//! `WorkspaceConfig` has carried a `version` field since the rebuild and
//! nothing ever read it. That is worse than having none, because it looks
//! like a guard.
//!
//! It also could not have worked. `config.json` is parsed by serde, so a
//! format change that alters its *shape* fails to parse before anything
//! gets to look at the version — the error would be "missing field", not
//! "wrong version". A version stamp has to be readable by every binary
//! that will ever meet it, including ones written after the format it is
//! stamping. So it lives in its own file and holds one line of text.
//!
//! ## Absent means 1
//!
//! A store written before this batch has no stamp. Absent is defined as
//! version 1, permanently, and nothing rewrites it — so opening a store
//! stays a pure read. (Batch 22.1's `doctor` opens a workspace and is
//! tested to change nothing; a migrate-on-open would have broken that.)
//!
//! Version 2 onwards must write the file.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// Name of the stamp file, inside the store directory.
pub const FORMAT_FILE: &str = "format";

/// What a Convergence store's on-disk layout is versioned as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreKind {
    /// A workspace's `.converge` directory.
    Workspace,
    /// A server's `--data-dir`.
    Server,
}

impl StoreKind {
    pub fn tag(&self) -> &'static str {
        match self {
            StoreKind::Workspace => "converge-workspace",
            StoreKind::Server => "converge-server",
        }
    }

    /// The version this binary reads and writes.
    ///
    /// Bump when a change would make an older binary *misread* a newer
    /// store, or the reverse. Adding a file nobody older looks for is
    /// not a bump; changing what an existing file means is.
    pub fn current(&self) -> u32 {
        match self {
            StoreKind::Workspace => 1,
            StoreKind::Server => 1,
        }
    }

    fn what(&self) -> &'static str {
        match self {
            StoreKind::Workspace => "workspace",
            StoreKind::Server => "server data directory",
        }
    }
}

/// Read a store's format version. Absent means 1.
pub fn read_version(store_root: &Path, kind: StoreKind) -> Result<u32> {
    let path = store_root.join(FORMAT_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    let text = text.trim();
    let Some(version) = text.strip_prefix(&format!("{}-", kind.tag())) else {
        // A stamp naming a different kind means the path is pointing at
        // the wrong thing entirely — a workspace passed as a data dir,
        // usually. Saying so beats "expected 1, found 3".
        bail!(
            "{} at {} is stamped {text:?}, which is not a {}",
            kind.what(),
            store_root.display(),
            kind.tag()
        );
    };
    version
        .parse()
        .with_context(|| format!("unreadable format stamp {text:?} at {}", path.display()))
}

/// Write the current stamp. Called at init, never on open.
pub fn write_version(store_root: &Path, kind: StoreKind) -> Result<()> {
    let path = store_root.join(FORMAT_FILE);
    std::fs::write(&path, format!("{}-{}\n", kind.tag(), kind.current()))
        .with_context(|| format!("write {}", path.display()))
}

/// Refuse a store this binary cannot correctly read or write.
///
/// Both directions are refused. An older binary opening a newer store is
/// the more dangerous case — it is the one that reads fields whose
/// meaning changed underneath it — and it is also the one people hit,
/// because downgrading is what you do when a new version misbehaves.
pub fn check_compatible(store_root: &Path, kind: StoreKind) -> Result<()> {
    let found = read_version(store_root, kind)?;
    let current = kind.current();
    if found == current {
        return Ok(());
    }
    if found > current {
        bail!(
            "this {} is format {found}, and this build of Convergence reads {current}.\n\
             It was written by a newer version. Upgrade Convergence, or point at a \n\
             different {}.\n\
             Nothing has been read or written.",
            kind.what(),
            kind.what()
        );
    }
    bail!(
        "this {} is format {found}, and this build of Convergence reads {current}.\n\
         It was written by an older version and cannot be read safely — the risk is \n\
         a silent misread, not a crash.\n\
         Use a Convergence that reads format {found}, or start a new {} and \n\
         re-publish into it.\n\
         Nothing has been read or written.",
        kind.what(),
        kind.what()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store written before the stamp existed reads as version 1 and
    /// is not touched — opening a store must stay a pure read.
    #[test]
    fn an_unstamped_store_is_version_one_and_stays_unstamped() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            read_version(dir.path(), StoreKind::Workspace).expect("read"),
            1
        );
        check_compatible(dir.path(), StoreKind::Workspace).expect("compatible");
        assert!(
            !dir.path().join(FORMAT_FILE).exists(),
            "checking compatibility wrote to the store"
        );
    }

    #[test]
    fn a_stamped_store_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_version(dir.path(), StoreKind::Server).expect("write");
        assert_eq!(
            read_version(dir.path(), StoreKind::Server).expect("read"),
            StoreKind::Server.current()
        );
        check_compatible(dir.path(), StoreKind::Server).expect("compatible");
    }

    /// The dangerous direction, and the one people actually hit: a newer
    /// store met by an older binary.
    #[test]
    fn a_newer_store_is_refused_by_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(FORMAT_FILE), "converge-workspace-99\n").expect("write");
        let err = check_compatible(dir.path(), StoreKind::Workspace).expect_err("refused");
        let message = format!("{err}");
        assert!(message.contains("format 99"), "{message}");
        assert!(
            message.contains("Upgrade Convergence"),
            "the refusal has to say what to do: {message}"
        );
        assert!(
            message.contains("Nothing has been read or written"),
            "the refusal has to say it was safe: {message}"
        );
    }

    /// The other direction still refuses rather than trying its luck.
    #[test]
    fn an_older_store_is_refused_rather_than_guessed_at() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(FORMAT_FILE), "converge-workspace-0\n").expect("write");
        let err = check_compatible(dir.path(), StoreKind::Workspace).expect_err("refused");
        assert!(format!("{err}").contains("silent misread"), "{err}");
    }

    /// A workspace passed where a data directory belongs is a different
    /// mistake with a different fix, so it gets a different message.
    #[test]
    fn a_stamp_of_the_wrong_kind_says_which_kind_it_is() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_version(dir.path(), StoreKind::Workspace).expect("write");
        let err = check_compatible(dir.path(), StoreKind::Server).expect_err("refused");
        let message = format!("{err}");
        assert!(
            message.contains("converge-workspace-1") && message.contains("converge-server"),
            "name both what it is and what was expected: {message}"
        );
    }
}
