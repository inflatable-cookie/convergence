use std::collections::{HashMap, HashSet};

use crate::store::LocalStore;

use super::rename_helpers::{
    IdentityKey, blob_prefix_suffix_score, min_blob_rename_matched_bytes, min_blob_rename_score,
    min_recipe_rename_matched_chunks, min_recipe_rename_score, recipe_prefix_suffix_score,
};
use super::rename_io::{load_blob_bytes, load_recipe};

pub(super) struct RenameDetection {
    pub(super) renames: Vec<(String, String, bool)>,
    pub(super) consumed_added: HashSet<String>,
    pub(super) consumed_deleted: HashSet<String>,
}

pub(super) fn detect_renames(
    store: &LocalStore,
    workspace_root: Option<&std::path::Path>,
    chunk_size_bytes: usize,
    added: &[String],
    deleted: &[String],
    base_ids: &HashMap<String, IdentityKey>,
    cur_ids: &HashMap<String, IdentityKey>,
) -> RenameDetection {
    let mut added_by_id: HashMap<IdentityKey, Vec<String>> = HashMap::new();
    for p in added {
        if let Some(id) = cur_ids.get(p) {
            added_by_id.entry(id.clone()).or_default().push(p.clone());
        }
    }

    let mut deleted_by_id: HashMap<IdentityKey, Vec<String>> = HashMap::new();
    for p in deleted {
        if let Some(id) = base_ids.get(p) {
            deleted_by_id.entry(id.clone()).or_default().push(p.clone());
        }
    }

    let mut renames = Vec::new();
    let mut consumed_added: HashSet<String> = HashSet::new();
    let mut consumed_deleted: HashSet<String> = HashSet::new();

    for (id, dels) in &deleted_by_id {
        let Some(adds) = added_by_id.get(id) else {
            continue;
        };
        if dels.len() == 1 && adds.len() == 1 {
            let from = dels[0].clone();
            let to = adds[0].clone();
            consumed_deleted.insert(from.clone());
            consumed_added.insert(to.clone());
            renames.push((from, to, false));
        }
    }

    let mut ctx = MatchCtx {
        store,
        workspace_root,
        chunk_size_bytes,
        added,
        deleted,
        base_ids,
        cur_ids,
        consumed_added: &mut consumed_added,
        consumed_deleted: &mut consumed_deleted,
        renames: &mut renames,
    };

    detect_blob_edit_renames(&mut ctx);
    detect_recipe_edit_renames(&mut ctx);

    RenameDetection {
        renames,
        consumed_added,
        consumed_deleted,
    }
}

struct MatchCtx<'a> {
    store: &'a LocalStore,
    workspace_root: Option<&'a std::path::Path>,
    chunk_size_bytes: usize,
    added: &'a [String],
    deleted: &'a [String],
    base_ids: &'a HashMap<String, IdentityKey>,
    cur_ids: &'a HashMap<String, IdentityKey>,
    consumed_added: &'a mut HashSet<String>,
    consumed_deleted: &'a mut HashSet<String>,
    renames: &'a mut Vec<(String, String, bool)>,
}

