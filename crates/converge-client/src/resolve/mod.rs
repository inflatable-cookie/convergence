mod apply;
mod types;
mod validate;
mod variants;

pub use self::apply::apply_resolution;
pub use self::types::{InvalidKeyDecision, OutOfRangeDecision, ResolutionValidation};
pub use self::validate::validate_resolution;
pub use self::variants::{superposition_variant_counts, superposition_variants};
