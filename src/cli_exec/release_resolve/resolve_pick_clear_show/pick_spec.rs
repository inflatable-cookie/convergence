use anyhow::Result;

#[derive(Debug)]
pub(super) enum PickSpecifier {
    VariantIndex(usize),
    KeyJson(String),
}

pub(super) fn parse_pick_specifier(
    variant: Option<u32>,
    key: Option<String>,
    variant_len: usize,
) -> Result<PickSpecifier> {
    match (variant, key) {
        (Some(_), Some(_)) => {
            anyhow::bail!("use either --variant or --key (not both)");
        }
        (None, None) => {
            anyhow::bail!("missing required flag: --variant or --key");
        }
        (Some(variant), None) => {
            if variant == 0 {
                anyhow::bail!("variant is 1-based (use --variant 1..{})", variant_len);
            }
            let idx = (variant - 1) as usize;
            if idx >= variant_len {
                anyhow::bail!("variant out of range (variants: {})", variant_len);
            }
            Ok(PickSpecifier::VariantIndex(idx))
        }
        (None, Some(key_json)) => Ok(PickSpecifier::KeyJson(key_json)),
    }
}

#[cfg(test)]
#[path = "../../../tests/cli_exec/release_resolve/resolve_pick_clear_show/pick_spec_tests.rs"]
mod tests;
