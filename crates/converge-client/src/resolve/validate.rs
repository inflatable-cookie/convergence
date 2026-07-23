use anyhow::Result;

use crate::model::{ObjectId, ResolutionDecision};
use crate::store::LocalStore;

use super::types::{InvalidKeyDecision, OutOfRangeDecision, ResolutionValidation};
use super::variants::superposition_variants;

pub fn validate_resolution(
    store: &LocalStore,
    root: &ObjectId,
    decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
) -> Result<ResolutionValidation> {
    let variants = superposition_variants(store, root)?;

    let mut missing = Vec::new();
    for p in variants.keys() {
        if !decisions.contains_key(p) {
            missing.push(p.clone());
        }
    }

    let mut extraneous = Vec::new();
    for p in decisions.keys() {
        if !variants.contains_key(p) {
            extraneous.push(p.clone());
        }
    }

    let mut out_of_range = Vec::new();
    let mut invalid_keys = Vec::new();
    for (path, decision) in decisions {
        let Some(vs) = variants.get(path) else {
            continue;
        };

        match decision {
            ResolutionDecision::Index(i) => {
                let idx = *i as usize;
                if idx >= vs.len() {
                    out_of_range.push(OutOfRangeDecision {
                        path: path.clone(),
                        index: *i,
                        variants: vs.len(),
                    });
                }
            }
            ResolutionDecision::Key(k) => {
                if !vs.iter().any(|v| &v.key() == k) {
                    invalid_keys.push(InvalidKeyDecision {
                        path: path.clone(),
                        wanted: k.clone(),
                        available: vs.iter().map(|v| v.key()).collect(),
                    });
                }
            }
        }
    }

    missing.sort();
    extraneous.sort();
    out_of_range.sort_by(|a, b| a.path.cmp(&b.path));
    invalid_keys.sort_by(|a, b| a.path.cmp(&b.path));

    let ok = missing.is_empty() && out_of_range.is_empty() && invalid_keys.is_empty();
    Ok(ResolutionValidation {
        ok,
        missing,
        extraneous,
        out_of_range,
        invalid_keys,
    })
}
