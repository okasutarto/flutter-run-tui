//! SelectedTargetCard, DESIGN.md 3.2.
//!
//! Every field is read off `app.target`, the device that was actually chosen,
//! rather than out of four separate strings copied beside it. The card cannot
//! then describe a device other than the one being run, which is the one thing
//! it exists to get right.

use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme;
use crate::widgets::{card, field, pill, separator, spread, strong, text};

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
    let Some(device) = &app.target else {
        return;
    };

    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let block = card("SELECTED TARGET", theme::CYAN);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let w = inner.width;

    // No active-status banner, and no command string. Both were in DESIGN.md 3.2
    // and both are gone, for four rows.
    //
    // The banner read `✔ 1 device active: iPhone 17 Pro (emulator) (iOS)`. Every
    // fact in it is already in the table below: the name is the `Device Target`
    // pill, `(emulator)` is `Type`, and the platform is the head of `Platform ID`.
    // What it added was the count, and the count is one by construction — this
    // card only exists once a device has been chosen.
    //
    // The command string read `❯ fvm flutter run -d <udid>`. It was display only:
    // `Session::spawn` builds its own argv, so nothing depended on it, and the
    // device it names is the row directly above it. Two rows plus its blank
    // separator, in the state where the log window is hungriest.
    let mut lines = vec![field(
        w,
        "Device Target",
        pill(format!(" {} ", device.name), theme::CYAN),
    )];

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "Platform ID",
        vec![strong(platform_id(app), theme::CYAN)],
    ));

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "OS Version / Arch",
        vec![text(os_version(app), theme::TEXT)],
    ));

    if plan.separators {
        lines.push(separator(w));
    }

    lines.push(field(
        w,
        "Type",
        vec![strong(app.target_kind(), theme::PURPLE)],
    ));

    frame.render_widget(Paragraph::new(lines), inner);
}

/// `android-arm64 (emulator-5554)`.
///
/// The id is part of this field rather than a row of its own: `targetPlatform`
/// alone does not distinguish two attached Pixels, and the id alone does not say
/// what it is.
fn platform_id(app: &App) -> String {
    let Some(device) = &app.target else {
        return "-".into();
    };

    if device.target_platform.is_empty() {
        return device.id.clone();
    }

    format!("{} ({})", device.target_platform, device.id)
}

/// Flutter's own `sdk` string, which is where the OS version and arch live.
///
/// A freshly booted device has none: it was launched from an AVD name or a UDID
/// and Flutter has not been asked about it since. The dash is honest; inventing
/// a version would not be.
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
