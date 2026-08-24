//! TerminalFooter (3.7).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Hit, State};
use crate::theme;
use crate::widgets::{keycap, spread, strong, text};

/// Top App Bar inspired by Noodle TUI breadcrumb header.
#[allow(dead_code)]
pub fn header(frame: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 || area.width < 30 {
        return;
    }

    // Left side: frun / project_name ▾   ⎇ branch [✔ clean]
    let mut left = vec![
        strong("frun", app.theme.cyan),
        text(" / ", app.theme.muted),
        strong(app.project.as_str(), app.theme.text),
        text(format!(" {}", theme::GLYPH_DOWN_TRIANGLE), app.theme.muted),
    ];

    if !app.branch.is_empty() && area.width > 70 {
        left.push(text("   ", app.theme.muted));
        left.push(strong(theme::GLYPH_GIT_BRANCH, app.theme.amber));
        left.push(Span::raw(" "));
        left.push(text(crate::widgets::elide(&app.branch, 24), app.theme.amber));

        let (git_str, git_color) = if app.dirty == 0 {
            ("✔ clean", app.theme.emerald)
        } else {
            ("● changed", app.theme.rose)
        };
        left.push(Span::raw(" "));
        left.push(text("[", app.theme.muted));
        left.push(text(git_str, git_color));
        left.push(text("]", app.theme.muted));
    }

    // Right side: ● Target Device · [ RUNNING ]
    let mut right = Vec::new();
    if let Some(target) = &app.target {
        right.push(strong(format!("{} ", theme::GLYPH_DOT), app.theme.emerald));
        right.push(strong(target.name.as_str(), app.theme.text));
        right.push(text(" · ", app.theme.muted));
    }

    let (state_label, state_color) = match app.state {
        State::Detecting => ("DETECTING", app.theme.cyan),
        State::Booting => ("BOOTING", app.theme.amber),
        State::MultipleDevices => ("SELECT DEVICE", app.theme.cyan),
        State::SingleDevice => ("LAUNCHING", app.theme.cyan),
        State::Building => ("BUILDING", app.theme.amber),
        State::BuildFailed => ("FAILED", app.theme.rose),
        State::Running => ("RUNNING", app.theme.emerald),
        State::ReloadInFlight => ("RELOADING", app.theme.purple),
        State::ReloadFailed => ("RELOAD FAILED", app.theme.rose),
        State::ReloadDropped => ("RELOAD DROPPED", app.theme.amber),
        State::Stopped => ("STOPPED", app.theme.muted),
        State::Switching => ("SWITCHING", app.theme.purple),
    };

    right.push(Span::styled(
        format!(" {state_label} "),
        Style::new().bg(app.theme.surface).fg(state_color).bold(),
    ));

    let line = spread(area.width, left, right);
    frame.render_widget(Paragraph::new(line), area);
}

fn new_tab<'a>(
    hints: &mut Vec<(&'a str, &'a str, ratatui::style::Color, Option<Action>)>,
    app: &App,
) {
    if !app.shift_enter {
        return;
    }

    hints.push((
        Action::NewTab.key(),
        Action::NewTab.label(),
        app.theme.cyan,
        Some(Action::NewTab),
    ));
}

