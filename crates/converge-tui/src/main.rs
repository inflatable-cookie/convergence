mod app;
mod trace;
mod wizard;

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use app::{Action, App, LastLine, ResolutionState, View, is_remote_command};
use wizard::{Wizard, WizardKind, WizardStep};

/// What a finished worker result is *for* (batch 17.2).
///
/// Tagged at spawn time rather than sniffed from argv on arrival: two
/// intents legitimately share a verb (the Bundles view and the inbox
/// screen both load `inbox`), so argv alone cannot say what to do with
/// the answer.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Intent {
    /// A user-typed command: record the result, refresh after.
    Command,
    /// Status + history for the root and history views.
    Refresh,
    /// Inbox report destined for the inbox screen.
    Inbox,
    /// Rows for a list view.
    Rows(View),
    /// `resolve list <ref>` destined for the resolution view.
    Resolution(String),
    /// Remote heartbeat from the event poller.
    Events,
}

/// Result of a worker-thread command.
type WorkerResult = (Vec<String>, Intent, anyhow::Result<serde_json::Value>);

/// Run one CLI verb on a worker thread and post the result back.
fn spawn_verb(
    app: &mut App,
    tx: &std::sync::mpsc::Sender<WorkerResult>,
    session: &std::sync::Arc<converge_cli::Session>,
    argv: Vec<String>,
    intent: Intent,
) {
    app.record_command(&argv);
    app.record_in_flight(&argv);
    let tx = tx.clone();
    let session = std::sync::Arc::clone(session);
    std::thread::spawn(move || {
        let result = converge_cli::execute_in(&session, argv.iter().cloned());
        let _ = tx.send((argv, intent, result));
    });
}

/// Status + history in one worker round trip. Both are local, but a
/// first scan of a large tree still costs real time, and the UI thread
/// is not where that belongs (spec §7.1).
fn spawn_refresh(
    tx: &std::sync::mpsc::Sender<WorkerResult>,
    session: &std::sync::Arc<converge_cli::Session>,
) {
    let tx = tx.clone();
    let session = std::sync::Arc::clone(session);
    std::thread::spawn(move || {
        let status = converge_cli::execute_in(&session, ["status"]);
        let history = converge_cli::execute_in(&session, ["history"]);
        let combined = serde_json::json!({
            "status": status.as_ref().ok(),
            "status_failed": status.is_err(),
            "history": history.unwrap_or(serde_json::Value::Null),
        });
        let _ = tx.send((vec!["status".into()], Intent::Refresh, Ok(combined)));
    });
}

