const USAGE: &str = "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]";

pub(super) struct PublishArgs {
    pub(super) snap_id: Option<String>,
    pub(super) scope: Option<String>,
    pub(super) gate: Option<String>,
    pub(super) metadata_only: bool,
}

pub(super) fn parse_publish_args(args: &[String]) -> Result<PublishArgs, String> {
    let mut snap_id: Option<String> = None;
    let mut scope: Option<String> = None;
    let mut gate: Option<String> = None;
    let mut metadata_only = false;

    // Flagless UX:
    // - `publish` (defaults to latest snap + configured scope/gate)
    // - `publish <snap> [scope] [gate]`
    // - `publish [snap <id>] [scope <id>] [gate <id>] [meta]`
    if !args.iter().any(|a| a.starts_with("--")) {
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "snap" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        return Err(USAGE.to_string());
                    };
                    snap_id = Some(v.clone());
                }
                "scope" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        return Err(USAGE.to_string());
                    };
                    scope = Some(v.clone());
                }
                "gate" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        return Err(USAGE.to_string());
                    };
                    gate = Some(v.clone());
                }
                "meta" | "metadata" | "metadata-only" => {
                    metadata_only = true;
                }
                a => {
                    if snap_id.is_none() {
                        snap_id = Some(a.to_string());
                    } else if scope.is_none() {
                        scope = Some(a.to_string());
                    } else if gate.is_none() {
                        gate = Some(a.to_string());
                    } else {
                        return Err(USAGE.to_string());
                    }
                }
            }
            i += 1;
        }
    } else {
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--snap-id" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --snap-id".to_string());
                    }
                    snap_id = Some(args[i].clone());
                }
                "--scope" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --scope".to_string());
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --gate".to_string());
                    }
                    gate = Some(args[i].clone());
                }
                "--metadata-only" => {
                    metadata_only = true;
                }
                a => {
                    return Err(format!("unknown arg: {}", a));
                }
            }
            i += 1;
        }
    }

    Ok(PublishArgs {
        snap_id,
        scope,
        gate,
        metadata_only,
    })
}
