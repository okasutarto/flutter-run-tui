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
use crate::widgets::{card, field, pill, separator, spread, strong, text};

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    if app.target.is_none() {
        return;
    }

    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    // `DEVICE INFO`, matching `PROJECT INFO` directly above it. It was
    // `SELECTED DEVICE`, which is two naming schemes on two stacked cards, and the
    // word it gave up was carried by the card's existence anyway: `render` returns
    // early without a target, and `State::has_target` hides the card in every
    // picker state, so nothing but a chosen device can put it on screen.
    let mut block = card("DEVICE INFO", theme::PURPLE);

    // What the run is doing, as a pill in the title bar: `RUNNING`, then `STOPPED`,
    // `DETACHED` or `DISCONNECTED` depending on how it ended.
    //
    // The three ending words were the whole reason the tracker block stayed on screen
    // after a build — one row plus the blank above it, carried through every frame of
    // a stopped session so that three words could be said once. This slot costs
    // nothing, and it was empty: `^D` vacated it when it moved inside the card, and
    // every other card uses its top-right for a count, a path or a status. A status is
    // the plainest case of that, so filling it here closes the one exception rather
    // than inventing an idiom.
    //
    // On *this* card because the three words are statements about the device. After
    // Flutter's own `d` the app is still running on it; after `^S` it is gone; after a
    // `Lost` the connection to it is what broke. `PROJECT INFO` was the other
    // candidate and is wrong twice over: nothing about a project is disconnected, and
    // its top-right is already spent on the cwd — which at any real path length would
    // have forced a drop rule where the thing to drop is either the news or a path you
    // know by heart.
    //
    // No `Status` label, because nothing else in a title slot has one: `5 devices`,
    // `[7 entries]` and `~/cwclub` are all bare, and the position is what says the
    // value describes the card as a whole rather than one row of it.
    //
    // A pill rather than a bare word, matching the chips on a device row: this is a
    // state the device is in, which is what those chips are for. Sitting on the border
    // its fill interrupts the rule, which is how a badge on a frame edge should read.
    //
    // Glyph inside the fill, so the pill is one object. Outside it, the glyph read as
    // a separate mark that happened to be next to a badge — and on a border row, where
    // `─` runs up to it on the left, a lone symbol between the rule and the pill has
    // nothing to attach itself to.
    //
    // Shown in every state that draws this card except the two the tracker block owns.
    // `has_tracker` is the same gate the log card's title uses for the build totals,
    // and that is the point: while the tracker is on screen it holds both the word and
    // the numbers, and when it is not, the word is here and the numbers are there.
    // Without the gate, `BUILDING` would be on screen twice — once in the tracker's
    // title and once in this pill.
    if !app.state.has_tracker() {
        let (glyph, label, color) = super::build::status(app);

        let mut spans = vec![text("─ ", theme::BORDER)];

        spans.extend(pill(format!(" {glyph} {label} "), color));
        spans.push(Span::raw(" "));

        block = block.title_top(Line::from(spans).right_aligned());
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let w = inner.width;

    // No active-status banner, and no command string. Both were in DESIGN.md 3.2
    // and both are gone, for four rows.
    //
    // The banner read `✔ 1 device active: iPhone 17 Pro (emulator) (iOS)`. Every
    // fact in it is already in the table below: the name is `Device Target`,
    // `(emulator)` is `Type`, and the platform is the head of `Platform ID`.
    // What it added was the count, and the count is one by construction — this
    // card only exists once a device has been chosen.
    //
    // The command string read `❯ fvm flutter run -d <udid>`. It was display only:
    // `Session::spawn` builds its own argv, so nothing depended on it, and the
    // device it names is the row directly above it. Two rows plus its blank
    // separator, in the state where the log window is hungriest.
    let device = app
        .target
        .as_ref()
        .expect("returned above when there is no target");

    // Plain bold text, not a pill.
    //
    // The pill was here to mark this as the card's headline value, and once the run
    // status became a pill in the title bar directly above it, two pills on one card
    // meant two different things: one is a *state*, which is what a filled chip says
    // everywhere else in this app — ` active `, ` running `, ` in use `, ` last used `
    // on a device row — and this one was a name.
    //
    // `TEXT` bold rather than a hue, and not `CYAN` like `Platform ID` below it: the
    // two adjacent rows would then look like one value split across them. Brightest
    // text with no colour is also how the device list draws the name of the selected
    // row, so the same fact is styled the same way in both places.
    let mut lines = vec![field(
        w,
        "Device Target",
        vec![strong(device.name.as_str(), theme::TEXT)],
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

    // `OS Version`, not `OS Version / Arch`, which is what DESIGN.md 3.2 asked
    // for and what this row could not deliver.
    //
    // No mobile platform puts an architecture in this string. Flutter's
    // `sdkNameAndVersion` is `Android 17 (API 37)` for Android and a runtime
    // identifier for an iOS simulator; the arch lives in `targetPlatform`, which
    // is the row directly above — `android-arm64 (emulator-5554)`. Only desktop
    // carries one, inside Flutter's own prose (`macOS 26.6.1 25G76 darwin-arm64`),
    // and it is still there.
    //
    // So the half-promise was either unfulfillable or a repeat of the line above
    // it. Naming the row after the one fact it always holds costs nothing: the
    // arch did not move and was never here.
    lines.push(field(
        w,
        "OS Version",
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

    // No control row. `[^D] Switch` closed this card for a while — a blank and a
    // right-aligned keycap — and it is on the footer now, in the five states where
    // the key does something (3.7).
    //
    // Two rows for one key was the cost, and the shape was the reason. It was the
    // only row in the card with nothing facing it on the left: eight rows of
    // label-and-value, then a keycap alone in the whitespace. The blank above it
    // existed to stop it reading as a fifth field's value, which is a row spent
    // saying that the row below it is not what it looks like.
    //
    // The footer already owned this job. The failure card's `[r] Retry Build
    // [q] Quit` and the picker's `↑↓ move  ⏎ run  Esc cancel` were both removed on
    // the same grounds, and this was the third and last of them. The card is a table
    // of facts about the device again, with the one state it is in as a pill in its
    // title.
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

/// Flutter's own `sdk` string, which is where the OS version lives.
///
/// A device frun booted itself used to have none, and this read `-` for a device
/// that was running and answerable. It is filled now from two places that both
/// pre-date the run: an Android emulator is asked over adb once
/// `sys.boot_completed` lands, and an iOS simulator carries the runtime `simctl`
/// filed it under from the moment it appears in the picker.
///
/// The dash survives for the case it was right about all along: a device nothing
/// has been able to tell us anything about. Inventing a version would not be
/// better.
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