fn main() -> Result<()> {
    // `--agent-trace <path>` is the only flag (UX spec §4.3).
    let mut args = std::env::args().skip(1);
    let mut trace_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--agent-trace" => trace_path = args.next().map(std::path::PathBuf::from),
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    let mut trace = trace::Trace::from_arg_or_env(trace_path.as_deref());
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut trace);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, trace: &mut trace::Trace) -> Result<()> {
    let mut app = App::default();
    // One session for the whole TUI lifetime (batch 15.3): the workspace
    // is discovered once, an idle refresh stats the tree instead of
    // rehashing it, and remote commands share one connection pool.
    let session = std::sync::Arc::new(converge_cli::Session::new());
    let (tx, rx) = std::sync::mpsc::channel::<WorkerResult>();
    spawn_refresh(&tx, &session);
    trace.session_start();

    // Event poller (doc 14 §5b): replaces blind remote refresh. Events are
    // hints — arrival triggers a status/inbox refresh through the normal
    // worker channel.
    {
        let tx = tx.clone();
        let session = std::sync::Arc::clone(&session);
        std::thread::spawn(move || {
            let mut cursor: u64 = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3));
                // The poll doubles as the reachability probe (audit
                // P4.22): its outcome is exactly "can we talk to the
                // server right now", so it is reported either way
                // instead of being swallowed on failure.
                let polled =
                    converge_cli::execute_in(&session, ["events", "--since", &cursor.to_string()]);
                let events = match polled {
                    Ok(events) => events,
                    Err(err) => {
                        let _ = tx.send((vec!["events".into()], Intent::Events, Err(err)));
                        continue;
                    }
                };
                let list = events.as_array().cloned().unwrap_or_default();
                cursor = list
                    .iter()
                    .filter_map(|e| e["seq"].as_u64())
                    .max()
                    .unwrap_or(cursor);
                let note = serde_json::json!({
                    "count": list.len(),
                    "kinds": list
                        .iter()
                        .filter_map(|e| e["kind"].as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
                let _ = tx.send((vec!["events".into()], Intent::Events, Ok(note)));
            }
        });
    }

    // Idle refresh (audit P2.9): a workspace changes under the TUI —
    // `watch` in another terminal, a teammate's publish — and a screen
    // that only updates on keystrokes quietly lies. Cheap because the
    // scan is dirstamp-gated (batch 15.3).
    const IDLE_REFRESH: Duration = Duration::from_secs(5);
    let mut last_refresh_started = std::time::Instant::now();

    loop {
        trace_screen(trace, &app);
        // Deliver finished worker results without blocking.
        while let Ok((argv, intent, result)) = rx.try_recv() {
            trace.command_result(&argv, &result);
            if intent != Intent::Events {
                app.finish_in_flight();
            }
            // A remote result is also a reachability answer.
            if is_remote_command(&argv) || intent == Intent::Events {
                app.reachable = Some(result.is_ok());
            }
            match intent {
                Intent::Refresh => absorb_refresh(&mut app, &result),
                Intent::Rows(view) => absorb_view_rows(&mut app, view, result),
                Intent::Resolution(target) => enter_resolution(&mut app, target, result),
                Intent::Inbox => {
                    match result {
                        Ok(report) => {
                            app.load_inbox_entries(&report);
                            app.record_result(Ok(serde_json::json!(format!(
                                "{} inbox item(s)",
                                app.inbox_entries.len()
                            ))));
                            if app.current_view() != View::Inbox {
                                app.frames.push(View::Inbox);
                            }
                        }
                        Err(err) => app.record_result(Err(err)),
                    }
                    app.mark_loaded(View::Inbox);
                }
                Intent::Events => absorb_events(&mut app, &tx, &session, result),
                Intent::Command => {
                    app.record_result(result);
                    spawn_refresh(&tx, &session);
                    last_refresh_started = std::time::Instant::now();
                }
            }
        }

        if last_refresh_started.elapsed() >= IDLE_REFRESH {
            spawn_refresh(&tx, &session);
            last_refresh_started = std::time::Instant::now();
        }

        terminal.draw(|frame| render(frame, &app))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        let action = app.handle_key(key);
        if let Some(action) = &action {
            trace.user_action(&action_label(action), &format!("{:?}", key.code));
        }
        match action {
            Some(Action::Quit) => {
                trace.session_end();
                return Ok(());
            }
            Some(Action::Enter(view)) => {
                if app.current_view() != view {
                    app.frames.push(view);
                }
                // List views load through their CLI verb on the worker
                // (batch 17.1): entering a view must never block the
                // event loop on the network (arch 15 §3).
                if let Some(argv) = view.loader() {
                    spawn_verb(&mut app, &tx, &session, argv, Intent::Rows(view));
                } else {
                    spawn_refresh(&tx, &session);
                }
            }
            Some(Action::StartWizard(kind)) => {
                app.wizard = Some(match kind {
                    WizardKind::Annotate(snap_id) => Wizard::annotate(snap_id),
                    WizardKind::Login => Wizard::login(),
                    // The gate comes from the status refresh already in
                    // hand (batch 17.2): probing the remote here was a
                    // synchronous network call to learn something the
                    // TUI had just been told.
                    WizardKind::Publish => {
                        Wizard::publish(app.remote_gate().as_deref(), Vec::new())
                    }
                });
            }
            Some(Action::LoadInbox) => {
                spawn_verb(&mut app, &tx, &session, vec!["inbox".into()], Intent::Inbox);
            }
            Some(Action::EnterResolution(target)) => {
                // `resolve list` may fetch a bundle's tree (batch 16.1),
                // so it runs on the worker like any other remote verb —
                // the event loop never blocks (arch 15 §3).
                let argv = vec!["resolve".into(), "list".into(), target.clone()];
                spawn_verb(&mut app, &tx, &session, argv, Intent::Resolution(target));
            }
            Some(Action::ApplyResolution) => {
                if let Some(resolution) = app.resolution.take() {
                    let decisions = resolution.keyed_decisions();
                    let path = std::env::temp_dir().join(format!(
                        "converge-tui-decisions-{}.json",
                        std::process::id()
                    ));
                    let argv = vec![
                        "resolve".to_string(),
                        "apply".to_string(),
                        resolution.snap_id.clone(),
                        path.display().to_string(),
                    ];
                    // Applying can fetch, hash, and materialize a whole
                    // tree, so it belongs on the worker like every other
                    // load (spec §7.1). The decisions file is written
                    // here and removed by the worker once consumed.
                    match std::fs::write(
                        &path,
                        serde_json::to_vec(&decisions).expect("serialize decisions"),
                    ) {
                        Ok(()) => {
                            app.record_command(&argv);
                            app.record_in_flight(&argv);
                            let tx = tx.clone();
                            let session = std::sync::Arc::clone(&session);
                            std::thread::spawn(move || {
                                let result =
                                    converge_cli::execute_in(&session, argv.iter().cloned());
                                let _ = std::fs::remove_file(&path);
                                let _ = tx.send((argv, Intent::Command, result));
                            });
                        }
                        Err(err) => app.record_result(Err(anyhow::Error::from(err))),
                    }
                    if app.current_view() == View::Resolution {
                        app.frames.pop();
                    }
                }
            }
            Some(Action::Run(argv)) => {
                // Every command goes to the worker (batch 17.2), not
                // only the remote ones: `snap` on a large tree and
                // `restore` are just as capable of stalling a frame as a
                // network call, and one path is one path to get right.
                spawn_verb(&mut app, &tx, &session, argv, Intent::Command);
            }
            None => {}
        }
    }
}

