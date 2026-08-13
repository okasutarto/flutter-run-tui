//! TerminalLogsView, DESIGN.md 3.5 and 6.1.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::data::{App, State};
use crate::theme;
use crate::widgets::{badge, card, spread, strong, text, wrap};

/// `0001␣14:32:01␣INF␣` measures 18 columns.
///
/// The spec called this 16; it is 18. Worth being exact about, because it is
/// subtracted from every log line and a two-column error compounds across a
/// wrapped stack trace.
const GUTTER: u16 = 18;

/// Columns the entry number gets, which is what makes the gutter a constant.
///
/// It was `{:>2}`, so the gutter silently grew a column at entry 100 and another
/// at 1000 while the continuation rows kept indenting to 18 — the first real run
/// hit 908 entries and every wrapped line sat one column off its own first line.
/// Four columns covers the 4000-entry cap in `App::log`.
const INDEX_W: usize = 4;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let block = card("APP LOGS STREAM", theme::CYAN).title_top(
        Line::from(vec![
            Span::raw(" "),
            text(format!("[{} entries] ", app.logs.len()), theme::MUTED),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Reserve the last row for the reload status, when there is one to show.
    let (stream, status) = if app.state == State::Running {
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

fn draw_stream(frame: &mut Frame, area: Rect, app: &App) {
    let msg_w = area.width.saturating_sub(GUTTER).max(8) as usize;

    // Build every visual row first, then keep the tail. Wrapping means one
    // entry is not one row, so the count cannot be known before wrapping.
    let mut rows: Vec<Line> = Vec::new();

    for (index, log) in app.logs.iter().enumerate() {
        for (i, chunk) in wrap(&log.message, msg_w).into_iter().enumerate() {
            if i == 0 {
                let mut spans = vec![
                    text(format!("{:>INDEX_W$} ", index + 1), theme::BORDER),
                    text(log.time.as_str(), theme::MUTED),
                    Span::raw(" "),
                ];

                spans.extend(badge(log.level.badge(), log.level.color()));
                spans.push(Span::raw(" "));
                spans.push(text(chunk, message_color(log.level)));

                rows.push(Line::from(spans));
            } else {
                // Continuation. The gutter is left empty rather than repeated:
                // consecutive rows of one exception share a timestamp and a
                // level, so reprinting them spends 18 columns on nothing.
                rows.push(Line::from(vec![
                    Span::raw(" ".repeat(GUTTER as usize)),
                    text(chunk, message_color(log.level)),
                ]));
            }
        }
    }

    // Bottom-anchored. A stream that fills from the top puts new output at the
    // top of a mostly blank box, far from where the eye already is.
    let height = area.height as usize;
    let start = rows.len().saturating_sub(height);
    let visible: Vec<Line> = rows.into_iter().skip(start).collect();

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

    /// The drawn gutter must be exactly `GUTTER` wide, at any entry count.
    ///
    /// Two numbers described the same column before this: the spans that draw the
    /// gutter, and the constant the continuation rows indent by. They agreed up to
    /// 99 entries and then quietly stopped — a real run reached 908 and every
    /// wrapped line was a column out.
    #[test]
    fn the_gutter_is_the_same_width_at_every_entry_count() {
        for index in [0usize, 9, 99, 999, 3999] {
            let drawn = format!(
                "{:>INDEX_W$} {} {} ",
                index + 1,
                "14:32:01",
                crate::data::Level::Inf.badge()
            );

            assert_eq!(
                width(&drawn),
                GUTTER as usize,
                "entry {} draws a {}-column gutter, budget is {GUTTER}: {drawn:?}",
                index + 1,
                width(&drawn),
            );
        }
    }
}
