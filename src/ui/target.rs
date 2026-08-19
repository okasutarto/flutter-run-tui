//! SelectedTargetCard, DESIGN.md 3.2.
//!
//! Every field is read off `app.target`, the device that was actually chosen,
//! rather than out of four separate strings copied beside it. The card cannot
//! then describe a device other than the one being run, which is the one thing
//! it exists to get right.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme;
use crate::widgets::{card, elide, field, pill, separator, spread, strong, text};

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    if app.target.is_none() {
        return;
    }

    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let mut block = card("DEVICE INFO", theme::PURPLE);

    if !app.state.has_tracker() {
        let (glyph, label, color) = super::build::status(app);

        let mut spans = vec![text("─ ", theme::BORDER)];
        spans.extend(pill(format!(" {glyph} {label} "), color));
        spans.push(Span::raw(" "));

        block = block.title_top(Line::from(spans).right_aligned());
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let device = app
        .target
        .as_ref()
        .expect("returned above when there is no target");

    let kind = app.target_kind();
    let os_str = os_version(app);
    let w = inner.width;

    let (os_glyph, os_color) = match device.platform {
        crate::probe::Platform::Android => (theme::GLYPH_ANDROID, theme::EMERALD),
        crate::probe::Platform::Ios => (theme::GLYPH_APPLE, theme::CYAN),
        crate::probe::Platform::Desktop => (theme::GLYPH_DESKTOP, theme::CYAN),
        crate::probe::Platform::Web => (theme::GLYPH_WEB, theme::CYAN),
    };

    let room = (w as usize).saturating_sub(
        crate::widgets::width("Device Target")
            + crate::widgets::width(kind)
            + 6,
    );

    let name_str = elide(&device.name, room.max(8));

    let target_value = if kind.is_empty() {
        vec![strong(name_str, theme::TEXT)]
    } else {
        vec![
            strong(name_str, theme::TEXT),
            Span::raw(" "),
            strong(format!("({kind})"), theme::PURPLE),
        ]
    };

    let mut lines = vec![field(w, "Device Target", target_value)];

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "OS Version",
        vec![
            strong(os_glyph, os_color),
            Span::raw("  "),
            strong(os_str, os_color),
        ],
    ));

    lines.retain(|l| !l.spans.is_empty() || plan.separators);

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Flutter's own `sdk` string, which is where the OS version lives.
fn os_version(app: &App) -> &str {
    match &app.target {
        Some(device) if !device.sdk.is_empty() => device.sdk.as_str(),
        _ => "-",
    }
}

fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let Some(device) = &app.target else {
        return;
    };

    let line = spread(
        area.width,
        vec![
            strong("✔ ", theme::EMERALD),
            strong(device.name.as_str(), theme::TEXT),
            Span::raw("  "),
            text(device.id.as_str(), theme::CYAN),
        ],
        vec![text(os_version(app), theme::MUTED)],
    );

    frame.render_widget(Paragraph::new(line), area);
}
