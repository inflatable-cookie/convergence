mod app;
mod render;
mod trace;
mod wizard;

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};

use app::{Action, App, ResolutionState, View, is_remote_command};
use wizard::{Wizard, WizardKind};

/// What a finished worker result is *for* (batch 17.2).
///
/// Tagged at spawn time rather than sniffed from argv on arrival: two
/// intents legitimately share a verb (the Candidates view and the inbox
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
    /// A `--preflight` answer: either proceed straight into `proceed`,
    /// or open the decision screen (batch 27.5).
    Preflight { proceed: Vec<String>, title: String },
}

/// Result of a worker-thread command.
/// `(argv, intent, quiet, result)` — quiet marks work the user did not
/// ask for, whose outcome must not land in the feedback line as if they
/// had (batch 27.3: the root showed `1 candidates` from a startup loader).
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
            // Somebody testing an installed build needs to know which
            // build it is, and this binary ships separately from
            // `converge` — so it is stamped with the same commit and
            // answers the same way (batch 22.1's build script, reused).
            "--version" | "-V" => {
                println!(
                    "converge-tui {} ({})",
                    env!("CARGO_PKG_VERSION"),
                    env!("CONVERGE_COMMIT")
                );
                return Ok(());
            }
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
    for view in [View::Candidates, View::Lanes, View::Releases, View::Gates] {
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
                Intent::Preflight { proceed, title } => match result {
                    // Nothing at risk: do the thing that was asked for.
                    // A screen here would be a prompt about nothing,
                    // which is how safety questions get trained out of
                    // people.
                    Ok(plan) => match app::Decision::from_plan(&plan, title, proceed.clone()) {
                        Some(decision) => app.decision = Some(decision),
                        None => {
                            spawn_verb(&mut app, &tx, &session, proceed, Intent::Command);
                        }
                    },
                    Err(err) => app.record_result(Err(err)),
                },
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
            terminal.draw(|frame| render::render(frame, &app))?;
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
            Some(Action::Preflight {
                ask,
                proceed,
                title,
            }) => {
                spawn_verb(
                    &mut app,
                    &tx,
                    &session,
                    ask,
                    Intent::Preflight { proceed, title },
                );
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
                    WizardKind::LaneMember(lane) => Wizard::lane_member(lane),
                    WizardKind::Yank(version) => Wizard::yank(version),
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
                // `resolve list` may fetch a candidate's tree (batch 16.1),
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
        Action::Preflight { ask, .. } => format!("preflight {}", ask.join(" ")),
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
        view @ (View::Candidates | View::Releases | View::Lanes | View::Gates | View::Secrets) => {
            app.rows
                .get(&view)
                .map(|rows| rows.iter().map(render::row_label).collect())
                .unwrap_or_default()
        }
        View::Root | View::Help => Vec::new(),
    };
    trace.screen_view(&screen_id, &selectable, &app.primary_action().0);
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
    // The inbox report is an object; its candidates section is the view.
    let rows = match view {
        View::Candidates => value["candidates"].as_array().cloned().unwrap_or_default(),
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
    // Summarised by kind: "49 remote event(s): candidate, candidate, candidate,
    // candidate…" ran off the screen edge saying one thing eleven times
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
        terminal
            .draw(|frame| render::render(frame, app))
            .expect("draw");
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
            (View::Candidates, "Enter: promote"),
            (View::Lanes, "Enter: pull selected lane"),
            (View::Releases, "Enter: fetch selected"),
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
            "candidates": [{"candidate_id": "b2", "gate_id": "intake", "recommendation": "resolve",
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
                "a full candidate id reached the dashboard: {line}"
            );
        }
    }

    /// An empty inbox should leave the dashboard alone rather than
    /// showing an empty heading.
    #[test]
    fn a_quiet_repo_gets_no_next_section() {
        let mut app = App::default();
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [], "publications": [], "candidates": []
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
    /// Batch 22.4: driving a repo with eleven candidates, the Candidates pane
    /// read "no candidates" — because it is fed by `inbox`, which reports
    /// only what needs attention. The empty state was the one place that
    /// difference showed, and it said the wrong thing.
    #[test]
    fn an_empty_candidates_view_explains_what_it_lists() {
        let mut app = App::default();
        app.frames.push(View::Candidates);
        app.rows.insert(View::Candidates, Vec::new());
        let text = screen(&app, 100, 20).join("\n");
        assert!(
            text.contains("nothing needs attention"),
            "an empty action queue is not an empty repo: {text}"
        );
        assert!(
            !text.contains("no candidates"),
            "the old message claimed the repo had none: {text}"
        );
    }
}
