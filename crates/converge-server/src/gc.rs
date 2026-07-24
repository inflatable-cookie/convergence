//! Mark-and-sweep GC (g02.008 batch 8.3). Dry-run first discipline:
//! nothing reachable may ever be collected.
//!
//! The object store is shared across repos (content addressing dedups
//! cross-repo), so the mark phase spans **all** repos regardless of which
//! repo's admin triggered the run; retention-driven metadata drops apply
//! only to the triggering repo.

use std::collections::HashSet;

use anyhow::{Context, Result, bail};
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
    pub reachable_objects: u64,
    pub swept_objects: u64,
    pub swept_bytes: u64,
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
        }

        // --- mark: every repo's surviving roots ---
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
        let cutoff = std::time::SystemTime::now()
            .checked_sub(grace)
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        for kind in [ObjectKind::Blob, ObjectKind::Manifest, ObjectKind::Recipe] {
            for (id, bytes, mtime) in self.objects.list(kind)? {
                if marked.contains(&(kind, id.as_str().to_string())) {
                    continue;
                }
                if mtime > cutoff {
                    continue; // grace: possibly mid-upload
                }
                report.swept_objects += 1;
                report.swept_bytes += bytes;
                if !dry_run {
                    self.objects.delete(kind, &id)?;
                }
            }
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
        let manifest: Manifest = serde_json::from_slice(&bytes).context("parse manifest")?;
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
        let recipe: FileRecipe = serde_json::from_slice(&bytes).context("parse recipe")?;
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
