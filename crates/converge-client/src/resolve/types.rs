use crate::model::VariantKey;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResolutionValidation {
    pub ok: bool,
    pub missing: Vec<String>,
    pub extraneous: Vec<String>,
    pub out_of_range: Vec<OutOfRangeDecision>,
    pub invalid_keys: Vec<InvalidKeyDecision>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OutOfRangeDecision {
    pub path: String,
    pub index: u32,
    pub variants: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InvalidKeyDecision {
    pub path: String,
    pub wanted: VariantKey,
    pub available: Vec<VariantKey>,
}
