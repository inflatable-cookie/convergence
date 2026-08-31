//! Tree listing and variant previews for choosers.
use anyhow::{Context, Result};
use serde::Serialize;

use converge_client::model::ObjectId;
use converge_client::workspace::Workspace;

/// One row of a `show` listing.
#[derive(Serialize)]
pub(crate) struct TreeEntry {
    pub(crate) name: String,
    pub(crate) kind: &'static str,
    pub(crate) size: Option<u64>,
    /// Variant count when the path is superposed — the reason `show`
    /// exists is to look at a tree before deciding, so an unresolved path
    /// must be visible as such rather than rendered as a file.
    pub(crate) variants: Option<usize>,
}

/// List one directory of a stored tree (batch 16.2, audit P4.18).
pub(crate) fn list_tree(ws: &Workspace, root: &ObjectId, path: &str) -> Result<Vec<TreeEntry>> {
    use converge_client::model::ManifestEntryKind as Kind;

    let mut current = root.clone();
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        let manifest = ws.store.get_manifest(&current)?;
        let entry = manifest
            .entries
            .into_iter()
            .find(|e| e.name == segment)
            .with_context(|| format!("{path}: no such path in this tree"))?;
        match entry.kind {
            Kind::Dir { manifest } => current = manifest,
            _ => anyhow::bail!("{path}: not a directory"),
        }
    }

    Ok(ws
        .store
        .get_manifest(&current)?
        .entries
        .into_iter()
        .map(|entry| match entry.kind {
            Kind::Dir { .. } => TreeEntry {
                name: format!("{}/", entry.name),
                kind: "dir",
                size: None,
                variants: None,
            },
            Kind::File { size, .. } | Kind::FileChunks { size, .. } => TreeEntry {
                name: entry.name,
                kind: "file",
                size: Some(size),
                variants: None,
            },
            Kind::Symlink { .. } => TreeEntry {
                name: entry.name,
                kind: "symlink",
                size: None,
                variants: None,
            },
            Kind::Superposition { variants } => TreeEntry {
                name: entry.name,
                kind: "superposition",
                size: None,
                variants: Some(variants.len()),
            },
        })
        .collect())
}

/// A bounded look at one variant's content (g02.023 batch 23.5).
pub(crate) struct VariantPreview {
    /// Empty when there is nothing readable to show; `why` says so.
    pub(crate) text: String,
    /// True when the content continues past what is shown.
    pub(crate) elided: bool,
    /// Why there is no text — and, for a binary, its size, because two
    /// variants both labelled "binary" are not a choice.
    pub(crate) why: String,
}

/// Bytes read before deciding a variant is not previewable text.
///
/// A variant can be a 4 GB render; the point of a preview is to tell two
/// versions apart, and nobody does that past a screenful. Chunked files
/// read only their first chunk, so the bound holds on the store as well
/// as on the output.
const PREVIEW_BYTES: usize = 2048;
const PREVIEW_LINES: usize = 12;

/// Drop the leading lines every variant shares, and report how many.
///
/// Only applies when more than one variant has text: with a single
/// readable variant there is nothing to compare against, and trimming
/// would hide the start of the only thing on offer.
pub(crate) fn trim_common_prefix(previews: &mut [VariantPreview]) -> usize {
    let textual: Vec<usize> = previews
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.text.is_empty())
        .map(|(i, _)| i)
        .collect();
    if textual.len() < 2 {
        return 0;
    }
    let lines: Vec<Vec<&str>> = textual
        .iter()
        .map(|i| previews[*i].text.lines().collect())
        .collect();
    let shortest = lines.iter().map(Vec::len).min().unwrap_or(0);
    // Never trim everything: if the variants agree over the whole
    // preview, the difference is past the budget and showing the head is
    // better than showing nothing.
    let mut common = 0;
    while common < shortest.saturating_sub(1) && lines.iter().all(|l| l[common] == lines[0][common])
    {
        common += 1;
    }
    if common == 0 {
        return 0;
    }
    for i in textual {
        previews[i].text = previews[i]
            .text
            .lines()
            .skip(common)
            .collect::<Vec<_>>()
            .join("\n");
    }
    common
}

/// Render a variant for a chooser, or say why it cannot be rendered.
///
/// Refusing to guess is the point: a resolution view that showed
/// mojibake for a binary would be worse than one that says "binary". A
/// preview exists so somebody can tell two versions apart, and an
/// honest "these are both binaries, 4.1 MB and 4.3 MB" does that better
/// than two screens of replacement characters.
pub(crate) fn variant_preview(
    store: &converge_client::store::LocalStore,
    key: &converge_client::model::VariantKey,
) -> VariantPreview {
    use converge_client::model::VariantKeyKind as K;
    let empty = |why: &str| VariantPreview {
        text: String::new(),
        elided: false,
        why: why.to_string(),
    };
    let declared_size = match &key.kind {
        K::File { size, .. } | K::ChunkedFile { size, .. } => Some(*size),
        _ => None,
    };
    let bytes = match &key.kind {
        K::File { blob, .. } => match store.get_blob(blob) {
            Ok(bytes) => bytes,
            // A variant whose blob is not local yet is normal for a
            // candidate fetched lazily; saying so beats an error.
            Err(_) => return empty("content not in the local store"),
        },
        K::ChunkedFile { recipe, .. } => {
            let Ok(recipe) = store.get_recipe(recipe) else {
                return empty("content not in the local store");
            };
            let Some(first) = recipe.chunks.first() else {
                return empty("empty file");
            };
            match store.get_blob(&first.blob) {
                Ok(bytes) => bytes,
                Err(_) => return empty("content not in the local store"),
            }
        }
        K::Dir { .. } => return empty("directory"),
        K::Symlink { target } => {
            return VariantPreview {
                text: format!("-> {target}"),
                elided: false,
                why: "symlink".to_string(),
            };
        }
        // Not an absence of content: a deliberate deletion, and the
        // chooser needs to see it as a real option.
        K::Tombstone => return empty("deleted in this variant"),
    };

    let looked_at = bytes.len().min(PREVIEW_BYTES);
    let head = &bytes[..looked_at];
    // A NUL in the first couple of kilobytes is the same heuristic
    // `git diff` uses, and it is right far more often than it is wrong.
    if head.contains(&0) {
        // Size included: two variants both labelled "binary" and nothing
        // else are not a choice, and the size is usually the thing that
        // tells a 4.1 MB render from a 4.3 MB one.
        return empty(&match declared_size {
            Some(size) => format!("binary, {size} bytes"),
            None => "binary".to_string(),
        });
    }
    let Ok(text) = std::str::from_utf8(head) else {
        // Could be a multi-byte character straddling the cut rather than
        // real binary, but the distinction does not change what we show.
        return empty("not valid UTF-8");
    };
    let mut lines: Vec<&str> = text.lines().take(PREVIEW_LINES).collect();
    let elided = bytes.len() > looked_at || text.lines().count() > lines.len();
    if elided && lines.len() == PREVIEW_LINES {
        lines.truncate(PREVIEW_LINES);
    }
    VariantPreview {
        text: lines.join("\n"),
        elided,
        why: String::new(),
    }
}
