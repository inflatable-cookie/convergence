mod chunk;
mod config;
mod ids;
mod manifest;
mod resolution;
mod snap;
mod wire;

pub use self::chunk::{ChunkParams, RECIPE_VERSION_CDC, chunk_data};

pub fn chunk_recipe_version() -> u32 {
    RECIPE_VERSION_CDC
}
pub use self::config::{
    ChunkingConfig, LaneSyncRecord, RemoteConfig, RetentionConfig, WorkflowProfile,
    WorkspaceConfig, WorkspaceState,
};
pub use self::ids::ObjectId;
pub use self::manifest::{
    Manifest, ManifestEntry, ManifestEntryKind, SuperpositionVariant, SuperpositionVariantKind,
};
pub use self::resolution::{Resolution, ResolutionDecision, VariantKey, VariantKeyKind};
pub use self::snap::{FileRecipe, FileRecipeChunk, SnapRecord, SnapStats, compute_snap_id};
pub use self::wire::{
    AddLaneMemberRequest, ApproveRequest, BundleProvenance, BundleRecord, BundleStatus,
    CreateLaneRequest, GateGraph, GateNode, InboxBundle, InboxLane, InboxPublication, InboxReport,
    LaneHead, LaneRecord, NegotiateRequest, NegotiateResponse, ObjectSet, PromoteRequest,
    PublicationRecord, PublishRequest, ReleaseRecord, ReleaseRequest, RetentionPolicy,
    SetLaneHeadRequest, WIRE_VERSION,
};
