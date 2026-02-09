use super::*;

pub(super) fn run_legacy_member(app: &mut App, args: &[String]) {
    // Back-compat: accept legacy flag form.
    let client = match app.remote_client() {
        Some(c) => c,
        None => {
            app.start_login_wizard();
            return;
        }
    };

    let sub = &args[0];
    let mut handle: Option<String> = None;
    let mut role: String = "read".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--handle" => {
                i += 1;
                if i >= args.len() {
                    app.push_error("missing value for --handle".to_string());
                    return;
                }
                handle = Some(args[i].clone());
            }
            "--role" => {
                i += 1;
                if i >= args.len() {
                    app.push_error("missing value for --role".to_string());
                    return;
                }
                role = args[i].clone();
            }
            a => {
                app.push_error(format!("unknown arg: {}", a));
                return;
            }
        }
        i += 1;
    }

    let Some(handle) = handle else {
        app.push_error("missing --handle".to_string());
        return;
    };

    match sub.as_str() {
        "add" => match client.add_repo_member(&handle, &role) {
            Ok(()) => {
                app.push_output(vec![format!("added {} ({})", handle, role)]);
                app.refresh_root_view();
            }
            Err(err) => app.push_error(format!("member add: {:#}", err)),
        },
        "remove" | "rm" => match client.remove_repo_member(&handle) {
            Ok(()) => {
                app.push_output(vec![format!("removed {}", handle)]);
                app.refresh_root_view();
            }
            Err(err) => app.push_error(format!("member remove: {:#}", err)),
        },
        _ => app.start_member_wizard(None),
    }
}
