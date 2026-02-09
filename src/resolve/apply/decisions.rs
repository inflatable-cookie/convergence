use anyhow::Result;

use crate::model::{ResolutionDecision, SuperpositionVariant, VariantKey};

pub(super) fn decision_to_index(
    path: &str,
    decision: &ResolutionDecision,
    variants: &[SuperpositionVariant],
) -> Result<usize> {
    match decision {
        ResolutionDecision::Index(idx) => {
            let idx = *idx as usize;
            if idx >= variants.len() {
                anyhow::bail!(
                    "resolution decision out of range for {} (idx {}, variants {})",
                    path,
                    idx,
                    variants.len()
                );
            }
            Ok(idx)
        }
        ResolutionDecision::Key(key) => match find_variant_index_by_key(variants, key) {
            Some(i) => Ok(i),
            None => {
                let mut available = Vec::new();
                for v in variants {
                    let kj = serde_json::to_string(&v.key())
                        .unwrap_or_else(|_| "<unserializable-key>".to_string());
                    available.push(kj);
                }
                anyhow::bail!(
                    "resolution variant_key not found for {} (wanted source={}); available keys: {}",
                    path,
                    key.source,
                    available.join(", ")
                );
            }
        },
    }
}

fn find_variant_index_by_key(variants: &[SuperpositionVariant], key: &VariantKey) -> Option<usize> {
    variants.iter().position(|v| &v.key() == key)
}
