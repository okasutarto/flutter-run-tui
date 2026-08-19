//! TerminalLogsView, DESIGN.md 3.5 and 6.1.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::data::{App, State};
use crate::theme;
use crate::widgets::{badge, card, separator, spread, strong, text, wrap};

/// Columns the level badge occupies, on every row.
///
/// `INF`, `WRN` and `ERR` are three cells. The reload bolt is one, so it is
/// padded to match rather than left to shrink the gutter on its own rows — which
/// it did: a `Reloaded 125 of 1824 libraries` line started two columns left of
/// every line around it, and any continuation of it indented to a column its own
/// first row did not use.
const BADGE_W: usize = 3;

/// `HH:MM:SS`, which `probe::Clock` guarantees.
const TIME_W: usize = 8;

/// Width of `14:32:01␣INF␣`.
///
/// Subtracted from every message and used as the continuation indent, so the two
/// cannot disagree.
///
/// **No entry number.** A right-aligned counter opened every row and was the first
/// thing the eye crossed on the way to the message, for a figure nothing reads: the
/// title bar carries the total, scroll position is reported there too, and no other
/// part of the app addresses a log line by its ordinal. It also grew — one column at
/// 10 entries, another at 100, a fourth at 1000 — so a chatty session spent four
/// columns of every row, and the widest gutter arrived exactly when the messages
/// were longest. The timestamp is the same anchor with a use.
const fn gutter() -> u16 {
    (TIME_W + 1 + BADGE_W + 1) as u16
}

/// Title, needed as a measurement as well as a label: the two title groups share
/// one border row, and ratatui clips rather than reflows when they meet.
const TITLE: &str = "APP LOGS STREAM";

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    // Says so when the window is not at the live tail. Without it, scrolling back
    // and then seeing no new output is indistinguishable from the app having gone
    // quiet.
    let count = if app.log_scroll > 0 {
        format!("[{} entries · scrolled] ", app.logs.len())
    } else {
        format!("[{} entries] ", app.logs.len())
    };

    // The entry count and nothing else.
    //
    // The build's three figures used to sit here, immediately left of it, with a
    // width guard dropping the group whole when the border row could not hold both.
    // Three frozen numbers beside one that changes on every line read as a single
    // run-on string, and the guard meant the frozen three vanished at exactly the
    // widths where a reader has least context — so they moved inside the card, to
    // `summary` below, and this slot went back to holding the one live figure.
    let right = vec![Span::raw(" "), text(count, theme::MUTED)];

    let block = card(TITLE, theme::PURPLE).title_top(Line::from(right).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // First row inside the card for the build's figures, in the states where the
    // tracker is not on screen holding them as stage rows (3.4).
    //
    // Two rows, not one: the figures and a blank under them. Without the blank the
    // first log line butts straight against a row that is not a log line, which is
    // the same defect `card()` bakes a title gap in to avoid — and here it is worse,
    // because both rows start in column one and the stream's own timestamps make the
    // collision look like a malformed entry.
    //
    // Rows of the stream, and the only thing in this card that is charged any: the log
    // window is the flexible middle, so `Budget::floor` adds both rather than letting
    // the stream's floor of twelve quietly become ten.
    //
    // Bought back from the two static cards in the same change: `Type` merged into
    // `Device Target` and `Version` into `Project`, four rows between them with their
    // rules. The stream is two rows better off than before this row existed.
    let (inner, summary) = match app.state.has_tracker() {
        true => (inner, None),

        false => (
            Rect {
                y: inner.y + 2,
                height: inner.height.saturating_sub(2),
                ..inner
            },
            Some(Rect { height: 1, ..inner }),
        ),
    };

    if let Some(row) = summary {
        let sep_row = Rect {
            y: row.y + 1,
            height: 1,
            ..row
        };
        frame.render_widget(
            Paragraph::new(Line::from(super::build::timings(app))),
            row,
        );
        frame.render_widget(
            Paragraph::new(separator(inner.width)),
            sep_row,
        );
    }

    // Reserve the last row for the reload status, but only when there is one to
    // show. Keyed on `reloading()` rather than on "not Running", because Building
    // now draws this view too and would otherwise reserve two rows to print
    // nothing in them.
    let (stream, status) = if !app.state.reloading() {
        (inner, None)
    } else {
        let split = Rect {
            height: inner.height.saturating_sub(2),
            ..inner
        };

        let bar = Rect {
            y: inner.y + inner.height.saturating_sub(1),
            height: 1,
            ..inner
        };

        (split, Some(bar))
    };

    draw_stream(frame, stream, app);

    if let Some(bar) = status {
        reload_status(frame, bar, app);
    }
}

