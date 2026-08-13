//! ProjectCard, DESIGN.md 3.1.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme;
use crate::ui::logo::Logo;
use crate::widgets::{card, elide, field, pill, separator, spread, strong, text};

/// Widest the branch pill may grow before it is elided.
///
/// A width cap on the metadata block itself used to live here, which is what
/// stopped this card responding to the terminal: the border stretched to the
/// window while the values stopped at column 44, and the row separators stopped
/// with them, hanging in the middle of the card. Values now right-align to the
/// card border, which is also what the design frames show.
const BRANCH_MAX: usize = 30;

/// Size of the logo artwork box, inside the wider left column.
const ART_W: u16 = 11;
const ART_H: u16 = 5;

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget, art: &mut Logo) {
    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let block = card("PROJECT INFO", theme::CYAN).title_top(
        Line::from(vec![
            Span::raw(" "),
            text(app.cwd, theme::MUTED),
            Span::raw("  "),
            text("[COPY]", theme::MUTED),
            Span::raw(" "),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let body = if plan.logo {
        // 20 columns, not 22: wide enough for `Cross-Platform CLI` at 18 cells
        // and nothing more. The artwork itself is narrower still, see `logo`.
        let split = Layout::horizontal([Constraint::Length(20), Constraint::Min(30)]).split(inner);

        logo(frame, split[0], art);
        split[1]
    } else {
        inner
    };

    // Full body width: values right-align to the card border and the row
    // separators span the whole block, both of which follow the terminal.
    let w = body.width;

    // Bound before the lines so the pill can borrow it. Elided because a real
    // branch name is not the tidy one in the mockup:
    // `feature/PROJ-4821-refactor-checkout-payment-sheet` is 48 characters and
    // ran the pill past the card, taking its closing cap with it.
    let branch = format!(" {} ", elide(app.branch, BRANCH_MAX));

    let git = if app.git_clean {
        pill(" ✔ Clean ", theme::EMERALD)
    } else {
        pill(" ● 3 changed ", theme::AMBER)
    };

    let mut lines = vec![
        field(
            w,
            "Project",
            pill(format!(" {} ", app.project), theme::CYAN),
        ),
        maybe_sep(w, plan),
        field(w, "Version", vec![text(app.version, theme::TEXT)]),
        maybe_sep(w, plan),
        field(w, "Branch", pill(branch, theme::AMBER)),
        maybe_sep(w, plan),
        field(w, "Git Status", git),
    ];

    lines.retain(|l| !l.spans.is_empty() || plan.separators);

    // Metadata fills its own region, with the stats row in a separate one below.
    //
    // It has to be its own Rect rather than another entry in `lines`, because a
    // three-column row cannot be expressed as one Line: three groups spaced
    // across a width need three areas.
    let meta_h = if plan.separators { 7 } else { 4 };

    let rows = Layout::vertical([
        Constraint::Length(meta_h),
        Constraint::Length(1), // blank
        Constraint::Length(1), // stats
    ])
    .split(body);

    frame.render_widget(Paragraph::new(lines), rows[0]);

    stats(frame, rows[2], app);
}

/// The three-column technical stats row from DESIGN.md 3.1.
///
/// Spaced across the width rather than grouped: three equal columns, the first
/// flush left, the last flush right. Grouping them left left two thirds of the
/// row empty and made the block look unfinished; spreading them as one Line
/// pushed `Runtime (FVM)` to the far border and separated it from the two values
/// it belongs with. Three columns is the shape the design asked for and it
/// solves both.
fn stats(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

    let groups = [
        (
            vec![
                text("Flutter ", theme::MUTED),
                strong(app.flutter, theme::CYAN),
            ],
            0,
        ),
        (
            vec![text("Dart ", theme::MUTED), strong(app.dart, theme::PURPLE)],
            1,
        ),
        (
            vec![
                text("Runtime ", theme::MUTED),
                strong("(FVM)", theme::PURPLE),
            ],
            2,
        ),
    ];

    for (spans, i) in groups {
        let line = Line::from(spans);

        let line = match i {
            0 => line,
            1 => line.centered(),
            _ => line.right_aligned(),
        };

        frame.render_widget(Paragraph::new(line), cols[i]);
    }
}

/// Empty line when separators have been conceded, so the row indices stay put.
fn maybe_sep(w: u16, plan: &Budget) -> Line<'static> {
    if plan.separators {
        separator(w)
    } else {
        Line::default()
    }
}

/// Degradation step 6: everything above in one row.
///
/// Nothing here changes during a run, so this is the honest shape once rows are
/// scarce.
fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let git = if app.git_clean {
        "✔ clean"
    } else {
        "● dirty"
    };

    let line = spread(
        area.width,
        vec![
            strong(app.project, theme::TEXT),
            Span::raw(" "),
            text(app.version, theme::MUTED),
            Span::raw("  "),
            text(elide(app.branch, 28), theme::AMBER),
            Span::raw("  "),
            text(git, theme::EMERALD),
        ],
        vec![
            text("Flutter ", theme::MUTED),
            text(app.flutter, theme::TEXT),
            text("  Dart ", theme::MUTED),
            text(app.dart, theme::TEXT),
            text("  FVM", theme::MUTED),
        ],
    );

    frame.render_widget(Paragraph::new(line), area);
}

/// The Flutter mark, drawn by `logo::Logo` through a real graphics protocol.
///
/// Nine rows, which is what `Budget::LOGO_H` charges for: seven for the artwork,
/// a blank, and the wordmark. The two label lines the design shows cost a row
/// each and have to be paid for in the budget, or the card clips them in
/// silence.
fn logo(frame: &mut Frame, area: Rect, art: &mut Logo) {
    // Centred on both axes inside the left column.
    //
    // `Fill` on both sides, not one. A single leading Fill absorbs the whole
    // remainder and pins the artwork to the opposite edge, which is how the mark
    // ended up sitting on the floor of the column. Two Fills of equal weight
    // split the slack, and get an odd remainder right without a special case.
    let rows = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(ART_H),
        Constraint::Fill(1),
    ])
    .split(area);

    // `Resize::Fit` scales to whichever dimension binds first, so at five rows
    // the height binds and any extra width goes unused. Bounding the box makes
    // the size deliberate, and the Fill columns centre it.
    let art_cols = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(ART_W),
        Constraint::Fill(1),
    ])
    .split(rows[1]);

    art.render(frame, art_cols[1]);
}
