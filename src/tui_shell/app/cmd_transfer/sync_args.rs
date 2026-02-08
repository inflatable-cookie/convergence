const USAGE: &str = "usage: sync [snap <id>] [lane <id>] [client <id>]";

pub(super) struct SyncArgs {
    pub(super) snap_id: Option<String>,
    pub(super) lane: String,
    pub(super) client_id: Option<String>,
}

pub(super) fn parse_sync_args(args: &[String]) -> Result<SyncArgs, String> {
    let mut snap_id: Option<String> = None;
    let mut lane: String = "default".to_string();
    let mut client_id: Option<String> = None;

    // Flagless UX:
    // - `sync` (defaults to latest snap + lane=default)
    // - `sync <snap> [lane] [client]`
    // - `sync [snap <id>] [lane <id>] [client <id>]`
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
                "lane" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        return Err(USAGE.to_string());
                    };
                    lane = v.clone();
                }
                "client" | "client-id" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        return Err(USAGE.to_string());
                    };
                    client_id = Some(v.clone());
                }
                a => {
                    if snap_id.is_none() {
                        snap_id = Some(a.to_string());
                    } else if lane == "default" {
                        lane = a.to_string();
                    } else if client_id.is_none() {
                        client_id = Some(a.to_string());
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
                "--lane" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --lane".to_string());
                    }
                    lane = args[i].clone();
                }
                "--client-id" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("missing value for --client-id".to_string());
                    }
                    client_id = Some(args[i].clone());
                }
                a => {
                    return Err(format!("unknown arg: {}", a));
                }
            }
            i += 1;
        }
    }

    Ok(SyncArgs {
        snap_id,
        lane,
        client_id,
    })
}
