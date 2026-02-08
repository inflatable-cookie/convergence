use super::*;

fn ts_ui_format() -> &'static [FormatItem<'static>] {
    static FMT: OnceLock<Vec<FormatItem<'static>>> = OnceLock::new();
    FMT.get_or_init(|| {
        time::format_description::parse(
            "[year]-[month repr:numerical padding:zero]-[day padding:zero] [hour padding:zero]:[minute padding:zero]Z",
        )
        .expect("valid time format")
    })
}

fn fmt_ts_abs(ts: &str) -> Option<String> {
    let dt = OffsetDateTime::parse(ts, &Rfc3339).ok()?;
    dt.format(ts_ui_format()).ok()
}

fn fmt_since(ts: &str, now: OffsetDateTime) -> Option<String> {
    let dt = OffsetDateTime::parse(ts, &Rfc3339).ok()?;
    let delta = now - dt;
    let secs = delta.whole_seconds();

    // Future timestamps are rare; show as absolute.
    if secs < 0 {
        return None;
    }

    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;

    let s = if secs < 60 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{}m ago", mins)
    } else if hours < 48 {
        format!("{}h ago", hours)
    } else if days < 14 {
        format!("{}d ago", days)
    } else {
        // Past that, prefer an absolute date.
        return None;
    };
    Some(s)
}

pub(in crate::tui_shell) fn fmt_ts_list(ts: &str, ctx: &RenderCtx) -> String {
    match ctx.ts_mode {
        TimestampMode::Relative => fmt_since(ts, ctx.now).unwrap_or_else(|| fmt_ts_ui(ts)),
        TimestampMode::Absolute => fmt_ts_ui(ts),
    }
}
pub(in crate::tui_shell) fn fmt_ts_ui(ts: &str) -> String {
    fmt_ts_abs(ts).unwrap_or_else(|| ts.to_string())
}

pub(in crate::tui_shell) fn now_ts() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<time>".to_string())
}
