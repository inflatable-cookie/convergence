use super::*;

pub(in crate::cli_exec) fn handle_init_command(
    force: bool,
    path: Option<std::path::PathBuf>,
) -> Result<()> {
    let root = path.unwrap_or(std::env::current_dir().context("get current dir")?);
    Workspace::init(&root, force)?;
    println!("Initialized Convergence workspace at {}", root.display());
    Ok(())
}

pub(in crate::cli_exec) fn handle_snap_command(message: Option<String>, json: bool) -> Result<()> {
    let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
    let snap = ws.create_snap(message)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snap).context("serialize snap json")?
        );
    } else {
        println!("{}", snap.id);
    }
    Ok(())
}

pub(in crate::cli_exec) fn handle_snaps_command(json: bool) -> Result<()> {
    let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
    let snaps = ws.list_snaps()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snaps).context("serialize snaps json")?
        );
    } else {
        for snap in snaps {
            let short = snap.id.chars().take(8).collect::<String>();
            let msg = snap.message.unwrap_or_default();
            if msg.is_empty() {
                println!("{} {}", short, snap.created_at);
            } else {
                println!("{} {} {}", short, snap.created_at, msg);
            }
        }
    }
    Ok(())
}

pub(in crate::cli_exec) fn handle_show_command(snap_id: String, json: bool) -> Result<()> {
    let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
    let snap = ws.show_snap(&snap_id)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snap).context("serialize snap")?
        );
    } else {
        println!("id: {}", snap.id);
        println!("created_at: {}", snap.created_at);
        if let Some(msg) = snap.message
            && !msg.is_empty()
        {
            println!("message: {}", msg);
        }
        println!("root_manifest: {}", snap.root_manifest.as_str());
        println!(
            "stats: files={} dirs={} symlinks={} bytes={}",
            snap.stats.files, snap.stats.dirs, snap.stats.symlinks, snap.stats.bytes
        );
    }
    Ok(())
}

pub(in crate::cli_exec) fn handle_restore_command(snap_id: String, force: bool) -> Result<()> {
    let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
    ws.restore_snap(&snap_id, force)?;
    println!("Restored {}", snap_id);
    Ok(())
}

pub(in crate::cli_exec) fn handle_mv_command(from: String, to: String) -> Result<()> {
    let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
    ws.move_path(std::path::Path::new(&from), std::path::Path::new(&to))?;
    println!("Moved {} -> {}", from, to);
    Ok(())
}
