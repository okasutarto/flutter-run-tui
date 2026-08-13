//! InteractivePrompt (3.6) and TerminalFooter (3.7).

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Hit, State};
use crate::theme;
use crate::widgets::{keycap, spread, strong, text};

/// Modal command line.
///
/// In NORMAL mode every unbound key is forwarded to Flutter, which has its own
/// interactive commands (`h`, `d`, `c`, `p`, `o`, `w`). A prompt that captured
/// keys at all times would silently remove functionality that works today, so
/// `:` opens this and takes the keyboard back.
pub fn prompt(frame: &mut Frame, area: Rect, app: &App) {
    let border = if app.command_mode {
        theme::CYAN
    } else {
        theme::BORDER
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let line = if app.command_mode {
        Line::from(vec![
            strong("➜ ", theme::EMERALD),
            text(app.cwd, theme::CYAN),
            Span::raw(" "),
            strong("❯ ", theme::CYAN),
            text(app.command_input.clone(), theme::TEXT),
            strong("█", theme::CYAN),
        ])
    } else {
        Line::from(vec![
            strong("➜ ", theme::EMERALD),
            text(app.cwd, theme::CYAN),
            Span::raw(" "),
            strong("❯ ", theme::MUTED),
            text("press : to type a command", theme::MUTED),
        ])
    };

    frame.render_widget(Paragraph::new(line), inner);
}

/// Hotkey cheatsheet. One row, always last, never scrolls.
///
/// No status bar and no grid metrics: session time, mode indicator and grid
/// dimensions changed no decision the user was about to make, and the row is
/// better spent on the keys that do.
///
/// Contents follow the active state, because advertising a key that does
/// nothing here is worse than not advertising it.
/// `[key] Label`, derived from the action rather than spelled out again.
///
/// Keeps the cheatsheet from disagreeing with what the key actually does, which
/// is the whole reason `Action` owns both strings.
fn action_hint(action: Action, color: ratatui::style::Color) -> Vec<Span<'static>> {
    let mut spans = keycap(action.key(), color);
    spans.push(text(" ", theme::MUTED));
    spans.push(text(action.label(), theme::MUTED));
    spans.push(text("  ", theme::MUTED));
    spans
}

pub fn footer(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    let mut left: Vec<Span> = Vec::new();

    match app.state {
        State::Detecting | State::Booting => {
            left.extend(keycap("^C", theme::ROSE));
            left.push(text(" Cancel", theme::MUTED));
        }

        State::NoDevices | State::MultipleDevices => {
            left.extend(keycap("↑↓", theme::CYAN));
            left.push(text(" Move  ", theme::MUTED));
            left.extend(keycap("⏎", theme::EMERALD));
            left.push(text(" Launch  ", theme::MUTED));
            left.extend(keycap("Esc", theme::ROSE));
            left.push(text(" Cancel", theme::MUTED));
        }

        State::SingleDevice | State::Building => {
            left.extend(keycap("^C", theme::ROSE));
            left.push(text(" Stop", theme::MUTED));
        }

        State::BuildFailed => {
            left.extend(keycap("r", theme::ROSE));
            left.push(text(" Retry Build  ", theme::MUTED));
            left.extend(keycap("q", theme::MUTED));
            left.push(text(" Quit", theme::MUTED));
        }

        State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped => {
            // Hit rectangles are measured from the spans as they are built,
            // not guessed. Hardcoding widths here meant the clickable region
            // and the drawn label were two independent numbers that happened
            // to agree.
            let mut x = area.x;

            for (action, color) in [
                (Action::Reload, theme::AMBER),
                (Action::Restart, theme::PURPLE),
            ] {
                let spans = action_hint(action, color);
                let w: usize = spans.iter().map(Span::width).sum();

                app.hits.push(Hit {
                    area: Rect {
                        x,
                        y: area.y,
                        width: w as u16,
                        height: 1,
                    },
                    action,
                });

                x += w as u16;
                left.extend(spans);
            }

            // Forwarded to Flutter rather than handled here, so they get no hit
            // region: there is nothing for frun to click on its own behalf.
            left.extend(keycap("h", theme::CYAN));
            left.push(text(" Help  ", theme::MUTED));
            left.extend(keycap("c", theme::CYAN));
            left.push(text(" Clear  ", theme::MUTED));

            left.extend(action_hint(Action::Quit, theme::ROSE));
        }
    }

    // The right side is optional, in priority order, and gets dropped from the
    // bottom up until it fits.
    //
    // `spread` only clips when it overflows, which on the running state meant
    // the tail of the footer silently disappeared. A cheatsheet that truncates
    // is worse than a shorter one, because you cannot tell whether the key you
    // are looking for is missing or just cut off.
    let (index, total) = app.position();

    // Ordered lowest priority first; the tail is what survives a narrow window.
    //
    // `Flutter <version> CLI` used to sit here and is now gone entirely. The
    // version is already on the ProjectCard, it changes no decision taken from
    // the footer, and it was costing 18 columns on the one row that must never
    // truncate.
    let mut optional: Vec<Vec<Span>> = Vec::new();

    // Prototype-only affordance, styled as chrome rather than content.
    // DESIGN.md removed the frame switcher because state is decided by what
    // Flutter is doing, so this must not read as a feature; it exists only
    // because a static frame that never advances looks like a hang.
    optional.push(vec![
        text(
            format!("proto {index}/{total} {}", app.state.slug()),
            theme::BORDER,
        ),
        text("  ⇥ next", theme::BORDER),
    ]);

    // What the layout gave up, so a missing element reads as a decision rather
    // than as a glitch.
    if plan.describe() != "full" {
        optional.push(vec![text(format!("[{}]", plan.describe()), theme::BORDER)]);
    }

    // Highest priority: only shown when the mouse is captured, which is not the
    // default. Printing `mouse off` while off is the default spends a permanent
    // slot on a non-event; capture is worth announcing because it takes text
    // selection away from the terminal, and the only other symptom is that
    // copying a stack trace quietly stops working.
    if app.mouse_on {
        optional.push(vec![strong("mouse on", theme::EMERALD)]);
    }

    let left_w: usize = left.iter().map(Span::width).sum();

    // Highest priority first, and stop at the first group that will not fit.
    //
    // `continue` here would be a bin-packing search: it kept a low-priority
    // group merely because it was smaller than the important one that had just
    // been rejected. If the thing that matters more cannot be shown, showing
    // something else in its place is not a saving.
    let mut kept: Vec<Vec<Span>> = Vec::new();
    let mut used = 0usize;

    for group in optional.into_iter().rev() {
        let group_w: usize = group.iter().map(Span::width).sum();
        let gap = if kept.is_empty() { 4 } else { 2 };

        if left_w + used + group_w + gap > area.width as usize {
            break;
        }

        used += group_w + gap;
        kept.push(group);
    }

    // Reversed for display so the highest priority ends up nearest the right
    // edge, where the eye lands first on a right-aligned group.
    let mut right: Vec<Span> = Vec::new();

    for (i, group) in kept.into_iter().rev().enumerate() {
        if i > 0 {
            right.push(text("  ", theme::MUTED));
        }

        right.extend(group);
    }

    frame.render_widget(Paragraph::new(spread(area.width, left, right)), area);
}
