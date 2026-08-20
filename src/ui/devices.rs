//! FlutterDeviceManager, DESIGN.md 3.3. States 2 through 5.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Device, Hit, State};
use crate::theme::Theme;
use crate::widgets::{card, elide, pill, spread, strong, text};
use crate::probe::Platform;

/// State 4. Two or more answered, so pick one.
pub fn render_picker(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    let title = if app.state == State::Switching {
        "SWITCH DEVICE"
    } else {
        "SELECT DEVICE"
    };

    let count = if app.refreshing {
        format!("{} {} rechecking ", app.spinner(), app.devices.len())
    } else {
        format!("{} devices ", app.devices.len())
    };

    let block = card(title, app.theme.purple, &app.theme)
        .title_top(Line::from(vec![Span::raw(" "), text(count, app.theme.muted)]).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Min(2), Constraint::Length(1)]).split(inner);

    list(frame, rows[0], app, plan);

    frame.render_widget(
        Paragraph::new(
            Line::from(text(
                "promoted to the top: running, in use, last used",
                app.theme.muted,
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
            strong(app.spinner(), app.theme.amber),
            text("  Booting ", app.theme.text),
            strong(name, app.theme.text),
            text("...", app.theme.text),
            Span::raw("   "),
            text(app.boot_clock(), app.theme.muted),
        ]),
        Line::default(),
        Line::from(text(waiting, app.theme.muted)),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// State 5. Exactly one device, so no picker at all.
pub fn render_single(frame: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            strong(app.spinner(), app.theme.cyan),
            text("  Launching Flutter...", app.theme.text),
        ]),
        Line::default(),
        Line::from(text(
            "  Nothing to choose, so nothing was asked.",
            app.theme.muted,
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// The scrolling target list, shared by states 2 and 4.
fn list(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    let step = if plan.roomy_devices { 2 } else { 1 };
    let visible = (area.height / step).max(1) as usize;

    if app.selected_device < app.scroll {
        app.scroll = app.selected_device;
    } else if app.selected_device >= app.scroll + visible {
        app.scroll = app.selected_device + 1 - visible;
    }

    let hits: Vec<Hit> = Vec::new();
    let mut pending = hits;

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
            &app.theme,
        );

        let is_last = index + 1 == app.devices.len() || slot + 1 == visible;

        if plan.roomy_devices && !is_last && y + 1 < area.y + area.height {
            frame.render_widget(
                Paragraph::new(crate::widgets::separator(row.width, &app.theme)),
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
                .style(Style::new().fg(app.theme.border)),
            area,
            &mut state,
        );
    }
}

const GAP: usize = 2;

const ACTIVE: &str = " active ";
const LAST_USED: &str = " last used ";
const RUNNING: &str = " running ";
const IN_USE: &str = " in use ";
const KEEP: &str = " ⏎ Keep ";

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
    theme: &Theme,
) {
    let w = crate::widgets::width;

    let active = !running && !in_use && device.boot.is_none() && device.platform.needs_boot();

    let id = elide(&device.id, 16);

    let fixed = GAP
        + w(device.platform.glyph())
        + GAP
        + w(&device.name)
        + if running { GAP + pill_width(KEEP) } else { 0 }
        + 1;

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

    let show_running = fits(running, GAP + pill_width(RUNNING));
    let show_in_use = fits(in_use, GAP + pill_width(IN_USE));
    let show_active = fits(active, GAP + pill_width(ACTIVE));
    let show_last_used = fits(device.last_used, GAP + pill_width(LAST_USED));
    let show_id = fits(!id.is_empty(), GAP + w(&id));
    let show_label = fits(true, GAP + w(device.platform.label()));

    let device_kind = match (&device.platform, device.virtual_device) {
        (Platform::Ios, true) => "Simulator",
        (Platform::Android, true) => "Emulator",
        (Platform::Ios, false) | (Platform::Android, false) => "Hardware",
        (Platform::Desktop, _) => "Desktop",
        (Platform::Web, _) => "Web",
    };

    let show_device_kind = fits(true, GAP + w(device_kind));

    let mut left = vec![
        if selected {
            strong("❯ ", theme.cyan)
        } else {
            Span::raw("  ")
        },
        strong(device.platform.glyph(), platform_color(device.platform, theme)),
        Span::raw("  "),
        if selected {
            strong(device.name.as_str(), theme.text)
        } else {
            text(device.name.as_str(), theme.text)
        },
    ];

    if show_running {
        left.push(Span::raw("  "));
        left.extend(pill(RUNNING, theme.emerald, theme));
    }

    if show_in_use {
        left.push(Span::raw("  "));
        left.extend(pill(IN_USE, theme.amber, theme));
    }

    if show_active {
        left.push(Span::raw("  "));
        left.extend(pill(ACTIVE, theme.cyan, theme));
    }

    if show_last_used {
        left.push(Span::raw("  "));
        left.extend(pill(LAST_USED, theme.purple, theme));
    }

    let mut right = Vec::new();

    if show_id {
        right.push(Span::raw("  "));
        right.push(text(id, theme.amber));
    }

    if show_label {
        right.push(Span::raw("  "));
        right.push(text(device.platform.label(), theme.muted));
    }

    if show_device_kind {
        right.push(Span::raw("  "));
        right.push(text(device_kind, theme.muted));
    }

    if running {
        right.push(Span::raw("  "));

        right.extend(pill(
            KEEP,
            if selected { theme.cyan } else { theme.muted },
            theme,
        ));
    }

    hits.push(Hit {
        area,
        action: Action::StartDevice,
    });

    let line = spread(area.width, left, right);

    let style = if selected {
        Style::new().bg(theme.surface)
    } else {
        Style::new()
    };

    frame.render_widget(Paragraph::new(line).style(style), area);
}

/// Platform identity colours leave cyan for focus and structure.
fn platform_color(platform: crate::data::Platform, theme: &Theme) -> ratatui::style::Color {
    match platform {
        crate::data::Platform::Android => theme.emerald,
        crate::data::Platform::Ios => theme.text,
        crate::data::Platform::Desktop => theme.amber,
        crate::data::Platform::Web => theme.purple,
    }
}
