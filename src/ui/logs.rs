//! TerminalLogsView, DESIGN.md 3.5 and 6.1.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::data::{App, State};
use crate::theme;
use crate::widgets::{badge, card, spread, strong, text, wrap};

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

/// Columns the entry number gets: as many as the largest number on screen needs,
/// and no more.
///
/// A fixed four used to sit here, on the grounds that a constant gutter is what
/// keeps a wrapped row aligned with its own first line. The constant was the right
/// idea and the wrong constant: at eight entries it right-aligned `1` into four
/// columns, so every log line began three columns adrift of the card it is inside
/// while the space stood empty. The invariant is that *one* number describes the
/// gutter, not that the number never changes, so it is computed from the count and
/// both rows read it from `gutter`.
///
/// The cost is a one-column shift of the message column as the log crosses 10, 100
/// and 1000 entries. That is three reflows in a session, against a hole on screen
/// for the first nine entries of every session.
fn index_w(count: usize) -> usize {
    count.max(1).to_string().len()
}

/// Width of `1␣14:32:01␣INF␣` at this entry count.
///
/// Subtracted from every message and used as the continuation indent, so the two
/// cannot disagree.
fn gutter(count: usize) -> u16 {
    (index_w(count) + 1 + TIME_W + 1 + BADGE_W + 1) as u16
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    // Says so when the window is not at the live tail. Without it, scrolling back
    // and then seeing no new output is indistinguishable from the app having gone
    // quiet.
    let count = if app.log_scroll > 0 {
        format!("[{} entries · scrolled] ", app.logs.len())
    } else {
        format!("[{} entries] ", app.logs.len())
    };

    let block = card("APP LOGS STREAM", theme::CYAN)
        .title_top(Line::from(vec![Span::raw(" "), text(count, theme::MUTED)]).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);

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
    let index_w = index_w(app.logs.len());
    let gutter = gutter(app.logs.len());

    let msg_w = area.width.saturating_sub(gutter).max(8) as usize;

    // Build every visual row first, then keep the tail. Wrapping means one
    // entry is not one row, so the count cannot be known before wrapping.
    let mut rows: Vec<Line> = Vec::new();

    for (index, log) in app.logs.iter().enumerate() {
        for (i, chunk) in wrap(&log.message, msg_w).into_iter().enumerate() {
            if i == 0 {
                let mut spans = vec![
                    text(format!("{:>index_w$} ", index + 1), theme::BORDER),
                    text(log.time.as_str(), theme::MUTED),
                    Span::raw(" "),
                ];

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
                // level, so reprinting them spends 18 columns on nothing.
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
    /// be the same number, at every entry count and for every level.
    ///
    /// Two numbers described this column before: the spans that draw it, and the
    /// constant the continuation rows indent by. They agreed up to 99 entries and
    /// then quietly stopped — a real run reached 908 and every wrapped line was a
    /// column out. They also disagreed on reload rows at any count, where the
    /// one-cell bolt made the drawn gutter two columns short.
    #[test]
    fn the_drawn_gutter_matches_the_continuation_indent() {
        use crate::data::Level;

        for count in [1usize, 9, 10, 99, 100, 999, 1000, 4000] {
            for level in [Level::Inf, Level::Wrn, Level::Err, Level::Reload] {
                let w = index_w(count);

                let drawn = format!(
                    "{:>w$} {} {:^BADGE_W$} ",
                    count,
                    "14:32:01",
                    level.badge(),
                );

                assert_eq!(
                    width(&drawn),
                    gutter(count) as usize,
                    "{count} entries at {level:?} draw {} columns, the indent is {}: {drawn:?}",
                    width(&drawn),
                    gutter(count),
                );
            }
        }
    }

    /// The point of computing it: the first entry of a session starts against the
    /// card's content edge rather than three columns inside it.
    #[test]
    fn a_short_log_does_not_reserve_columns_it_cannot_use() {
        assert_eq!(index_w(1), 1);
        assert_eq!(gutter(1), 15, "three columns narrower than the old constant");

        // And it still grows to the 18 that a four-digit count needs, which is
        // what the fixed constant was.
        assert_eq!(index_w(4000), 4);
        assert_eq!(gutter(4000), 18);
    }
}
