use super::super::*;

pub(crate) fn backfill_provenance_user_ids(
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

pub(crate) fn backfill_acl_user_ids(repo: &mut Repo, handle_to_id: &HashMap<String, String>) {
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

pub(crate) fn default_repo_state(state: &AppState, repo_id: &str) -> Repo {
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
