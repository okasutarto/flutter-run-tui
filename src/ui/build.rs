//! BuildPhaseTracker, DESIGN.md 3.4. States 6 and 7.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Hit, State};
use crate::theme;
use crate::widgets::{alert_card, card, keycap, spread, strong, text};

/// Widest the filled bar itself may grow.
///
/// This applies to the bar graphic only, not to the row it sits on. A stage row
/// spans the card so its duration right-aligns to the border, but a progress bar
/// 118 columns long conveys nothing a 44-column one does not, and it turns a
/// glance into a scan.
///
/// The previous version capped the whole row at 66 columns, which is what left
/// this card ignoring the terminal: the border reached the window edge while
/// every duration stopped at column 66.
const BAR_MAX: u16 = 44;

/// How long a stage runs before its row starts showing a clock.
///
/// The same three seconds `frun-runner` used. Short enough to catch a stall,
/// long enough that a stage which finishes normally never shows a number.
const ELAPSED_AFTER: std::time::Duration = std::time::Duration::from_secs(3);

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
            // Live while building, final once it is not. A build time that only
            // appears at the end tells you nothing during the wait that matters.
            strong(app.build_clock(), theme::TEXT),
            text("   Sync ", theme::MUTED),
            strong(app.sync_time.clone(), theme::TEXT),
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

/// Filled bar with a step counter, against the stage count the platform implies.
///
/// The denominator used to be `app.stages.len()`, which is the number of stages
/// *announced so far*. That runs the bar backwards: at `Flutter started` it is 1
/// of 1 and shows full, and when Gradle appears it becomes 1 of 2 and drops to
/// half.
///
/// The fix is that the total was knowable all along. The platform is chosen
/// before the build starts and 3.4's trigger table is per-platform, so
/// `Platform::stage_count` is the answer. It is an upper bound, never a floor,
/// which is the direction that keeps the bar honest: a skipped stage leaves it
/// short of full until completion fills it, rather than reaching 100% early and
/// carrying on.
fn progress(frame: &mut Frame, area: Rect, app: &App) {
    let done = app.stages.iter().filter(|s| s.done).count();
    let total = app.expected_stages();
    let finished = app.state.build_done();

    // The bar is bounded; the row it sits on is not.
    let bar_w = area.width.saturating_sub(24).min(BAR_MAX) as usize;

    let filled = if finished {
        bar_w
    } else {
        bar_w * done / total.max(1)
    };

    let right = if finished {
        vec![
            // The count that actually ran, not the estimate.
            strong(format!("{} stages", app.stages.len()), theme::EMERALD),
            text("  complete", theme::MUTED),
        ]
    } else {
        vec![
            text("Stage ", theme::MUTED),
            // Clamped, because the estimate is an upper bound and a skipped
            // stage must not produce `Stage 6/5`.
            strong(format!("{}", (done + 1).min(total)), theme::AMBER),
            text(format!("/{total}"), theme::MUTED),
        ]
    };

    let line = spread(
        area.width,
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
    // Full width, so durations right-align to the card border.
    let w = area.width;

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

            // A clock on the stage still running, once it has been running long
            // enough to wonder about. `frun-runner` had this and the reason it
            // gave still holds: the spinner cycles the same frames whether the
            // stage is working or wedged, and the elapsed time is the difference.
            //
            // Withheld for the first few seconds so a normal fast stage does not
            // flash a number on its way past.
            let right = if stage.done || failed {
                stage.duration.clone()
            } else if stage.started.elapsed() >= ELAPSED_AFTER {
                crate::flutter::clock(stage.started.elapsed())
            } else {
                String::new()
            };

            spread(
                w,
                vec![
                    strong(glyph, color),
                    Span::raw(" "),
                    text(stage.label.as_str(), theme::TEXT),
                ],
                vec![text(right, theme::MUTED)],
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
            strong(app.build_clock(), theme::TEXT),
            text("  sync ", theme::MUTED),
            strong(app.sync_time.clone(), theme::TEXT),
        ],
    );

    frame.render_widget(Paragraph::new(line), area);
}

/// State 7 detail: the compiler output, with a code frame when there is one.
pub fn render_failure(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(failure) = &app.failure else {
        return;
    };

    // "COMPILER ERROR" only when it is one. A Gradle dependency failure, a
    // signing error or a missing toolchain names no source position, and calling
    // those a compiler error sends you looking in the wrong place.
    let title = if failure.location.is_some() {
        "COMPILER ERROR"
    } else {
        "BUILD ERROR"
    };

    let block = alert_card(title, theme::ROSE)
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

    let mut lines = Vec::new();

    for chunk in crate::widgets::wrap(&failure.summary, inner.width as usize) {
        lines.push(Line::from(strong(chunk, theme::ROSE)));
    }

    // The position, split out from the summary so it is machine-shaped rather
    // than prose: this is what was read to build the code frame, and what an
    // editor jump would consume.
    if let Some((file, line, column)) = &failure.location {
        lines.push(Line::from(vec![
            text(file.as_str(), theme::MUTED),
            text(":", theme::BORDER),
            strong(line.to_string(), theme::TEXT),
            text(":", theme::BORDER),
            strong(column.to_string(), theme::TEXT),
        ]));
    }

    lines.push(Line::default());

    // The lines either side of the reported position, read from the file. One
    // line of context is usually the difference between recognising the mistake
    // and opening the editor.
    let hot_line = failure.location.as_ref().map(|(_, line, _)| *line);

    for (number, source) in &failure.context {
        let hot = Some(*number) == hot_line;

        lines.push(Line::from(vec![
            text(
                format!("{number:>4} "),
                if hot { theme::ROSE } else { theme::MUTED },
            ),
            if hot {
                strong(source.as_str(), theme::TEXT)
            } else {
                text(source.as_str(), theme::MUTED)
            },
        ]));
    }

    if !failure.context.is_empty() && failure.caret_col > 0 {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(5 + failure.caret_col.saturating_sub(1) as usize)),
            strong("^", theme::ROSE),
        ]));
    }

    // No code frame: the tail of what the build printed takes its place. Showing
    // nothing here would leave the one screen whose whole job is explaining the
    // failure with nothing but an exit code.
    if failure.context.is_empty() {
        for out in &failure.output {
            for chunk in crate::widgets::wrap(out, inner.width as usize) {
                lines.push(Line::from(text(chunk, theme::MUTED)));
            }
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(text(failure.note.as_str(), theme::AMBER)));

    lines.push(Line::default());

    // The action row follows the message rather than being pinned to the bottom
    // of the card. Pinning it put the card's slack in the middle, between the
    // verdict and the only thing that acts on it, which reads as a rendering
    // fault; below the action row the same slack reads as room.
    //
    // Truncated to leave it a row: whatever is cut is the oldest build output,
    // and a Retry the layout swallowed is worse than a line of Gradle noise.
    let action_row = Rect {
        y: inner.y + lines.len().min(inner.height.saturating_sub(1) as usize) as u16,
        height: 1,
        ..inner
    };

    lines.truncate(inner.height.saturating_sub(1) as usize);

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

    app.hits.push(Hit {
        area: Rect {
            width: 20,
            ..action_row
        },
        action: Action::RetryBuild,
    });

    frame.render_widget(Paragraph::new(lines), inner);
    frame.render_widget(Paragraph::new(Line::from(actions)), action_row);
}
