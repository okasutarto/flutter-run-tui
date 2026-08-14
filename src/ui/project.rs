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

/// The wordmark under the artwork.
///
/// The mark alone says Flutter, not which tool is drawing it, which matters on a
/// screenshot and in a tmux pane full of other Flutter output.
const LABEL: &str = "flutter-run-tui";
const LABEL_W: u16 = LABEL.len() as u16;

/// Content width of the logo column: whichever of the artwork and its wordmark
/// is wider.
///
/// Taken from the label rather than fixed at `ART_W`, because the wordmark is
/// four columns wider than the artwork and a column sized to the artwork would
/// clip it — silently, which is how the target card lost fields before (7.5).
const LOGO_W: u16 = if ART_W > LABEL_W { ART_W } else { LABEL_W };

/// Blank columns either side of the logo block, measured from the card border.
///
/// Both sides, which is the point: the gap between the logo and the metadata
/// beside it is the same as the gap between the logo and the card edge, so the
/// column reads as centred rather than shoved against the border. It was one
/// column on the right against three on the left, because the slack was left to
/// two `Fill`s and whatever they rounded to.
const GUTTER: u16 = 3;

/// The card's own horizontal padding, which `card()` has already taken out of
/// `inner`.
///
/// Subtracted from the left gutter rather than ignored: the padding is blank
/// space that counts toward the gap the eye sees, so charging `GUTTER` again on
/// top of it is what would put the logo off-centre in the other direction.
const CARD_PAD: u16 = 1;

/// Total width the logo column claims out of the card body.
const LOGO_COL: u16 = (GUTTER - CARD_PAD) + LOGO_W + GUTTER;

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget, art: &mut Logo) {
    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let block = card("PROJECT INFO", theme::CYAN).title_top(
        Line::from(vec![
            Span::raw(" "),
            text(app.cwd.as_str(), theme::MUTED),
            Span::raw("  "),
            text("[COPY]", theme::MUTED),
            Span::raw(" "),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The logo column is not optional and is not in the degradation ladder. It
    // occupies the same rows as the metadata beside it, so dropping it reclaimed
    // nothing — see `Budget::project_h`. It goes when the whole card collapses,
    // which is the branch above.
    //
    // Sized to the wordmark plus a symmetric gutter, nothing more. It was 20
    // columns to fit an 18-cell `Cross-Platform CLI` subtitle; that label is
    // gone, so the columns it was holding belong to the metadata.
    //
    // Dropped on a narrow window, where the artwork column is the difference
    // between a value fitting and being elided.
    let body = if inner.width >= LOGO_COL + 30 {
        let split =
            Layout::horizontal([Constraint::Length(LOGO_COL), Constraint::Min(30)]).split(inner);

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
    let branch = format!(" {} ", elide(&app.branch, BRANCH_MAX));

    // The count, not just the fact. "3 changed" is what `git status --porcelain`
    // actually answered, and it is the difference between a stray formatting
    // change and a working tree you are about to lose track of.
    let git = match app.dirty {
        0 => pill(" ✔ Clean ", theme::EMERALD),
        1 => pill(" ● 1 changed ", theme::AMBER),
        n => pill(format!(" ● {n} changed "), theme::AMBER),
    };

    let mut lines = vec![
        field(
            w,
            "Project",
            pill(format!(" {} ", app.project), theme::CYAN),
        ),
        maybe_sep(w, plan),
        field(w, "Version", vec![text(app.version.as_str(), theme::TEXT)]),
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
                strong(app.flutter.as_str(), theme::CYAN),
            ],
            0,
        ),
        (
            vec![
                text("Dart ", theme::MUTED),
                strong(app.dart.as_str(), theme::PURPLE),
            ],
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
    let (git, git_color) = if app.dirty == 0 {
        ("✔ clean".to_string(), theme::EMERALD)
    } else {
        (format!("● {} changed", app.dirty), theme::AMBER)
    };

    let line = spread(
        area.width,
        vec![
            strong(app.project.as_str(), theme::TEXT),
            Span::raw(" "),
            text(app.version.as_str(), theme::MUTED),
            Span::raw("  "),
            text(elide(&app.branch, 28), theme::AMBER),
            Span::raw("  "),
            text(git, git_color),
        ],
        vec![
            text("Flutter ", theme::MUTED),
            text(app.flutter.as_str(), theme::TEXT),
            text("  Dart ", theme::MUTED),
            text(app.dart.as_str(), theme::TEXT),
            text("  FVM", theme::MUTED),
        ],
    );

    frame.render_widget(Paragraph::new(line), area);
}

/// The Flutter mark, drawn by `logo::Logo` through a real graphics protocol, with
/// the tool's wordmark under it.
///
/// Seven rows: five of artwork, a blank, and the wordmark. Not charged in
/// `Budget`, and it does not need to be — the metadata beside it is never shorter
/// than six rows, so the card's body height is still the metadata's.
fn logo(frame: &mut Frame, area: Rect, art: &mut Logo) {
    // Explicit gutters rather than `Fill` on both sides. The artwork and the
    // label are centred inside the same content column, which keeps them on one
    // axis, and the column's own margins are stated so the gap to the metadata
    // matches the gap to the card edge instead of being whatever the layout
    // rounded to.
    let cols = Layout::horizontal([
        Constraint::Length(GUTTER - CARD_PAD),
        Constraint::Length(LOGO_W),
        Constraint::Fill(1),
    ])
    .split(area);

    // A blank row between the mark and the wordmark, conceded when the column is
    // too short to hold one.
    //
    // Not unconditional: with the separators given up the card's body is six
    // rows, and a fixed gap makes the block seven. Over-constraining a vertical
    // Layout does not error, it squeezes — and what gets squeezed to nothing is
    // the wordmark, so the gap would cost the very thing it is spacing.
    let gap = u16::from(area.height >= ART_H + 2);

    // Centred vertically, with the label row travelling with the artwork.
    //
    // `Fill` on both sides, not one. A single leading Fill absorbs the whole
    // remainder and pins the artwork to the opposite edge, which is how the mark
    // ended up sitting on the floor of the column. Two Fills of equal weight
    // split the slack, and get an odd remainder right without a special case.
    let rows = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(ART_H),
        Constraint::Length(gap),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(cols[1]);

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

    frame.render_widget(
        Paragraph::new(Line::from(text(LABEL, theme::MUTED)).centered()),
        rows[3],
    );
}
