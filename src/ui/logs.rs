//! TerminalLogsView, DESIGN.md 3.5 and 6.1.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::data::{App, State};
use crate::theme::{self, Theme};
use crate::widgets::{badge, card, separator, spread, strong, text, wrap};

/// Columns the level badge occupies, on every row.
const BADGE_W: usize = 3;

/// `HH:MM:SS`, which `probe::Clock` guarantees.
const TIME_W: usize = 8;

const fn gutter() -> u16 {
    (TIME_W + 1 + BADGE_W + 1) as u16
}

const TITLE: &str = "APP LOGS STREAM";

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let count = if app.log_scroll > 0 {
        format!("[{} entries · scrolled] ", app.logs.len())
    } else {
        format!("[{} entries] ", app.logs.len())
    };

    let right = vec![Span::raw(" "), text(count, app.theme.muted)];

    let block = card(TITLE, app.theme.purple, &app.theme).title_top(Line::from(right).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);

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
            Paragraph::new(separator(inner.width, &app.theme)),
            sep_row,
        );
    }

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

    let mut rows: Vec<Line> = Vec::new();

    for log in app.logs.iter() {
        for (i, chunk) in wrap(&log.message, msg_w).into_iter().enumerate() {
            if i == 0 {
                let mut spans = vec![text(log.time.as_str(), app.theme.muted), Span::raw(" ")];

                spans.extend(badge(
                    format!("{:^BADGE_W$}", log.level.badge()),
                    log.level.color(&app.theme),
                    &app.theme,
                ));
                spans.push(Span::raw(" "));
                spans.push(text(chunk, message_color(log.level, &app.theme)));

                rows.push(Line::from(spans));
            } else {
                rows.push(Line::from(vec![
                    Span::raw(" ".repeat(gutter as usize)),
                    text(chunk, message_color(log.level, &app.theme)),
                ]));
            }
        }
    }

    let height = area.height as usize;

    let max_scroll = rows.len().saturating_sub(height);
    app.log_scroll = app.log_scroll.min(max_scroll);

    let start = rows.len().saturating_sub(height + app.log_scroll);

    let visible: Vec<Line> = rows.into_iter().skip(start).take(height.max(1)).collect();

    frame.render_widget(Paragraph::new(visible), area);
}

fn message_color(level: crate::data::Level, theme: &Theme) -> ratatui::style::Color {
    use crate::data::Level;

    match level {
        Level::Err => theme.rose,
        Level::Wrn => theme.amber,
        Level::Reload => theme.purple,
        Level::Inf => theme.text,
    }
}

fn reload_status(frame: &mut Frame, area: Rect, app: &App) {
    let line = match app.state {
        State::ReloadInFlight => spread(
            area.width,
            vec![
                strong(app.spinner(), app.theme.purple),
                text(format!("  {} ", theme::GLYPH_BOLT), app.theme.purple),
                text(app.reload_note.as_str(), app.theme.text),
            ],
            vec![text(app.pending_clock(), app.theme.muted)],
        ),

        State::ReloadFailed => Line::from(vec![
            strong("✖ ", app.theme.rose),
            strong("Failed  ", app.theme.rose),
            text(app.reload_note.as_str(), app.theme.text),
        ]),

        State::ReloadDropped => Line::from(vec![
            strong("⚠ ", app.theme.amber),
            text(app.reload_note.as_str(), app.theme.text),
        ]),

        _ => Line::default(),
    };

    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::width;

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
