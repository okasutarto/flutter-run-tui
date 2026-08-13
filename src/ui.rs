//! Rendering. Two screens, one palette.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Padding, Paragraph};
use ratatui::Frame;

use crate::data::{Action, App, Hit, Level, Phase};
use crate::theme;
use crate::widgets::{card, elide, field, keycap, pill, spread, strong, text};

/// Rows the dashboard needs before the log window gets anything.
///
/// Reported by `--rows` so the cost of the layout is a number we can
/// argue about rather than a feeling.
pub const DASHBOARD_CHROME: u16 = 9 + 9 + 7 + 3 + 1 + 5;

/// Widest a label/value pair is allowed to span.
///
/// Right-aligning a value against the full card width only works while the
/// card is narrow. Past roughly this point the gap stops reading as
/// alignment and starts reading as two unrelated columns.
const METADATA_W: u16 = 44;

/// Same ceiling for the build step rows, which carry a label on the left
/// and a duration on the right.
const STEP_W: u16 = 62;

/// Widest the dashboard is allowed to grow.
///
/// A terminal has no max-width container, so on a wide window every card
/// stretches to the edge and the cards become long thin strips with their
/// contents pinned to opposite ends. Capping the shell is the same fix as
/// a max-width wrapper on the web.
///
/// The streaming view is deliberately exempt: log lines want every column
/// they can get, and they carry the app's own text, which must not be
/// reflowed to suit a layout.
///
/// 100 is the width DESIGN.md was drawn at, and it is what makes the two
/// side-by-side cards land near 48 columns each — narrow enough that a
/// right-aligned value still reads as belonging to its label, so those
/// cards need no cap of their own.
const DASHBOARD_MAX_W: u16 = 100;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Rebuilt every frame. A stale hit list is worse than none: it would
    // leave invisible buttons behind at the coordinates a card used to
    // occupy before the layout degraded.
    app.hits.clear();

    if area.height < 12 || area.width < 60 {
        render_too_small(frame, area);
        return;
    }

    match app.phase {
        Phase::Dashboard => render_dashboard(frame, area, app),
        Phase::Streaming => render_streaming(frame, area, app),
    }
}

fn render_too_small(frame: &mut Frame, area: Rect) {
    let msg = Paragraph::new(vec![
        Line::from(strong("terminal too small", theme::ROSE)),
        Line::from(text("needs at least 60x12", theme::MUTED)),
    ])
    .centered();

    frame.render_widget(msg, area);
}

// ============================================================
// Dashboard
// ============================================================
// Header bar removed per review: the active-device tag it carried is
// already the entire subject of the Selected Target card, and the session
// clock moved to the footer where the other volatile numbers live.

/// Shortest window that can still show a dashboard with a usable log area.
///
/// Below this, `Layout` has no choice but to shrink the fixed-height cards,
/// and every card collapses to just its top border. That is worse than any
/// fallback, because it looks like the app is broken rather than cramped.
const DASHBOARD_MIN_H: u16 = 29;

fn render_dashboard(frame: &mut Frame, area: Rect, app: &mut App) {
    let area = Rect {
        width: area.width.min(DASHBOARD_MAX_W),
        ..area
    };

    // Too short for the dashboard at all. The streaming view is already
    // designed for this exact situation, minimum chrome and everything else
    // given to the logs, so fall through to it rather than inventing a
    // third layout.
    if area.height < DASHBOARD_MIN_H {
        render_streaming(frame, area, app);
        return;
    }

    // Graded degradation, cheapest thing first.
    //
    //   logo    costs 4 rows and carries no information: you already know
    //           this is Flutter
    //   prompt  costs 4 rows including its gap, and every command it offers
    //           has a key binding shown in the footer
    let logo = area.height >= 40;
    let prompt = area.height >= 33;

    let mut constraints = vec![
        Constraint::Length(if logo { 9 } else { 5 }),
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Min(3),
    ];

    if prompt {
        constraints.push(Constraint::Length(3));
    }

    constraints.push(Constraint::Length(1));

    let chunks = Layout::vertical(constraints).spacing(1).split(area);

    project_card(frame, chunks[0], app, logo);

    let cols = Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
        .spacing(1)
        .split(chunks[1]);

    target_card(frame, cols[0], app);
    controls_card(frame, cols[1], app);

    build_card(frame, chunks[2], app);
    logs_card(frame, chunks[3], app);

    if prompt {
        prompt_bar(frame, chunks[4]);
    }

    // Footer is always last, whatever got dropped above it.
    footer(frame, chunks[chunks.len() - 1], app);
}