pub fn footer(frame: &mut Frame, area: Rect, app: &mut App, plan: Option<&Budget>) {
    let expand = if app.expanded { "Collapse" } else { "Expand" };
    let hint = |action: Action, color| (action.key(), action.label(), color, Some(action));

    let hints: Vec<(&str, &str, ratatui::style::Color, Option<Action>)> = match app.state {
        State::Detecting | State::Booting => vec![
            ("^C", "Cancel", app.theme.rose, None),
            hint(Action::Theme, app.theme.cyan),
        ],

        State::MultipleDevices => {
            let mut hints = vec![
                ("↑↓", "Move", app.theme.cyan, None),
                ("⏎", "Launch", app.theme.emerald, None),
            ];

            new_tab(&mut hints, app);
            hints.push(hint(Action::Theme, app.theme.cyan));
            hints.push(("Esc", "Cancel", app.theme.rose, None));

            hints
        }

        State::Switching => {
            let mut hints = vec![
                ("↑↓", "Move", app.theme.cyan, None),
                ("⏎", "Switch", app.theme.emerald, None),
            ];

            new_tab(&mut hints, app);
            hints.push(hint(Action::Theme, app.theme.cyan));
            hints.push(("Esc", "Back", app.theme.rose, None));

            hints
        }

        State::SingleDevice => vec![
            hint(Action::Stop, app.theme.rose),
            hint(Action::Theme, app.theme.cyan),
        ],

        State::Building => vec![
            hint(Action::Switch, app.theme.cyan),
            hint(Action::Theme, app.theme.cyan),
            hint(Action::StopRun, app.theme.amber),
            hint(Action::Stop, app.theme.rose),
        ],

        State::Stopped => vec![
            ("r", "Build again", app.theme.emerald, Some(Action::RetryBuild)),
            hint(Action::Switch, app.theme.cyan),
            ("e", expand, app.theme.cyan, None),
            hint(Action::Theme, app.theme.cyan),
            hint(Action::Quit, app.theme.muted),
        ],

        State::BuildFailed => vec![
            hint(Action::RetryBuild, app.theme.rose),
            hint(Action::Theme, app.theme.cyan),
            ("q", "Quit", app.theme.muted, None),
            ("^C", "Stop", app.theme.rose, None),
        ],

        State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped => {
            vec![
                hint(Action::Reload, app.theme.amber),
                hint(Action::Restart, app.theme.purple),
                hint(Action::Switch, app.theme.cyan),
                ("e", expand, app.theme.cyan, None),
                hint(Action::Theme, app.theme.cyan),
                hint(Action::StopRun, app.theme.amber),
                ("q", "Quit", app.theme.rose, None),
                hint(Action::Stop, app.theme.rose),
            ]
        }
    };

    let draw = |labelled: bool| -> Vec<Vec<Span>> {
        hints
            .iter()
            .map(|(key, label, color, _)| {
                let mut spans = keycap(key, *color, &app.theme);

                if labelled {
                    spans.push(text(" ", app.theme.muted));
                    spans.push(text(*label, app.theme.muted));
                }

                spans
            })
            .collect()
    };

    let measure = |rendered: &[Vec<Span>]| -> Vec<usize> {
        rendered
            .iter()
            .map(|spans| spans.iter().map(Span::width).sum())
            .collect()
    };

    let gaps = hints.len().saturating_sub(1);
    let fits = |sum: usize, gap: u16| sum + gaps * gap as usize <= area.width as usize;

    let mut rendered = draw(true);
    let mut natural = measure(&rendered);
    let mut sum: usize = natural.iter().sum();
    let mut gap: u16 = 2;

    if !fits(sum, gap) {
        gap = 1;
    }

    if !fits(sum, gap) {
        rendered = draw(false);
        natural = measure(&rendered);
        sum = natural.iter().sum();
        gap = if fits(sum, 2) { 2 } else { 1 };
    }

    let content = sum + gaps * gap as usize;

    let (index, total) = app.position();
    let mut optional: Vec<Vec<Span>> = Vec::new();

    if !app.live {
        optional.push(vec![
            text(
                format!("proto {index}/{total} {}", app.state.slug()),
                app.theme.border,
            ),
            text("  ⇥ next", app.theme.border),
        ]);
    }

    if let Some(plan) = plan {
        let given_up = plan.describe(app.state);

        if given_up != "full" {
            optional.push(vec![text(format!("[{given_up}]"), app.theme.border)]);
        }
    }

    if app.mouse_on {
        optional.push(vec![strong("mouse on", app.theme.emerald)]);
    }

    let mut kept: Vec<Vec<Span>> = Vec::new();
    let mut used = 0usize;

    for group in optional.into_iter().rev() {
        let group_w: usize = group.iter().map(Span::width).sum();

        if content + used + group_w + 4 > area.width as usize {
            break;
        }

        used += group_w + 2;
        kept.push(group);
    }

    let mut right: Vec<Span> = Vec::new();

    for (i, group) in kept.into_iter().rev().enumerate() {
        if i > 0 {
            right.push(text("  ", app.theme.muted));
        }

        right.extend(group);
    }

    let right_w: usize = right.iter().map(Span::width).sum();

    let cols =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(right_w as u16)])
            .spacing(if right.is_empty() { 0 } else { 2 })
            .split(area);

    let mut x = cols[0].x + cols[0].width.saturating_sub(content as u16) / 2;

    for ((spans, width), (_, _, _, action)) in rendered.into_iter().zip(&natural).zip(&hints) {
        let slot = Rect {
            x,
            y: cols[0].y,
            width: *width as u16,
            height: 1,
        };

        if let Some(action) = action {
            app.hits.push(Hit {
                area: slot,
                action: *action,
            });
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), slot);

        x += *width as u16 + gap;
    }

    if !right.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(right).right_aligned()), cols[1]);
    }
}
