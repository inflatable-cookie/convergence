#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum IdentityKey {
    Blob(String),
    Recipe(String),
    Symlink(String),
}

#[derive(Clone, Debug)]
pub(super) enum StatusChange {
    Added(String),
    Modified(String),
    Deleted(String),
    Renamed {
        from: String,
        to: String,
        modified: bool,
    },
}

impl StatusChange {
    pub(super) fn sort_key(&self) -> (&str, &str) {
        match self {
            StatusChange::Added(p) => ("A", p.as_str()),
            StatusChange::Modified(p) => ("M", p.as_str()),
            StatusChange::Deleted(p) => ("D", p.as_str()),
            StatusChange::Renamed { from, .. } => ("R", from.as_str()),
        }
    }
}

pub(super) fn blob_prefix_suffix_score(a: &[u8], b: &[u8]) -> (usize, usize, usize, f64) {
    if a.is_empty() && b.is_empty() {
        return (0, 0, 0, 1.0);
    }

    let min = a.len().min(b.len());
    let max = a.len().max(b.len());
    if max == 0 {
        return (0, 0, 0, 1.0);
    }

    let mut prefix = 0usize;
    while prefix < min && a[prefix] == b[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (min - prefix) && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix] {
        suffix += 1;
    }

    let score = ((prefix + suffix) as f64) / (max as f64);
    (prefix, suffix, max, score)
}

pub(super) fn min_blob_rename_score(max_len: usize) -> f64 {
    // Adaptive threshold: small files should still rename-match after small edits.
    // Keep it conservative to avoid spurious matches.
    if max_len <= 512 {
        0.65
    } else if max_len <= 4 * 1024 {
        0.70
    } else if max_len <= 16 * 1024 {
        0.78
    } else {
        0.85
    }
}

pub(super) fn min_blob_rename_matched_bytes(max_len: usize) -> usize {
    // Guardrail for tiny files where many candidates might otherwise tie.
    if max_len <= 128 {
        max_len / 2
    } else if max_len <= 4 * 1024 {
        32
    } else {
        0
    }
}

pub(super) fn default_chunk_size_bytes() -> usize {
    // Keep in sync with workspace defaults.
    4 * 1024 * 1024
}

pub(super) fn recipe_prefix_suffix_score(
    a: &crate::model::FileRecipe,
    b: &crate::model::FileRecipe,
) -> (usize, usize, usize, f64) {
    let a_ids: Vec<&str> = a.chunks.iter().map(|c| c.blob.as_str()).collect();
    let b_ids: Vec<&str> = b.chunks.iter().map(|c| c.blob.as_str()).collect();

    if a_ids.is_empty() && b_ids.is_empty() {
        return (0, 0, 0, 1.0);
    }

    let min = a_ids.len().min(b_ids.len());
    let max = a_ids.len().max(b_ids.len());
    if max == 0 {
        return (0, 0, 0, 1.0);
    }

    let mut prefix = 0usize;
    while prefix < min && a_ids[prefix] == b_ids[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (min - prefix)
        && a_ids[a_ids.len() - 1 - suffix] == b_ids[b_ids.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let score = ((prefix + suffix) as f64) / (max as f64);
    (prefix, suffix, max, score)
}

pub(super) fn min_recipe_rename_score(max_chunks: usize) -> f64 {
    if max_chunks <= 8 {
        0.60
    } else if max_chunks <= 32 {
        0.75
    } else {
        0.90
    }
}

pub(super) fn min_recipe_rename_matched_chunks(max_chunks: usize) -> usize {
    if max_chunks <= 8 {
        2
    } else if max_chunks <= 32 {
        4
    } else {
        0
    }
}