fn project_card(frame: &mut Frame, area: Rect, app: &App, roomy: bool) {
    // Padding belongs on the Block, not on a second Block inside the
    // Paragraph: `inner()` already subtracts it, so applying it twice
    // makes every line one column too wide and silently clips the values
    // at the right border.
    let block = card("project", theme::VIOLET)
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                text("~/cwclub", theme::MUTED),
                Span::raw(" "),
            ])
            .right_aligned(),
        )
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The logo sits in a reserved Rect. In the real build this is where
    // ratatui-image renders assets/flutter.png through the kitty graphics
    // protocol, with a halfblocks fallback when the terminal cannot do it.
    let body = if roomy {
        let split = Layout::horizontal([Constraint::Length(24), Constraint::Min(30)]).split(inner);

        logo(frame, split[0]);
        split[1]
    } else {
        inner
    };

    let git = if app.git_clean {
        vec![text("● clean", theme::EMERALD)]
    } else {
        vec![text("● 3 changed", theme::AMBER)]
    };

    // Bound before the lines so the pill can borrow it: the spans hold a
    // reference to this string until the Paragraph is rendered.
    //
    // Elided, because a real branch name is not the tidy one in the design.
    // `feature/PROJ-4821-refactor-checkout-payment-sheet` is 48 characters
    // and ran the pill straight past the card, taking its closing cap with
    // it. Budget: the field width minus the label and the pill's own caps
    // and padding.
    let branch = format!(" {} ", elide(app.branch, METADATA_W as usize - 14));

    if roomy {
        // Cap the label/value span.
        //
        // `spread` pushes the value to the far edge of whatever width it
        // is given, and after the logo column this card still had ~74
        // columns. The result was "project" on the far left and "cwclub"
        // on the far right with 60 columns of nothing between them, so the
        // pair no longer read as belonging to each other.
        let w = body.width.min(METADATA_W);

        let lines = vec![
            field(w, "project", vec![strong(app.project, theme::TEXT)]),
            field(w, "version", vec![text(app.version, theme::TEXT)]),
            field(w, "branch", pill(&branch, theme::AMBER)),
            field(w, "git", git),
            Line::raw(""),
            Line::from(vec![
                text("flutter ", theme::MUTED),
                text(app.flutter, theme::TEXT),
                text("  ·  dart ", theme::MUTED),
                text(app.dart, theme::TEXT),
                text("  ·  fvm", theme::MUTED),
            ]),
        ];

        frame.render_widget(Paragraph::new(lines), body);
        return;
    }

    // Compact: two columns rather than a shorter list.
    //
    // Truncating the field list would drop git status and the toolchain
    // versions silently, which is the worst kind of responsive behaviour:
    // the layout still looks deliberate, so you have no way to tell that
    // information is missing. Reflowing keeps all six facts.
    let cols = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
        .spacing(2)
        .split(body);

    let (lw, rw) = (cols[0].width, cols[1].width);

    frame.render_widget(
        Paragraph::new(vec![
            field(lw, "project", vec![strong(app.project, theme::TEXT)]),
            field(lw, "version", vec![text(app.version, theme::TEXT)]),
            field(lw, "branch", pill(&branch, theme::AMBER)),
        ]),
        cols[0],
    );

    frame.render_widget(
        Paragraph::new(vec![
            field(rw, "flutter", vec![text(app.flutter, theme::TEXT)]),
            field(rw, "dart", vec![text(app.dart, theme::TEXT)]),
            field(rw, "git", git),
        ]),
        cols[1],
    );
}

