use super::*;

mod decisions;
mod jumps;

pub(super) use self::decisions::{superpositions_clear_decision, superpositions_pick_variant};
pub(super) use self::jumps::{superpositions_jump_next_invalid, superpositions_jump_next_missing};
