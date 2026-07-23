// Salvage posture (g02.003 Batch 3.1): modules kept whole while their
// g01-era callers are archived on `archive/g01`; the allow lifts as the
// rebuild wires real callers (Batch 3.2+).
#![allow(dead_code)]

pub use converge_model as model;

pub mod diff;
pub mod resolve;
pub mod store;
pub mod workspace;
