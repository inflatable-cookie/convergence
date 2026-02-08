use super::*;

pub(super) fn repo_data_dir(state: &AppState, repo_id: &str) -> PathBuf {
    state.data_dir.join(repo_id)
}

pub(super) fn repo_state_path(state: &AppState, repo_id: &str) -> PathBuf {
    repo_data_dir(state, repo_id).join("repo.json")
}

pub(super) fn persist_repo(state: &AppState, repo: &Repo) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(repo).context("serialize repo")?;
    let path = repo_state_path(state, &repo.id);
    write_atomic_overwrite(&path, &bytes).context("write repo.json")?;
    Ok(())
}

pub(super) fn load_repos_from_disk(
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

pub(super) fn load_repo_from_disk(
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

pub(super) fn backfill_provenance_user_ids(
    repo: &mut Repo,
    handle_to_id: &HashMap<String, String>,
) {
    for p in &mut repo.publications {
        if p.publisher_user_id.is_none() {
            p.publisher_user_id = handle_to_id.get(&p.publisher).cloned();
        }
    }
    for b in &mut repo.bundles {
        if b.created_by_user_id.is_none() {
            b.created_by_user_id = handle_to_id.get(&b.created_by).cloned();
        }
        if b.approval_user_ids.is_empty() && !b.approvals.is_empty() {
            for a in &b.approvals {
                if let Some(id) = handle_to_id.get(a) {
                    b.approval_user_ids.push(id.clone());
                }
            }
            b.approval_user_ids.sort();
            b.approval_user_ids.dedup();
        }
    }
    for p in &mut repo.promotions {
        if p.promoted_by_user_id.is_none() {
            p.promoted_by_user_id = handle_to_id.get(&p.promoted_by).cloned();
        }
    }

    for r in &mut repo.releases {
        if r.released_by_user_id.is_none() {
            r.released_by_user_id = handle_to_id.get(&r.released_by).cloned();
        }
    }
}

pub(super) fn backfill_acl_user_ids(repo: &mut Repo, handle_to_id: &HashMap<String, String>) {
    if repo.owner_user_id.is_none() {
        repo.owner_user_id = handle_to_id.get(&repo.owner).cloned();
    }
    if repo.reader_user_ids.is_empty() && !repo.readers.is_empty() {
        for h in &repo.readers {
            if let Some(id) = handle_to_id.get(h) {
                repo.reader_user_ids.insert(id.clone());
            }
        }
    }
    if repo.publisher_user_ids.is_empty() && !repo.publishers.is_empty() {
        for h in &repo.publishers {
            if let Some(id) = handle_to_id.get(h) {
                repo.publisher_user_ids.insert(id.clone());
            }
        }
    }

    for lane in repo.lanes.values_mut() {
        if lane.member_user_ids.is_empty() && !lane.members.is_empty() {
            for h in &lane.members {
                if let Some(id) = handle_to_id.get(h) {
                    lane.member_user_ids.insert(id.clone());
                }
            }
        }
    }
}

pub(super) fn default_repo_state(state: &AppState, repo_id: &str) -> Repo {
    let mut readers = HashSet::new();
    readers.insert(state.default_user.clone());
    let reader_user_ids = HashSet::new();
    let mut publishers = HashSet::new();
    publishers.insert(state.default_user.clone());
    let publisher_user_ids = HashSet::new();

    let mut members = HashSet::new();
    members.insert(state.default_user.clone());
    let member_user_ids = HashSet::new();
    let default_lane = Lane {
        id: "default".to_string(),
        members,
        member_user_ids,
        heads: HashMap::new(),
        head_history: HashMap::new(),
    };
    let mut lanes = HashMap::new();
    lanes.insert(default_lane.id.clone(), default_lane);

    let gate_graph = GateGraph {
        version: 1,
        gates: vec![GateDef {
            id: "dev-intake".to_string(),
            name: "Dev Intake".to_string(),
            upstream: vec![],
            allow_releases: true,
            allow_superpositions: false,
            allow_metadata_only_publications: false,
            required_approvals: 0,
        }],
    };

    let mut scopes = HashSet::new();
    scopes.insert("main".to_string());

    Repo {
        id: repo_id.to_string(),
        owner: state.default_user.clone(),
        owner_user_id: None,
        readers,
        reader_user_ids,
        publishers,
        publisher_user_ids,
        lanes,
        gate_graph,
        scopes,
        snaps: HashSet::new(),
        publications: Vec::new(),
        bundles: Vec::new(),
        pinned_bundles: HashSet::new(),
        promotions: Vec::new(),
        promotion_state: HashMap::new(),

        releases: Vec::new(),
    }
}

pub(super) fn load_snap_ids_from_disk(state: &AppState, repo_id: &str) -> Result<HashSet<String>> {
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

pub(super) fn load_bundles_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Bundle>> {
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

pub(super) fn load_bundle_from_disk(
    state: &AppState,
    repo_id: &str,
    bundle_id: &str,
) -> Result<Bundle, Response> {
    let path = repo_data_dir(state, repo_id)
        .join("bundles")
        .join(format!("{}.json", bundle_id));
    if !path.exists() {
        return Err(not_found());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let bundle: Bundle =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(bundle)
}

pub(super) fn load_promotions_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Promotion>> {
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

pub(super) fn load_releases_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Release>> {
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

pub(super) fn rebuild_promotion_state(
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

pub(super) fn write_if_absent(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub(super) fn write_atomic_overwrite(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}
