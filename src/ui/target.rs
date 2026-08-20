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

    let mut block = card("DEVICE INFO", app.theme.purple, &app.theme);

    if !app.state.has_tracker() {
        let (glyph, label, color) = super::build::status(app);

        let mut spans = vec![text("─ ", app.theme.border)];
        spans.extend(pill(format!(" {glyph} {label} "), color, &app.theme));
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
        crate::probe::Platform::Android => (theme::GLYPH_ANDROID, app.theme.emerald),
        crate::probe::Platform::Ios => (theme::GLYPH_APPLE, app.theme.cyan),
        crate::probe::Platform::Desktop => (theme::GLYPH_DESKTOP, app.theme.cyan),
        crate::probe::Platform::Web => (theme::GLYPH_WEB, app.theme.cyan),
    };

    let room = (w as usize).saturating_sub(
        crate::widgets::width("Device Target")
            + crate::widgets::width(kind)
            + 6,
    );

    let name_str = elide(&device.name, room.max(8));

    let target_value = if kind.is_empty() {
        vec![strong(name_str, app.theme.text)]
    } else {
        vec![
            strong(name_str, app.theme.text),
            Span::raw(" "),
            strong(format!("({kind})"), app.theme.purple),
        ]
    };

    let mut lines = vec![field(w, "Device Target", target_value, &app.theme)];

    if plan.separators {
        lines.push(separator(w, &app.theme));
    }

    lines.push(field(
        w,
        "OS Version",
        vec![
            strong(os_glyph, os_color),
            Span::raw("  "),
            strong(os_str, os_color),
        ],
        &app.theme,
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
            strong("✔ ", app.theme.emerald),
            strong(device.name.as_str(), app.theme.text),
            Span::raw("  "),
            text(device.id.as_str(), app.theme.cyan),
        ],
        vec![text(os_version(app), app.theme.muted)],
    );

    frame.render_widget(Paragraph::new(line), area);
}
