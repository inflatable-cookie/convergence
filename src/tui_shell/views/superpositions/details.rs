use ratatui::text::Line;

use crate::model::{ResolutionDecision, SuperpositionVariantKind};

use super::SuperpositionsView;

pub(super) fn detail_lines(view: &SuperpositionsView) -> Vec<Line<'static>> {
    if view.items.is_empty() {
        return vec![Line::from("(no selection)")];
    }

    let idx = view.selected.min(view.items.len().saturating_sub(1));
    let (path, variants_count) = &view.items[idx];
    let mut out = Vec::new();
    out.push(Line::from(format!("path: {}", path)));
    out.push(Line::from(format!("variants: {}", variants_count)));
    out.push(Line::from(format!(
        "root_manifest: {}",
        view.root_manifest.as_str()
    )));

    if let Some(validation) = &view.validation {
        out.push(Line::from(""));
        out.push(Line::from(format!(
            "validation: {}",
            if validation.ok { "ok" } else { "invalid" }
        )));
        if !validation.missing.is_empty() {
            out.push(Line::from(format!("missing: {}", validation.missing.len())));
        }
        if !validation.invalid_keys.is_empty() {
            out.push(Line::from(format!(
                "invalid_keys: {}",
                validation.invalid_keys.len()
            )));
        }
        if !validation.out_of_range.is_empty() {
            out.push(Line::from(format!(
                "out_of_range: {}",
                validation.out_of_range.len()
            )));
        }
        if !validation.extraneous.is_empty() {
            out.push(Line::from(format!(
                "extraneous: {}",
                validation.extraneous.len()
            )));
        }
    }

    let chosen = view.decisions.get(path);
    out.push(Line::from(""));
    match chosen {
        None => out.push(Line::from("decision: (missing)")),
        Some(ResolutionDecision::Index(i)) => {
            out.push(Line::from(format!("decision: index {}", i)))
        }
        Some(ResolutionDecision::Key(key)) => {
            let key_json = serde_json::to_string(key).unwrap_or_else(|_| "<key>".to_string());
            out.push(Line::from(format!("decision: key {}", key_json)));
        }
    }

    if let Some(variants) = view.variants.get(path) {
        out.push(Line::from(""));
        out.push(Line::from("variants:"));
        for (i, variant) in variants.iter().enumerate() {
            let key_json =
                serde_json::to_string(&variant.key()).unwrap_or_else(|_| "<key>".to_string());
            out.push(Line::from(format!(
                "  #{} source={}",
                i + 1,
                variant.source
            )));
            out.push(Line::from(format!("    key={}", key_json)));
            match &variant.kind {
                SuperpositionVariantKind::File { blob, mode, size } => {
                    out.push(Line::from(format!(
                        "    file blob={} mode={:#o} size={}",
                        blob.as_str(),
                        mode,
                        size
                    )))
                }
                SuperpositionVariantKind::FileChunks { recipe, mode, size } => {
                    out.push(Line::from(format!(
                        "    chunked_file recipe={} mode={:#o} size={}",
                        recipe.as_str(),
                        mode,
                        size
                    )))
                }
                SuperpositionVariantKind::Dir { manifest } => out.push(Line::from(format!(
                    "    dir manifest={} ",
                    manifest.as_str()
                ))),
                SuperpositionVariantKind::Symlink { target } => {
                    out.push(Line::from(format!("    symlink target={}", target)))
                }
                SuperpositionVariantKind::Tombstone => out.push(Line::from("    tombstone")),
            }
        }
    }

    out
}
