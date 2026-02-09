use super::*;

pub(super) fn try_prompt_first_member(app: &mut App, args: &[String]) -> bool {
    // Prompt-first UX:
    // - `member` -> wizard
    // - `member add` / `member remove` -> wizard
    // - `member add <handle> [read|publish]`
    // - `member remove <handle>`
    let sub = args[0].as_str();
    if !matches!(sub, "add" | "remove" | "rm") {
        return false;
    }

    let action = if sub == "add" {
        Some(MemberAction::Add)
    } else {
        Some(MemberAction::Remove)
    };
    if args.len() == 1 {
        app.start_member_wizard(action);
        return true;
    }
    let handle = args[1].trim().to_string();
    if handle.is_empty() {
        app.start_member_wizard(action);
        return true;
    }

    let client = match app.remote_client() {
        Some(c) => c,
        None => {
            app.start_login_wizard();
            return true;
        }
    };

    match action {
        Some(MemberAction::Add) => {
            let role = args.get(2).cloned().unwrap_or_else(|| "read".to_string());
            let role_lc = role.to_lowercase();
            if role_lc != "read" && role_lc != "publish" {
                app.push_error("role must be read or publish".to_string());
                return true;
            }
            match client.add_repo_member(&handle, &role_lc) {
                Ok(()) => {
                    app.push_output(vec![format!("added {} ({})", handle, role_lc)]);
                    app.refresh_root_view();
                }
                Err(err) => app.push_error(format!("member add: {:#}", err)),
            }
        }
        Some(MemberAction::Remove) => match client.remove_repo_member(&handle) {
            Ok(()) => {
                app.push_output(vec![format!("removed {}", handle)]);
                app.refresh_root_view();
            }
            Err(err) => app.push_error(format!("member remove: {:#}", err)),
        },
        None => {
            app.start_member_wizard(None);
        }
    }

    true
}
