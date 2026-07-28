mod app;
mod trace;
mod wizard;

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
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
/// `(argv, intent, quiet, result)` — quiet marks work the user did not
/// ask for, whose outcome must not land in the feedback line as if they
/// had (batch 27.3: the root showed `1 bundles` from a startup loader).
type WorkerResult = (Vec<String>, Intent, bool, anyhow::Result<serde_json::Value>);

/// Run one CLI verb on a worker thread and post the result back.
fn spawn_verb(
    app: &mut App,
    tx: &std::sync::mpsc::Sender<WorkerResult>,
    session: &std::sync::Arc<converge_cli::Session>,
    argv: Vec<String>,
    intent: Intent,
) {
    spawn_verb_inner(app, tx, session, argv, intent, false)
}

/// A load the user did not ask for: fills data, announces nothing.
fn spawn_verb_quiet(
    app: &mut App,
    tx: &std::sync::mpsc::Sender<WorkerResult>,
    session: &std::sync::Arc<converge_cli::Session>,
    argv: Vec<String>,
    intent: Intent,
) {
    spawn_verb_inner(app, tx, session, argv, intent, true)
}

fn spawn_verb_inner(
    app: &mut App,
    tx: &std::sync::mpsc::Sender<WorkerResult>,
    session: &std::sync::Arc<converge_cli::Session>,
    argv: Vec<String>,
    intent: Intent,
    background: bool,
) {
    // Background data loads — startup tile fills, dashboard refreshes,
    // the reload after a command — are not something the user typed, and
    // the feedback line saying `> inbox` when nobody typed it reads as
    // the machine acting on its own (batch 27.3 screenshot). Only
    // user-driven intents are announced.
    let announce = !matches!(intent, Intent::InboxData);
    if announce && !background {
        app.record_command(&argv);
        app.record_in_flight(&argv);
    }
    let tx = tx.clone();
    let session = std::sync::Arc::clone(session);
    std::thread::spawn(move || {
        let result = converge_cli::execute_in(&session, argv.iter().cloned());
        let _ = tx.send((argv, intent, background, result));
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
        let _ = tx.send((vec!["status".into()], Intent::Refresh, true, Ok(combined)));
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
    // The root tiles preview real rows (batch 27.3), so their loaders
    // run at startup rather than waiting for somebody to open each
    // view. Rows intents store data without navigating.
    for view in [View::Bundles, View::Lanes, View::Releases, View::Gates] {
        if let Some(argv) = view.loader() {
            spawn_verb_quiet(&mut app, &tx, &session, argv, Intent::Rows(view));
        }
    }
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
                        let _ = tx.send((vec!["events".into()], Intent::Events, true, Err(err)));
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
                let _ = tx.send((vec!["events".into()], Intent::Events, true, Ok(note)));
            }
        });
    }

    // Idle refresh (audit P2.9): a workspace changes under the TUI —
    // `watch` in another terminal, a teammate's publish — and a screen
    // that only updates on keystrokes quietly lies. Cheap because the
    // scan is dirstamp-gated (batch 15.3).
    const IDLE_REFRESH: Duration = Duration::from_secs(5);
    let mut last_refresh_started = std::time::Instant::now();

    // Draw only when something changed. The previous loop redrew the
    // whole screen twenty times a second while idle — every poll
    // timeout fell through to `terminal.draw` — which is why an idle
    // dashboard was burning CPU (operator, batch 27.3). Worker results,
    // key presses and resizes mark the frame dirty; a timeout draws
    // nothing.
    let mut dirty = true;
    loop {
        // Deliver finished worker results without blocking.
        while let Ok((argv, intent, quiet, result)) = rx.try_recv() {
            dirty = true;
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
                Intent::Rows(view) => absorb_view_rows(&mut app, view, quiet, result),
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
                        spawn_verb_quiet(&mut app, &tx, &session, argv, Intent::Rows(view));
                    }
                }
            }
        }

        if last_refresh_started.elapsed() >= IDLE_REFRESH {
            spawn_refresh(&tx, &session);
            last_refresh_started = std::time::Instant::now();
        }

        if dirty {
            trace_screen(trace, &app);
            terminal.draw(|frame| render(frame, &app))?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let event = event::read()?;
        // Resize only rendered correctly before because every loop pass
        // redrew regardless; with dirty-gating it has to be explicit.
        if matches!(event, Event::Resize(..)) {
            dirty = true;
            continue;
        }
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        dirty = true;
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
                    WizardKind::Release(id) => Wizard::release(id, app.release_versions()),
                    WizardKind::Promote(id) => Wizard::promote(id, app.gate_names()),
                    WizardKind::Fetch => Wizard::fetch(app.release_versions()),
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
                                let _ = tx.send((argv, Intent::Command, false, result));
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
    if !s("bundle_id").is_empty() && !s("version").is_empty() {
        return format!(
            "{}  {}  by {}  {}",
            s("version"),
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
fn absorb_view_rows(
    app: &mut App,
    view: View,
    quiet: bool,
    result: anyhow::Result<serde_json::Value>,
) {
    let value = match result {
        Ok(value) => value,
        // Errors always surface — a background load failing is real
        // news even when its success would have been noise.
        Err(err) => return app.record_result(Err(err)),
    };
    // The inbox report is an object; its bundles section is the view.
    let rows = match view {
        View::Bundles => value["bundles"].as_array().cloned().unwrap_or_default(),
        View::Gates => value["gates"].as_array().cloned().unwrap_or_default(),
        _ => value.as_array().cloned().unwrap_or_default(),
    };
    if !quiet {
        app.record_result(Ok(serde_json::json!(format!(
            "{} {}",
            rows.len(),
            view.title().to_lowercase()
        ))));
    }
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
    // Summarised by kind: "49 remote event(s): bundle, bundle, bundle,
    // bundle…" ran off the screen edge saying one thing eleven times
    // (batch 27.3 screenshot). Nobody reads a comma list for a tally.
    let kinds = value["kinds"].as_str().unwrap_or("");
    let mut tally: Vec<(String, usize)> = Vec::new();
    for kind in kinds.split(", ").filter(|k| !k.is_empty()) {
        match tally.iter_mut().find(|(name, _)| name == kind) {
            Some((_, n)) => *n += 1,
            None => tally.push((kind.to_string(), 1)),
        }
    }
    let summary = tally
        .iter()
        .map(|(name, n)| {
            if *n == 1 {
                name.clone()
            } else {
                format!("{name} ×{n}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    app.record_result(Ok(serde_json::json!(format!(
        "{count} remote event(s): {summary}"
    ))));
    spawn_refresh(tx, session);
    // Remote events change the inbox, not just status. Root needs it too
    // now that the dashboard ranks from it, and that refresh must not
    // navigate — hence the data-only intent (batch 23.4).
    if app.current_view() == View::Inbox {
        spawn_verb_quiet(app, tx, session, vec!["inbox".into()], Intent::Inbox);
    } else if !app.inbox_entries.is_empty() || app.current_view() == View::Root {
        spawn_verb_quiet(app, tx, session, vec!["inbox".into()], Intent::InboxData);
    }
}

fn render(frame: &mut Frame, app: &App) {
    let suggestion_rows = app.suggestions.len().min(9) as u16;
    let [header, body, last, suggestions, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(suggestion_rows),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // Header: what this workspace is, how fresh, and whether the
    // server is reachable.
    // Counts on the left; the remote on the right, coloured by
    // reachability — green means the URL answers, red means it does
    // not, gray means nobody has asked yet. The brand label and the
    // freshness timer are gone (batch 27.3 trim): the binary's name is
    // not information, and the timer restated what "online" implies.
    let left = format!(
        " {} snaps, {} pending changes",
        app.snaps.len(),
        app.pending_changes
    );
    let remote_target = app
        .status
        .as_ref()
        .and_then(|s| s["remote"]["target"].as_str())
        .unwrap_or("")
        .to_string();
    let remote_colour = match app.reachable {
        Some(true) => Color::Green,
        Some(false) => Color::Red,
        None => Color::Gray,
    };
    let pad = (header.width as usize).saturating_sub(left.len() + remote_target.len() + 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(left),
            Span::raw(" ".repeat(pad)),
            Span::styled(remote_target, Style::default().fg(remote_colour)),
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
            // A dashboard in sections, not a paragraph in a void. The
            // screenshot that reopened this (batch 27.3) was twelve
            // lines of hashes floating in black, an unselectable list,
            // and an `Enter: promote` that named no target.
            // The Your work / Server panels are gone (batch 27.3
            // trim): the header carries the counts and the remote, and
            // everything else those panels said is one keypress away.
            // -- The hub: six tiles, each a place to look, the
            // selected one opened by Enter. The first pass put a
            // command behind Enter and the operator named the cost:
            // it removes agency the moment the screen loads.
            let grid_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Ratio(1, 3),
                    Constraint::Ratio(1, 3),
                    Constraint::Ratio(1, 3),
                ])
                .split(body);
            let selected_tile = app.root_selected.min(app::ROOT_TILES.len() - 1);
            for (row_index, row_area) in grid_rows.iter().enumerate() {
                let columns = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(*row_area);
                for (col_index, cell) in columns.iter().enumerate() {
                    let tile_index = row_index * 2 + col_index;
                    let Some((view, name)) = app::ROOT_TILES.get(tile_index) else {
                        continue;
                    };
                    let selected = tile_index == selected_tile;
                    let lines = root_tile_preview(app, *view);
                    let border = if selected {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    let title = format!(
                        " {} {}. {} ",
                        if selected { "▶" } else { " " },
                        tile_index + 1,
                        name
                    );
                    frame.render_widget(
                        Paragraph::new(lines).block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title)
                                .border_style(border),
                        ),
                        *cell,
                    );
                }
            }
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
                    View::Releases => &["no releases yet.", "  release <bundle> --as 1.0.0"],
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
            // One verb per line with what it does (batch 27.2) — the
            // packed name grid told a reader nothing they could act on.
            for (name, help) in app::COMMANDS {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {name:<10}"), Style::default().fg(Color::Yellow)),
                    Span::styled(*help, Style::default().fg(Color::Gray)),
                ]));
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

    // One line of feedback, no box (batch 27.3 trim): the latest
    // result — or the command in flight — coloured by what it is.
    // Errors red, output plain, command echo cyan. The four-row "Last"
    // pane restated history nobody asked for.
    let latest = if let Some(label) = &app.in_flight {
        Line::styled(
            format!("… {label} (running)"),
            Style::default().fg(Color::Yellow),
        )
    } else {
        match app.last.last() {
            Some(LastLine::Command(text)) => {
                Line::styled(format!("> {text}"), Style::default().fg(Color::Cyan))
            }
            Some(LastLine::Output(text)) => Line::raw(text.clone()),
            Some(LastLine::Error(text)) => {
                Line::styled(text.clone(), Style::default().fg(Color::Red))
            }
            None => Line::raw(""),
        }
    };
    frame.render_widget(Paragraph::new(latest), last);

    // Suggestions palette: verb in yellow, its help beside it, the
    // selection reversed — the legacy panel, back (batch 27.2). Visible
    // the moment the console opens, because the empty state is exactly
    // when somebody needs the menu.
    if !app.suggestions.is_empty() {
        // Window around the selection: 37 verbs, nine rows, and a list
        // that does not follow the highlight strands it off-screen —
        // the legacy panel scrolled, so this one does.
        let rows = suggestions.height as usize;
        let start = app
            .suggestion_index
            .saturating_sub(rows.saturating_sub(1))
            .min(
                app.suggestions
                    .len()
                    .saturating_sub(rows.min(app.suggestions.len())),
            );
        let items: Vec<ListItem> = app
            .suggestions
            .iter()
            .enumerate()
            .skip(start)
            .take(rows.max(1))
            .map(|(i, s)| {
                let style = if i == app.suggestion_index {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {s:<10}"),
                        style.patch(Style::default().fg(Color::Yellow)),
                    ),
                    Span::styled(
                        App::command_help(s),
                        style.patch(Style::default().fg(Color::Gray)),
                    ),
                ]))
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

/// What a root tile shows before you open it: a preview where data is
/// in hand, and a plain description of what lives there where it is not
/// — so every tile teaches what its section is for.
fn root_tile_preview(app: &App, view: View) -> Vec<Line<'static>> {
    match view {
        View::Inbox => {
            if app.recommendations.is_empty() {
                return vec![Line::styled(
                    "nothing is waiting on you",
                    Style::default().fg(Color::Gray),
                )];
            }
            app.recommendations
                .iter()
                .take(4)
                .map(|r| {
                    let owners = if r.owners.is_empty() {
                        String::new()
                    } else {
                        format!("  ({})", r.owners.join(", "))
                    };
                    let colour = match r.kind {
                        converge_cli::ActionKind::Resolve => Color::Red,
                        converge_cli::ActionKind::Approve | converge_cli::ActionKind::Promote => {
                            Color::Yellow
                        }
                        converge_cli::ActionKind::LanePull => Color::Cyan,
                        converge_cli::ActionKind::Publication => Color::Gray,
                    };
                    Line::styled(
                        format!("{}{owners}", r.headline),
                        Style::default().fg(colour),
                    )
                })
                .collect()
        }
        // The last few snaps, newest first — the same rows the History
        // screen leads with, so the tile is a genuine preview of it.
        View::History => app
            .snaps
            .iter()
            .take(4)
            .map(|s| {
                let id = s["id"].as_str().map(short_id).unwrap_or_default();
                let message = s["message"].as_str().unwrap_or("(automatic)");
                Line::raw(format!("{id}  {message}"))
            })
            .collect(),
        view => {
            let Some(rows) = app.rows.get(&view).filter(|r| !r.is_empty()) else {
                return vec![Line::styled(
                    match view {
                        View::Bundles => "no bundles waiting",
                        View::Lanes => "no lane activity",
                        View::Releases => "nothing released yet",
                        View::Gates => "loading…",
                        _ => "",
                    }
                    .to_string(),
                    Style::default().fg(Color::Gray),
                )];
            };
            rows.iter()
                .take(4)
                .map(|row| {
                    let text = match view {
                        View::Bundles => format!(
                            "{}  {}",
                            row["bundle_id"].as_str().map(short_id).unwrap_or_default(),
                            row["recommendation"].as_str().unwrap_or("")
                        ),
                        View::Lanes => format!(
                            "{}  {}",
                            row["lane_id"].as_str().unwrap_or(""),
                            row["updated_at"]
                                .as_str()
                                .map(|t| t.get(..10).unwrap_or(t))
                                .unwrap_or("")
                        ),
                        View::Releases => format!(
                            "{}  {}",
                            row["version"]
                                .as_str()
                                .map(|v| format!("v{v}"))
                                .unwrap_or_default(),
                            row["bundle_id"].as_str().map(short_id).unwrap_or_default()
                        ),
                        View::Gates => {
                            let upstreams = row["upstreams"]
                                .as_array()
                                .map(|u| {
                                    u.iter()
                                        .filter_map(|v| v.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            format!(
                                "{}  {}",
                                row["gate_id"].as_str().unwrap_or(""),
                                if upstreams.is_empty() {
                                    "entry".to_string()
                                } else {
                                    format!("after {upstreams}")
                                }
                            )
                        }
                        _ => String::new(),
                    };
                    Line::raw(text)
                })
                .collect()
        }
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
        let text = screen(&app, 100, 40).join("\n");

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
        // The root is a hub (batch 27.3, second pass): the inbox tile
        // previews the ranked work, the selection is visible on a tile,
        // and Enter *opens* — it never runs a mutation from here,
        // because that is what the operator meant by removing agency.
        assert!(
            text.contains("▶ 1. inbox"),
            "the selected tile is not visible: {text}"
        );
        // The per-tile "Enter opens…" label went in the 27.3 trim; the
        // footer's Enter label carries it once, for the selected tile.
        assert!(
            text.contains("Enter: open inbox"),
            "the footer does not say what Enter does: {text}"
        );
        // And the header carries the remote, coloured by reachability,
        // instead of a brand label and a timer.
        assert!(
            text.contains("acme/default/intake"),
            "the remote is not in the header: {text}"
        );
        assert!(
            !text.contains(" converge "),
            "the brand label is back: {text}"
        );
        assert!(
            !text.contains("Enter runs: converge"),
            "the root offers to run a command again: {text}"
        );
        // The 23.1 finding, guarded here too: a 64-character id in a
        // dashboard row pushes everything after it off the edge.
        for line in screen(&app, 100, 40) {
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
        let text = screen(&app, 100, 40).join("\n");
        assert!(
            text.contains("nothing is waiting on you"),
            "a quiet inbox tile should say so: {text}"
        );
        assert!(text.contains("Enter: open inbox"));
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
