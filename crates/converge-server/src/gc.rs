//! Mark-and-sweep GC (g02.008 batch 8.3). Dry-run first discipline:
//! nothing reachable may ever be collected.
//!
//! The object store is shared across repos (content addressing dedups
//! cross-repo), so the mark phase spans **all** repos regardless of which
//! repo's admin triggered the run; retention-driven metadata drops apply
//! only to the triggering repo.

use std::collections::HashSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use converge_model::{FileRecipe, Manifest, ManifestEntryKind, ObjectId, SuperpositionVariantKind};

use crate::authz::{AuthzContext, Capability};
use crate::engine::Engine;
use crate::retention;
use crate::storage::ObjectKind;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GcReport {
    pub dry_run: bool,
    pub dropped_releases: u64,
    pub dropped_bundles: u64,
    pub dropped_publications: u64,
    pub pruned_events: u64,
    pub reachable_objects: u64,
    pub swept_objects: u64,
    pub swept_bytes: u64,
    /// Abandoned upload pins cleared. Reported so a deployment that has
    /// been leaking them can see it stop.
    pub expired_pins: u64,
}

/// How long an uploaded-but-unpublished object keeps its pin.
///
/// Upload and publish are seconds apart in the same command, so a day is
/// generous by orders of magnitude. It is deliberately not tight: the
/// cost of expiring too early is a failed publish, and the cost of
/// expiring too late is some disk for a day.
const PIN_GRACE_SECS: i64 = 24 * 60 * 60;

