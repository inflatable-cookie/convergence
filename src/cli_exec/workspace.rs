use super::*;

pub(super) fn discover_workspace() -> Result<Workspace> {
    Workspace::discover(&std::env::current_dir().context("get current dir")?)
}

pub(super) fn with_workspace<F>(f: F) -> Result<()>
where
    F: FnOnce(&Workspace) -> Result<()>,
{
    let ws = discover_workspace()?;
    f(&ws)
}
