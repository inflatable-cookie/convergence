pub(super) fn superposition_entry(
    name: String,
    kinds: Vec<(String, Option<converge::model::ManifestEntryKind>)>,
) -> converge::model::ManifestEntry {
    let mut variants = Vec::new();
    for (pub_id, kind) in kinds {
        let vkind = match kind {
            Some(converge::model::ManifestEntryKind::File { blob, mode, size }) => {
                converge::model::SuperpositionVariantKind::File { blob, mode, size }
            }
            Some(converge::model::ManifestEntryKind::FileChunks { recipe, mode, size }) => {
                converge::model::SuperpositionVariantKind::FileChunks { recipe, mode, size }
            }
            Some(converge::model::ManifestEntryKind::Dir { manifest }) => {
                converge::model::SuperpositionVariantKind::Dir { manifest }
            }
            Some(converge::model::ManifestEntryKind::Symlink { target }) => {
                converge::model::SuperpositionVariantKind::Symlink { target }
            }
            Some(converge::model::ManifestEntryKind::Superposition { variants }) => {
                let _ = variants;
                converge::model::SuperpositionVariantKind::Tombstone
            }
            None => converge::model::SuperpositionVariantKind::Tombstone,
        };
        variants.push(converge::model::SuperpositionVariant {
            source: pub_id,
            kind: vkind,
        });
    }

    converge::model::ManifestEntry {
        name,
        kind: converge::model::ManifestEntryKind::Superposition { variants },
    }
}
