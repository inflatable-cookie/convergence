use super::*;

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
