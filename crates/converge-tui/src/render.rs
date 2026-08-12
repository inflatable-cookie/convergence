//! The whole screen: header, the active view, the Last strip,
//! the suggestion palette, the wizard and decision modals, and the
//! footer that is the navigation surface.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app;
use crate::wizard;

use app::{App, LastLine, ResolutionState, View};
use wizard::WizardStep;

pub(crate) fn render(frame: &mut Frame, app: &App) {
    let suggestion_rows = app.suggestions.len().min(9) as u16;
    let [header, body, last, suggestions, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(suggestion_rows),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(frame, app, header);
    render_body(frame, app, body);
    render_last(frame, app, last);
    render_suggestions(frame, app, suggestions);
    render_wizard(frame, app, body);
    render_decision(frame, app, body);
    render_footer(frame, app, input);
}

fn render_header(frame: &mut Frame, app: &App, header: Rect) {
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
}

fn render_last(frame: &mut Frame, app: &App, last: Rect) {
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
}

fn render_suggestions(frame: &mut Frame, app: &App, suggestions: Rect) {
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
}

fn render_wizard(frame: &mut Frame, app: &App, body: Rect) {
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
}

fn render_decision(frame: &mut Frame, app: &App, body: Rect) {
    // wizard: it covers the screen because there is nothing else worth
    // looking at until it is answered.
    if let Some(decision) = &app.decision {
        let mut lines = vec![
            Line::styled(
                decision.title.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(
                "what is at stake",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        for risk in &decision.risks {
            lines.push(Line::styled(
                format!("  {risk}"),
                Style::default().fg(Color::Yellow),
            ));
        }
        lines.push(Line::raw(""));
        for opt in &decision.options {
            // The key is what the person presses, so it leads and it is
            // the only coloured thing on the line.
            let mut spans = vec![
                Span::styled(
                    format!("  [{}] ", opt.key),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    opt.label.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];
            if opt.recommended {
                spans.push(Span::styled(
                    "  (recommended)",
                    Style::default().fg(Color::Green),
                ));
            }
            lines.push(Line::from(spans));
            lines.push(Line::styled(
                format!("      {}", opt.detail),
                Style::default().fg(Color::Gray),
            ));
        }
        frame.render_widget(ratatui::widgets::Clear, body);
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Your work would be affected"),
            ),
            body,
        );
    }
}

fn render_footer(frame: &mut Frame, app: &App, input: Rect) {
    // mode it lists every destination with the bare key that reaches it
    // — visible, not learned, because the previous scheme was Alt-only
    // and stock macOS terminals never deliver Alt, so from 23.1 to 27.1
    // there was no working navigation at all. In command mode it is the
    // console with a caret.
    if let Some(decision) = &app.decision {
        let mut spans = vec![Span::styled(
            "choose: ",
            Style::default().add_modifier(Modifier::BOLD),
        )];
        for opt in &decision.options {
            spans.push(Span::styled(
                format!("{} ", opt.key),
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::styled(
                format!("{}  ", opt.label),
                Style::default().fg(Color::Gray),
            ));
        }
        spans.push(Span::styled("Esc ", Style::default().fg(Color::Cyan)));
        spans.push(Span::styled(
            "leave it alone",
            Style::default().fg(Color::Gray),
        ));
        frame.render_widget(Paragraph::new(Line::from(spans)), input);
    } else if app.quit_confirm || app.pending_confirm.is_some() {
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
        // The footer lists what THIS view can do (operator: the global
        // jumps "aren't that useful from a sibling section"). The full
        // section list stays on the root, whose tiles are the sections,
        // and in Help; every view keeps `Esc back` as the way home.
        let pairs: &[(&str, &str)] = match app.current_view() {
            View::Root => &[
                ("↑↓←→", "choose "),
                ("1-6", "open "),
                ("h", "history "),
                ("i", "inbox "),
                ("c", "candidates "),
                ("l", "lanes "),
                ("e", "releases "),
                ("g", "gates "),
                ("s", "secrets "),
            ],
            View::History => &[
                ("↑↓", "select "),
                ("d", "diff vs head "),
                ("m", "annotate "),
            ],
            View::Candidates => &[("↑↓", "select "), ("p", "promote "), ("e", "release ")],
            View::Gates => &[("↑↓", "select "), ("a", "add "), ("d", "remove ")],
            View::Lanes => &[("↑↓", "select "), ("p", "push "), ("m", "add member ")],
            View::Releases => &[("↑↓", "select "), ("y", "yank ")],
            View::Secrets => &[("↑↓", "select "), ("r", "rotate "), ("u", "unshare ")],
            View::Inbox => &[("↑↓", "select ")],
            View::Resolution => &[
                ("1-9", "pick variant "),
                ("n", "next missing "),
                ("f", "next invalid "),
            ],
            _ => &[("↑↓", "select ")],
        };
        for (k, l) in pairs {
            spans.push(key(k));
            spans.push(Span::raw(" "));
            spans.push(label(l));
        }
        for (k, l) in [
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

fn render_body(frame: &mut Frame, app: &App, body: Rect) {
    match app.current_view() {
        View::Root if app.workspace_missing => render_workspace_missing(frame, app, body),
        View::Root => render_root(frame, app, body),
        View::Resolution => render_resolution(frame, app, body),
        View::Inbox => render_inbox(frame, app, body),
        view @ (View::Candidates | View::Releases | View::Lanes | View::Gates) => {
            render_rows(frame, app, view, body)
        }
        View::Secrets => render_secrets(frame, app, body),
        View::Help => render_help(frame, app, body),
        View::History => render_history(frame, app, body),
    }
}

fn render_workspace_missing(frame: &mut Frame, app: &App, body: Rect) {
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

fn render_root(frame: &mut Frame, app: &App, body: Rect) {
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

fn render_resolution(frame: &mut Frame, app: &App, body: Rect) {
    let empty = ResolutionState::default();
    let resolution = app.resolution.as_ref().unwrap_or(&empty);
    // 65/35 list + detail (spec §6). Batch 23.1 recorded the
    // flat list as a decision-correctness problem rather than
    // polish: the screen asked you to choose between two file
    // contents and showed you neither.
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)]).areas(body);

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
                        detail.push(Line::styled("    …", Style::default().fg(Color::DarkGray)));
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
        Paragraph::new(detail).block(Block::default().borders(Borders::ALL).title("Variants")),
        detail_area,
    );
}

fn render_inbox(frame: &mut Frame, app: &App, body: Rect) {
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
            // full 64-character candidate id and all, so it was
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

fn render_rows(frame: &mut Frame, app: &App, view: View, body: Rect) {
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
        // with eleven candidates in it, this pane read "no candidates
        // (or not loaded yet)" — because the Candidates view is fed
        // by `inbox`, which reports only what needs attention,
        // and every candidate was ready to promote with no
        // approvals required. The name promises a list; the
        // source is an action queue, and the empty state was the
        // only place that difference showed.
        // A list item does not wrap, so long copy is split by
        // hand rather than silently truncated at the pane edge.
        for line in match view {
            View::Candidates => &[
                "nothing needs attention here.",
                "this view lists candidates waiting on you — an approval, or a",
                "superposition to resolve — not every candidate in the repo.",
            ][..],
            View::Releases => &["no releases yet.", "  release <candidate> --as 1.0.0"],
            View::Lanes => &["no lanes yet."],
            View::Gates => &["no gate graph loaded."],
            _ => &["nothing here yet."],
        } {
            items.push(ListItem::new(*line));
        }
    }
    frame.render_widget(List::new(items).block(view_block(app)), body);
}

fn render_secrets(frame: &mut Frame, app: &App, body: Rect) {
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

fn render_help(frame: &mut Frame, app: &App, body: Rect) {
    let mut lines = vec![
        Line::styled("keys", Style::default().add_modifier(Modifier::BOLD)),
        Line::raw("  Enter: primary action   Esc: back   q: quit"),
        Line::raw("  go anywhere: h history  i inbox  c candidates  l lanes"),
        Line::raw("               e releases  g gates  s secrets  r root"),
        Line::raw("  `:` opens the command console (Tab completes, Esc closes)"),
        Line::raw("  in-view: History m annotate d diff  ·  Candidates p promote e release"),
        Line::raw("           Lanes p push m add member  ·  Releases y yank"),
        Line::styled(
            "  Enter on a lane or release brings it into your workspace; if that",
            Style::default().fg(Color::DarkGray),
        ),
        Line::styled(
            "  would cost you anything, it says what and offers a way to keep it.",
            Style::default().fg(Color::DarkGray),
        ),
        Line::raw("           Gates a add d remove  ·  Secrets r rotate u unshare"),
        Line::styled(
            "  wizards: type a bare `member add`, `fetch`, `release <id>`, `promote <id>`",
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

fn render_history(frame: &mut Frame, app: &App, body: Rect) {
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

/// One row of a list view, rendered from whatever the verb returned.
///
/// Deliberately field-driven rather than per-view formatters: these are
/// CLI payloads, and a view that invented its own vocabulary would be
/// the divergence the argv contract exists to prevent.
pub(crate) fn row_label(row: &serde_json::Value) -> String {
    let s = |key: &str| row[key].as_str().unwrap_or("").to_string();
    if !s("candidate_id").is_empty() && !s("version").is_empty() {
        return format!(
            "{}  {}  by {}  {}",
            s("version"),
            short_id(&s("candidate_id")),
            s("released_by"),
            s("created_at")
        );
    }
    if !s("candidate_id").is_empty() {
        // The title leads (operator: candidates "keyed only by the hash
        // ID... need to be named"). A candidate is a derived artifact, so
        // its name is the newest work inside it; the id stays, short
        // and last, for the moment somebody needs to paste it.
        return format!(
            "\"{}\"  @ {}  -> {}  ({}/{} approvals)  {}",
            s("title"),
            s("gate_id"),
            s("recommendation"),
            row["approvals"],
            row["required_approvals"],
            short_id(&s("candidate_id"))
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
                        View::Candidates => "no candidates waiting",
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
                        View::Candidates => format!(
                            "\"{}\"  {}",
                            row["title"].as_str().unwrap_or(""),
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
                            row["candidate_id"]
                                .as_str()
                                .map(short_id)
                                .unwrap_or_default()
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
