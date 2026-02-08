use super::*;

pub(super) fn validate_object_id(id: &str) -> Result<()> {
    if id.len() != 64 {
        return Err(anyhow::anyhow!("object id must be 64 hex chars"));
    }
    if !id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(anyhow::anyhow!("object id must be lowercase hex"));
    }
    Ok(())
}

pub(super) fn validate_repo_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("repo id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!("repo id must be lowercase alnum or '-'"));
    }
    Ok(())
}

pub(super) fn validate_scope_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("scope id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '/')
    {
        return Err(anyhow::anyhow!(
            "scope id must be lowercase alnum or '-', '/'"
        ));
    }
    Ok(())
}

pub(super) fn validate_release_channel(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("release channel cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!(
            "release channel must be lowercase alnum or '-'"
        ));
    }
    Ok(())
}

pub(super) fn validate_user_handle(handle: &str) -> Result<()> {
    if handle.is_empty() {
        return Err(anyhow::anyhow!("user handle cannot be empty"));
    }
    if !handle
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!(
            "user handle must be lowercase alnum or '-'"
        ));
    }
    Ok(())
}

pub(super) fn validate_gate_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("gate id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!("gate id must be lowercase alnum or '-'"));
    }
    Ok(())
}

pub(super) fn validate_lane_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("lane id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!("lane id must be lowercase alnum or '-'"));
    }
    Ok(())
}