fn action_label(action: &Action) -> String {
    match action {
        Action::Run(argv) => argv.join(" "),
        Action::Enter(view) => format!("enter {}", view.title()),
        Action::StartWizard(kind) => format!("wizard {kind:?}"),
        Action::EnterResolution(snap) => format!("resolve {snap}"),
        Action::LoadInbox => "inbox".into(),
        Action::ApplyResolution => "resolve apply".into(),
        Action::Quit => "quit".into(),
    }
}

/// Emit the current screen's semantic signature (deduped inside Trace).
fn trace_screen(trace: &mut trace::Trace, app: &App) {
    if !trace.enabled() {
        return;
    }
    let screen_id = format!(
        "{}:{}",
        app.context.label().to_lowercase(),
        app.current_view().title().to_lowercase()
    );
    let selectable: Vec<String> = match app.current_view() {
        View::History => app
            .snaps
            .iter()
            .filter_map(|s| s["id"].as_str().map(str::to_string))
            .collect(),
        View::Resolution => app
            .resolution
            .as_ref()
            .map(|r| r.paths.iter().map(|(p, _)| p.clone()).collect())
            .unwrap_or_default(),
        View::Inbox => app
            .inbox_entries
            .iter()
            .map(|(label, _)| label.clone())
            .collect(),
        view @ (View::Bundles | View::Releases | View::Lanes | View::Gates) => app
            .rows
            .get(&view)
            .map(|rows| rows.iter().map(row_label).collect())
            .unwrap_or_default(),
        View::Root | View::Help => Vec::new(),
    };
    trace.screen_view(&screen_id, &selectable, app.primary_action().0);
}

