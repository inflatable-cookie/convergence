pub(super) fn allow_empty_text_input(action: &super::super::TextInputAction) -> bool {
    matches!(
        action,
        super::super::TextInputAction::LoginScope
            | super::super::TextInputAction::LoginGate
            | super::super::TextInputAction::BootstrapDisplayName
            | super::super::TextInputAction::FetchId
            | super::super::TextInputAction::FetchUser
            | super::super::TextInputAction::FetchOptions
            | super::super::TextInputAction::PublishSnap
            | super::super::TextInputAction::PublishStart
            | super::super::TextInputAction::PublishScope
            | super::super::TextInputAction::PublishGate
            | super::super::TextInputAction::PublishMeta
            | super::super::TextInputAction::SyncStart
            | super::super::TextInputAction::SyncLane
            | super::super::TextInputAction::SyncClient
            | super::super::TextInputAction::SyncSnap
            | super::super::TextInputAction::ReleaseChannel
            | super::super::TextInputAction::ReleaseNotes
            | super::super::TextInputAction::PinAction
            | super::super::TextInputAction::MemberRole
            | super::super::TextInputAction::BrowseFilter
            | super::super::TextInputAction::BrowseLimit
            | super::super::TextInputAction::GateGraphAddGateUpstream
            | super::super::TextInputAction::GateGraphEditUpstream
    )
}

pub(super) fn validate_text_input(
    action: &super::super::TextInputAction,
    raw: &str,
) -> Result<(), String> {
    match action {
        super::super::TextInputAction::ChunkingSet => {
            let norm = raw.replace(',', " ");
            let parts = norm.split_whitespace().collect::<Vec<_>>();
            if parts.len() != 2 {
                Err("format: <chunk_size_mib> <threshold_mib>".to_string())
            } else {
                let chunk = parts[0].parse::<u64>().ok();
                let threshold = parts[1].parse::<u64>().ok();
                match (chunk, threshold) {
                    (Some(c), Some(t)) if c > 0 && t > 0 => {
                        if t < c {
                            Err("threshold must be >= chunk_size".to_string())
                        } else {
                            Ok(())
                        }
                    }
                    _ => Err("invalid number".to_string()),
                }
            }
        }
        super::super::TextInputAction::RetentionKeepLast
        | super::super::TextInputAction::RetentionKeepDays => {
            let v = raw.to_lowercase();
            if v == "unset" || v == "none" {
                Ok(())
            } else {
                match raw.parse::<u64>() {
                    Ok(n) if n > 0 => Ok(()),
                    _ => Err("expected a positive number (or 'unset')".to_string()),
                }
            }
        }
        _ => Ok(()),
    }
}
