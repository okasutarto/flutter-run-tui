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
///
/// `plan` is `None` when the degradation ladder never ran, which is the expanded
/// log view: it bypasses the `Budget` because there is no ladder to solve when one
/// block owns the frame. Reporting a plan there described concessions that were
/// never made — `[separators, dense devices, build collapsed, cards collapsed]`
/// about cards that are not on screen — and it cost 56 columns of the one row that
/// must never truncate, which collapsed the spacing between the keys.
pub fn footer(frame: &mut Frame, area: Rect, app: &mut App, plan: Option<&Budget>) {
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

        // Same three keys, two different words. `⏎` here replaces a run rather
        // than starting the first one, and `Esc` goes back to a live session
        // instead of cancelling out with 130. One key meaning two things has to
        // say which one it means.
        State::Switching => vec![
            ("↑↓", "Move", theme::CYAN, None),
            ("⏎", "Switch", theme::EMERALD, None),
            ("Esc", "Back", theme::ROSE, None),
        ],

        // No scroll and no expand: there is no log card during a build, so both
        // keys would be advertised while doing nothing.
        State::SingleDevice | State::Building => vec![("^C", "Stop", theme::ROSE, None)],

        State::BuildFailed => vec![
            hint(Action::RetryBuild, theme::ROSE),
            ("q", "Quit", theme::MUTED, None),
            ("^C", "Stop", theme::ROSE, None),
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
                ("^C", "Stop", theme::ROSE, None),
            ]
        }
    };

    // Rendered before anything is measured, so the width the layout reserves for
    // the keys is the width the keys actually occupy.
    //
    // This used to be estimated as `key.len() + label.len() + 4`, which is bytes
    // and not columns: `↑↓` is six bytes and two cells, so the estimate ran four
    // columns over on every state that shows the arrows and dropped diagnostics
    // that would have fitted.
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
    // than as a glitch. Only when a layout decision was actually taken: see the
    // note on `plan` above.
    if let Some(plan) = plan {
        if plan.describe() != "full" {
            optional.push(vec![text(format!("[{}]", plan.describe()), theme::BORDER)]);
        }
    }

    // Only when capture is on, which is not the default. Printing `mouse off`
    // while off is the default spends a permanent slot on a non-event.
    if app.mouse_on {
        optional.push(vec![strong("mouse on", theme::EMERALD)]);
    }

    // Highest priority first, stopping at the first group that will not fit.
    // `continue` here would be a bin-packing search, keeping a low-priority group
    // merely because it was smaller than the important one just rejected.
    let mut kept: Vec<Vec<Span>> = Vec::new();
    let mut used = 0usize;

    for group in optional.into_iter().rev() {
        let group_w: usize = group.iter().map(Span::width).sum();

        // Four columns held back: the two the Layout spaces the groups apart by,
        // and two so the last key does not butt against the diagnostics.
        if content + used + group_w + 4 > area.width as usize {
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
        //
        // Only when there are diagnostics to keep clear of. The gap was
        // unconditional, so a row with nothing on its right still gave up two
        // columns to separate the keys from an empty group, and the last key
        // stopped two short of the edge on every state that reports nothing.
        Layout::horizontal([Constraint::Min(10), Constraint::Length(right_w as u16)])
            .spacing(if right.is_empty() { 0 } else { 2 })
            .split(area);

    // True space-between: each hint keeps its natural width and the leftover is
    // split into equal gaps between them.
    //
    // Equal-ratio columns were the first attempt and they clip: seven hints
    // across ninety columns gives thirteen each, and `[r] Hot reload` needs
    // fourteen. Forcing equal widths on unequal content truncates the longest,
    // which on a cheatsheet is the worst thing it can do.
    let gaps = hints.len().saturating_sub(1);
    let slack = (cols[0].width as usize).saturating_sub(content);

    // The remainder is handed out one column at a time to the leftmost gaps
    // rather than discarded. `slack / gaps` alone left up to `gaps - 1` columns
    // unused against the right edge — five of them at 106 columns with seven
    // hints — so the last key stopped short and the row read as left-aligned with
    // a ragged tail instead of spaced across the width.
    let gap = slack.checked_div(gaps).unwrap_or(0);
    let mut extra = slack.checked_rem(gaps).unwrap_or(0);

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

        if extra > 0 {
            x += 1;
            extra -= 1;
        }
    }

    if !right.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(right).right_aligned()), cols[1]);
    }
}
