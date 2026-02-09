use anyhow::Result;

use crate::model::{ObjectId, ResolutionDecision};
use crate::store::LocalStore;

use super::super::validate_resolution;

pub(super) fn ensure_resolution_valid(
    store: &LocalStore,
    root: &ObjectId,
    decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
) -> Result<()> {
    // Validate up front so users get a single actionable error.
    let report = validate_resolution(store, root, decisions)?;
    if !report.ok {
        let mut parts = Vec::new();
        if !report.missing.is_empty() {
            parts.push(format!("missing=[{}]", head(&report.missing)));
        }
        if !report.extraneous.is_empty() {
            parts.push(format!("extraneous=[{}]", head(&report.extraneous)));
        }
        if !report.out_of_range.is_empty() {
            parts.push(format!("out_of_range={}", report.out_of_range.len()));
        }
        if !report.invalid_keys.is_empty() {
            parts.push(format!("invalid_keys={}", report.invalid_keys.len()));
        }
        anyhow::bail!("resolution invalid: {}", parts.join(" "));
    }

    Ok(())
}

fn head(xs: &[String]) -> String {
    const LIMIT: usize = 10;
    if xs.len() <= LIMIT {
        xs.join(", ")
    } else {
        format!("{} ... (+{})", xs[..LIMIT].join(", "), xs.len() - LIMIT)
    }
}
