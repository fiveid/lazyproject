use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{AppState, DisplayRow, FetchState};
use crate::git::{GitInfo, RemoteProvider};
use crate::scanner::ProjectEntry;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn format_relative_time(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(ts);
    let diff = (now - ts).max(0);
    if diff < 60 {
        format!("{diff}s")
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86_400 {
        format!("{}h", diff / 3600)
    } else if diff < 86_400 * 30 {
        format!("{}d", diff / 86_400)
    } else if diff < 86_400 * 365 {
        format!("{}mo", diff / (86_400 * 30))
    } else {
        format!("{}y", diff / (86_400 * 365))
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1}G", b / GIB)
    } else if b >= MIB {
        format!("{:.0}M", b / MIB)
    } else if b >= KIB {
        format!("{:.0}K", b / KIB)
    } else {
        format!("{bytes}B")
    }
}

pub fn format_dirty_compact(git: &GitInfo) -> String {
    if !git.is_dirty() {
        return "✓".to_string();
    }
    let mut parts = Vec::new();
    if git.staged > 0 {
        parts.push(format!("+{}", git.staged));
    }
    if git.modified > 0 {
        parts.push(format!("~{}", git.modified));
    }
    if git.untracked > 0 {
        parts.push(format!("?{}", git.untracked));
    }
    if git.conflicted > 0 {
        parts.push(format!("!{}", git.conflicted));
    }
    parts.join(" ")
}

pub fn format_sync_compact(git: &GitInfo) -> String {
    match (git.ahead, git.behind) {
        (0, 0) => "-".to_string(),
        (a, 0) => format!("↑{a}"),
        (0, b) => format!("↓{b}"),
        (a, b) => format!("↑{a} ↓{b}"),
    }
}

pub fn render(frame: &mut Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(chunks[1]);

    render_project_table(frame, app, body[0]);
    render_inspector(frame, app, body[1]);
    render_footer(frame, app, chunks[2]);

    if app.show_help {
        render_help_modal(frame, frame.area());
    }
}

