use super::super::*;

pub(crate) fn repo_data_dir(state: &AppState, repo_id: &str) -> PathBuf {
    state.data_dir.join(repo_id)
}

pub(crate) fn repo_state_path(state: &AppState, repo_id: &str) -> PathBuf {
    repo_data_dir(state, repo_id).join("repo.json")
}

pub(crate) fn persist_repo(state: &AppState, repo: &Repo) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(repo).context("serialize repo")?;
    let path = repo_state_path(state, &repo.id);
    write_atomic_overwrite(&path, &bytes).context("write repo.json")?;
    Ok(())
}

pub(crate) fn load_bundle_from_disk(
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

pub(crate) fn write_if_absent(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
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

pub(crate) fn write_atomic_overwrite(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
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
