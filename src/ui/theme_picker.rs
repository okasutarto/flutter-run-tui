//! Theme selector modal with real-time live preview.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::data::{Action, App, Hit};
use crate::theme::ThemeKind;
use crate::widgets::{card, pill, separator, strong, text};

/// Calculate centered area for the theme picker modal.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = card("SELECT THEME", app.theme.cyan, &app.theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(ThemeKind::ALL.len() as u16),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let themes = ThemeKind::ALL;
    let mut lines = Vec::new();

    for (i, &kind) in themes.iter().enumerate() {
        let selected = i == app.theme_picker_index;
        let is_current = kind == app.saved_theme.map(|t| t.kind).unwrap_or(app.theme.kind);
        let palette = kind.palette();

        let row_area = Rect {
            x: rows[0].x,
            y: rows[0].y + i as u16,
            width: rows[0].width,
            height: 1,
        };
        app.hits.push(Hit {
            area: row_area,
            action: Action::SelectTheme(i),
        });

        let cursor = if selected { " ❯ " } else { "   " };
        let num = if i < 9 {
            format!("[{}] ", i + 1)
        } else {
            "[0] ".to_string()
        };
        let name = format!("{:<16}", kind.name());

        let mut spans = vec![
            if selected {
                strong(cursor, app.theme.cyan)
            } else {
                text(cursor, app.theme.muted)
            },
            text(num, app.theme.muted),
            if selected {
                strong(name, app.theme.text)
            } else {
                text(name, app.theme.text)
            },
            Span::raw("  "),
            // Color swatches of that theme
            Span::styled("■ ", Style::new().fg(palette.cyan)),
            Span::styled("■ ", Style::new().fg(palette.emerald)),
            Span::styled("■ ", Style::new().fg(palette.amber)),
            Span::styled("■ ", Style::new().fg(palette.rose)),
            Span::styled("■ ", Style::new().fg(palette.purple)),
        ];

        if is_current {
            spans.push(Span::raw("  "));
            spans.extend(pill(" CURRENT ", app.theme.emerald, &app.theme));
        } else if selected {
            spans.push(Span::raw("  "));
            spans.extend(pill(" PREVIEW ", app.theme.amber, &app.theme));
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), rows[0]);

    if rows.len() > 1 {
        frame.render_widget(
            Paragraph::new(separator(rows[1].width, &app.theme)),
            rows[1],
        );
    }

    if rows.len() > 2 {
        let hint = Line::from(vec![
            text(" ↑/↓ / j/k", app.theme.cyan),
            text(" Move   ", app.theme.muted),
            text("⏎", app.theme.emerald),
            text(" Apply   ", app.theme.muted),
            text("Esc", app.theme.rose),
            text(" Cancel", app.theme.muted),
        ]);
        frame.render_widget(Paragraph::new(hint).centered(), rows[2]);
    }
}