fn render_header(frame: &mut Frame, app: &AppState, area: Rect) {
    let dirty_count = app
        .git_projects
        .iter()
        .filter(|p| p.git.as_ref().is_some_and(|g| g.is_dirty()))
        .count();
    let ahead_count = app
        .git_projects
        .iter()
        .filter(|p| p.git.as_ref().is_some_and(|g| g.ahead > 0))
        .count();
    let behind_count = app
        .git_projects
        .iter()
        .filter(|p| p.git.as_ref().is_some_and(|g| g.behind > 0))
        .count();

    let title_line = Line::from(vec![
        Span::styled(
            " lazyproject ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            app.root.display().to_string(),
            Style::default().fg(Color::Gray),
        ),
        Span::raw("   "),
        Span::styled(
            format!("{} git", app.git_projects.len()),
            Style::default().fg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{dirty_count} dirty"),
            Style::default().fg(if dirty_count > 0 {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw("  "),
        Span::styled(
            format!("↑{ahead_count}"),
            Style::default().fg(if ahead_count > 0 {
                Color::Magenta
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw(" "),
        Span::styled(
            format!("↓{behind_count}"),
            Style::default().fg(if behind_count > 0 {
                Color::Red
            } else {
                Color::DarkGray
            }),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let header = Paragraph::new(title_line).block(block);
    frame.render_widget(header, area);
}

fn provider_color(provider: &RemoteProvider) -> Color {
    match provider {
        RemoteProvider::GitHub => Color::Cyan,
        RemoteProvider::GitLab => Color::LightRed,
        RemoteProvider::Bitbucket => Color::Blue,
        RemoteProvider::Other => Color::Green,
        RemoteProvider::None => Color::DarkGray,
    }
}

fn fetch_indicator(app: &AppState, proj: &ProjectEntry) -> (String, Color) {
    match app.fetch_states.get(&proj.path) {
        Some(FetchState::Fetching) => {
            let spinner = SPINNER_FRAMES[app.tick % SPINNER_FRAMES.len()];
            (spinner.to_string(), Color::Yellow)
        }
        Some(FetchState::Failed(_)) => ("●".to_string(), Color::Red),
        Some(FetchState::Done) => ("●".to_string(), Color::Green),
        Some(FetchState::Idle) | None => (" ".to_string(), Color::DarkGray),
    }
}

fn render_project_table(frame: &mut Frame, app: &AppState, area: Rect) {
    let rows = app.display_rows();

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|row| match row {
            DisplayRow::Git(idx) => build_git_row(app, &app.git_projects[*idx]),
            DisplayRow::NonGit(idx) => build_non_git_row(app, &app.non_git_projects[*idx]),
        })
        .collect();

    let mut table_state = TableState::default();
    if !rows.is_empty() {
        table_state.select(Some(app.selected.min(rows.len() - 1)));
    }

    let non_git_count = app.non_git_projects.len();
    let non_git_hint = if non_git_count == 0 {
        String::new()
    } else if app.show_non_git {
        format!(" (+{non_git_count} non-git shown, n to hide)")
    } else {
        format!(" (+{non_git_count} non-git, n to show)")
    };

    let title = if app.filter_query.is_empty() {
        format!(" Projects{non_git_hint} ")
    } else {
        format!(" Projects (filter: {}){non_git_hint} ", app.filter_query)
    };

    let border_color = if !app.inspector_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let header = Row::new(vec![
        Cell::from(" "),
        Cell::from("Host"),
        Cell::from("Project"),
        Cell::from("Branch"),
        Cell::from("Sync"),
        Cell::from("Tree"),
        Cell::from("Size"),
        Cell::from("Age"),
    ])
    .style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Min(16),
            Constraint::Length(14),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .column_spacing(1)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶");

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn build_git_row(app: &AppState, proj: &ProjectEntry) -> Row<'static> {
    let Some(git) = &proj.git else {
        return Row::new(vec![Cell::from(proj.name.clone())]);
    };

    let (dot, dot_color) = fetch_indicator(app, proj);
    let sync_str = format_sync_compact(git);
    let sync_color = if git.behind > 0 {
        Color::Red
    } else if git.ahead > 0 {
        Color::Magenta
    } else {
        Color::DarkGray
    };

    let dirty_str = format_dirty_compact(git);
    let dirty_color = if git.is_dirty() {
        Color::Yellow
    } else {
        Color::Green
    };

    let size_str = app
        .disk_usage
        .get(&proj.path)
        .map(|u| format_bytes(u.total_bytes))
        .unwrap_or_else(|| "…".to_string());

    Row::new(vec![
        Cell::from(dot).style(Style::default().fg(dot_color)),
        Cell::from(git.provider.short_badge())
            .style(Style::default().fg(provider_color(&git.provider))),
        Cell::from(proj.name.clone()).style(Style::default().fg(Color::White)),
        Cell::from(git.branch.clone()).style(Style::default().fg(if git.detached {
            Color::Red
        } else {
            Color::Blue
        })),
        Cell::from(sync_str).style(Style::default().fg(sync_color)),
        Cell::from(dirty_str).style(Style::default().fg(dirty_color)),
        Cell::from(size_str).style(Style::default().fg(Color::Gray)),
        Cell::from(format_relative_time(proj.last_activity_ts))
            .style(Style::default().fg(Color::DarkGray)),
    ])
}

fn build_non_git_row(app: &AppState, proj: &ProjectEntry) -> Row<'static> {
    let size_str = app
        .disk_usage
        .get(&proj.path)
        .map(|u| format_bytes(u.total_bytes))
        .unwrap_or_else(|| "…".to_string());

    Row::new(vec![
        Cell::from(" "),
        Cell::from("DIR").style(Style::default().fg(Color::DarkGray)),
        Cell::from(proj.name.clone()).style(Style::default().fg(Color::Gray)),
        Cell::from("-").style(Style::default().fg(Color::DarkGray)),
        Cell::from("-").style(Style::default().fg(Color::DarkGray)),
        Cell::from("-").style(Style::default().fg(Color::DarkGray)),
        Cell::from(size_str).style(Style::default().fg(Color::DarkGray)),
        Cell::from(format_relative_time(proj.last_activity_ts))
            .style(Style::default().fg(Color::DarkGray)),
    ])
}

fn render_inspector(frame: &mut Frame, app: &AppState, area: Rect) {
    let border_color = if app.inspector_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let title = if app.inspector_focused {
        " Inspector (focused - j/k scroll, Esc/Enter back) "
    } else {
        " Inspector (Enter to focus) "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let Some(proj) = app.selected_project() else {
        let p = Paragraph::new("No project selected.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block)
            .wrap(Wrap { trim: true });
        frame.render_widget(p, area);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            proj.name.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
        Span::raw(proj.path.display().to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Activity: ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!(
            "{} ago",
            format_relative_time(proj.last_activity_ts)
        )),
    ]));

    if let Some(usage) = app.disk_usage.get(&proj.path) {
        let mut disk_spans = vec![
            Span::styled("Disk: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format_bytes(usage.total_bytes)),
        ];
        if usage.reclaimable_bytes > 0 {
            disk_spans.push(Span::styled(
                format!(
                    " ({} reclaimable deps/build)",
                    format_bytes(usage.reclaimable_bytes)
                ),
                Style::default().fg(Color::Yellow),
            ));
        }
        lines.push(Line::from(disk_spans));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Disk: ", Style::default().fg(Color::DarkGray)),
            Span::styled("calculating...", Style::default().fg(Color::DarkGray)),
        ]));
    }

    if let Some(FetchState::Failed(err)) = app.fetch_states.get(&proj.path) {
        lines.push(Line::from(vec![
            Span::styled("Fetch Error: ", Style::default().fg(Color::Red)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
        ]));
    }

    if let Some(git) = &proj.git {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Git",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(err) = &git.error {
            lines.push(Line::from(Span::styled(
                format!("  Error: {err}"),
                Style::default().fg(Color::Red),
            )));
        } else {
            lines.push(Line::from(vec![
                Span::raw("  Branch: "),
                Span::styled(git.branch.clone(), Style::default().fg(Color::Blue)),
                if let Some(up) = &git.upstream {
                    Span::styled(format!(" -> {up}"), Style::default().fg(Color::DarkGray))
                } else {
                    Span::styled(" (no upstream)", Style::default().fg(Color::DarkGray))
                },
            ]));

            lines.push(Line::from(vec![
                Span::raw("  Sync:   "),
                Span::styled(
                    format!("↑{} ahead", git.ahead),
                    Style::default().fg(if git.ahead > 0 {
                        Color::Magenta
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("↓{} behind", git.behind),
                    Style::default().fg(if git.behind > 0 {
                        Color::Red
                    } else {
                        Color::DarkGray
                    }),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::raw("  Tree:   "),
                if git.is_dirty() {
                    Span::styled(
                        format!(
                            "staged:+{} modified:~{} untracked:?{} conflicted:!{}",
                            git.staged, git.modified, git.untracked, git.conflicted
                        ),
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    Span::styled("clean", Style::default().fg(Color::Green))
                },
            ]));

            let remote_display = git.remote_url.as_deref().unwrap_or("none");
            lines.push(Line::from(vec![
                Span::raw(format!("  Remote ({}): ", git.provider.label())),
                Span::styled(remote_display, Style::default().fg(Color::Gray)),
            ]));

            if !git.recent_commits.is_empty() {
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled(
                    "Recent Commits",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                for c in &git.recent_commits {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {} ", c.short_sha),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            format!("({}) ", format_relative_time(c.timestamp)),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(c.subject.clone()),
                    ]));
                }
            }
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "Toolchain & Platform",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    if proj.toolchain.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No mise or package manifest detected",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        if let Some(src) = &proj.toolchain.mise_source {
            lines.push(Line::from(Span::styled(
                format!("  Mise ({src}):"),
                Style::default().fg(Color::Green),
            )));
            for tool in &proj.toolchain.mise_tools {
                lines.push(Line::from(format!("    {} = {}", tool.name, tool.version)));
            }
        }
        for eco in &proj.toolchain.ecosystems {
            let pm = eco
                .package_manager
                .as_deref()
                .map(|p| format!(" [{p}]"))
                .unwrap_or_default();
            let detail = eco
                .detail
                .as_deref()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default();
            lines.push(Line::from(format!("  {}{pm}{detail}", eco.name)));
        }
    }

    let inspector = Paragraph::new(lines)
        .block(block)
        .scroll((app.inspector_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(inspector, area);
}

fn render_footer(frame: &mut Frame, app: &AppState, area: Rect) {
    let line1 = if app.filter_active {
        Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Cyan)),
            Span::raw(app.filter_query.clone()),
            Span::styled("_", Style::default().fg(Color::Yellow)),
            Span::styled(
                "   (Enter to apply, Esc to clear)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                if app.busy { "● " } else { "○ " },
                Style::default().fg(if app.busy {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            ),
            Span::raw(app.status_message.clone()),
        ])
    };

    let line2 = Line::from(Span::styled(
        "j/k: move  Enter: focus inspector  n: toggle non-git  o: open tab  f/F: fetch one/all  /: filter  r: refresh  ?: help  q: quit",
        Style::default().fg(Color::DarkGray),
    ));

    let footer = Paragraph::new(vec![line1, line2]);
    frame.render_widget(footer, area);
}

fn render_help_modal(frame: &mut Frame, area: Rect) {
    let modal_area = Rect {
        x: area.width.saturating_sub(64) / 2,
        y: area.height.saturating_sub(19) / 2,
        width: area.width.min(64),
        height: area.height.min(19),
    };
    frame.render_widget(Clear, modal_area);

    let text = vec![
        Line::from(Span::styled(
            "lazyproject Keybindings",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::raw("  j / Down        Move selection down (or scroll inspector)"),
        Line::raw("  k / Up          Move selection up (or scroll inspector)"),
        Line::raw("  g / G           Jump to top / bottom"),
        Line::raw("  Enter / Tab     Focus / unfocus Inspector pane"),
        Line::raw("  n               Toggle showing non-Git directories"),
        Line::raw("  o               Open project in new terminal tab"),
        Line::raw("  f               Fetch remote for selected repo"),
        Line::raw("  F               Fetch remotes for all repos"),
        Line::raw("  /               Filter projects by name"),
        Line::raw("  r               Refresh workspace"),
        Line::raw("  ?               Toggle this help"),
        Line::raw("  q / Ctrl+C      Quit"),
        Line::raw(""),
        Line::from(Span::styled(
            "Press Esc or ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let modal = Paragraph::new(text).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(modal, modal_area);
}
