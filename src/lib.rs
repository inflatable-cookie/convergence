// Salvage posture (g02.002): modules are kept whole while their g01-era
// callers are archived on `archive/g01`; this allow lifts with the rebuild.
#![allow(dead_code)]

pub mod diff;
pub mod model;
pub mod resolve;
pub mod store;
pub mod workspace;