/// One row of a list view, rendered from whatever the verb returned.
///
/// Deliberately field-driven rather than per-view formatters: these are
/// CLI payloads, and a view that invented its own vocabulary would be
/// the divergence the argv contract exists to prevent.
fn row_label(row: &serde_json::Value) -> String {
    let s = |key: &str| row[key].as_str().unwrap_or("").to_string();
    if !s("bundle_id").is_empty() && !s("channel").is_empty() {
        return format!(
            "{}  {}  by {}  {}",
            s("channel"),
            short_id(&s("bundle_id")),
            s("released_by"),
            s("created_at")
        );
    }
    if !s("bundle_id").is_empty() {
        return format!(
            "{}  @ {}  -> {}  ({}/{} approvals)",
            short_id(&s("bundle_id")),
            s("gate_id"),
            s("recommendation"),
            row["approvals"],
            row["required_approvals"]
        );
    }
    if !s("lane_id").is_empty() {
        return format!(
            "{}  owner {}  {}",
            s("lane_id"),
            s("owner"),
            s("visibility")
        );
    }
    if !s("gate_id").is_empty() {
        let upstreams = row["upstreams"]
            .as_array()
            .map(|u| {
                u.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        return format!(
            "{}  {}  {} approval(s)  {}{}",
            s("gate_id"),
            if upstreams.is_empty() {
                "entry".to_string()
            } else {
                format!("after {upstreams}")
            },
            row["required_approvals"],
            s("strategy"),
            if row["may_release"].as_bool().unwrap_or(false) {
                "  releasable"
            } else {
                ""
            }
        );
    }
    row.to_string()
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

/// A finished `resolve list <ref>` becomes the resolution view.
fn enter_resolution(app: &mut App, target: String, result: anyhow::Result<serde_json::Value>) {
    let value = match result {
        Ok(value) => value,
        Err(err) => return app.record_result(Err(err)),
    };
    let mut paths: Vec<(String, Vec<serde_json::Value>)> = value
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_array().cloned().unwrap_or_default()))
                .collect()
        })
        .unwrap_or_default();
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    app.record_result(Ok(serde_json::json!(format!(
        "{} superposed path(s)",
        paths.len()
    ))));
    app.resolution = Some(ResolutionState {
        snap_id: target,
        paths,
        decisions: Default::default(),
        selected: 0,
    });
    if app.current_view() != View::Resolution {
        app.frames.push(View::Resolution);
    }
    app.mark_loaded(View::Resolution);
}

/// Store a finished list-view load.
fn absorb_view_rows(app: &mut App, view: View, result: anyhow::Result<serde_json::Value>) {
    let value = match result {
        Ok(value) => value,
        Err(err) => return app.record_result(Err(err)),
    };
    // The inbox report is an object; its bundles section is the view.
    let rows = match view {
        View::Bundles => value["bundles"].as_array().cloned().unwrap_or_default(),
        View::Gates => value["gates"].as_array().cloned().unwrap_or_default(),
        _ => value.as_array().cloned().unwrap_or_default(),
    };
    app.record_result(Ok(serde_json::json!(format!(
        "{} {}",
        rows.len(),
        view.title().to_lowercase()
    ))));
    app.row_selected.insert(view, 0);
    app.rows.insert(view, rows);
    app.mark_loaded(view);
}

/// Status + history from one refresh round trip (batch 17.2).
fn absorb_refresh(app: &mut App, result: &anyhow::Result<serde_json::Value>) {
    let Ok(value) = result else {
        return; // a failed refresh leaves the last good screen in place
    };
    app.workspace_missing = value["status_failed"].as_bool().unwrap_or(false);
    app.status = (!app.workspace_missing).then(|| value["status"].clone());
    app.pending_changes = app
        .status
        .as_ref()
        .and_then(|s| s["pending"]["count"].as_u64())
        .unwrap_or(0) as usize;
    app.snaps = value["history"].as_array().cloned().unwrap_or_default();
    app.mark_loaded(View::Root);
    app.mark_loaded(View::History);
}

