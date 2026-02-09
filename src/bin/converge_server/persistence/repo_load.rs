use super::super::*;

pub(crate) fn load_repos_from_disk(
    state: &AppState,
    handle_to_id: &HashMap<String, String>,
) -> Result<HashMap<String, Repo>> {
    let mut out = HashMap::new();
    if !state.data_dir.is_dir() {
        return Ok(out);
    }

    for entry in std::fs::read_dir(&state.data_dir).context("read data dir")? {
        let entry = entry.context("read data dir entry")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let repo_id = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-utf8 repo dir name"))?;

        let repo = load_repo_from_disk(state, &repo_id, handle_to_id)
            .with_context(|| format!("load repo {}", repo_id))?;
        out.insert(repo_id, repo);
    }

    Ok(out)
}

pub(crate) fn load_repo_from_disk(
    state: &AppState,
    repo_id: &str,
    handle_to_id: &HashMap<String, String>,
) -> Result<Repo> {
    let mut repo = if repo_state_path(state, repo_id).exists() {
        let bytes = std::fs::read(repo_state_path(state, repo_id)).context("read repo.json")?;
        serde_json::from_slice::<Repo>(&bytes).context("parse repo.json")?
    } else {
        default_repo_state(state, repo_id)
    };

    // Ensure id matches directory (best-effort).
    repo.id = repo_id.to_string();

    // Hydrate lists from existing on-disk records (needed for older data dirs).
    let snaps = load_snap_ids_from_disk(state, repo_id).unwrap_or_default();
    if !snaps.is_empty() {
        repo.snaps = snaps;
    }

    let bundles = load_bundles_from_disk(state, repo_id).unwrap_or_default();
    if !bundles.is_empty() {
        repo.bundles = bundles;
    }

    let promotions = load_promotions_from_disk(state, repo_id).unwrap_or_default();
    if !promotions.is_empty() {
        repo.promotions = promotions;
        repo.promotion_state = rebuild_promotion_state(&repo.promotions);
    }

    let releases = load_releases_from_disk(state, repo_id).unwrap_or_default();
    if !releases.is_empty() {
        repo.releases = releases;
    }

    // Backfill user_id fields for older on-disk records (best-effort).
    backfill_provenance_user_ids(&mut repo, handle_to_id);
    backfill_acl_user_ids(&mut repo, handle_to_id);

    Ok(repo)
}

pub(crate) fn load_snap_ids_from_disk(state: &AppState, repo_id: &str) -> Result<HashSet<String>> {
    let dir = repo_data_dir(state, repo_id).join("objects/snaps");
    if !dir.is_dir() {
        return Ok(HashSet::new());
    }

    let mut out = HashSet::new();
    for entry in std::fs::read_dir(&dir).context("read snaps dir")? {
        let entry = entry.context("read snaps dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.len() == 64 {
            out.insert(stem.to_string());
        }
    }
    Ok(out)
}

pub(crate) fn load_bundles_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Bundle>> {
    let dir = repo_data_dir(state, repo_id).join("bundles");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).context("read bundles dir")? {
        let entry = entry.context("read bundles dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let bundle: Bundle =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        out.push(bundle);
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub(crate) fn load_promotions_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Promotion>> {
    let dir = repo_data_dir(state, repo_id).join("promotions");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).context("read promotions dir")? {
        let entry = entry.context("read promotions dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let p: Promotion =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        out.push(p);
    }
    out.sort_by(|a, b| b.promoted_at.cmp(&a.promoted_at));
    Ok(out)
}

pub(crate) fn load_releases_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Release>> {
    let dir = repo_data_dir(state, repo_id).join("releases");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).context("read releases dir")? {
        let entry = entry.context("read releases dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let r: Release =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        out.push(r);
    }
    out.sort_by(|a, b| b.released_at.cmp(&a.released_at));
    Ok(out)
}

pub(crate) fn rebuild_promotion_state(
    promotions: &[Promotion],
) -> HashMap<String, HashMap<String, String>> {
    let mut tmp: HashMap<String, HashMap<String, (String, String)>> = HashMap::new();
    for p in promotions {
        let scope_entry = tmp.entry(p.scope.clone()).or_default();
        match scope_entry.get(&p.to_gate) {
            None => {
                scope_entry.insert(
                    p.to_gate.clone(),
                    (p.promoted_at.clone(), p.bundle_id.clone()),
                );
            }
            Some((prev_time, _prev_bundle)) => {
                if p.promoted_at > *prev_time {
                    scope_entry.insert(
                        p.to_gate.clone(),
                        (p.promoted_at.clone(), p.bundle_id.clone()),
                    );
                }
            }
        }
    }

    tmp.into_iter()
        .map(|(scope, m)| {
            let m = m
                .into_iter()
                .map(|(to_gate, (_t, bundle_id))| (to_gate, bundle_id))
                .collect::<HashMap<_, _>>();
            (scope, m)
        })
        .collect()
}
