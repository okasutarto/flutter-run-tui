//! ProjectCard, DESIGN.md 3.1.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme::Theme;
use crate::ui::logo::Logo;
use crate::widgets::{card, elide, field, spread, strong, text};

/// Widest the branch pill may grow before it is elided.
const BRANCH_MAX: usize = 30;

/// Size of the logo artwork box, inside the wider left column.
const ART_W: u16 = 7;
const ART_H: u16 = 3;

/// The wordmark under the artwork.
const LABEL: &str = "flutter-run-tui";
const LABEL_W: u16 = LABEL.len() as u16;

/// Content width of the logo column: whichever of the artwork and its wordmark is wider.
const LOGO_W: u16 = if ART_W > LABEL_W { ART_W } else { LABEL_W };

/// Blank columns either side of the logo block, measured from the card border.
const GUTTER: u16 = 3;

/// The card's own horizontal padding, which `card()` has already taken out of `inner`.
const CARD_PAD: u16 = 1;

/// Total width the logo column claims out of the card body.
const LOGO_COL: u16 = (GUTTER - CARD_PAD) + LOGO_W + GUTTER;

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget, art: &mut Logo) {
    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let block = card("Project", app.theme.purple, &app.theme).title_top(
        Line::from(vec![
            Span::raw(" "),
            text(app.cwd.as_str(), app.theme.muted),
            Span::raw(" "),
        ])
        .right_aligned(),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let body = if inner.width >= LOGO_COL + 30 {
        let split =
            Layout::horizontal([Constraint::Length(LOGO_COL), Constraint::Min(30)]).split(inner);

        logo(frame, split[0], art, &app.theme);
        split[1]
    } else {
        inner
    };

    let w = body.width;

    let branch = vec![strong(elide(&app.branch, BRANCH_MAX), app.theme.amber)];

    let git = match app.dirty {
        0 => vec![strong("✔ clean", app.theme.emerald)],
        1 => vec![strong("● 1 changed", app.theme.rose)],
        n => vec![strong(format!("● {n} changed"), app.theme.rose)],
    };

    let project = vec![
        strong(app.project.as_str(), app.theme.cyan),
        Span::raw("  "),
        strong(app.version.as_str(), app.theme.muted),
    ];

    let lines = vec![
        field(w, "Name", project, &app.theme),
        field(w, "Branch", branch, &app.theme),
        field(w, "Git Status", git, &app.theme),
    ];

    let rows = Layout::vertical([
        Constraint::Length(3), // metadata: consecutive compact rows
        Constraint::Length(1), // stats: Flutter · Dart · Runtime
    ])
    .split(body);

    frame.render_widget(Paragraph::new(lines), rows[0]);

    stats(frame, rows[1], app);
}

/// The technical stats row from DESIGN.md 3.1.
fn stats(frame: &mut Frame, area: Rect, app: &App) {
    let line = Line::from(vec![
        strong("Flutter ", app.theme.muted),
        strong(app.flutter.as_str(), app.theme.cyan),
        text("  ·  ", app.theme.muted),
        strong("Dart ", app.theme.muted),
        strong(app.dart.as_str(), app.theme.purple),
        text("  ·  ", app.theme.muted),
        strong("Runtime ", app.theme.muted),
        strong(app.toolchain.label(), app.theme.text),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}



/// Degradation step 6: everything above in one row.
fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let (git, git_color) = if app.dirty == 0 {
        ("✔ clean".to_string(), app.theme.emerald)
    } else {
        (format!("● {} changed", app.dirty), app.theme.amber)
    };

    let line = spread(
        area.width,
        vec![
            strong(app.project.as_str(), app.theme.text),
            Span::raw(" "),
            text(app.version.as_str(), app.theme.muted),
            Span::raw("  "),
            text(elide(&app.branch, 28), app.theme.amber),
            Span::raw("  "),
            text(git, git_color),
        ],
        vec![
            text("Flutter ", app.theme.muted),
            text(app.flutter.as_str(), app.theme.text),
            text("  Dart ", app.theme.muted),
            text(app.dart.as_str(), app.theme.text),
            text(format!("  {}", app.toolchain.label()), app.theme.muted),
        ],
    );

    frame.render_widget(Paragraph::new(line), area);
}

fn logo(frame: &mut Frame, area: Rect, art: &mut Logo, theme: &Theme) {
    let cols = Layout::horizontal([
        Constraint::Length(GUTTER - CARD_PAD),
        Constraint::Length(LOGO_W),
        Constraint::Fill(1),
    ])
    .split(area);

    let rows = Layout::vertical([
        Constraint::Length(ART_H), // 3 rows: height matches Name, Branch, Git Status
        Constraint::Length(1),     // 1 row wordmark: matches stats row
    ])
    .split(cols[1]);

    let art_cols = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(ART_W),
        Constraint::Fill(1),
    ])
    .split(rows[0]);

    art.render(frame, art_cols[1]);

    frame.render_widget(
        Paragraph::new(Line::from(strong(LABEL, theme.muted)).centered()),
        rows[1],
    );
}
