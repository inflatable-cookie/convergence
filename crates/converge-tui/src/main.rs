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

/// Result of a worker-thread command.
type WorkerResult = (Vec<String>, anyhow::Result<serde_json::Value>);

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
    let (tx, rx) = std::sync::mpsc::channel::<WorkerResult>();
    refresh(&mut app);
    trace.session_start();

    loop {
        trace_screen(trace, &app);
        // Deliver finished worker results without blocking.
        while let Ok((argv, result)) = rx.try_recv() {
            trace.command_result(&argv, &result);
            app.finish_in_flight();
            app.record_result(result);
            refresh(&mut app);
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
                app.frames.push(view);
                refresh(&mut app);
            }
            Some(Action::StartWizard(kind)) => {
                app.wizard = Some(match kind {
                    WizardKind::Login => Wizard::login(),
                    WizardKind::Publish => {
                        let default_gate = converge_cli::execute(["remote"])
                            .ok()
                            .and_then(|r| r["gate"].as_str().map(str::to_string));
                        Wizard::publish(default_gate.as_deref(), Vec::new())
                    }
                });
            }
            Some(Action::EnterResolution(snap_id)) => {
                app.record_command(&["resolve".into(), "list".into(), snap_id.clone()]);
                match converge_cli::execute(["resolve".into(), "list".into(), snap_id.clone()]) {
                    Ok(value) => {
                        let mut paths: Vec<(String, u64)> = value
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
                                    .collect()
                            })
                            .unwrap_or_default();
                        paths.sort();
                        app.record_result(Ok(serde_json::json!(format!(
                            "{} superposed path(s)",
                            paths.len()
                        ))));
                        app.resolution = Some(ResolutionState {
                            snap_id,
                            paths,
                            decisions: Default::default(),
                            selected: 0,
                        });
                        app.frames.push(View::Resolution);
                    }
                    Err(err) => app.record_result(Err(err)),
                }
            }
            Some(Action::ApplyResolution) => {
                if let Some(resolution) = app.resolution.take() {
                    let decisions: std::collections::BTreeMap<&String, &u32> =
                        resolution.decisions.iter().collect();
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
                    app.record_command(&argv);
                    let result = std::fs::write(
                        &path,
                        serde_json::to_vec(&decisions).expect("serialize decisions"),
                    )
                    .map_err(anyhow::Error::from)
                    .and_then(|_| converge_cli::execute(argv.iter().cloned()));
                    let _ = std::fs::remove_file(&path);
                    app.record_result(result);
                    if app.current_view() == View::Resolution {
                        app.frames.pop();
                    }
                    refresh(&mut app);
                }
            }
            Some(Action::Run(argv)) => {
                app.record_command(&argv);
                if is_remote_command(&argv) {
                    // Never block the event loop on the network.
                    app.record_in_flight(&argv);
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let result = converge_cli::execute(argv.iter().cloned());
                        let _ = tx.send((argv, result));
                    });
                } else {
                    let result = converge_cli::execute(argv.iter().cloned());
                    trace.command_result(&argv, &result);
                    app.record_result(result);
                    refresh(&mut app);
                }
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
        View::Root => Vec::new(),
    };
    trace.screen_view(&screen_id, &selectable, app.primary_action().0);
}

/// Pull view data through the CLI layer (arch 15: no bespoke semantics).
/// Root views render from the `status` report alone; the snap list feeds
/// the history view.
fn refresh(app: &mut App) {
    app.status = converge_cli::execute(["status"]).ok();
    app.pending_changes = app
        .status
        .as_ref()
        .and_then(|s| s["pending"]["count"].as_u64())
        .unwrap_or(0) as usize;
    app.snaps = converge_cli::execute(["history"])
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
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
                .map(|(i, (path, count))| {
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
        View::History => {
            let items: Vec<ListItem> = app
                .snaps
                .iter()
                .map(|s| {
                    ListItem::new(format!(
                        "{}  {}  {}",
                        s["id"].as_str().unwrap_or("?"),
                        s["created_at"].as_str().unwrap_or(""),
                        s["message"].as_str().unwrap_or("")
                    ))
                })
                .collect();
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
