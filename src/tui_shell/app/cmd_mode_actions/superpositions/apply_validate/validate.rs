use super::super::super::*;

pub(super) fn cmd_superpositions_validate_mode(app: &mut App, args: &[String]) {
    if !args.is_empty() {
        app.push_error("usage: validate".to_string());
        return;
    }

    let Some(ws) = app.require_workspace() else {
        return;
    };

    let out: std::result::Result<String, String> =
        match app.current_view_mut::<SuperpositionsView>() {
            Some(v) => {
                v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
                v.updated_at = now_ts();
                let ok = v.validation.as_ref().is_some_and(|r| r.ok);
                Ok(format!("validation: {}", if ok { "ok" } else { "invalid" }))
            }
            None => Err("not in superpositions mode".to_string()),
        };

    match out {
        Ok(line) => app.push_output(vec![line]),
        Err(err) => app.push_error(err),
    }
}
