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
use crate::data::{Action, App, Hit, State};
use crate::theme;
use crate::widgets::{card, field, keycap, pill, separator, spread, strong, text};

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
    let mut block = card("DEVICE INFO", theme::CYAN);

    // How the run ended, as a pill in the title bar. `STOPPED`, `DETACHED`,
    // `DISCONNECTED`.
    //
    // These were the whole reason the tracker block stayed on screen after a build —
    // one row plus the blank above it, carried through every frame of a stopped
    // session so that three words could be said once. This slot costs nothing, and it
    // was empty: `^D` vacated it when it moved inside the card, and every other card
    // uses its top-right for a count, a path or a status. A status is the plainest
    // case of that, so filling it here closes the one exception rather than inventing
    // an idiom.
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
    // its fill interrupts the rule, which is how a badge on a frame edge is supposed
    // to read. The glyph stays outside the fill, where a colour behind a word does not
    // have to compete with a symbol on top of it.
    //
    // Only in `Stopped`. A live run has nothing to report here that the streaming log
    // window below is not reporting continuously and more precisely, and leaving the
    // slot empty until then is what makes the pill's *arrival* the signal — the eye
    // catches a thing appearing far more reliably than a word changing in place. That
    // property is the one thing worth preserving from the banner this replaces.
    if app.state == State::Stopped {
        let (glyph, label, color) = super::build::status(app);

        let mut spans = vec![text("─ ", theme::BORDER), strong(glyph, color), Span::raw(" ")];

        spans.extend(pill(format!(" {label} "), color));
        spans.push(Span::raw(" "));

        block = block.title_top(Line::from(spans).right_aligned());
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let w = inner.width;

    // Advertised only where the key does something. Before a run there is no
    // session to move, and with no cached list there is nothing to move it to.
    //
    // `has_build`, deliberately, and not the narrower `has_tracker` the frame uses
    // to decide whether the tracker block is laid out: this asks whether there is a
    // run to move, which is true throughout a run, and the tracker is absent for
    // most of one.
    let switch = app.state.has_build() && !app.devices.is_empty();

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
    // Bound here rather than at the top of the function, because the `Hit` at the
    // bottom needs `&mut app` and a borrow taken before the early returns would
    // still be live at that point.
    let device = app
        .target
        .as_ref()
        .expect("returned above when there is no target");

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

    // The switch control, on a row of its own inside the card.
    //
    // It used to ride in the border beside the title, where it cost no rows at all,
    // and that was the whole argument for putting it there. Two things paid for
    // moving it in. The top border is now one label and nothing else, which is what
    // every other card's top-left says and what its top-right is for — a count, a
    // path, a status — never a control. And a keycap drawn on a border is not
    // clickable: `render` took `&App`, so there was nowhere to register a `Hit`, and
    // 3.1 is explicit that a control which does nothing on click is worse than no
    // control. As a content row it is a rectangle, so it gets one.
    //
    // Right-aligned, on its own row below `Type`, with a blank row between the two
    // and no separator.
    //
    // The blank is what stops it reading as a fifth field's value. Sitting directly
    // under `Type` it was the fourth row of a four-row table, right-aligned in the
    // column the values occupy, and the eye groups by proximity before it reads
    // brackets — so the keycap arrived as data belonging to the row above it. A blank
    // says the table ended without spending a rule on saying it.
    //
    // A separator would say the same thing and say it louder than a control needs.
    // Both cost one row; the blank is the one that does not divide the card in two.
    let mut right = Vec::new();

    if switch {
        right.extend(keycap(Action::Switch.key(), theme::CYAN));
        right.push(text(format!(" {}", Action::Switch.label()), theme::MUTED));
    }

    // The row is charged in `target_h` unconditionally, so it is drawn
    // unconditionally: skipping it when empty would leave a blank row above the
    // bottom border in the states that have no control, and drawing an empty
    // `spread` costs the same nothing.
    let control = {
        // Measured, not counted: `^D` is two cells in one glyph short of it, and the
        // hit rectangle has to sit exactly under what was drawn.
        //
        // Read after the blank is pushed, so it is the control's own row and not the
        // gap above it — a hit rectangle one row high, registered one row too early,
        // is a click that lands on nothing.
        let width: usize = right.iter().map(Span::width).sum();

        lines.push(Line::default());
        let row = lines.len() as u16;

        lines.push(spread(w, Vec::new(), right));

        // No control, no region. `spread` pads an empty group to nothing, so the
        // rectangle would be zero-wide at the right border — a click target that
        // cannot be hit but is still consulted on every mouse event.
        (width > 0).then(|| Rect {
            x: inner.x + inner.width.saturating_sub(width as u16),
            y: inner.y + row,
            width: width as u16,
            height: 1,
        })
    };

    frame.render_widget(Paragraph::new(lines), inner);

    // After the draw, which is what releases the borrow on `app.target` that every
    // line above holds.
    if let Some(area) = control {
        app.hits.push(Hit {
            area,
            action: Action::Switch,
        });
    }
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
