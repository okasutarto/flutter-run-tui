//! FlutterDeviceManager, DESIGN.md 3.3. States 2 through 5.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{Action, App, Device, Hit};
use crate::theme;
use crate::widgets::{card, elide, pill, spread, strong, text};

/// State 2. Zero devices answered, so offer everything launchable.
pub fn render_bootable(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    let block = card("NO DEVICE RUNNING", theme::AMBER).title_top(
        Line::from(vec![
            Span::raw(" "),
            text(format!("{} targets ", app.devices.len()), theme::MUTED),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(text(
            "Nothing is attached. These can be started:",
            theme::MUTED,
        ))),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(strong("Start a Device", theme::TEXT))),
        rows[1],
    );

    list(frame, rows[2], app, plan, true);

    frame.render_widget(
        Paragraph::new(spread(
            rows[3].width,
            vec![
                strong(format!("{} ", theme::GLYPH_BOLT), theme::AMBER),
                text("Use ↑↓ arrow keys & Enter to launch device", theme::MUTED),
            ],
            vec![
                text("Press ", theme::MUTED),
                strong("Enter", theme::TEXT),
                text(" to launch", theme::MUTED),
            ],
        )),
        rows[3],
    );
}

/// State 4. Two or more answered, so pick one.
pub fn render_picker(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget) {
    let block = card("SELECT TARGET", theme::CYAN).title_top(
        Line::from(vec![
            Span::raw(" "),
            text(format!("{} devices ", app.devices.len()), theme::MUTED),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Min(2), Constraint::Length(1)]).split(inner);

    list(frame, rows[0], app, plan, false);

    frame.render_widget(
        Paragraph::new(spread(
            rows[1].width,
            vec![text("↑↓ move   ⏎ run   Esc cancel", theme::MUTED)],
            vec![text(
                "last used device is promoted to the top",
                theme::MUTED,
            )],
        )),
        rows[1],
    );
}

/// State 3. Booting, possibly for minutes.
pub fn render_booting(frame: &mut Frame, area: Rect, app: &App) {
    let name = app
        .devices
        .get(app.selected_device)
        .map(|d| d.name)
        .unwrap_or("device");

    let lines = vec![
        Line::default(),
        Line::from(vec![
            strong(app.spinner(), theme::AMBER),
            text("  Booting ", theme::TEXT),
            strong(name, theme::TEXT),
            text("...", theme::TEXT),
            // An elapsed clock, not just a spinner. Android waits on
            // sys.boot_completed and the existing implementation gives up at
            // 180 seconds; frames alone cannot tell a slow boot from a hung
            // one, and three minutes is long enough for that to matter.
            text("   42s", theme::MUTED),
        ]),
        Line::default(),
        Line::from(text(
            "  waiting for sys.boot_completed   ·   gives up at 180s",
            theme::MUTED,
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// State 5. Exactly one device, so no picker at all.
pub fn render_single(frame: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::default(),
        Line::from(vec![
            strong("✔ ", theme::EMERALD),
            text("1 device detected, selected automatically", theme::TEXT),
        ]),
        Line::default(),
        Line::from(vec![
            text("  ", theme::MUTED),
            strong(app.target_name, theme::TEXT),
            text("   ", theme::MUTED),
            text(app.target_platform_id, theme::MUTED),
        ]),
        Line::default(),
        Line::from(text(
            "  Nothing to choose, so nothing is asked.",
            theme::MUTED,
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// The scrolling target list, shared by states 2 and 4.
fn list(frame: &mut Frame, area: Rect, app: &mut App, plan: &Budget, show_start: bool) {
    // Roomy: two cell-rows per target plus a separator. Dense is the fourth
    // concession in the degradation ladder.
    let step = if plan.roomy_devices { 3 } else { 1 };
    let visible = (area.height / step).max(1) as usize;

    // Keep the selection on screen without letting the window jump around.
    if app.selected_device < app.scroll {
        app.scroll = app.selected_device;
    } else if app.selected_device >= app.scroll + visible {
        app.scroll = app.selected_device + 1 - visible;
    }

    let hits: Vec<Hit> = Vec::new();
    let mut pending = hits;

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
            show_start,
            &mut pending,
        );

        // Separator under each roomy row, except the last visible one.
        if plan.roomy_devices && y + 2 < area.y + area.height {
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

fn draw_row(
    frame: &mut Frame,
    area: Rect,
    device: &Device,
    selected: bool,
    show_start: bool,
    hits: &mut Vec<Hit>,
) {
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
            strong(device.name, theme::TEXT)
        } else {
            text(device.name, theme::TEXT)
        },
    ];

    if device.last_used {
        left.push(Span::raw("  "));
        left.extend(pill(" last used ", theme::PURPLE));
    }

    // The id is what Flutter and the boot tools actually address, and it is
    // frequently the only way to tell two similarly named targets apart.
    let mut right = vec![
        text(elide(device.id, 16), theme::BORDER),
        Span::raw("  "),
        text(device.platform.label(), theme::MUTED),
    ];

    if device.virtual_device {
        right.push(Span::raw("  "));
        right.push(text("virtual", theme::PURPLE));
    }

    if show_start {
        right.push(Span::raw("  "));

        // Desktop and web are always available, so they have nothing to boot.
        // The label says which is happening rather than pretending both are
        // the same operation.
        let label = if device.platform.needs_boot() {
            " ▶ Start "
        } else {
            " ▶ Run "
        };

        right.extend(pill(
            label,
            if selected { theme::CYAN } else { theme::MUTED },
        ));

        hits.push(Hit {
            area,
            action: Action::StartDevice,
        });
    }

    let line = spread(area.width, left, right);

    let style = if selected {
        Style::new().bg(theme::SURFACE)
    } else {
        Style::new()
    };

    frame.render_widget(Paragraph::new(line).style(style), area);
}
