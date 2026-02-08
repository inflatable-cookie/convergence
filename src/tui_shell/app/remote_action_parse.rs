#[derive(Debug, Default)]
pub(super) struct PinArgs {
    pub(super) bundle_id: Option<String>,
    pub(super) unpin: bool,
}

#[derive(Debug, Default)]
pub(super) struct ApproveArgs {
    pub(super) bundle_id: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct PromoteArgs {
    pub(super) bundle_id: Option<String>,
    pub(super) to_gate: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct ReleaseArgs {
    pub(super) channel: Option<String>,
    pub(super) bundle_id: Option<String>,
    pub(super) notes: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct SuperpositionsArgs {
    pub(super) bundle_id: Option<String>,
    pub(super) filter: Option<String>,
}

pub(super) fn parse_pin_args(args: &[String]) -> Result<PinArgs, String> {
    let mut out = PinArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle-id" | "bundle" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --bundle-id".to_string());
                }
                out.bundle_id = Some(args[i].clone());
            }
            "--unpin" | "unpin" => {
                out.unpin = true;
            }
            a => {
                if !a.starts_with("--") && out.bundle_id.is_none() {
                    out.bundle_id = Some(a.to_string());
                } else {
                    return Err(format!("unknown arg: {}", a));
                }
            }
        }
        i += 1;
    }
    Ok(out)
}

pub(super) fn parse_approve_args(args: &[String]) -> Result<ApproveArgs, String> {
    let mut out = ApproveArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle-id" | "bundle" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --bundle-id".to_string());
                }
                out.bundle_id = Some(args[i].clone());
            }
            a => {
                if !a.starts_with("--") && out.bundle_id.is_none() {
                    out.bundle_id = Some(a.to_string());
                } else {
                    return Err(format!("unknown arg: {}", a));
                }
            }
        }
        i += 1;
    }
    Ok(out)
}

pub(super) fn parse_promote_args(args: &[String]) -> Result<PromoteArgs, String> {
    let mut out = PromoteArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle-id" | "bundle" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --bundle-id".to_string());
                }
                out.bundle_id = Some(args[i].clone());
            }
            "--to-gate" | "to" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --to-gate".to_string());
                }
                out.to_gate = Some(args[i].clone());
            }
            a => {
                if !a.starts_with("--") {
                    if out.bundle_id.is_none() {
                        out.bundle_id = Some(a.to_string());
                    } else if out.to_gate.is_none() {
                        out.to_gate = Some(a.to_string());
                    } else {
                        return Err(format!("unknown arg: {}", a));
                    }
                } else {
                    return Err(format!("unknown arg: {}", a));
                }
            }
        }
        i += 1;
    }
    Ok(out)
}

pub(super) fn parse_release_args(args: &[String]) -> Result<ReleaseArgs, String> {
    let mut out = ReleaseArgs::default();

    if !args.iter().any(|a| a.starts_with("--")) && args.len() >= 2 {
        out.channel = Some(args[0].clone());
        out.bundle_id = Some(args[1].clone());
        if args.len() > 2 {
            out.notes = Some(args[2..].join(" "));
        }
    }

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--channel" | "channel" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --channel".to_string());
                }
                out.channel = Some(args[i].clone());
            }
            "--bundle-id" | "bundle" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --bundle-id".to_string());
                }
                out.bundle_id = Some(args[i].clone());
            }
            "--notes" | "notes" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --notes".to_string());
                }
                out.notes = Some(args[i].clone());
            }
            a => {
                if a.starts_with("--") {
                    return Err(format!("unknown arg: {}", a));
                }
            }
        }
        i += 1;
    }

    Ok(out)
}

pub(super) fn parse_superpositions_args(args: &[String]) -> Result<SuperpositionsArgs, String> {
    let mut out = SuperpositionsArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle-id" | "bundle" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --bundle-id".to_string());
                }
                out.bundle_id = Some(args[i].clone());
            }
            "--filter" | "filter" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --filter".to_string());
                }
                out.filter = Some(args[i].clone());
            }
            a => {
                if !a.starts_with("--") && out.bundle_id.is_none() {
                    out.bundle_id = Some(a.to_string());
                } else {
                    return Err(format!("unknown arg: {}", a));
                }
            }
        }
        i += 1;
    }
    Ok(out)
}