fn logo(frame: &mut Frame, area: Rect) {
    // Placeholder mark, not the Flutter logo.
    //
    // The previous attempt tried to draw the real logo in half-blocks and
    // came out as an unreadable blob, which is worse than not trying: it
    // looks like a rendering fault. A plain chevron at least reads as a
    // deliberate mark.
    //
    // The actual fix is ratatui-image pointed at assets/flutter-trim.png,
    // rendered through the kitty graphics protocol. That needs the `image`
    // crate and a terminal capability query, so it is a separate step.
    let art = vec![
        Line::from(text("      ▄▄██", theme::CYAN)),
        Line::from(text("    ▄▄██▀", theme::CYAN)),
        Line::from(text("  ▄▄██▀", theme::CYAN)),
        Line::from(text(" ▄██▀", theme::CYAN)),
        Line::from(text(" ▀██▄", theme::CYAN)),
        Line::from(text("   ▀▀██▄▄", theme::CYAN)),
        Line::from(text("       ▀▀██", theme::CYAN)),
    ];

    frame.render_widget(Paragraph::new(art), area);
}

fn target_card(frame: &mut Frame, area: Rect, app: &App) {
    let block = card("selected target", theme::VIOLET).padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // No cap: this card is already narrow enough that the value hugging
    // the right border is exactly the alignment the design asks for.
    let w = inner.width;

    let detected = format!(
        "{} device{} detected",
        app.device_count,
        if app.device_count == 1 { "" } else { "s" }
    );

    let lines = vec![
        Line::raw(""),
        field(w, "device", vec![strong(app.device, theme::TEXT)]),
        field(w, "platform", vec![text(app.platform, theme::TEXT)]),
        field(w, "id", vec![text(app.device_id, theme::MUTED)]),
        Line::raw(""),
        Line::from(vec![
            strong("✓ ", theme::EMERALD),
            text(&detected, theme::MUTED),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn controls_card(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = card("hot controls", theme::AMBER).padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // A 2x2 grid rather than hand-spaced spans.
    //
    // Laying the buttons out with `Layout` is what makes them clickable at
    // all: the same call that positions them yields the Rect each one
    // occupies, so hit testing needs no second set of coordinates that
    // could disagree with what was drawn.
    let rows = Layout::vertical([
        Constraint::Length(1), // top breathing room
        Constraint::Length(1), // reload / restart
        Constraint::Length(1),
        Constraint::Length(1), // quit / stop
        Constraint::Length(1),
        Constraint::Min(0), // hint
    ])
    .split(inner);

    let grid = [
        (
            rows[1],
            Action::Reload,
            theme::AMBER,
            Action::Restart,
            theme::VIOLET,
        ),
        (
            rows[3],
            Action::Quit,
            theme::ROSE,
            Action::Stop,
            theme::MUTED,
        ),
    ];

    for (row, left, left_color, right, right_color) in grid {
        let cols =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(row);

        button(frame, cols[0], app, left, left_color);
        button(frame, cols[1], app, right, right_color);
    }

    frame.render_widget(
        Paragraph::new(Line::from(text(
            ": command mode  ·  other keys go to flutter",
            theme::MUTED,
        ))),
        rows[5],
    );
}

/// One clickable control.
///
/// The whole row is the target, not just the keycap: a 3-cell click target
/// is unreasonable with a mouse, and the label is the part you are actually
/// aiming at.
fn button(frame: &mut Frame, area: Rect, app: &mut App, action: Action, color: Color) {
    let hovered = app.hover == Some(action);

    app.hits.push(Hit { area, action });

    // Hover is the terminal equivalent of the audit's "no hover states"
    // note: the cell background lifts, which is the only affordance a cell
    // grid can offer.
    let bg = if hovered {
        Style::new().bg(Color::Rgb(30, 30, 34))
    } else {
        Style::new()
    };

    let mut spans = vec![Span::raw(" ")];
    spans.extend(keycap(action.key(), color));
    spans.push(Span::raw("  "));

    spans.push(if hovered {
        strong(action.label(), color)
    } else {
        text(action.label(), theme::TEXT)
    });

    frame.render_widget(Paragraph::new(Line::from(spans)).style(bg), area);
}

fn build_card(frame: &mut Frame, area: Rect, app: &App) {
    let block = card("build phase", theme::CYAN)
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                text("total ", theme::MUTED),
                strong(app.total_build, theme::TEXT),
                Span::raw(" "),
            ])
            .right_aligned(),
        )
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Telemetry used to occupy the right half of this row. With it removed
    // the card went full width and the durations flew out to column 98,
    // which is why they looked detached from their labels.
    let w = inner.width.min(STEP_W);

    let lines: Vec<Line> = app
        .steps
        .iter()
        .map(|step| {
            let glyph = if step.done { "✓" } else { "⠋" };
            let color = if step.done {
                theme::EMERALD
            } else {
                theme::AMBER
            };

            spread(
                w,
                vec![
                    strong(glyph, color),
                    Span::raw(" "),
                    text(step.label, theme::TEXT),
                ],
                vec![text(step.duration, theme::MUTED)],
            )
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn logs_card(frame: &mut Frame, area: Rect, app: &mut App) {
    let count = format!("{} entries  ·  / to filter ", app.logs.len());

    let block = card("app logs", theme::CYAN)
        .title_top(Line::from(vec![Span::raw(" "), text(&count, theme::MUTED)]).right_aligned())
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Bottom-anchored here too. This card absorbs the leftover height, so
    // on a tall window nine log lines sat at the top of a box with a dozen
    // empty rows under them, and each new line arrived at the top of mostly
    // blank space. Same reasoning as the streaming view.
    log_stream(frame, inner, app);
}

/// Bottom-anchored log tail.
///
/// A `List` fills from the top, which is wrong for a stream: with 9 lines
/// in a 15-row area you get the logs at the top and dead space above the
/// status bar, so each new line appears far from where your eye already
/// is. Anchoring to the bottom means new output always arrives in the same
/// place, directly above the status row — the behaviour a terminal gives
/// you for free and that a full-screen app has to reimplement.
fn log_stream(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|log| ListItem::new(log_line(log)))
        .collect();

    let list = List::new(items)
        .highlight_style(Style::new().bg(Color::Rgb(26, 26, 30)))
        .highlight_symbol("");

    let mut state = ListState::default().with_selected(Some(app.selected_log));

    let count = app.logs.len() as u16;

    // Only shrink the area while the stream is short. Once it overflows,
    // the List's own scrolling has to own the full height.
    let target = if count < area.height {
        Rect {
            x: area.x,
            y: area.y + area.height - count,
            width: area.width,
            height: count,
        }
    } else {
        area
    };

    frame.render_stateful_widget(list, target, &mut state);
}

fn log_line(log: &crate::data::LogLine) -> Line<'_> {
    let color = match log.level {
        Level::Info => theme::CYAN,
        Level::Warn => theme::AMBER,
        Level::Error => theme::ROSE,
        Level::Build => theme::VIOLET,
    };

    let mut spans = vec![text(log.time, theme::MUTED), Span::raw("  ")];
    spans.extend(pill(log.level.badge(), color));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        format!("{:<10}", log.source),
        Style::new().fg(theme::MUTED),
    ));
    spans.push(text(log.message, theme::TEXT));

    Line::from(spans)
}