/// Seconds since the epoch, saturating rather than panicking on a clock
/// set before 1970.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Engine<'_> {
    /// Retention-driven metadata drops for `authz`'s repo, then a global
    /// mark from every repo's roots, then a grace-windowed sweep.
    pub fn gc(
        &self,
        authz: &AuthzContext,
        dry_run: bool,
        now: &str,
        grace: std::time::Duration,
    ) -> Result<GcReport> {
        if !self.meta.has_grant(
            authz.subject(),
            authz.repo_id(),
            "*",
            Capability::Admin.as_str(),
        )? {
            bail!("gc requires the admin capability");
        }
        let mut report = GcReport {
            dry_run,
            ..Default::default()
        };

        // --- retention: what drops from the triggering repo ---
        let policy = self.meta.get_retention(authz.repo_id())?;
        let releases = self.meta.list_releases(authz.repo_id())?;
        let dropped_release_bundles = retention::releases_to_drop(&releases, &policy);

        // Bundles referenced by *surviving* releases or serving as window
        // bases are protected.
        let dropped_release_set: HashSet<&String> = dropped_release_bundles.iter().collect();
        let mut protected: HashSet<String> = releases
            .iter()
            .filter(|r| !dropped_release_set.contains(&r.bundle_id))
            .map(|r| r.bundle_id.clone())
            .collect();
        let bundles = self.meta.list_bundles_all_scopes(authz.repo_id())?;
        for bundle in &bundles {
            if let Some(base) = &bundle.base_bundle_id {
                protected.insert(base.clone());
            }
        }

        // A publication declares the base it was written against, and the
        // merge re-reads that base every time the window is folded. Drop
        // the bundle it names and the fold cannot complete: every
        // subsequent publish to that gate fails, with the same error,
        // forever.
        //
        // Batch 22.4 did exactly this to a live repo by doing nothing
        // unusual — `retention set --keep-bundles 5` then `gc --execute`.
        // Two things made it permanent. Publications only leave a window
        // when it advances, a window only advances on promotion, and a
        // single-gate repo cannot promote (finding 33); and the client
        // retries against a base it re-derives, so it never stops asking.
        //
        // Enumerated over scopes and gates rather than partitions: a
        // partition row is only written once a window advances, so a repo
        // that has never promoted has publications and no partitions at
        // all — which is precisely the repo this happened to.
        //
        // A repo with no declared gate graph or no scopes has no
        // publications to protect, so absence is empty rather than an
        // error: making GC *require* a gate graph would fail it on repos
        // that never had one.
        let scopes = self.meta.list_scopes(authz.repo_id()).unwrap_or_default();
        let gates = self
            .meta
            .get_gate_graph(authz.repo_id())
            .unwrap_or(converge_model::GateGraph { gates: Vec::new() });
        for scope in &scopes {
            for gate in &gates.gates {
                for (_, publication) in
                    self.meta
                        .list_publications_after(authz.repo_id(), scope, &gate.gate_id, 0)?
                {
                    if let Some(base) = &publication.base_bundle_id {
                        protected.insert(base.clone());
                    }
                }
            }
        }

        let dropped_bundles = retention::bundles_to_drop(&bundles, &policy, &protected);

        let mut dropped_publications = Vec::new();
        for (scope, gate, floor) in self.meta.list_partitions(authz.repo_id())? {
            let publications: Vec<(u64, String, String)> = self
                .meta
                .list_publications_after(authz.repo_id(), &scope, &gate, 0)?
                .into_iter()
                .map(|(seq, p)| (seq, p.publication_id, p.created_at))
                .collect();
            dropped_publications.extend(retention::publications_to_drop(
                &publications,
                &policy,
                floor,
                now,
            ));
        }

        report.dropped_releases = dropped_release_bundles.len() as u64;
        report.dropped_bundles = dropped_bundles.len() as u64;
        report.dropped_publications = dropped_publications.len() as u64;

        if !dry_run {
            self.meta
                .delete_releases_for_bundles(authz.repo_id(), &dropped_release_bundles)?;
            self.meta
                .delete_bundles(authz.repo_id(), &dropped_bundles)?;
            self.meta
                .delete_publications(authz.repo_id(), &dropped_publications)?;
            // Events are hints, so they prune on count alone (batch 14.4);
            // pruning raises the repo's event floor so a stale cursor is
            // told it has a gap rather than silently missing history.
            if let Some(keep) = policy.keep_events {
                report.pruned_events = self.meta.prune_events(authz.repo_id(), keep)?;
            }
        }

        // --- mark: every repo's surviving roots ---
        // Global by necessity: the object store is deduplicated across
        // repos, so marking only this repo's roots would sweep another
        // repo's live content (doc 14 §2).
        let mut marked: HashSet<(ObjectKind, String)> = HashSet::new();
        let dropped_bundle_set: HashSet<&String> = dropped_bundles.iter().collect();
        let dropped_publication_set: HashSet<&String> = dropped_publications.iter().collect();
        for repo in self.meta.list_repos()? {
            let this_repo = repo == authz.repo_id();
            for bundle in self.meta.list_bundles_all_scopes(&repo)? {
                if this_repo && dropped_bundle_set.contains(&bundle.bundle_id) {
                    continue;
                }
                if let Some(root) = &bundle.root_manifest {
                    self.mark_manifest(root, &mut marked)?;
                }
            }
            for (scope, gate, _) in self.meta.list_partitions(&repo)? {
                for (_, publication) in
                    self.meta.list_publications_after(&repo, &scope, &gate, 0)?
                {
                    if this_repo && dropped_publication_set.contains(&publication.publication_id) {
                        continue;
                    }
                    self.mark_manifest(&publication.root_manifest, &mut marked)?;
                }
            }
            for lane in self.meta.list_lanes(&repo)? {
                if let Some(head) = self.meta.get_lane_head(&repo, &lane.lane_id)? {
                    self.mark_snap_lineage(&repo, &head.snap_id, &mut marked)?;
                }
            }
            for release in self.meta.list_releases(&repo)? {
                if this_repo && dropped_release_set.contains(&release.bundle_id) {
                    continue;
                }
                if let Ok(bundle) = self.meta.get_bundle(&release.bundle_id)
                    && let Some(root) = &bundle.root_manifest
                {
                    self.mark_manifest(root, &mut marked)?;
                }
            }
        }
        report.reachable_objects = marked.len() as u64;

        // --- sweep with grace window ---
        let pin_cutoff = unix_now() - PIN_GRACE_SECS;
        let cutoff = std::time::SystemTime::now()
            .checked_sub(grace)
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        for kind in [ObjectKind::Blob, ObjectKind::Manifest, ObjectKind::Recipe] {
            for (id, bytes, mtime) in self.objects.list(kind)? {
                if marked.contains(&(kind, id.as_str().to_string())) {
                    continue;
                }
                // Upload pins are the real protection for not-yet-referenced
                // objects (batch 12.2); the grace window only covers the
                // sub-millisecond store-write → pin-write gap.
                //
                // A pin is released when the tree it belongs to is
                // published. Batch 22.4 found that when that publish
                // never happens the pin stayed for the life of the
                // deployment — the table had no timestamp, so nothing
                // could tell a three-second-old upload from a
                // three-month-old abandoned one, and every aborted
                // publish leaked storage GC reported as unreachable and
                // declined to sweep on every run.
                if self.meta.is_object_pinned(kind, &id, pin_cutoff)? {
                    continue;
                }
                if mtime > cutoff {
                    continue;
                }
                report.swept_objects += 1;
                report.swept_bytes += bytes;
                if !dry_run {
                    self.objects.delete(kind, &id)?;
                    self.meta.remove_object_associations(kind, &id)?;
                }
            }
        }

        // Tidying only: the sweep above already ignored these, so this
        // changes no decision. It keeps the table from growing without
        // bound on a busy server, and it is skipped on a dry run
        // because a dry run must leave the deployment exactly as it
        // found it.
        if !dry_run {
            report.expired_pins = self.meta.sweep_stale_pins(pin_cutoff)?;
        }
        Ok(report)
    }

    fn mark_manifest(
        &self,
        id: &ObjectId,
        marked: &mut HashSet<(ObjectKind, String)>,
    ) -> Result<()> {
        if !marked.insert((ObjectKind::Manifest, id.as_str().to_string())) {
            return Ok(());
        }
        // A root may reference objects already swept in a previous run if
        // its metadata row was dropped between; tolerate missing reads.
        let Ok(bytes) = self.objects.get(ObjectKind::Manifest, id) else {
            return Ok(());
        };
        let manifest: Manifest = converge_model::encoding::decode_manifest(&bytes)?;
        for entry in manifest.entries {
            match entry.kind {
                ManifestEntryKind::File { blob, .. } => {
                    marked.insert((ObjectKind::Blob, blob.as_str().to_string()));
                }
                ManifestEntryKind::FileChunks { recipe, .. } => {
                    self.mark_recipe(&recipe, marked)?;
                }
                ManifestEntryKind::Dir { manifest } => self.mark_manifest(&manifest, marked)?,
                ManifestEntryKind::Symlink { .. } => {}
                ManifestEntryKind::Superposition { variants } => {
                    for variant in variants {
                        match variant.kind {
                            SuperpositionVariantKind::File { blob, .. } => {
                                marked.insert((ObjectKind::Blob, blob.as_str().to_string()));
                            }
                            SuperpositionVariantKind::FileChunks { recipe, .. } => {
                                self.mark_recipe(&recipe, marked)?;
                            }
                            SuperpositionVariantKind::Dir { manifest } => {
                                self.mark_manifest(&manifest, marked)?;
                            }
                            SuperpositionVariantKind::Symlink { .. }
                            | SuperpositionVariantKind::Tombstone => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn mark_recipe(&self, id: &ObjectId, marked: &mut HashSet<(ObjectKind, String)>) -> Result<()> {
        if !marked.insert((ObjectKind::Recipe, id.as_str().to_string())) {
            return Ok(());
        }
        let Ok(bytes) = self.objects.get(ObjectKind::Recipe, id) else {
            return Ok(());
        };
        let recipe: FileRecipe = converge_model::encoding::decode_recipe(&bytes)?;
        for chunk in recipe.chunks {
            marked.insert((ObjectKind::Blob, chunk.blob.as_str().to_string()));
        }
        Ok(())
    }

    fn mark_snap_lineage(
        &self,
        repo_id: &str,
        head: &str,
        marked: &mut HashSet<(ObjectKind, String)>,
    ) -> Result<()> {
        let mut stack = vec![head.to_string()];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(record) = self.meta.get_snap_record(repo_id, &id)? {
                self.mark_manifest(&record.root_manifest, marked)?;
                stack.extend(record.parents);
            }
        }
        Ok(())
    }
}
