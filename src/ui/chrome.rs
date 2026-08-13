//! TerminalFooter (3.7).
//!
//! The InteractivePrompt that used to live here is gone. It cost three rows and a
//! gap, and it had nothing to command: Flutter's interactive session reads single
//! keypresses, so a typed line could not be forwarded to it — sending `quit`
//! would have sent `q` and quit on the first character. Every key it might have
//! carried is on the footer row below, which costs one row and never scrolls.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Hit, State};
use crate::theme;
use crate::widgets::{keycap, strong, text};

/// Hotkey cheatsheet. One row, always last, never scrolls.
///
/// No status bar and no grid metrics: session time, mode indicator and grid
/// dimensions changed no decision the user was about to make, and the row is
/// better spent on the keys that do.
///
/// Contents follow the active state, because advertising a key that does
/// nothing here is worse than not advertising it.
pub fn footer(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    // Hints as groups, so they can be spaced across the row rather than run
    // together at the left edge. A group carries its Action when frun owns the
    // key, which is what earns it a clickable region.
    let expand = if app.expanded { "Collapse" } else { "Expand" };

    // A hint is either an Action frun owns, whose key and label come from the
    // Action itself so the cheatsheet cannot disagree with what the key does, or a
    // literal for the keys forwarded to Flutter and the ones the terminal owns.
    let hint = |action: Action, color| (action.key(), action.label(), color, Some(action));

    let hints: Vec<(&str, &str, ratatui::style::Color, Option<Action>)> = match app.state {
        State::Detecting | State::Booting => vec![("^C", "Cancel", theme::ROSE, None)],

        State::NoDevices | State::MultipleDevices => vec![
            ("↑↓", "Move", theme::CYAN, None),
            ("⏎", "Launch", theme::EMERALD, None),
            ("Esc", "Cancel", theme::ROSE, None),
        ],

        // No scroll and no expand: there is no log card during a build, so both
        // keys would be advertised while doing nothing.
        State::SingleDevice | State::Building => vec![("^C", "Force stop", theme::ROSE, None)],

        State::BuildFailed => vec![
            hint(Action::RetryBuild, theme::ROSE),
            ("q", "Quit", theme::MUTED, None),
            ("^C", "Force stop", theme::ROSE, None),
        ],

        State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped => {
            vec![
                hint(Action::Reload, theme::AMBER),
                hint(Action::Restart, theme::PURPLE),
                ("↑↓", "Scroll", theme::CYAN, None),
                ("e", expand, theme::CYAN, None),
                // Forwarded to Flutter, so no hit region: there is nothing for
                // frun to click on its own behalf.
                ("h", "Help", theme::CYAN, None),
                ("q", "Quit", theme::ROSE, None),
                ("^C", "Force stop", theme::ROSE, None),
            ]
        }
    };

    // Diagnostics keep their slot at the right edge, and the hints are spaced
    // across whatever is left. Ordered lowest priority first; the tail survives a
    // narrow window.
    let (index, total) = app.position();
    let mut optional: Vec<Vec<Span>> = Vec::new();

    // Prototype-only, and only while the data is mock. DESIGN.md removed the
    // frame switcher because state is decided by what Flutter is doing;
    // advertising `⇥ next` during a real run would advertise a key that now
    // belongs to Flutter.
    if !app.live {
        optional.push(vec![
            text(
                format!("proto {index}/{total} {}", app.state.slug()),
                theme::BORDER,
            ),
            text("  ⇥ next", theme::BORDER),
        ]);
    }

    // What the layout gave up, so a missing element reads as a decision rather
    // than as a glitch.
    if plan.describe() != "full" {
        optional.push(vec![text(format!("[{}]", plan.describe()), theme::BORDER)]);
    }

    // Only when capture is on, which is not the default. Printing `mouse off`
    // while off is the default spends a permanent slot on a non-event.
    if app.mouse_on {
        optional.push(vec![strong("mouse on", theme::EMERALD)]);
    }

    let hint_w: usize = hints.iter().map(|(k, l, _, _)| k.len() + l.len() + 4).sum();

    // Highest priority first, stopping at the first group that will not fit.
    // `continue` here would be a bin-packing search, keeping a low-priority group
    // merely because it was smaller than the important one just rejected.
    let mut kept: Vec<Vec<Span>> = Vec::new();
    let mut used = 0usize;

    for group in optional.into_iter().rev() {
        let group_w: usize = group.iter().map(Span::width).sum();

        if hint_w + used + group_w + 4 > area.width as usize {
            break;
        }

        used += group_w + 2;
        kept.push(group);
    }

    let mut right: Vec<Span> = Vec::new();

    for (i, group) in kept.into_iter().rev().enumerate() {
        if i > 0 {
            right.push(text("  ", theme::MUTED));
        }

        right.extend(group);
    }

    let right_w: usize = right.iter().map(Span::width).sum();

    let cols =
        // Spaced, so the last hint cannot butt against the diagnostics:
        // `[Esc] Cancelproto 4/11` was the result of them being adjacent.
        Layout::horizontal([Constraint::Min(10), Constraint::Length(right_w as u16)])
            .spacing(2)
            .split(area);

    // True space-between: each hint keeps its natural width and the leftover is
    // split into equal gaps between them.
    //
    // Equal-ratio columns were the first attempt and they clip: seven hints
    // across ninety columns gives thirteen each, and `[r] Hot reload` needs
    // fourteen. Forcing equal widths on unequal content truncates the longest,
    // which on a cheatsheet is the worst thing it can do.
    let rendered: Vec<Vec<Span>> = hints
        .iter()
        .map(|(key, label, color, _)| {
            let mut spans = keycap(key, *color);
            spans.push(text(" ", theme::MUTED));
            spans.push(text(*label, theme::MUTED));
            spans
        })
        .collect();

    let natural: Vec<usize> = rendered
        .iter()
        .map(|s| s.iter().map(Span::width).sum())
        .collect();

    let content: usize = natural.iter().sum();
    let gaps = hints.len().saturating_sub(1).max(1);
    let slack = (cols[0].width as usize).saturating_sub(content);
    let gap = slack / gaps;

    let mut x = cols[0].x;

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

        x += (*width + gap) as u16;
    }

    if !right.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(right).right_aligned()), cols[1]);
    }
}