fn prompt_bar(frame: &mut Frame, area: Rect) {
    let block = ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::new().fg(theme::BORDER))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let line = Line::from(vec![
        strong("➜ ", theme::EMERALD),
        text("~/cwclub", theme::CYAN),
        Span::raw(" "),
        strong("❯ ", theme::VIOLET),
        text("press r / R, or : for a command", theme::MUTED),
    ]);

    frame.render_widget(Paragraph::new(line), inner);
}

fn footer(frame: &mut Frame, area: Rect, app: &App) {
    let mut left = pill(" NORMAL ", theme::VIOLET);
    left.push(Span::raw("  "));

    // Echo the last action so a click that landed is distinguishable from
    // one that missed. Without this, clicking a button that toggles a state
    // you cannot see reads as the mouse not working at all.
    match app.last_action {
        Some(label) => {
            left.push(text("last ", theme::MUTED));
            left.push(strong(label, theme::EMERALD));
        }
        None => {
            left.push(text("session ", theme::MUTED));
            left.push(text(app.session, theme::TEXT));
        }
    }

    let right = vec![
        text("mouse ", theme::MUTED),
        if app.mouse_on {
            strong("on", theme::EMERALD)
        } else {
            strong("off", theme::MUTED)
        },
        text("  m", theme::TEXT),
        text(" toggle  ", theme::MUTED),
        text("tab", theme::TEXT),
        text(" phase  ", theme::MUTED),
        text("q", theme::TEXT),
        text(" quit", theme::MUTED),
    ];

    frame.render_widget(Paragraph::new(spread(area.width, left, right)), area);
}

