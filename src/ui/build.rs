//! BuildPhaseTracker, DESIGN.md 3.4. States 6 and 7.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{App, Ending, State};
use crate::theme;
use crate::widgets::{alert_card, card, spread, strong, text};

/// Widest the filled bar itself may grow.
const BAR_MAX: u16 = 44;

pub(super) fn status(app: &App) -> (&'static str, &'static str, ratatui::style::Color) {
    match app.state {
        State::BuildFailed => ("✖", "BUILD FAILED", app.theme.rose),
        State::Stopped if app.ending == Some(Ending::Detached) => {
            (theme::GLYPH_STOP, "DETACHED", app.theme.muted)
        }
        State::Stopped if app.ending == Some(Ending::Lost) => {
            (theme::GLYPH_WARN, "DISCONNECTED", app.theme.rose)
        }
        State::Stopped => (theme::GLYPH_STOP, "STOPPED", app.theme.muted),
        s if s.build_done() => (theme::GLYPH_PLAY, "RUNNING", app.theme.emerald),
        State::Switching if app.run_state().holds_session() => {
            (theme::GLYPH_PLAY, "RUNNING", app.theme.emerald)
        }
        State::Switching => (theme::GLYPH_STOP, "STOPPED", app.theme.muted),
        _ => (app.spinner(), "BUILDING", app.theme.amber),
    }
}

pub(super) fn timings(app: &App) -> Vec<Span<'static>> {
    vec![
        text("Startup ", app.theme.emerald),
        strong(app.startup_clock(), app.theme.text),
        text("   Build ", app.theme.emerald),
        strong(app.build_clock(), app.theme.text),
        text("   Sync ", app.theme.emerald),
        strong(app.sync_time.clone(), app.theme.text),
        text("   Total ", app.theme.emerald),
        strong(app.total_clock(), app.theme.text),
    ]
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
    if !plan.full_build {
        collapsed(frame, area, app);
        return;
    }

    let (_, title, color) = status(app);
    let block = card(title, color, &app.theme);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1), // progress bar
        Constraint::Length(1), // blank
        Constraint::Min(1),    // stage list
    ])
    .split(inner);

    progress(frame, rows[0], app);
    stages(frame, rows[2], app);
}

fn progress(frame: &mut Frame, area: Rect, app: &App) {
    let done = app.stages.iter().filter(|s| s.done).count();
    let total = app.expected_stages();
    let finished = app.state.build_done();

    let bar_w = area.width.saturating_sub(24).min(BAR_MAX) as usize;

    let (reached, denominator) = if finished {
        let ran = app.stages.len();
        (ran, ran)
    } else {
        ((done + 1).min(total), total)
    };

    let filled = bar_w * reached / denominator.max(1);

    let colour = if finished {
        app.theme.emerald
    } else {
        app.theme.amber
    };

    let right = vec![
        text("Stage ", app.theme.muted),
        strong(format!("{reached}"), colour),
        text(format!("/{denominator}"), app.theme.muted),
    ];

    let line = spread(
        area.width,
        vec![
            text("[", app.theme.border),
            strong(
                "▓".repeat(filled),
                if finished {
                    app.theme.emerald
                } else {
                    app.theme.amber
                },
            ),
            text("░".repeat(bar_w.saturating_sub(filled)), app.theme.border),
            text("]", app.theme.border),
        ],
        right,
    );

    frame.render_widget(Paragraph::new(line), area);
}

fn stages(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width;

    let lines: Vec<Line> = app
        .stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let last = i + 1 == app.stages.len();
            let failed = app.state == State::BuildFailed && last;

            let (glyph, color) = if failed {
                ("✖", app.theme.rose)
            } else if stage.done {
                ("✔", app.theme.emerald)
            } else {
                (app.spinner(), app.theme.amber)
            };

            let right = if stage.done || failed {
                stage.duration.clone()
            } else {
                crate::flutter::elapsed(stage.started.elapsed())
            };

            spread(
                w,
                vec![
                    strong(glyph, color),
                    Span::raw(" "),
                    text(stage.label.as_str(), app.theme.text),
                ],
                vec![text(right, app.theme.muted)],
            )
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let (glyph, label, color) = status(app);

    let line = spread(
        area.width,
        vec![strong(glyph, color), Span::raw(" "), strong(label, color)],
        timings(app),
    );

    frame.render_widget(Paragraph::new(line), area);
}

pub fn render_failure(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(failure) = &app.failure else {
        return;
    };

    let title = if failure.location.is_some() {
        "COMPILER ERROR"
    } else {
        "BUILD ERROR"
    };

    let block = alert_card(title, app.theme.rose)
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                text("Exit code ", app.theme.muted),
                strong(app.exit_code.to_string(), app.theme.rose),
                Span::raw(" "),
            ])
            .right_aligned(),
        )
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    for chunk in crate::widgets::wrap(&failure.summary, inner.width as usize) {
        lines.push(Line::from(strong(chunk, app.theme.rose)));
    }

    if let Some((file, line, column)) = &failure.location {
        lines.push(Line::from(vec![
            text(file.as_str(), app.theme.muted),
            text(":", app.theme.border),
            strong(line.to_string(), app.theme.text),
            text(":", app.theme.border),
            strong(column.to_string(), app.theme.text),
        ]));
    }

    lines.push(Line::default());

    let hot_line = failure.location.as_ref().map(|(_, line, _)| *line);

    for (number, source) in &failure.context {
        let hot = Some(*number) == hot_line;

        lines.push(Line::from(vec![
            text(
                format!("{number:>4} "),
                if hot { app.theme.rose } else { app.theme.muted },
            ),
            if hot {
                strong(source.as_str(), app.theme.text)
            } else {
                text(source.as_str(), app.theme.muted)
            },
        ]));
    }

    if !failure.context.is_empty() && failure.caret_col > 0 {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(5 + failure.caret_col.saturating_sub(1) as usize)),
            strong("^", app.theme.rose),
        ]));
    }

    if failure.context.is_empty() {
        for out in &failure.output {
            for chunk in crate::widgets::wrap(out, inner.width as usize) {
                lines.push(Line::from(text(chunk, app.theme.muted)));
            }
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(text(failure.note.as_str(), app.theme.amber)));

    lines.truncate(inner.height as usize);

    frame.render_widget(Paragraph::new(lines), inner);
}
