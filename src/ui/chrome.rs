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

/// `[⇧⏎] Launch in new tab` (8.4), in the three states that show a list.
///
/// Only where the key can arrive. In the legacy encoding `⇧⏎` is plain CR, so the
/// hint would advertise a key indistinguishable from `⏎` — the `[COPY]` failure of
/// 3.1. `App::shift_enter` carries what the terminal answered.
///
/// It is a key, not a diagnostic, so it is never dropped: below the width where the
/// row fits with its words, `footer` drops every label and keeps every keycap.
fn new_tab<'a>(
    hints: &mut Vec<(&'a str, &'a str, ratatui::style::Color, Option<Action>)>,
    app: &App,
) {
    if !app.shift_enter {
        return;
    }

    // Cyan, not the emerald `⏎ Launch` wears. Both launch, but two adjacent emerald
    // keycaps read as one control that lost its spacing.
    hints.push((
        Action::NewTab.key(),
        Action::NewTab.label(),
        theme::CYAN,
        Some(Action::NewTab),
    ));
}

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

        State::MultipleDevices => {
            let mut hints = vec![
                ("↑↓", "Move", theme::CYAN, None),
                ("⏎", "Launch", theme::EMERALD, None),
            ];

            new_tab(&mut hints, app);
            hints.push(("Esc", "Cancel", theme::ROSE, None));

            hints
        }

        // Same three keys, two different words. `⏎` here replaces a run rather
        // than starting the first one, and `Esc` goes back to a live session
        // instead of cancelling out with 130. One key meaning two things has to
        // say which one it means.
        State::Switching => {
            let mut hints = vec![
                ("↑↓", "Move", theme::CYAN, None),
                ("⏎", "Switch", theme::EMERALD, None),
            ];

            new_tab(&mut hints, app);
            hints.push(("Esc", "Back", theme::ROSE, None));

            hints
        }

        // No scroll and no expand: there is no log card during a build, so both
        // keys would be advertised while doing nothing.
        State::SingleDevice => vec![hint(Action::Stop, theme::ROSE)],

        State::Building => vec![
            hint(Action::StopRun, theme::AMBER),
            hint(Action::Stop, theme::ROSE),
        ],

        // Nothing is running, so nothing can be reloaded or stopped. What is left is
        // what to do next: build again, move to another device, read the log, leave.
        State::Stopped => vec![
            // `Build again`, not `Retry Build`: nothing failed here, the run was
            // ended on purpose. Same `Action`, so the click and the key stay one
            // path; only the word on the cheatsheet differs.
            ("r", "Build again", theme::EMERALD, Some(Action::RetryBuild)),
            hint(Action::Switch, theme::CYAN),
            ("↑↓", "Scroll", theme::CYAN, None),
            ("e", expand, theme::CYAN, None),
            hint(Action::Quit, theme::MUTED),
        ],

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
                hint(Action::StopRun, theme::AMBER),
                ("q", "Quit", theme::ROSE, None),
                hint(Action::Stop, theme::ROSE),
                // `[h] Help` was here and is the one hint that could find itself:
                // pressing `h` makes Flutter print its own key list. The row has no
                // truncation rule of its own — below 80 columns the tail is clipped
                // at the buffer edge in silence — so the eight-hint version was
                // losing the stop keys instead, which are the ones that matter when
                // something is wrong.
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
    let draw = |labelled: bool| -> Vec<Vec<Span>> {
        hints
            .iter()
            .map(|(key, label, color, _)| {
                let mut spans = keycap(key, *color);

                if labelled {
                    spans.push(text(" ", theme::MUTED));
                    spans.push(text(*label, theme::MUTED));
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

    // The gaps between hints are content, and this is what the fit tests below have
    // to ask about. Leaving them out is what let those tests pass on a row that then
    // overflowed: at 106 columns the seven running hints measured 84 against 87
    // available and were clipped at the buffer edge in silence (7.5), taking
    // `[^C] Force stop` with them — the one key that matters when Flutter is wedged.
    let gaps = hints.len().saturating_sub(1);

    // Two columns reads as separate keys, one still reads as a list, and the second
    // tier is worth having: at 80 columns the running row fits with single spacing
    // and would otherwise have to drop its words.
    let fits = |sum: usize, gap: u16| sum + gaps * gap as usize <= area.width as usize;

    let mut rendered = draw(true);
    let mut natural = measure(&rendered);
    let mut sum: usize = natural.iter().sum();
    let mut gap: u16 = 2;

    if !fits(sum, gap) {
        gap = 1;
    }

    // Keys without their words, rather than a row missing its last keys. `[^C]`
    // alone is still a key you can press; a `[^C` that was cut off is not, and
    // nothing on screen would say it had been.
    if !fits(sum, gap) {
        rendered = draw(false);
        natural = measure(&rendered);
        sum = natural.iter().sum();
        gap = if fits(sum, 2) { 2 } else { 1 };
    }

    let content = sum + gaps * gap as usize;

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
        let given_up = plan.describe(app.state);

        if given_up != "full" {
            optional.push(vec![text(format!("[{given_up}]"), theme::BORDER)]);
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

    // Right-aligned, at a fixed two columns between hints.
    //
    // Space-between came first: the leftover was split into equal gaps so the row
    // spanned the full width. It kept the keys in the same place at any size, but it
    // also stretched them apart from each other as the window grew, until reading
    // the row meant crossing ninety columns of blank to find the next key. Fixed
    // spacing keeps the group readable as a group, and putting it on the right edge
    // keeps it next to the diagnostics rather than leaving a gulf between them.
    //
    // Equal-ratio columns were the first attempt before that and they clip: seven
    // hints across ninety columns gives thirteen each, and `[^C] Force stop` needs
    // fifteen. Forcing equal widths on unequal content truncates the longest, which
    // on a cheatsheet is the worst thing it can do.
    //
    // Clamped: below the width the row needs, this is the left edge, and the tiers
    // above have already given up the spacing and then the words.
    let mut x = cols[0].x + cols[0].width.saturating_sub(content as u16);

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