// ============================================================
// Streaming
// ============================================================
// Same palette, same components. The difference is what gets the rows.

fn render_streaming(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    // Everything the dashboard spent 20 rows on, in one line. None of it
    // changes during a run, so none of it needs more than this.
    let meta_left = vec![
        strong("frun", theme::VIOLET),
        Span::raw("  "),
        text(app.project, theme::MUTED),
        Span::raw(" "),
        text(app.version, theme::TEXT),
        Span::raw("  "),
        text(" ", theme::MUTED),
        text(app.branch, theme::AMBER),
        Span::raw("  "),
        text("flutter ", theme::MUTED),
        text(app.flutter, theme::TEXT),
    ];

    let meta_right = vec![
        text("built in ", theme::MUTED),
        strong(app.total_build, theme::TEXT),
    ];

    frame.render_widget(
        Paragraph::new(spread(area.width, meta_left, meta_right)),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(text(
            &"─".repeat(area.width as usize),
            theme::BORDER,
        ))),
        chunks[1],
    );

    log_stream(frame, chunks[2], app);

    let status = if app.reloading {
        Line::from(vec![
            strong(" ⠋ ", theme::AMBER),
            text("hot reloading", theme::TEXT),
            text("  ·  3s", theme::MUTED),
        ])
    } else {
        Line::from(vec![
            strong(" ● ", theme::EMERALD),
            text("running", theme::TEXT),
            text("  ·  waiting for changes", theme::MUTED),
        ])
    };

    frame.render_widget(Paragraph::new(status), chunks[3]);

    frame.render_widget(
        Paragraph::new(Line::from(text(
            &"─".repeat(area.width as usize),
            theme::BORDER,
        ))),
        chunks[4],
    );

    let mut left = pill(" NORMAL ", theme::VIOLET);
    left.push(Span::raw("  "));
    left.push(strong("● ", theme::EMERALD));
    left.push(text(app.device, theme::TEXT));
    left.push(text("  ·  ", theme::MUTED));
    left.push(text(app.session, theme::MUTED));

    let right = vec![
        text("r", theme::TEXT),
        text(" reload  ", theme::MUTED),
        text("R", theme::TEXT),
        text(" restart  ", theme::MUTED),
        text("/", theme::TEXT),
        text(" filter  ", theme::MUTED),
        text("q", theme::TEXT),
        text(" quit ", theme::MUTED),
    ];

    frame.render_widget(Paragraph::new(spread(area.width, left, right)), chunks[5]);
}
