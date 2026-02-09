use super::*;

pub(in crate::cli_exec) fn handle_diff_command(
    from: Option<String>,
    to: Option<String>,
    json: bool,
) -> Result<()> {
    let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;

    let diffs = match (from.as_deref(), to.as_deref()) {
        (None, None) => {
            let head = ws.store.get_head()?.context("no HEAD snap")?;
            let head_snap = ws.store.get_snap(&head)?;
            let from_tree = converge::diff::tree_from_store(&ws.store, &head_snap.root_manifest)?;

            let (cur_root, cur_manifests, _stats) = ws.current_manifest_tree()?;
            let to_tree = converge::diff::tree_from_memory(&cur_manifests, &cur_root)?;

            converge::diff::diff_trees(&from_tree, &to_tree)
        }
        (Some(_), None) | (None, Some(_)) => {
            anyhow::bail!(
                "use both --from and --to for snap diffs, or omit both for workspace vs HEAD"
            )
        }
        (Some(from), Some(to)) => {
            let from_snap = ws.store.get_snap(from)?;
            let to_snap = ws.store.get_snap(to)?;
            let from_tree = converge::diff::tree_from_store(&ws.store, &from_snap.root_manifest)?;
            let to_tree = converge::diff::tree_from_store(&ws.store, &to_snap.root_manifest)?;
            converge::diff::diff_trees(&from_tree, &to_tree)
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&diffs).context("serialize diff json")?
        );
    } else {
        for d in &diffs {
            match d {
                converge::diff::DiffLine::Added { path, .. } => println!("A {}", path),
                converge::diff::DiffLine::Deleted { path, .. } => println!("D {}", path),
                converge::diff::DiffLine::Modified { path, .. } => println!("M {}", path),
            }
        }
        println!("{} changes", diffs.len());
    }
    Ok(())
}