/// A heartbeat from the event poller: reachability, plus an inbox
/// reload when something actually happened (batch 15.3).
fn absorb_events(
    app: &mut App,
    tx: &std::sync::mpsc::Sender<WorkerResult>,
    session: &std::sync::Arc<converge_cli::Session>,
    result: anyhow::Result<serde_json::Value>,
) {
    let Ok(value) = result else {
        return; // silence is the point: an unreachable server is shown
        // in the header, not shouted into the Last strip
    };
    let count = value["count"].as_u64().unwrap_or(0);
    if count == 0 {
        return;
    }
    app.record_result(Ok(serde_json::json!(format!(
        "{count} remote event(s): {}",
        value["kinds"].as_str().unwrap_or("")
    ))));
    spawn_refresh(tx, session);
    // Remote events change the inbox, not just status — but only reload
    // it when it is on screen or already loaded.
    if !app.inbox_entries.is_empty() || app.current_view() == View::Inbox {
        spawn_verb(app, tx, session, vec!["inbox".into()], Intent::Inbox);
    }
}

fn render(frame: &mut Frame, app: &App) {
    let suggestion_rows = app.suggestions.len().min(9) as u16;
    let [header, body, last, suggestions, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(4),
        Constraint::Length(suggestion_rows),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // Header: workspace context, named not color-only.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" converge [{}] ", app.context.label()),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} snaps, {} pending changes",
                app.snaps.len(),
                app.pending_changes
            )),
            // Freshness and reachability, so a stale or disconnected
            // screen says so instead of looking authoritative
            // (audit P2.9, P4.22).
            Span::raw(
                app.view_age()
                    .map(|age| format!("  ·  {age}"))
                    .unwrap_or_default(),
            ),
            Span::styled(
                match app.reachability() {
                    "" => String::new(),
                    label => format!("  ·  {label}"),
                },
                Style::default().fg(if app.reachable == Some(false) {
                    Color::Yellow
                } else {
                    Color::Gray
                }),
            ),
        ]))
        .style(Style::default().bg(match app.context {
            app::Context::Local => Color::DarkGray,
            app::Context::Remote => Color::Blue,
        })),
        header,
    );

    // Body: active view.
    match app.current_view() {
        View::Root if app.context == app::Context::Remote => {
            let (primary, _) = app.primary_action();
            let remote = app.status.as_ref().map(|s| s["remote"].clone());
            let target = remote
                .as_ref()
                .filter(|r| r["configured"].as_bool().unwrap_or(false))
                .and_then(|r| r["target"].as_str().map(str::to_string))
                .unwrap_or_else(|| "not configured (run login)".to_string());
            let last_published = remote
                .as_ref()
                .and_then(|r| r["last_published_snap"].as_str().map(str::to_string))
                .unwrap_or_else(|| "none".to_string());
            let last_seen = remote
                .as_ref()
                .and_then(|r| r["last_seen_bundle"].as_str().map(str::to_string))
                .unwrap_or_else(|| "none".to_string());
            let lines = vec![
                Line::raw(format!("remote: {target}")),
                Line::raw(format!("last published snap: {last_published}")),
                Line::raw(format!("last seen bundle: {last_seen}")),
                Line::raw(""),
                Line::styled(
                    format!("Enter: {primary}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];
            frame.render_widget(Paragraph::new(lines).block(view_block(app)), body);
        }
        View::Root if app.workspace_missing => {
            // A TUI started outside a workspace used to render an empty
            // shell and fail every refresh silently (audit P1.5).
            let lines = vec![
                Line::raw("no converge workspace in this directory"),
                Line::raw(""),
                Line::styled(
                    "Enter: init  (creates .converge here)",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Line::raw("Esc, then q: quit and cd somewhere else"),
            ];
            frame.render_widget(Paragraph::new(lines).block(view_block(app)), body);
        }
        View::Root => {
            let (primary, _) = app.primary_action();
            let head = app.status.as_ref().map(|s| s["head"].clone());
            let head_line = head
                .as_ref()
                .and_then(|h| h["id"].as_str().map(str::to_string))
                .map(|id| {
                    format!(
                        "head: {id} ({})",
                        head.as_ref()
                            .and_then(|h| h["trigger"].as_str())
                            .unwrap_or("?")
                    )
                })
                .unwrap_or_else(|| "head: none".to_string());
            let auto = app
                .status
                .as_ref()
                .and_then(|s| s["snaps"]["automatic"].as_u64())
                .unwrap_or(0);
            let lines = vec![
                Line::raw(format!("pending changes: {}", app.pending_changes)),
                Line::raw(head_line),
                Line::raw(format!(
                    "automatic captures: {auto} (run `watch` in a terminal for continuous capture)"
                )),
                Line::raw(""),
                Line::styled(
                    format!("Enter: {primary}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];
            frame.render_widget(Paragraph::new(lines).block(view_block(app)), body);
        }
        View::Resolution => {
            let empty = ResolutionState::default();
            let resolution = app.resolution.as_ref().unwrap_or(&empty);
            let mut items: Vec<ListItem> = resolution
                .paths
                .iter()
                .enumerate()
                .map(|(i, (path, keys))| {
                    let count = keys.len();
                    let decision = resolution
                        .decisions
                        .get(path)
                        .map(|d| format!("variant {}", d + 1))
                        .unwrap_or_else(|| "undecided".to_string());
                    let style = if i == resolution.selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!("{path}  [{count} variants]  {decision}")).style(style)
                })
                .collect();
            items.push(ListItem::new(""));
            items.push(ListItem::new(format!(
                "{} undecided of {}   keys: 1-9 pick  0 clear  Enter next/apply",
                resolution.undecided(),
                resolution.paths.len()
            )));
            frame.render_widget(List::new(items).block(view_block(app)), body);
        }
        View::Inbox => {
            let mut items: Vec<ListItem> = app
                .inbox_entries
                .iter()
                .enumerate()
                .map(|(i, (label, argv))| {
                    let style = if i == app.inbox_selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    let suffix = argv
                        .as_ref()
                        .map(|a| format!("  [Enter: {}]", a.join(" ")))
                        .unwrap_or_default();
                    ListItem::new(format!("{label}{suffix}")).style(style)
                })
                .collect();
            if items.is_empty() {
                items.push(ListItem::new("inbox empty"));
            }
            frame.render_widget(List::new(items).block(view_block(app)), body);
        }
        view @ (View::Bundles | View::Releases | View::Lanes | View::Gates) => {
            let rows = app.rows.get(&view).cloned().unwrap_or_default();
            let selected = app.row_selected.get(&view).copied().unwrap_or(0);
            let mut items: Vec<ListItem> = rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let style = if i == selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(row_label(row)).style(style)
                })
                .collect();
            if items.is_empty() {
                items.push(ListItem::new(format!(
                    "no {} (or not loaded yet)",
                    view.title().to_lowercase()
                )));
            }
            frame.render_widget(List::new(items).block(view_block(app)), body);
        }
        View::Help => {
            let mut lines = vec![
                Line::styled("keys", Style::default().add_modifier(Modifier::BOLD)),
                Line::raw("  Enter: primary action   Esc: back   q: quit   Tab: context"),
                Line::raw("  Alt+h history   Alt+i inbox   Alt+b bundles   Alt+l lanes"),
                Line::raw("  Alt+e releases  Alt+g gates   Alt+? help    Alt+r root"),
                Line::raw(""),
                Line::styled(
                    "verbs (type any of these)",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];
            for chunk in app::COMMANDS.chunks(8) {
                lines.push(Line::raw(format!("  {}", chunk.join("  "))));
            }
            lines.push(Line::raw(""));
            let remote = app.status.as_ref().map(|s| s["remote"].clone());
            lines.push(Line::raw(format!(
                "remote: {}",
                remote
                    .as_ref()
                    .and_then(|r| r["target"].as_str().map(str::to_string))
                    .unwrap_or_else(|| "not configured".into())
            )));
            frame.render_widget(Paragraph::new(lines).block(view_block(app)), body);
        }
        View::History => {
            let mut items: Vec<ListItem> = app
                .snaps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let style = if i == app.history_selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!(
                        "{}  {}  {}",
                        s["id"].as_str().unwrap_or("?"),
                        s["created_at"].as_str().unwrap_or(""),
                        s["message"].as_str().unwrap_or("")
                    ))
                    .style(style)
                })
                .collect();
            items.push(ListItem::new(""));
            items.push(ListItem::new(
                "keys: Enter restore (confirm)  d diff vs head  m annotate",
            ));
            frame.render_widget(List::new(items).block(view_block(app)), body);
        }
    }

    // Last strip: command echo cyan, output white, errors red (UX spec §4).
    let mut last_lines: Vec<Line> = app
        .last
        .iter()
        .map(|entry| match entry {
            LastLine::Command(text) => Line::styled(text.clone(), Style::default().fg(Color::Cyan)),
            LastLine::Output(text) => Line::raw(text.clone()),
            LastLine::Error(text) => Line::styled(text.clone(), Style::default().fg(Color::Red)),
        })
        .collect();
    if let Some(label) = &app.in_flight {
        last_lines.push(Line::styled(
            format!("… {label} (running)"),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(
        Paragraph::new(last_lines).block(Block::default().borders(Borders::TOP).title("Last")),
        last,
    );

    // Suggestions palette.
    if !app.suggestions.is_empty() {
        let items: Vec<ListItem> = app
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let style = if i == app.suggestion_index {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(s.clone()).style(style)
            })
            .collect();
        frame.render_widget(List::new(items), suggestions);
    }

    // Wizard modal overlays the body when active.
    if let Some(wizard) = &app.wizard {
        let mut lines = vec![Line::styled(
            wizard.title,
            Style::default().add_modifier(Modifier::BOLD),
        )];
        match wizard.step {
            WizardStep::Field(_) => {
                let field = wizard.current_field().expect("field step");
                lines.push(Line::raw(format!("{}: {}", field.prompt, wizard.input)));
                if let wizard::FieldKind::Choice { options } = &field.kind {
                    lines.push(Line::raw(format!("options: {}", options.join(", "))));
                }
                lines.push(Line::styled(
                    "Enter: next  Esc: back/cancel",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            WizardStep::Review => {
                for (field, value) in wizard.fields.iter().zip(&wizard.values) {
                    lines.push(Line::raw(format!("{}: {}", field.name, value)));
                }
                lines.push(Line::styled(
                    "Enter: run  Esc: back",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        if let Some(error) = &wizard.error {
            lines.push(Line::styled(error.clone(), Style::default().fg(Color::Red)));
        }
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Wizard")),
            body,
        );
    }

    // Input line with prompt and key legend.
    let legend = if app.quit_confirm {
        "quit? Enter/y: yes  any other key: no".to_string()
    } else if let Some((label, _)) = &app.pending_confirm {
        format!("{label}? Enter/y: yes  any other key: no")
    } else {
        format!(
            "Enter: {}  Esc: back  Tab: context  q: quit",
            app.primary_action().0
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(app.prompt(), Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::raw(app.input.clone()),
            Span::raw("  "),
            Span::styled(legend, Style::default().fg(Color::DarkGray)),
        ])),
        input,
    );
}

fn view_block(app: &App) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(app.current_view().title())
}
