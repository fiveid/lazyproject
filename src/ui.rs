use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{AppState, DisplayRow};
use crate::git::RemoteProvider;
use crate::scanner::ProjectEntry;

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
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(chunks[1]);

    render_project_list(frame, app, body[0]);
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

fn render_project_list(frame: &mut Frame, app: &AppState, area: Rect) {
    let rows = app.display_rows();
    let non_git_total = app.non_git_projects.len();

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            DisplayRow::Git(idx) => render_git_row(&app.git_projects[*idx]),
            DisplayRow::NonGitToggle => {
                let arrow = if app.non_git_expanded { "▾" } else { "▸" };
                let label = format!(" {arrow} +{non_git_total} projects not on Git");
                ListItem::new(Line::from(Span::styled(
                    label,
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )))
            }
            DisplayRow::NonGit(idx) => {
                let proj = &app.non_git_projects[*idx];
                let rel = format_relative_time(proj.last_activity_ts);
                ListItem::new(Line::from(vec![
                    Span::styled("   [DIR] ", Style::default().fg(Color::DarkGray)),
                    Span::styled(proj.name.clone(), Style::default().fg(Color::Gray)),
                    Span::raw("  "),
                    Span::styled(rel, Style::default().fg(Color::DarkGray)),
                ]))
            }
        })
        .collect();

    let mut list_state = ListState::default();
    if !rows.is_empty() {
        list_state.select(Some(app.selected.min(rows.len() - 1)));
    }

    let title = if app.filter_query.is_empty() {
        " Projects ".to_string()
    } else {
        format!(" Projects (filter: {}) ", app.filter_query)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_git_row(proj: &ProjectEntry) -> ListItem<'static> {
    let Some(git) = &proj.git else {
        return ListItem::new(proj.name.clone());
    };

    let mut spans = vec![
        Span::styled(
            format!("[{}] ", git.provider.short_badge()),
            Style::default().fg(provider_color(&git.provider)),
        ),
        Span::styled(
            proj.name.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            git.branch.clone(),
            Style::default().fg(if git.detached {
                Color::Red
            } else {
                Color::Blue
            }),
        ),
    ];

    if git.ahead > 0 {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("↑{}", git.ahead),
            Style::default().fg(Color::Magenta),
        ));
    }
    if git.behind > 0 {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("↓{}", git.behind),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }
    if git.is_dirty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "*",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("✓", Style::default().fg(Color::Green)));
    }

    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format_relative_time(proj.last_activity_ts),
        Style::default().fg(Color::DarkGray),
    ));

    ListItem::new(Line::from(spans))
}

fn render_inspector(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .title(" Inspector ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let Some(proj) = app.selected_project() else {
        let msg = if app.non_git_projects.is_empty() && app.git_projects.is_empty() {
            "No projects found in this directory."
        } else {
            "Press Enter or Space to expand/collapse non-Git directories."
        };
        let p = Paragraph::new(msg)
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
                            "staged:{} modified:{} untracked:{} conflicted:{}",
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
        "j/k: move  Enter: expand non-git  o: open tab  f/F: fetch one/all  /: filter  r: refresh  ?: help  q: quit",
        Style::default().fg(Color::DarkGray),
    ));

    let footer = Paragraph::new(vec![line1, line2]);
    frame.render_widget(footer, area);
}

fn render_help_modal(frame: &mut Frame, area: Rect) {
    let modal_area = Rect {
        x: area.width.saturating_sub(62) / 2,
        y: area.height.saturating_sub(18) / 2,
        width: area.width.min(62),
        height: area.height.min(18),
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
        Line::raw("  j / Down        Move selection down"),
        Line::raw("  k / Up          Move selection up"),
        Line::raw("  g / G           Jump to top / bottom"),
        Line::raw("  Enter / Space   Expand or collapse non-Git projects"),
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
