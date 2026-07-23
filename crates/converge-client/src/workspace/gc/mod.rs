mod execute;
mod prune;
mod reachability;
mod retention_plan;

#[derive(Clone, Debug, Default)]
pub(crate) struct GcReport {
    pub(crate) kept_snaps: usize,
    pub(crate) pruned_snaps: usize,
    pub(crate) deleted_blobs: usize,
    pub(crate) deleted_manifests: usize,
    pub(crate) deleted_recipes: usize,
}
