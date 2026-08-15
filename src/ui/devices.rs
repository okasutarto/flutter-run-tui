//! FlutterDeviceManager, DESIGN.md 3.3. States 2 through 5.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Device, Hit, State};
use crate::theme;
use crate::widgets::{card, elide, pill, spread, strong, text};

// State 2, `NO DEVICE RUNNING`, was rendered here and is gone.
//
// It drew the same rows this file's picker draws, from the same `list()`, under an
// amber `Nothing is attached. These can be started:` — and it could not be reached.
// `devices_answered` branched on `Device::attached`, which is `platform.needs_boot()`
// and so counts bootable rows too, so a single installed simulator made the picker the
// only answer. Nothing booted opens `SELECT DEVICE` with every row offering a boot,
// which is 7.6's merged list doing what the heading used to say.
//
// The rows it did not have are worth naming, because they are the reason not to bring
// it back as a variant: a subtitle and a `Start a Device` heading, two rows spent
// saying what the chips on the rows say, in the frame with the most targets to show.

/// State 4. Two or more answered, so pick one.
pub fn render_picker(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    // `SWITCH DEVICE`, not `SELECT DEVICE`, when there is a run to replace. The
    // list is identical and the consequence is not: one starts a run, the other
    // ends one and starts another. With the target card off screen here, the title
    // is the only place that difference has room to be said.
    let title = if app.state == State::Switching {
        "SWITCH DEVICE"
    } else {
        "SELECT DEVICE"
    };

    // The count, and whether it is still being checked. Rows arriving 1-2s after the
    // list opened have to be accounted for, or a device appearing or vanishing under
    // the cursor reads as a glitch rather than as an answer.
    let count = if app.refreshing {
        format!("{} {} rechecking ", app.spinner(), app.devices.len())
    } else {
        format!("{} devices ", app.devices.len())
    };

    let block = card(title, theme::CYAN)
        .title_top(Line::from(vec![Span::raw(" "), text(count, theme::MUTED)]).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Min(2), Constraint::Length(1)]).split(inner);

    list(frame, rows[0], app, plan);

    // The keys used to sit on the left of this row: `↑↓ move  ⏎ run  Esc cancel`.
    // The footer already carries all three (`↑↓ Move`, `⏎ Launch`, `Esc Cancel`)
    // for exactly these two states, so the row was saying a second time, in a
    // second wording, what the footer says once and everywhere.
    //
    // The ordering note stays. It is not a keybinding but a fact about the list
    // above it, and nothing else on screen accounts for why the top row is the top
    // row.
    //
    // All three ranks, because `last used` alone stopped being the answer: a row the
    // run is on and a row another run holds both outrank it now (8.4), and a note that
    // names one of three reasons explains the top row only by accident.
    frame.render_widget(
        Paragraph::new(
            Line::from(text(
                "promoted to the top: running, in use, last used",
                theme::MUTED,
            ))
            .right_aligned(),
        ),
        rows[1],
    );
}