fn draw_stream(frame: &mut Frame, area: Rect, app: &mut App) {
    let gutter = gutter();

    let msg_w = area.width.saturating_sub(gutter).max(8) as usize;

    // Build every visual row first, then keep the tail. Wrapping means one
    // entry is not one row, so the count cannot be known before wrapping.
    let mut rows: Vec<Line> = Vec::new();

    for log in app.logs.iter() {
        for (i, chunk) in wrap(&log.message, msg_w).into_iter().enumerate() {
            if i == 0 {
                let mut spans = vec![text(log.time.as_str(), theme::MUTED), Span::raw(" ")];

                spans.extend(badge(
                    format!("{:^BADGE_W$}", log.level.badge()),
                    log.level.color(),
                ));
                spans.push(Span::raw(" "));
                spans.push(text(chunk, message_color(log.level)));

                rows.push(Line::from(spans));
            } else {
                // Continuation. The gutter is left empty rather than repeated:
                // consecutive rows of one exception share a timestamp and a
                // level, so reprinting them spends 13 columns on nothing.
                rows.push(Line::from(vec![
                    Span::raw(" ".repeat(gutter as usize)),
                    text(chunk, message_color(log.level)),
                ]));
            }
        }
    }

    // Bottom-anchored. A stream that fills from the top puts new output at the
    // top of a mostly blank box, far from where the eye already is.
    let height = area.height as usize;

    // Clamped here rather than where the key is handled, because the ceiling is
    // the number of *visual* rows and that is only known after wrapping at this
    // width. One Dart exception is eight rows, so an entry count would be the
    // wrong unit.
    let max_scroll = rows.len().saturating_sub(height);
    app.log_scroll = app.log_scroll.min(max_scroll);

    let start = rows.len().saturating_sub(height + app.log_scroll);

    let visible: Vec<Line> = rows.into_iter().skip(start).take(height.max(1)).collect();

    let target = if visible.len() < height {
        Rect {
            y: area.y + area.height - visible.len() as u16,
            height: visible.len() as u16,
            ..area
        }
    } else {
        area
    };

    frame.render_widget(Paragraph::new(visible), target);
}

fn message_color(level: crate::data::Level) -> ratatui::style::Color {
    use crate::data::Level;

    match level {
        Level::Err => theme::ROSE,
        Level::Wrn => theme::AMBER,
        Level::Reload => theme::PURPLE,
        Level::Inf => theme::TEXT,
    }
}

/// States 9, 10 and 11 share this row. Three outcomes, not two: Flutter can
/// accept a key and fail, or never take the key at all.
fn reload_status(frame: &mut Frame, area: Rect, app: &App) {
    let line = match app.state {
        State::ReloadInFlight => spread(
            area.width,
            vec![
                strong(app.spinner(), theme::PURPLE),
                text(format!("  {} ", theme::GLYPH_BOLT), theme::PURPLE),
                text(app.reload_note.as_str(), theme::TEXT),
            ],
            // The clock is the difference between slow and stuck. Once Flutter
            // has acknowledged the key there is no timeout left to apply — a big
            // restart may legitimately take a while — so elapsed time is the only
            // honest thing to show.
            vec![text(app.pending_clock(), theme::MUTED)],
        ),

        State::ReloadFailed => Line::from(vec![
            strong("✖ ", theme::ROSE),
            strong("Failed  ", theme::ROSE),
            text(app.reload_note.as_str(), theme::TEXT),
        ]),

        // Neither success nor failure: the operation never started. Flutter
        // discards keys it cannot service and reports nothing on stdout, so
        // without this the spinner would run forever.
        State::ReloadDropped => Line::from(vec![
            strong("⚠ ", theme::AMBER),
            text(app.reload_note.as_str(), theme::TEXT),
        ]),

        _ => Line::default(),
    };

    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::width;

    /// What the drawn gutter measures and what a continuation row indents by must
    /// be the same number, for every level.
    ///
    /// Two numbers described this column before: the spans that draw it, and the
    /// constant the continuation rows indent by. They disagreed on reload rows,
    /// where the one-cell bolt made the drawn gutter two columns short, and — while
    /// the gutter still carried an entry number — above 99 entries, where a real run
    /// reached 908 and every wrapped line was a column out.
    ///
    /// The entry number is gone, so the count no longer enters into it. The badge
    /// still does: `INF` is three cells and the reload bolt is one, padded to match.
    #[test]
    fn the_drawn_gutter_matches_the_continuation_indent() {
        use crate::data::Level;

        for level in [Level::Inf, Level::Wrn, Level::Err, Level::Reload] {
            let drawn = format!("{} {:^BADGE_W$} ", "14:32:01", level.badge());

            assert_eq!(
                width(&drawn),
                gutter() as usize,
                "{level:?} draws {} columns, the indent is {}: {drawn:?}",
                width(&drawn),
                gutter(),
            );
        }
    }
}
