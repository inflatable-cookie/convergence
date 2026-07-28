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
    /// Inbox report wanted for its *data* only — the Root dashboard's
    /// ranked recommendations (batch 23.4). Distinct from `Inbox`
    /// because that one navigates on arrival, and a dashboard refresh
    /// that yanked you into another view would be a bug.
    InboxData,
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
    let mut app = App {
        // Read once, here, so the reducer never touches the environment.
        passphrase_available: std::env::var("CONVERGE_PASSPHRASE").is_ok(),
        ..App::default()
    };
    // One session for the whole TUI lifetime (batch 15.3): the workspace
    // is discovered once, an idle refresh stats the tree instead of
    // rehashing it, and remote commands share one connection pool.
    let session = std::sync::Arc::new(converge_cli::Session::new());
    let (tx, rx) = std::sync::mpsc::channel::<WorkerResult>();
    spawn_refresh(&tx, &session);
    // The dashboard leads with ranked recommendations, so the inbox is
    // needed before anyone asks for it. On the worker, so a slow or
    // unreachable server delays a panel rather than the first frame.
    spawn_verb(
        &mut app,
        &tx,
        &session,
        vec!["inbox".into()],
        Intent::InboxData,
    );
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
                Intent::InboxData => {
                    if let Ok(report) = result {
                        app.load_inbox_entries(&report);
                    }
                    app.mark_loaded(View::Inbox);
                }
                Intent::Events => absorb_events(&mut app, &tx, &session, result),
                Intent::Command => {
                    app.record_result_for(&argv, result);
                    spawn_refresh(&tx, &session);
                    last_refresh_started = std::time::Instant::now();
                    // `spawn_refresh` brings back status and history,
                    // which is what the Root and History screens read —
                    // and nothing else. So a command that changed what
                    // the *current* list view is showing left it stale:
                    // batch 26.5 added a gate through the wizard, was
                    // returned to the gate screen, and saw the graph it
                    // had before. The command had worked; the screen was
                    // the last thing to know.
                    let view = app.current_view();
                    if let Some(argv) = view.loader() {
                        spawn_verb(&mut app, &tx, &session, argv, Intent::Rows(view));
                    }
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
            // Deliberately not run: the command needs a value this
            // program must not accept (app::SECRET_VALUES_ARE_NOT_A_VIEW).
            Some(Action::HandOver(command)) => {
                // One line, because the strip shows three and the second
                // one would be the one clipped. The screen carries the
                // reason; this carries the command.
                app.say(app::LastLine::Output(format!(
                    "run in a terminal: {command}"
                )));
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
                        Wizard::publish(app.remote_gate().as_deref(), app.gate_names())
                    }
                    WizardKind::Member => Wizard::member(Vec::new()),
                    WizardKind::Release(id) => Wizard::release(id, app.channel_names()),
                    WizardKind::Promote(id) => Wizard::promote(id, app.gate_names()),
                    WizardKind::Fetch => Wizard::fetch(app.channel_names()),
                    // Existing gates come from the loaded Gates view, so
                    // the upstream field offers something real without a
                    // synchronous probe (17.2). Every repo has at least
                    // one gate, so an empty list means the view has not
                    // answered yet — and a graph edit should not be built
                    // on a list nobody has seen.
                    WizardKind::Gate if app.gate_names().is_empty() => {
                        app.say(app::LastLine::Output(
                            "gates are still loading — try again in a moment".into(),
                        ));
                        continue;
                    }
                    WizardKind::Gate => Wizard::gate(app.gate_names()),
                });
            }
            Some(Action::LoadInbox) => {
                spawn_verb(&mut app, &tx, &session, vec!["inbox".into()], Intent::Inbox);
            }
            Some(Action::EnterResolution(target)) => {
                // `resolve list` may fetch a bundle's tree (batch 16.1),
                // so it runs on the worker like any other remote verb —
                // the event loop never blocks (arch 15 §3).
                // `--preview` so the view can show what it is asking
                // you to choose between (batch 23.5).
                let argv = vec![
                    "resolve".into(),
                    "list".into(),
                    target.clone(),
                    "--preview".into(),
                ];
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
        Action::HandOver(command) => format!("hand over: {command}"),
        Action::Quit => "quit".into(),
    }
}