fn detect_blob_edit_renames(ctx: &mut MatchCtx<'_>) {
    const MAX_BYTES: usize = 1024 * 1024;

    let mut remaining_added_blobs = Vec::new();
    for p in ctx.added {
        if ctx.consumed_added.contains(p) {
            continue;
        }
        let Some(id) = ctx.cur_ids.get(p) else {
            continue;
        };
        let IdentityKey::Blob(blob) = id else {
            continue;
        };
        remaining_added_blobs.push((p.clone(), blob.clone()));
    }

    let mut remaining_deleted_blobs = Vec::new();
    for p in ctx.deleted {
        if ctx.consumed_deleted.contains(p) {
            continue;
        }
        let Some(id) = ctx.base_ids.get(p) else {
            continue;
        };
        let IdentityKey::Blob(blob) = id else {
            continue;
        };
        remaining_deleted_blobs.push((p.clone(), blob.clone()));
    }

    let mut used_added: HashSet<String> = HashSet::new();
    for (from_path, from_blob) in remaining_deleted_blobs {
        let Some(from_bytes) = load_blob_bytes(ctx.store, None, "", &from_blob) else {
            continue;
        };
        if from_bytes.len() > MAX_BYTES {
            continue;
        }

        let mut best: Option<(String, String, f64)> = None;
        for (to_path, to_blob) in &remaining_added_blobs {
            if used_added.contains(to_path) {
                continue;
            }
            let Some(to_bytes) = load_blob_bytes(ctx.store, ctx.workspace_root, to_path, to_blob)
            else {
                continue;
            };
            if to_bytes.len() > MAX_BYTES {
                continue;
            }

            let diff = from_bytes.len().abs_diff(to_bytes.len());
            let max = from_bytes.len().max(to_bytes.len());
            if diff > 8192 && (diff as f64) / (max as f64) > 0.20 {
                continue;
            }

            let (prefix, suffix, max_len, score) = blob_prefix_suffix_score(&from_bytes, &to_bytes);
            let min_score = min_blob_rename_score(max_len);
            let min_matched = min_blob_rename_matched_bytes(max_len);
            if score >= min_score && (prefix + suffix) >= min_matched {
                match &best {
                    None => best = Some((to_path.clone(), to_blob.clone(), score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best = Some((to_path.clone(), to_blob.clone(), score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((to_path, _to_blob, _score)) = best {
            used_added.insert(to_path.clone());
            ctx.consumed_deleted.insert(from_path.clone());
            ctx.consumed_added.insert(to_path.clone());
            ctx.renames.push((from_path, to_path, true));
        }
    }
}

fn detect_recipe_edit_renames(ctx: &mut MatchCtx<'_>) {
    const MAX_CHUNKS: usize = 2048;

    let mut remaining_added_recipes = Vec::new();
    for p in ctx.added {
        if ctx.consumed_added.contains(p) {
            continue;
        }
        let Some(id) = ctx.cur_ids.get(p) else {
            continue;
        };
        let IdentityKey::Recipe(r) = id else {
            continue;
        };
        remaining_added_recipes.push((p.clone(), r.clone()));
    }

    let mut remaining_deleted_recipes = Vec::new();
    for p in ctx.deleted {
        if ctx.consumed_deleted.contains(p) {
            continue;
        }
        let Some(id) = ctx.base_ids.get(p) else {
            continue;
        };
        let IdentityKey::Recipe(r) = id else {
            continue;
        };
        remaining_deleted_recipes.push((p.clone(), r.clone()));
    }

    let mut used_added_recipe_paths: HashSet<String> = HashSet::new();
    for (from_path, from_recipe) in remaining_deleted_recipes {
        let Some(from_recipe_obj) =
            load_recipe(ctx.store, None, "", &from_recipe, ctx.chunk_size_bytes)
        else {
            continue;
        };
        if from_recipe_obj.chunks.len() > MAX_CHUNKS {
            continue;
        }

        let mut best: Option<(String, String, f64)> = None;
        for (to_path, to_recipe) in &remaining_added_recipes {
            if used_added_recipe_paths.contains(to_path) {
                continue;
            }
            let Some(to_recipe_obj) = load_recipe(
                ctx.store,
                ctx.workspace_root,
                to_path,
                to_recipe,
                ctx.chunk_size_bytes,
            ) else {
                continue;
            };
            if to_recipe_obj.chunks.len() > MAX_CHUNKS {
                continue;
            }

            let diff = from_recipe_obj
                .chunks
                .len()
                .abs_diff(to_recipe_obj.chunks.len());
            let max = from_recipe_obj.chunks.len().max(to_recipe_obj.chunks.len());
            if diff > 4 && (diff as f64) / (max as f64) > 0.20 {
                continue;
            }

            let (prefix, suffix, max_chunks, score) =
                recipe_prefix_suffix_score(&from_recipe_obj, &to_recipe_obj);
            let min_score = min_recipe_rename_score(max_chunks);
            let min_matched = min_recipe_rename_matched_chunks(max_chunks);
            if score >= min_score && (prefix + suffix) >= min_matched {
                match &best {
                    None => best = Some((to_path.clone(), to_recipe.clone(), score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best = Some((to_path.clone(), to_recipe.clone(), score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((to_path, _to_recipe, _score)) = best {
            used_added_recipe_paths.insert(to_path.clone());
            ctx.consumed_deleted.insert(from_path.clone());
            ctx.consumed_added.insert(to_path.clone());
            ctx.renames.push((from_path, to_path, true));
        }
    }
}
