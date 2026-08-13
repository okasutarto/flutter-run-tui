//! BuildPhaseTracker, DESIGN.md 3.4. States 6 and 7.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Hit, State};
use crate::theme;
use crate::widgets::{alert_card, card, keycap, spread, strong, text};

/// Widest a stage row may span, so the duration does not fly out to column 140
/// and detach from its label.
const STEP_W: u16 = 66;

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
    if !plan.full_build {
        collapsed(frame, area, app);
        return;
    }

    let failed = app.state == State::BuildFailed;
    let done = app.state.build_done();

    let (title, color) = if failed {
        ("BUILD FAILED", theme::ROSE)
    } else if done {
        ("BUILD FINISHED", theme::EMERALD)
    } else {
        ("BUILDING", theme::AMBER)
    };

    let block = card(title, color).title_top(
        Line::from(vec![
            Span::raw(" "),
            text("Build time ", theme::MUTED),
            strong(app.build_time, theme::TEXT),
            text("   Sync ", theme::MUTED),
            strong(app.sync_time, theme::TEXT),
            Span::raw(" "),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Slot model: the blank between the bar and the stage list is its own slot.
    // Charged in `Budget::build_h`, without which the Layout would keep the old
    // height and clip the last stage.
    let rows = Layout::vertical([
        Constraint::Length(1), // progress bar
        Constraint::Length(1), // blank
        Constraint::Min(1),    // stage list
    ])
    .split(inner);

    progress(frame, rows[0], app);
    stages(frame, rows[2], app);
}

/// Filled bar with a step counter and no denominator.
///
/// The total is not knowable in advance: stage count depends on platform, and
/// Flutter skips stages when it can (no `pod install` if `Podfile.lock` is
/// current, no install when attaching to an already-installed app). Any
/// denominator shown mid-build is a guess, and a bar that reaches 100% and then
/// keeps working is worse than no bar. Once the build ends the count is known
/// and gets stated.
fn progress(frame: &mut Frame, area: Rect, app: &App) {
    let done = app.stages.iter().filter(|s| s.done).count();
    let total = app.stages.len();
    let finished = app.state.build_done();

    let bar_w = area.width.min(STEP_W).saturating_sub(24) as usize;

    let filled = if finished {
        bar_w
    } else if total == 0 {
        0
    } else {
        bar_w * done / total.max(1)
    };

    let right = if finished {
        vec![
            strong(format!("{total} stages"), theme::EMERALD),
            text("  complete", theme::MUTED),
        ]
    } else {
        vec![
            text("Stage ", theme::MUTED),
            strong(format!("{}", done + 1), theme::AMBER),
        ]
    };

    let line = spread(
        area.width.min(STEP_W),
        vec![
            text("[", theme::BORDER),
            strong(
                "▓".repeat(filled),
                if finished {
                    theme::EMERALD
                } else {
                    theme::AMBER
                },
            ),
            text("░".repeat(bar_w.saturating_sub(filled)), theme::BORDER),
            text("]", theme::BORDER),
        ],
        right,
    );

    frame.render_widget(Paragraph::new(line), area);
}

fn stages(frame: &mut Frame, area: Rect, app: &App) {
    let w = area.width.min(STEP_W);

    let lines: Vec<Line> = app
        .stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let last = i + 1 == app.stages.len();
            let failed = app.state == State::BuildFailed && last;

            let (glyph, color) = if failed {
                ("✖", theme::ROSE)
            } else if stage.done {
                ("✔", theme::EMERALD)
            } else {
                (app.spinner(), theme::AMBER)
            };

            spread(
                w,
                vec![
                    strong(glyph, color),
                    Span::raw(" "),
                    text(stage.label, theme::TEXT),
                ],
                vec![text(stage.duration, theme::MUTED)],
            )
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

/// Degradation step 5, and the most consequential one.
///
/// State-dependent rather than size-dependent: once the build has succeeded
/// every row here is static, so holding eight rows while the log window is
/// starved cannot be justified.
fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let (glyph, color, label) = match app.state {
        State::BuildFailed => ("✖", theme::ROSE, "build failed"),
        s if s.build_done() => ("✔", theme::EMERALD, "build finished"),
        _ => (app.spinner(), theme::AMBER, "building"),
    };

    let line = spread(
        area.width,
        vec![
            strong(glyph, color),
            Span::raw(" "),
            text(label, theme::TEXT),
            Span::raw("  "),
            text(format!("{} stages", app.stages.len()), theme::MUTED),
        ],
        vec![
            text("build ", theme::MUTED),
            strong(app.build_time, theme::TEXT),
            text("  sync ", theme::MUTED),
            strong(app.sync_time, theme::TEXT),
        ],
    );

    frame.render_widget(Paragraph::new(line), area);
}

/// State 7 detail: the compiler output, with a code frame.
pub fn render_failure(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(failure) = &app.failure else {
        return;
    };

    let block = alert_card("COMPILER ERROR", theme::ROSE)
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                text("Exit code ", theme::MUTED),
                strong(app.exit_code.to_string(), theme::ROSE),
                Span::raw(" "),
            ])
            .right_aligned(),
        )
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(strong(failure.summary, theme::ROSE)),
        // The position, split out from the summary so it is machine-shaped
        // rather than prose: this is what gets read to build the code frame,
        // and what an editor jump would consume.
        Line::from(vec![
            text(failure.file, theme::MUTED),
            text(":", theme::BORDER),
            strong(failure.line.to_string(), theme::TEXT),
            text(":", theme::BORDER),
            strong(failure.column.to_string(), theme::TEXT),
        ]),
        Line::default(),
    ];

    // Dart emits the offending line and a caret itself, so that much is free
    // passthrough. The lines either side come from reading the file at the
    // reported position, which is usually the difference between recognising
    // the mistake and opening the editor.
    for (number, source) in failure.context {
        let hot = *number == failure.line;

        lines.push(Line::from(vec![
            text(
                format!("{number:>3} "),
                if hot { theme::ROSE } else { theme::MUTED },
            ),
            if hot {
                strong(*source, theme::TEXT)
            } else {
                text(*source, theme::MUTED)
            },
        ]));
    }

    lines.push(Line::from(vec![
        Span::raw(" ".repeat(4 + failure.caret_pad)),
        strong(failure.caret, theme::ROSE),
        Span::raw(" "),
        text(failure.caret_note, theme::ROSE),
    ]));

    lines.push(Line::default());
    lines.push(Line::from(text(failure.tail, theme::MUTED)));
    lines.push(Line::default());

    // Retry is a pty restart, not a keypress forwarded to Flutter: kill the
    // child, reap it, respawn with stage state reset. `r` is free here because
    // there is no live session to hot reload.
    let mut actions = keycap("r", theme::ROSE);
    actions.push(Span::raw(" "));
    actions.push(strong("Retry Build", theme::TEXT));
    actions.push(Span::raw("    "));
    actions.extend(keycap("q", theme::MUTED));
    actions.push(Span::raw(" "));
    actions.push(text("Quit", theme::MUTED));

    let action_row = Rect {
        x: inner.x,
        y: inner.y + lines.len() as u16,
        width: inner.width,
        height: 1,
    };

    lines.push(Line::from(actions));

    if action_row.y < inner.y + inner.height {
        app.hits.push(Hit {
            area: Rect {
                width: 20,
                ..action_row
            },
            action: Action::RetryBuild,
        });
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
