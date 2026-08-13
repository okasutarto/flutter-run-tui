//! Frame assembly. One module per component, this file decides what appears.

mod build;
mod chrome;
mod devices;
pub mod logo;
mod logs;
mod project;
mod target;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::{self, Budget};
use crate::data::{App, State};
use crate::theme;
use crate::widgets::{strong, text};

pub fn render(frame: &mut Frame, app: &mut App, art: &mut logo::Logo) {
    let full = frame.area();

    // Rebuilt every frame. A stale hit list leaves invisible buttons behind at
    // the coordinates a card occupied before the layout degraded.
    app.hits.clear();

    if full.width < budget::MIN_W || full.height < budget::MIN_H {
        too_small(frame, full);
        return;
    }

    let area = budget::clamp_width(full);
    let plan = Budget::solve(area, app.state);

    // Zoomed: the middle region and the footer, nothing else. The Budget is
    // bypassed rather than taught about this, because there is no ladder to solve
    // when only one block is on screen.
    //
    // Width is deliberately `full` and not the clamped area: the 142-column cap
    // exists so a card cannot stretch a label to one edge and its value to the
    // other, and there are no cards here. More columns means fewer wrapped rows.
    if app.zoom && app.state.has_logs() {
        let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)])
            .spacing(1)
            .split(full);

        logs::render(frame, rows[0], app);
        chrome::footer(frame, rows[1], app, &plan);

        return;
    }

    // Vertical stack, built from the same height functions the budget used to
    // decide the ladder.
    let mut rows = vec![Constraint::Length(plan.project_h())];

    if app.state.has_target() {
        rows.push(Constraint::Length(plan.target_h()));
    }

    if app.state.has_build() {
        rows.push(Constraint::Length(plan.build_h(app.state)));
    }

    // The flexible middle: device list, failure card, or log stream depending
    // on the state. Everything else is fixed.
    rows.push(Constraint::Min(3));

    rows.push(Constraint::Length(1));

    let chunks = Layout::vertical(rows).spacing(1).split(area);
    let mut i = 0;

    project::render(frame, chunks[i], app, &plan, art);
    i += 1;

    if app.state.has_target() {
        target::render(frame, chunks[i], app, &plan);
        i += 1;
    }

    if app.state.has_build() {
        build::render(frame, chunks[i], app, &plan);
        i += 1;
    }

    let middle = chunks[i];
    i += 1;

    // No prompt bar between the middle and the footer. It was three rows plus a
    // separating gap, spent on a command line with nothing to command: Flutter
    // reads single keys, so a typed string could not be forwarded (`quit` would
    // have sent `q` and quit), and every key it could have offered is already in
    // the footer. Those four rows are the log window's.

    match app.state {
        State::Detecting => detecting(frame, middle, app),
        State::NoDevices => devices::render_bootable(frame, middle, app, &plan),
        State::Booting => devices::render_booting(frame, middle, app),
        State::MultipleDevices => devices::render_picker(frame, middle, app, &plan),
        State::SingleDevice => devices::render_single(frame, middle, app),
        State::BuildFailed => build::render_failure(frame, middle, app),

        // Building included: Flutter is printing during the build and that output
        // is what fills the long gap before the first stage after `Launching`.
        State::Building
        | State::Running
        | State::ReloadInFlight
        | State::ReloadFailed
        | State::ReloadDropped => logs::render(frame, middle, app),
    }

    chrome::footer(frame, chunks[i], app, &plan);
}

fn too_small(frame: &mut Frame, area: Rect) {
    let msg = Paragraph::new(vec![
        Line::from(strong("terminal too small", theme::ROSE)),
        Line::from(text(
            format!("needs at least {}x{}", budget::MIN_W, budget::MIN_H),
            theme::MUTED,
        )),
    ])
    .centered();

    frame.render_widget(msg, area);
}

/// State 1. Deliberately sparse: nothing is known yet, and inventing content to
/// fill the space would be inventing content.
fn detecting(frame: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            strong(app.spinner(), theme::CYAN),
            text("  Detecting Flutter devices...", theme::TEXT),
        ]),
        Line::default(),
        Line::from(text("  fvm flutter devices --machine", theme::MUTED)),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}


