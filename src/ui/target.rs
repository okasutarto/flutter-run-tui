//! SelectedTargetCard, DESIGN.md 3.2.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme;
use crate::widgets::{card, field, pill, separator, spread, strong, text};

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let block = card("SELECTED TARGET", theme::CYAN).padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let w = inner.width;

    let mut lines = vec![
        // Active status banner. Emerald, and it names the device, because this
        // is the line that answers "am I about to run on the right thing".
        Line::from(vec![
            strong("✔ 1 device active: ", theme::EMERALD),
            strong(app.target_name, theme::EMERALD),
        ]),
        Line::default(),
        field(
            w,
            "Device Target",
            pill(format!(" {} ", app.target_name), theme::CYAN),
        ),
    ];

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "Platform ID",
        vec![strong(app.target_platform_id, theme::CYAN)],
    ));

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "OS Version / Arch",
        vec![text(app.target_os, theme::TEXT)],
    ));

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "Type",
        vec![strong(app.target_kind, theme::PURPLE)],
    ));

    // The command string, which lived in the header bar before that was
    // removed. It belongs here: this card already describes what runs and
    // where.
    lines.push(Line::default());
    lines.push(Line::from(vec![
        text("❯ ", theme::MUTED),
        text(app.command, theme::MUTED),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let line = spread(
        area.width,
        vec![
            strong("✔ ", theme::EMERALD),
            strong(app.target_name, theme::TEXT),
            Span::raw("  "),
            text(app.target_platform_id, theme::CYAN),
        ],
        vec![text(app.target_os, theme::MUTED)],
    );

    frame.render_widget(Paragraph::new(line), area);
}