/// State 3. Booting, possibly for minutes.
pub fn render_booting(frame: &mut Frame, area: Rect, app: &App) {
    let name = if app.boot_name.is_empty() {
        "device"
    } else {
        app.boot_name.as_str()
    };

    // What is actually being waited on, which differs by platform: Android polls
    // a property, a simulator blocks in `bootstatus`.
    //
    // The target first, when this boot is of the device already being run — a retry
    // that found it shut down. The cursor is the right answer only for a first pick,
    // and after a recheck it can be sitting on a row of the other platform.
    let platform = app
        .target
        .as_ref()
        .filter(|device| device.name == app.boot_name)
        .map(|device| device.platform)
        .or_else(|| app.selected().map(|device| device.platform));

    let waiting = match platform {
        Some(crate::data::Platform::Ios) => {
            "  waiting on simctl bootstatus   ·   blocks until ready"
        }
        _ => "  waiting for sys.boot_completed   ·   gives up at 180s",
    };

    let lines = vec![
        Line::from(vec![
            strong(app.spinner(), theme::AMBER),
            text("  Booting ", theme::TEXT),
            strong(name, theme::TEXT),
            text("...", theme::TEXT),
            Span::raw("   "),
            // An elapsed clock, not just a spinner. Android waits on
            // sys.boot_completed for up to 180 seconds; frames alone cannot tell
            // a slow boot from a hung one, and three minutes is long enough for
            // that to matter.
            text(app.boot_clock(), theme::MUTED),
        ]),
        Line::default(),
        Line::from(text(waiting, theme::MUTED)),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// State 5. Exactly one device, so no picker at all.
///
/// This used to repeat the detection result and the device name below the
/// SelectedTargetCard, which said the same thing twice and said it in the wrong
/// order: detection precedes selection, so a "1 device detected" line sitting
/// under the target card inverts the causality. DESIGN.md 3.3 mode 5 is explicit
/// that the flow goes straight to the card, and the card's banner already reads
/// `✔ 1 device active: <name>`.
///
/// What belongs here is what comes next, which in the existing implementation is
/// the launch itself.
pub fn render_single(frame: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            strong(app.spinner(), theme::CYAN),
            text("  Launching Flutter...", theme::TEXT),
        ]),
        Line::default(),
        Line::from(text(
            "  Nothing to choose, so nothing was asked.",
            theme::MUTED,
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// The scrolling target list, shared by states 2 and 4.
fn list(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    // Roomy: one row per target plus a separator, and no blank between them.
    //
    // The separator already divides one target from the next; adding a blank on
    // top of it spent a row per device to say the same thing twice, and pushed
    // a third of the list off screen. Dense drops the separator too, and is the
    // fourth concession in the degradation ladder.
    let step = if plan.roomy_devices { 2 } else { 1 };
    let visible = (area.height / step).max(1) as usize;

    // Keep the selection on screen without letting the window jump around.
    if app.selected_device < app.scroll {
        app.scroll = app.selected_device;
    } else if app.selected_device >= app.scroll + visible {
        app.scroll = app.selected_device + 1 - visible;
    }

    let hits: Vec<Hit> = Vec::new();
    let mut pending = hits;

    // Which row the run is on, and only while switching away from a live one.
    // Outside `Switching` there is no run yet: `app.target` is either empty or, in
    // the mocks, a device that shares an id with a row it has no relationship to.
    //
    // The dead cases are the ones worth spelling out, and there are two. The list can
    // be opened from `Stopped`, where the device is still booted but the app on it is
    // gone; and from `BuildFailed`, where the run never opened at all — a device
    // switched off mid-build lands there, which is how this was found. In both,
    // ` running ` is false and ` ⏎ Keep ` offers to keep nothing. Those rows still
    // carry ` active ` and ` last used `, which is all that is true of them.
    //
    // Asked as `holds_session`, so the two are one question. `!= Stopped` was the
    // earlier spelling and it named the case instead of the property, which is why
    // `BuildFailed` slipped past it.
    let running_id = if app.state == State::Switching && app.run_state().holds_session() {
        app.target.as_ref().map(|device| device.id.clone())
    } else {
        None
    };

    for slot in 0..visible {
        let index = app.scroll + slot;

        let Some(device) = app.devices.get(index) else {
            break;
        };

        let y = area.y + (slot as u16) * step;

        if y >= area.y + area.height {
            break;
        }

        let row = Rect {
            x: area.x,
            y,
            width: area.width.saturating_sub(1),
            height: 1,
        };

        draw_row(
            frame,
            row,
            device,
            index == app.selected_device,
            running_id.as_deref() == Some(device.id.as_str()),
            app.in_use(&device.id),
            &mut pending,
        );

        // Separator between rows, so the last one gets none: a rule sitting
        // directly above the card's bottom border reads as a stray line rather
        // than as a division between two things.
        let is_last = index + 1 == app.devices.len() || slot + 1 == visible;

        if plan.roomy_devices && !is_last && y + 1 < area.y + area.height {
            frame.render_widget(
                Paragraph::new(crate::widgets::separator(row.width)),
                Rect {
                    y: y + 1,
                    height: 1,
                    ..row
                },
            );
        }
    }

    app.hits.extend(pending);

    if app.devices.len() > visible {
        let mut state = ScrollbarState::new(app.devices.len()).position(app.scroll);

        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(theme::BORDER)),
            area,
            &mut state,
        );
    }
}

/// Gap between two things on a row. One value, so the width accounting below
/// cannot disagree with the spans it is accounting for.
const GAP: usize = 2;

const ACTIVE: &str = " active ";
const LAST_USED: &str = " last used ";

/// The row the current run is on, while switching.
///
/// A separate word from `active`, deliberately. `active` means the device is up,
/// which is true of every simulator left booted; this means *your app is running
/// on it*, which is true of exactly one row. One word for both facts would make
/// the list unable to answer the only question it is open to answer.
const RUNNING: &str = " running ";

/// The row another frun is on (8.4).
///
/// A third word, and the reason it is not `running` is the same reason `running` is
/// not `active`: the two say who. `running` is *this* tab and is answered by `⏎ Keep`;
/// this is somebody else's, and `Enter` refuses it. In the switch list both can be on
/// screen at once, on different rows, so one word for both would make the only
/// question the list is open to answer unanswerable.
///
/// Amber, not emerald. Every other chip on a row describes something you can have.
const IN_USE: &str = " in use ";

/// What `Enter` does on the row already running: nothing but close the list.
///
/// The only per-row verb left. `▶ Run` used to sit on every other row and was
/// dropped: once the Start/Run split went, it was the same seven columns repeated
/// down the list saying what the footer says once — `[⏎] Launch` in the picker,
/// `[⏎] Switch` in the switch list. The hint row inside this card was removed for
/// exactly that reason, and this was the same duplication one column to the right.
///
/// This one stays because it is the exception rather than the rule: it is the row
/// where `Enter` does *not* build, and nothing else on the row says so.
const KEEP: &str = " ⏎ Keep ";

/// What a pill costs: its text plus the two cap columns `pill` adds.
fn pill_width(label: &str) -> usize {
    crate::widgets::width(label) + 2
}

fn draw_row(
    frame: &mut Frame,
    area: Rect,
    device: &Device,
    selected: bool,
    running: bool,
    in_use: bool,
    hits: &mut Vec<Hit>,
) {
    let w = crate::widgets::width;

    // Both chips, independently, because they are independent facts.
    //
    // `active` says what Enter costs: launch now, or boot first and wait.
    // `last used` says which device you normally reach for. A device can be both,
    // and suppressing the second then hid the answer to "is the one I usually use
    // the one that is up?" — which is the question the pair exists to answer.
    //
    // `active` requires `needs_boot()`: macOS and Chrome are always available, so
    // `active` would describe a state they do not have.
    //
    // Suppressed on the running row: `running` already says the device is up, and
    // saying it twice in two words on one row is how a list stops being read.
    //
    // Suppressed on a taken row for the same reason and a sharper one. ` in use `
    // implies the device is up — nothing can be running on a device that is not — so
    // the pair would spend two chips on one fact, and the fact `active` is there to
    // carry is that `Enter` launches now. On this row `Enter` does not launch at all.
    let active = !running && !in_use && device.boot.is_none() && device.platform.needs_boot();

    let id = elide(&device.id, 16);

    // Never dropped: the caret, the platform glyph and the name — how you tell this
    // row from the next one — plus ` ⏎ Keep ` on the one row that has it, since a
    // row whose `Enter` behaves differently has to say so.
    let fixed = GAP
        + w(device.platform.glyph())
        + GAP
        + w(&device.name)
        + if running { GAP + pill_width(KEEP) } else { 0 }
        // `spread` keeps at least one column between the two groups.
        + 1;

    // Everything else is charged against what is left, in falling order of what
    // it tells you, because `spread` pads to fit and then silently clips: a row
    // carrying both chips ran off the right edge at 70 columns and took `▶ Run`
    // with it. Same reasoning as the footer's right-hand group in 3.7 — a row that
    // truncates is worse than a short one, since you cannot tell a dropped span
    // from a cut one.
    //
    // Once one span does not fit, nothing after it is drawn either. Letting a
    // later, cheaper span through would print `virtual` on a row whose platform
    // label had just been dropped for want of room.
    let mut room = (area.width as usize).saturating_sub(fixed);
    let mut stopped = false;

    let mut fits = |wanted: bool, cost: usize| {
        if !wanted {
            return false;
        }

        if stopped || cost > room {
            stopped = true;
            return false;
        }

        room -= cost;
        true
    };

    // `running` outranks everything: it is the row the list was opened to move
    // away from, and losing it to a narrow window would leave the frame unable to
    // say where the run currently is.
    let show_running = fits(running, GAP + pill_width(RUNNING));
    // Directly behind it, and ahead of `active` for the reason `active` outranks
    // `last used`, only more so: this is the one row where `Enter` is refused, and a
    // narrow window dropping it would leave a device that looks free and answers
    // nothing. The two are never on the same row — `in_use` excludes this tab's own
    // target — so the order between them is a ranking, not a fight for space.
    let show_in_use = fits(in_use, GAP + pill_width(IN_USE));
    // `active` outranks `last used`: it is the one that changes the consequence
    // of pressing Enter, where a preference costs nothing either way.
    let show_active = fits(active, GAP + pill_width(ACTIVE));
    let show_last_used = fits(device.last_used, GAP + pill_width(LAST_USED));
    // The id is what Flutter and the boot tools actually address, and it is
    // frequently the only way to tell two similarly named targets apart, so it
    // outranks both descriptive tags.
    let show_id = fits(!id.is_empty(), GAP + w(&id));
    let show_label = fits(true, GAP + w(device.platform.label()));
    let show_virtual = fits(device.virtual_device, GAP + w("virtual"));

    let mut left = vec![
        if selected {
            strong("❯ ", theme::CYAN)
        } else {
            Span::raw("  ")
        },
        // Nerd Font, single-width. Emoji would be two cells and break the grid.
        strong(device.platform.glyph(), theme::CYAN),
        Span::raw("  "),
        if selected {
            strong(device.name.as_str(), theme::TEXT)
        } else {
            text(device.name.as_str(), theme::TEXT)
        },
    ];

    if show_running {
        left.push(Span::raw("  "));
        left.extend(pill(RUNNING, theme::CYAN));
    }

    if show_in_use {
        left.push(Span::raw("  "));
        left.extend(pill(IN_USE, theme::AMBER));
    }

    if show_active {
        left.push(Span::raw("  "));
        left.extend(pill(ACTIVE, theme::EMERALD));
    }

    if show_last_used {
        left.push(Span::raw("  "));
        left.extend(pill(LAST_USED, theme::PURPLE));
    }

    let mut right = Vec::new();

    if show_id {
        right.push(Span::raw("  "));
        right.push(text(id, theme::BORDER));
    }

    if show_label {
        right.push(Span::raw("  "));
        right.push(text(device.platform.label(), theme::MUTED));
    }

    if show_virtual {
        right.push(Span::raw("  "));
        right.push(text("virtual", theme::PURPLE));
    }

    // One row can say what `Enter` does, and it is the row where `Enter` does
    // nothing. Every other row shares one answer, and the footer gives it once.
    if running {
        right.push(Span::raw("  "));

        right.extend(pill(
            KEEP,
            if selected { theme::CYAN } else { theme::MUTED },
        ));
    }

    hits.push(Hit {
        area,
        action: Action::StartDevice,
    });

    let line = spread(area.width, left, right);

    let style = if selected {
        Style::new().bg(theme::SURFACE)
    } else {
        Style::new()
    };

    frame.render_widget(Paragraph::new(line).style(style), area);
}