/// Emit the current screen's semantic signature (deduped inside Trace).
fn trace_screen(trace: &mut trace::Trace, app: &App) {
    if !trace.enabled() {
        return;
    }
    let screen_id = app.current_view().title().to_lowercase();
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
        view @ (View::Bundles | View::Releases | View::Lanes | View::Gates | View::Secrets) => app
            .rows
            .get(&view)
            .map(|rows| rows.iter().map(row_label).collect())
            .unwrap_or_default(),
        View::Root | View::Help => Vec::new(),
    };
    trace.screen_view(&screen_id, &selectable, &app.primary_action().0);
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
    // `resolve list --preview` wraps each variant as
    // `{key, source, preview, elided, why}`; the plain form is the bare
    // key. Both are read here so the view degrades to key-only rather
    // than to empty if the payload ever arrives without previews.
    let mut paths: Vec<(String, Vec<serde_json::Value>)> = Vec::new();
    let mut previews: std::collections::BTreeMap<String, Vec<app::VariantPreview>> =
        Default::default();
    for (path, variants) in value.as_object().into_iter().flatten() {
        let variants = variants.as_array().cloned().unwrap_or_default();
        let keys: Vec<serde_json::Value> = variants
            .iter()
            .map(|v| {
                if v.get("key").is_some() {
                    v["key"].clone()
                } else {
                    v.clone()
                }
            })
            .collect();
        if variants.iter().any(|v| v.get("key").is_some()) {
            previews.insert(
                path.clone(),
                variants
                    .iter()
                    .map(|v| app::VariantPreview {
                        source: v["source"].as_str().unwrap_or("?").to_string(),
                        text: v["preview"].as_str().unwrap_or("").to_string(),
                        elided: v["elided"].as_bool().unwrap_or(false),
                        why: v["why"].as_str().unwrap_or("").to_string(),
                    })
                    .collect(),
            );
        }
        paths.push((path.clone(), keys));
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    app.record_result(Ok(serde_json::json!(format!(
        "{} superposed path(s)",
        paths.len()
    ))));
    app.resolution = Some(ResolutionState {
        snap_id: target,
        paths,
        previews,
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
    // Remote events change the inbox, not just status. Root needs it too
    // now that the dashboard ranks from it, and that refresh must not
    // navigate — hence the data-only intent (batch 23.4).
    if app.current_view() == View::Inbox {
        spawn_verb(app, tx, session, vec!["inbox".into()], Intent::Inbox);
    } else if !app.inbox_entries.is_empty() || app.current_view() == View::Root {
        spawn_verb(app, tx, session, vec!["inbox".into()], Intent::InboxData);
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

    // Header: what this workspace is, how fresh, and whether the
    // server is reachable.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" converge ", Style::default().add_modifier(Modifier::BOLD)),
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
        .style(Style::default().bg(Color::DarkGray)),
        header,
    );

    // Body: active view.
    match app.current_view() {
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
            // One Root. There used to be two — a local one and a remote
            // one behind a mode toggle — each using four lines of a
            // thirty-line pane to withhold what the other one showed.
            let (primary, _) = app.primary_action();
            let head = app.status.as_ref().map(|s| s["head"].clone());
            let head_line = head
                .as_ref()
                .and_then(|h| h["id"].as_str().map(str::to_string))
                .map(|id| {
                    format!(
                        "head: {} ({})",
                        short_id(&id),
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
            let remote = app.status.as_ref().map(|s| s["remote"].clone());
            let configured = remote
                .as_ref()
                .and_then(|r| r["configured"].as_bool())
                .unwrap_or(false);
            let target = remote
                .as_ref()
                .filter(|_| configured)
                .and_then(|r| r["target"].as_str().map(str::to_string))
                .unwrap_or_else(|| "not configured (run login)".to_string());
            let last_published = remote
                .as_ref()
                .and_then(|r| r["last_published_snap"].as_str().map(str::to_string))
                .map(|id| short_id(&id))
                .unwrap_or_else(|| "none".to_string());
            let last_seen = remote
                .as_ref()
                .and_then(|r| r["last_seen_bundle"].as_str().map(str::to_string))
                .map(|id| short_id(&id))
                .unwrap_or_else(|| "none".to_string());
            let flow = app
                .status
                .as_ref()
                .and_then(|s| s["profile"]["flow"].as_str().map(str::to_string))
                .unwrap_or_default();
            let mut lines = vec![
                Line::raw(head_line),
                Line::raw(format!(
                    "pending changes: {}    automatic captures: {auto}",
                    app.pending_changes
                )),
                Line::raw(""),
                Line::raw(format!("remote: {target}")),
                Line::raw(format!(
                    "last published snap: {last_published}    last seen bundle: {last_seen}"
                )),
                Line::styled(flow, Style::default().fg(Color::DarkGray)),
            ];

            // Ranked next actions (spec §4.7, batch 23.4). Ordered by
            // what blocks other people; the ranking lives in
            // `converge_cli::inbox_actions`, so this panel and the Inbox
            // view cannot disagree about what matters.
            if !app.recommendations.is_empty() {
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "next",
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                for recommendation in &app.recommendations {
                    let owners = if recommendation.owners.is_empty() {
                        String::new()
                    } else {
                        format!("  ({})", recommendation.owners.join(", "))
                    };
                    // The view, never the argv. A bundle id is 64
                    // characters and spelling one out here pushes the
                    // rest of the line off the edge; the Inbox is where
                    // a row is a command you can paste.
                    let where_to = format!("  → {}", recommendation.view);
                    lines.push(Line::styled(
                        format!("  {}{owners}{where_to}", recommendation.headline),
                        // Blocking work is not the same colour as news.
                        match recommendation.kind {
                            converge_cli::ActionKind::Resolve => Style::default().fg(Color::Yellow),
                            converge_cli::ActionKind::Publication => {
                                Style::default().fg(Color::DarkGray)
                            }
                            _ => Style::default(),
                        },
                    ));
                }
            }

            lines.push(Line::raw(""));
            lines.push(Line::styled(
                format!("Enter: {primary}"),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            frame.render_widget(Paragraph::new(lines).block(view_block(app)), body);
        }
        View::Resolution => {
            let empty = ResolutionState::default();
            let resolution = app.resolution.as_ref().unwrap_or(&empty);
            // 65/35 list + detail (spec §6). Batch 23.1 recorded the
            // flat list as a decision-correctness problem rather than
            // polish: the screen asked you to choose between two file
            // contents and showed you neither.
            let [list_area, detail_area] =
                Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .areas(body);

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
            // Live counts (UX spec §5): computed from what is on screen,
            // so they update on every keystroke without a round trip.
            let validation = resolution.validation();
            items.push(ListItem::new(format!(
                "{} missing, {} invalid of {}",
                validation.missing,
                validation.invalid,
                resolution.paths.len()
            )));
            items.push(ListItem::new(
                "keys: 1-9 pick  0 clear  Alt+n next missing  Alt+f next invalid",
            ));
            frame.render_widget(List::new(items).block(view_block(app)), list_area);

            // Detail: the variants for the selected path, numbered to
            // match the keys that pick them.
            let mut detail: Vec<Line> = Vec::new();
            if let Some((path, keys)) = resolution.paths.get(resolution.selected) {
                let previews = resolution.previews.get(path);
                let chosen = resolution.decisions.get(path).copied();
                for (index, key) in keys.iter().enumerate() {
                    let preview = previews.and_then(|p| p.get(index));
                    let source = preview
                        .map(|p| p.source.clone())
                        .or_else(|| key["source"].as_str().map(str::to_string))
                        .unwrap_or_else(|| "?".into());
                    let picked = chosen == Some(index as u32);
                    detail.push(Line::styled(
                        format!("{}{} {source}", if picked { "▸ " } else { "  " }, index + 1),
                        if picked {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ));
                    match preview {
                        Some(preview) if !preview.text.is_empty() => {
                            for line in preview.text.lines() {
                                detail.push(Line::raw(format!("    {line}")));
                            }
                            if preview.elided {
                                detail.push(Line::styled(
                                    "    …",
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                        }
                        // No text is a fact about the variant, not a
                        // failure to load one: "binary" and "deleted in
                        // this variant" are both things you choose
                        // between (batch 23.5).
                        Some(preview) => detail.push(Line::styled(
                            format!("    ({})", preview.why),
                            Style::default().fg(Color::DarkGray),
                        )),
                        None => detail.push(Line::styled(
                            "    (no preview loaded)",
                            Style::default().fg(Color::DarkGray),
                        )),
                    }
                    detail.push(Line::raw(""));
                }
            }
            if detail.is_empty() {
                detail.push(Line::styled(
                    "no superpositions",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            frame.render_widget(
                Paragraph::new(detail)
                    .block(Block::default().borders(Borders::ALL).title("Variants")),
                detail_area,
            );
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
                    // The row's action used to be spelled out here,
                    // full 64-character bundle id and all, so it was
                    // always cut off at the right edge. The hint bar
                    // already names what Enter does.
                    let _ = &argv;
                    ListItem::new(label.clone()).style(style)
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
                // Say what empty *means* (batch 22.4). Driving a repo
                // with eleven bundles in it, this pane read "no bundles
                // (or not loaded yet)" — because the Bundles view is fed
                // by `inbox`, which reports only what needs attention,
                // and every bundle was ready to promote with no
                // approvals required. The name promises a list; the
                // source is an action queue, and the empty state was the
                // only place that difference showed.
                // A list item does not wrap, so long copy is split by
                // hand rather than silently truncated at the pane edge.
                for line in match view {
                    View::Bundles => &[
                        "nothing needs attention here.",
                        "this view lists bundles waiting on you — an approval, or a",
                        "superposition to resolve — not every bundle in the repo.",
                    ][..],
                    View::Releases => &["no releases yet.", "  release <bundle> --channel <name>"],
                    View::Lanes => &["no lanes yet."],
                    View::Gates => &["no gate graph loaded."],
                    _ => &["nothing here yet."],
                } {
                    items.push(ListItem::new(*line));
                }
            }
            frame.render_widget(List::new(items).block(view_block(app)), body);
        }
        View::Secrets => {
            let rows = app.rows.get(&View::Secrets).cloned().unwrap_or_default();
            let selected = app.row_selected.get(&View::Secrets).copied().unwrap_or(0);
            let mut items: Vec<ListItem> = Vec::new();
            for (i, row) in rows.iter().enumerate() {
                let style = if i == selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                let readers: Vec<&str> = row["readers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| r.as_str())
                    .collect();
                items.push(
                    ListItem::new(format!(
                        "{}  owner {}  readable by: {}",
                        row["name"].as_str().unwrap_or("?"),
                        row["owner"].as_str().unwrap_or("?"),
                        if readers.is_empty() {
                            "owner only".to_string()
                        } else {
                            readers.join(", ")
                        }
                    ))
                    .style(style),
                );
                // The question an audit actually asks is when the
                // *credential* last changed, not when its recipient list
                // did — so the value version leads, as it does in the
                // CLI's audit output (batch 20.3).
                items.push(ListItem::new(format!(
                    "    value v{} last changed {}",
                    row["value_version"],
                    row["value_updated_at"].as_str().unwrap_or("unknown")
                )));
                for stale in row["stale"].as_array().into_iter().flatten() {
                    items.push(
                        ListItem::new(format!(
                            "    stale: {} — {}",
                            stale["subject"]
                                .as_str()
                                .unwrap_or(stale["key_id"].as_str().unwrap_or("?")),
                            stale["why"].as_str().unwrap_or("")
                        ))
                        .style(Style::default().fg(Color::Yellow)),
                    );
                }
            }
            if rows.is_empty() {
                items.push(ListItem::new("no secrets in this repo (or not loaded yet)"));
            }
            items.push(ListItem::new(""));
            // The hint bar lying about what a key does is exactly the
            // 23.1 finding, so this one tracks the state that decides it.
            items.push(ListItem::new(if app.passphrase_available {
                "keys: r rotate (hands over the command)  u unshare stale recipients (confirm)"
            } else {
                "keys: r rotate  u unshare stale recipients — both hand the command over"
            }));
            // Said on the screen, not just in a doc: someone looking for
            // a value should learn here that it is not coming.
            items.push(
                ListItem::new(app::SECRET_VALUES_ARE_NOT_A_VIEW)
                    .style(Style::default().fg(Color::DarkGray)),
            );
            frame.render_widget(List::new(items).block(view_block(app)), body);
        }
        View::Help => {
            let mut lines = vec![
                Line::styled("keys", Style::default().add_modifier(Modifier::BOLD)),
                Line::raw("  Enter: primary action   Esc: back   q: quit   Tab: complete"),
                Line::raw("  Alt+h history   Alt+i inbox   Alt+b bundles   Alt+l lanes"),
                Line::raw("  Alt+e releases  Alt+g gates   Alt+s secrets  Alt+? help   Alt+r root"),
                Line::raw("  in-view: History m annotate d diff  ·  Bundles p promote e release"),
                Line::raw("           Secrets r rotate u unshare"),
                Line::styled(
                    "  wizards: type a bare `member add`, `fetch`, `release <id>`, `promote <id>`",
                    Style::default().fg(Color::DarkGray),
                ),
                Line::styled(
                    "  (macOS Terminal and iTerm send composed characters for Option;",
                    Style::default().fg(Color::DarkGray),
                ),
                Line::styled(
                    "   enable \"Use Option as Meta key\", or type the verb instead.)",
                    Style::default().fg(Color::DarkGray),
                ),
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
            // Workflow profile (UX spec §4.6): guidance, phrased for the
            // domain. Term renaming stays deferred — see the spec's
            // implementation-status section.
            if let Some(status) = &app.status {
                lines.push(Line::raw(format!(
                    "profile: {}",
                    status["profile"]["name"].as_str().unwrap_or("software")
                )));
                if let Some(flow) = status["profile"]["flow"].as_str() {
                    lines.push(Line::raw(format!("  {flow}")));
                }
            }
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
                    // Short id, like every other list view. The full
                    // 64 characters used to push the message off the
                    // right edge, which left the one column a person
                    // actually reads invisible.
                    ListItem::new(format!(
                        "{}  {}  {}",
                        short_id(s["id"].as_str().unwrap_or("?")),
                        s["created_at"]
                            .as_str()
                            .unwrap_or("")
                            .get(..19)
                            .unwrap_or(""),
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
                lines.push(Line::raw(format!(
                    "{}: {}",
                    field.prompt,
                    field.display(&wizard.input)
                )));
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
                    // Review shows what will run, and a credential
                    // reviewed in the clear is a credential on screen.
                    lines.push(Line::raw(format!(
                        "{}: {}",
                        field.name,
                        field.display(value)
                    )));
                }
                // For a verb the console would confirm, the review step
                // is that confirmation — so it has to name the
                // consequence rather than say "run" (batch 23.3).
                lines.push(match app::confirmation_prompt(&wizard.build_argv()) {
                    Some(what) => Line::styled(
                        format!("Enter: {what}   Esc: back"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    None => Line::styled(
                        "Enter: run  Esc: back",
                        Style::default().fg(Color::DarkGray),
                    ),
                });
            }
        }
        if let Some(error) = &wizard.error {
            lines.push(Line::styled(error.clone(), Style::default().fg(Color::Red)));
        }
        // Clear first (batch 23.3). A wizard is an overlay, and without
        // this it composited character-by-character over whatever view
        // was behind it: "Add member" and a head id shared a line, and
        // "subject: dana" ran into "pending changes: 0". Reducer tests
        // could not see it because nothing they touch draws.
        frame.render_widget(ratatui::widgets::Clear, body);
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Wizard")),
            body,
        );
    }

    // The footer is the navigation surface (batch 27.1). In navigate
    // mode it lists every destination with the bare key that reaches it
    // — visible, not learned, because the previous scheme was Alt-only
    // and stock macOS terminals never deliver Alt, so from 23.1 to 27.1
    // there was no working navigation at all. In command mode it is the
    // console with a caret.
    if app.quit_confirm || app.pending_confirm.is_some() {
        let legend = if app.quit_confirm {
            "quit? Enter/y: yes  any other key: no".to_string()
        } else if let Some((label, _)) = &app.pending_confirm {
            format!("{label}? Enter/y: yes  any other key: no")
        } else {
            String::new()
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                legend,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))),
            input,
        );
    } else if app.command_mode {
        // The caret is drawn in the line rather than moved with the
        // terminal cursor: one render path, and the trace sees what the
        // user sees.
        let (before, after) = app.input.split_at(app.cursor.min(app.input.len()));
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(app.prompt(), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::raw(before.to_string()),
                Span::styled("|", Style::default().fg(Color::Cyan)),
                Span::raw(after.to_string()),
                Span::raw("  "),
                Span::styled(
                    "Enter: run  Esc: close console  Tab: complete",
                    Style::default().fg(Color::DarkGray),
                ),
            ])),
            input,
        );
    } else {
        let key = |k: &'static str| Span::styled(k, Style::default().fg(Color::Yellow));
        let label = |l: &'static str| Span::styled(l, Style::default().fg(Color::Gray));
        let mut spans = vec![Span::styled(
            format!("Enter: {}  ", app.primary_action().0),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )];
        // Uniform "key word" pairs, one space between pairs: the yellow
        // letter is the key even where it is not the word's initial
        // (releases is `e` — `r` is root). Kept compact so the whole
        // bar survives a 100-column terminal.
        for (k, l) in [
            ("h", "history "),
            ("i", "inbox "),
            ("b", "bundles "),
            ("l", "lanes "),
            ("e", "releases "),
            ("g", "gates "),
            ("s", "secrets "),
            (":", "command "),
            ("?", "help "),
            ("Esc", "back "),
            ("q", "quit"),
        ] {
            spans.push(key(k));
            spans.push(Span::raw(" "));
            spans.push(label(l));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), input);
    }
}

fn view_block(app: &App) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(app.current_view().title())
}

#[cfg(test)]
mod screen_tests {
    //! What a person actually sees.
    //!
    //! Batch 23.1 drove the real binary through a pty to find out that
    //! the hint bar named the wrong key on six screens — something forty
    //! reducer tests could not catch, because a reducer test never looks
    //! at the rendering. These render into a `TestBackend` buffer and
    //! assert on the text, so the same class of defect fails in CI
    //! instead of waiting for someone to sit in front of it.

    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render `app` and return the screen as lines of text.
    fn screen(app: &App, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        terminal.draw(|frame| render(frame, app)).expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// One secret readable by two people, one recipient gone stale.
    fn audited_secrets() -> Vec<serde_json::Value> {
        vec![serde_json::json!({
            "name": "DATABASE_URL",
            "owner": "alice",
            "version": 4,
            "value_version": 2,
            "value_updated_at": "2026-07-20T09:00:00Z",
            "readers": ["alice", "bob"],
            "stale": [{
                "key_id": "b89042d66a02",
                "subject": "carol",
                "why": "no longer a member of this repo",
            }],
        })]
    }

    fn secrets_app() -> App {
        let mut app = App::default();
        app.frames.push(View::Secrets);
        app.rows.insert(View::Secrets, audited_secrets());
        app
    }

    #[test]
    fn the_secrets_screen_answers_who_can_read_what() {
        let text = screen(&secrets_app(), 100, 24).join("\n");
        assert!(text.contains("Secrets"), "no title: {text}");
        assert!(
            text.contains("DATABASE_URL") && text.contains("owner alice"),
            "the secret and its owner should be on screen: {text}"
        );
        assert!(
            text.contains("readable by: alice, bob"),
            "readers are the question this screen answers: {text}"
        );
        assert!(
            text.contains("value v2 last changed 2026-07-20"),
            "when the credential last changed, not its recipient list: {text}"
        );
        assert!(
            text.contains("stale: carol"),
            "a stale recipient is the thing worth acting on: {text}"
        );
    }

    /// The screen must never become a way to read a credential.
    #[test]
    fn secret_values_are_not_a_view() {
        let text = screen(&secrets_app(), 100, 24).join("\n");
        assert!(
            text.contains("never shown or typed here"),
            "the screen should say values are not coming: {text}"
        );
        // `secret get` is the only verb that decrypts, and nothing on
        // this screen offers it.
        assert!(
            !text.contains("secret get"),
            "the secrets screen offered a way to read a value: {text}"
        );
    }

    /// The 23.1 finding, as a test: the hint bar renders
    /// `primary_action().0`, so a screen that names the wrong key fails.
    #[test]
    fn the_hint_bar_names_each_screens_own_key() {
        for (view, expected) in [
            (View::History, "Enter: restore selected"),
            (View::Bundles, "Enter: open selected"),
            (View::Secrets, "Enter: (r rotate, u unshare)"),
        ] {
            let mut app = App::default();
            app.frames.push(view);
            let text = screen(&app, 100, 24).join("\n");
            assert!(
                text.contains(expected),
                "{} should advertise {expected:?}: {text}",
                view.title()
            );
        }
    }

    /// Batch 23.1 found History leading with a 64-character id, which
    /// pushed the message a person reads off the right edge.
    #[test]
    fn history_rows_leave_room_for_the_message() {
        let mut app = App::default();
        app.frames.push(View::History);
        app.snaps = vec![serde_json::json!({
            "id": "e5c175cd97d6870a2104771661362e10700c7311c3f0a172c0b9d5c9d3c6d725",
            "created_at": "2026-07-25T20:39:17.997231Z",
            "message": "initial layout",
        })];
        let text = screen(&app, 80, 24).join("\n");
        assert!(
            text.contains("e5c175cd97d6") && text.contains("initial layout"),
            "short id and the message should both fit at 80 columns: {text}"
        );
        assert!(
            !text.contains("e5c175cd97d6870a21047716"),
            "the full id is back, and it will eat the message again: {text}"
        );
    }

    /// A credential must not be legible on the review screen, which is
    /// the last thing shown before it is used.
    #[test]
    fn the_login_review_screen_does_not_echo_the_token() {
        let mut wizard = wizard::Wizard::login();
        for answer in ["http://server", "s3cr3t-token", "acme", "default", "intake"] {
            wizard.input = answer.to_string();
            wizard.submit();
        }
        let app = App {
            wizard: Some(wizard),
            ..App::default()
        };
        let text = screen(&app, 100, 24).join("\n");
        assert!(
            text.contains("http://server"),
            "the url is not a secret: {text}"
        );
        assert!(
            !text.contains("s3cr3t-token"),
            "the access token was printed on the review screen: {text}"
        );
        assert!(text.contains("••••"), "masked, not omitted: {text}");
    }

    /// A wizard's review step is the confirmation for a verb the console
    /// would confirm, so it has to say what is about to happen.
    #[test]
    fn a_review_step_names_the_consequence_it_is_confirming() {
        let mut wizard = wizard::Wizard::promote("d".repeat(64), vec!["review".into()]);
        wizard.input = "review".into();
        wizard.submit();
        let app = App {
            wizard: Some(wizard),
            ..App::default()
        };
        let text = screen(&app, 100, 24).join("\n");
        assert!(
            text.contains("Enter: promote dddddddddddd"),
            "the review legend should name the act, not say 'run': {text}"
        );

        // A verb the console would not confirm keeps the plain legend.
        let mut wizard = wizard::Wizard::annotate("e".repeat(64));
        wizard.input = "a message".into();
        wizard.submit();
        let app = App {
            wizard: Some(wizard),
            ..App::default()
        };
        assert!(screen(&app, 100, 24).join("\n").contains("Enter: run"));
    }

    /// The Last strip is a visible record; argv carrying a token puts
    /// the token in it.
    #[test]
    fn a_token_never_reaches_the_last_strip() {
        let mut app = App::default();
        app.record_command(&[
            "login".into(),
            "--url".into(),
            "http://server".into(),
            "--token".into(),
            "s3cr3t-token".into(),
        ]);
        let text = screen(&app, 100, 24).join("\n");
        assert!(
            !text.contains("s3cr3t-token") && text.contains("<redacted>"),
            "the token should be redacted where argv is shown: {text}"
        );
        assert!(
            text.contains("http://server"),
            "only the credential goes: {text}"
        );
    }

    /// The dashboard's whole job: what needs doing, how much of it, and
    /// who is waiting — in blocking order.
    #[test]
    fn the_dashboard_ranks_counts_and_names_owners() {
        let mut app = App {
            status: Some(serde_json::json!({
                "head": {"id": "a".repeat(64), "trigger": "explicit"},
                "snaps": {"automatic": 0},
                "remote": {"configured": true, "target": "acme/default/intake @ http://s"},
            })),
            ..App::default()
        };
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [{"lane_id": "personal/erin", "updated_at": "t"}],
            "publications": [{"publisher": "alice", "gate_id": "intake"}],
            "bundles": [{"bundle_id": "b2", "gate_id": "intake", "recommendation": "resolve",
                         "approvals": 0, "required_approvals": 0, "contributors": ["carol", "dana"]}]
        }));
        let text = screen(&app, 100, 24).join("\n");

        let blocked = text.find("blocked by superpositions").expect("resolve row");
        let lane = text.find("with work to pull").expect("lane row");
        let news = text.find("in an open window").expect("publication row");
        assert!(
            blocked < lane && lane < news,
            "ranked by what blocks other people: {text}"
        );
        assert!(text.contains("(carol)"), "the owner is named: {text}");
        assert!(
            text.contains("(erin)"),
            "a personal lane names its owner: {text}"
        );
        assert!(
            text.contains("→ bundles"),
            "the row points at the view that lists it: {text}"
        );
        // The 23.1 finding, guarded here too: a 64-character id in a
        // dashboard row pushes everything after it off the edge.
        for line in screen(&app, 100, 24) {
            assert!(
                !line.contains(&"b".repeat(20)),
                "a full bundle id reached the dashboard: {line}"
            );
        }
    }

    /// An empty inbox should leave the dashboard alone rather than
    /// showing an empty heading.
    #[test]
    fn a_quiet_repo_gets_no_next_section() {
        let mut app = App::default();
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [], "publications": [], "bundles": []
        }));
        let text = screen(&app, 100, 24).join("\n");
        assert!(
            !text.contains("next"),
            "nothing to say, so say nothing: {text}"
        );
        assert!(text.contains("Enter: history"));
    }

    fn resolution_app() -> App {
        let mut app = App::default();
        app.frames.push(View::Resolution);
        let key = |source: &str| serde_json::json!({"source": source, "type": "file"});
        app.resolution = Some(ResolutionState {
            snap_id: "s".into(),
            paths: vec![(
                "docs/plan.md".into(),
                vec![key("lane-a"), key("lane-b"), key("lane-c")],
            )],
            previews: [(
                "docs/plan.md".to_string(),
                vec![
                    app::VariantPreview {
                        source: "lane-a".into(),
                        text: "alice's plan\nsecond line".into(),
                        elided: true,
                        why: String::new(),
                    },
                    app::VariantPreview {
                        source: "lane-b".into(),
                        text: String::new(),
                        elided: false,
                        why: "binary".into(),
                    },
                    app::VariantPreview {
                        source: "lane-c".into(),
                        text: String::new(),
                        elided: false,
                        why: "deleted in this variant".into(),
                    },
                ],
            )]
            .into_iter()
            .collect(),
            decisions: Default::default(),
            selected: 0,
        });
        app
    }

    /// Batch 23.1 recorded the flat list as a decision-correctness
    /// problem: it asked you to choose between file contents and showed
    /// you none of them.
    #[test]
    fn the_resolution_view_shows_what_it_asks_you_to_choose_between() {
        let text = screen(&resolution_app(), 120, 24).join("\n");
        assert!(text.contains("docs/plan.md"), "the path is listed: {text}");
        assert!(
            text.contains("alice's plan"),
            "the variant's content is the whole point: {text}"
        );
        assert!(
            text.contains("1 lane-a") && text.contains("2 lane-b"),
            "variants are numbered to match the keys that pick them: {text}"
        );
    }

    /// "Binary" and "deleted" are facts about a variant, not failures to
    /// load one — a chooser has to be able to pick the deletion.
    #[test]
    fn unpreviewable_variants_say_what_they_are() {
        let text = screen(&resolution_app(), 120, 24).join("\n");
        assert!(text.contains("(binary)"), "{text}");
        assert!(text.contains("(deleted in this variant)"), "{text}");
    }

    #[test]
    fn the_chosen_variant_is_marked_in_the_detail_pane() {
        let mut app = resolution_app();
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('2'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let text = screen(&app, 120, 24).join("\n");
        assert!(
            text.contains("▸ 2 lane-b"),
            "the pick should be visible where the variants are: {text}"
        );
    }
    /// Batch 22.4: driving a repo with eleven bundles, the Bundles pane
    /// read "no bundles" — because it is fed by `inbox`, which reports
    /// only what needs attention. The empty state was the one place that
    /// difference showed, and it said the wrong thing.
    #[test]
    fn an_empty_bundles_view_explains_what_it_lists() {
        let mut app = App::default();
        app.frames.push(View::Bundles);
        app.rows.insert(View::Bundles, Vec::new());
        let text = screen(&app, 100, 20).join("\n");
        assert!(
            text.contains("nothing needs attention"),
            "an empty action queue is not an empty repo: {text}"
        );
        assert!(
            !text.contains("no bundles"),
            "the old message claimed the repo had none: {text}"
        );
    }
}
