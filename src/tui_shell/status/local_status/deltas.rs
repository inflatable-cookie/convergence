use crate::model::ObjectId;
use crate::workspace::Workspace;

use super::super::rename_helpers::{IdentityKey, StatusChange};
use super::super::text_delta::{count_lines_utf8, line_delta_utf8};
use super::identity_maps::IdentityMap;

pub(super) fn line_delta_for_change(
    ws: &Workspace,
    change: &StatusChange,
    base_ids: Option<&IdentityMap>,
    cur_ids: &IdentityMap,
) -> Option<(usize, usize)> {
    match change {
        StatusChange::Added(path) => {
            if let Some(IdentityKey::Blob(_)) = cur_ids.get(path) {
                let bytes = std::fs::read(ws.root.join(std::path::Path::new(path))).ok();
                bytes.and_then(|b| count_lines_utf8(&b)).map(|n| (n, 0))
            } else {
                None
            }
        }
        StatusChange::Deleted(path) => {
            let id = base_ids.and_then(|m| m.get(path));
            if let Some(IdentityKey::Blob(bid)) = id {
                let bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                bytes.and_then(|b| count_lines_utf8(&b)).map(|n| (0, n))
            } else {
                None
            }
        }
        StatusChange::Modified(path) => {
            let base = base_ids.and_then(|m| m.get(path));
            let cur = cur_ids.get(path);
            if let (Some(IdentityKey::Blob(bid)), Some(IdentityKey::Blob(_))) = (base, cur) {
                let old_bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                let new_bytes = std::fs::read(ws.root.join(std::path::Path::new(path))).ok();
                if let (Some(a), Some(b)) = (old_bytes, new_bytes) {
                    line_delta_utf8(&a, &b)
                } else {
                    None
                }
            } else {
                None
            }
        }
        StatusChange::Renamed { from, to, modified } => {
            if !*modified {
                return None;
            }
            let base = base_ids.and_then(|m| m.get(from));
            let cur = cur_ids.get(to);
            if let (Some(IdentityKey::Blob(bid)), Some(IdentityKey::Blob(_))) = (base, cur) {
                let old_bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                let new_bytes = std::fs::read(ws.root.join(std::path::Path::new(to))).ok();
                if let (Some(a), Some(b)) = (old_bytes, new_bytes) {
                    line_delta_utf8(&a, &b)
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}
