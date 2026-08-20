//! ProjectCard, DESIGN.md 3.1.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme::Theme;
use crate::ui::logo::Logo;
use crate::widgets::{card, elide, field, pill, separator, spread, strong, text};

/// Widest the branch pill may grow before it is elided.
const BRANCH_MAX: usize = 30;

/// Size of the logo artwork box, inside the wider left column.
const ART_W: u16 = 11;
const ART_H: u16 = 5;

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

    let block = card("PROJECT INFO", app.theme.purple, &app.theme).title_top(
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

    let branch = format!(" {} ", elide(&app.branch, BRANCH_MAX));

    let git = match app.dirty {
        0 => pill(" ✔ Clean ", app.theme.emerald, &app.theme),
        1 => pill(" ● 1 changed ", app.theme.rose, &app.theme),
        n => pill(format!(" ● {n} changed "), app.theme.rose, &app.theme),
    };

    let mut project = pill(format!(" {} ", app.project), app.theme.cyan, &app.theme);

    project.push(text(format!("  {}", app.version), app.theme.text));

    let mut lines = vec![
        field(w, "Project", project, &app.theme),
        maybe_sep(w, plan, &app.theme),
        field(w, "Branch", pill(branch, app.theme.amber, &app.theme), &app.theme),
        maybe_sep(w, plan, &app.theme),
        field(w, "Git Status", git, &app.theme),
    ];

    lines.retain(|l| !l.spans.is_empty() || plan.separators);

    let meta_h = if plan.separators { 5 } else { 3 };

    let rows = Layout::vertical([
        Constraint::Length(meta_h),
        Constraint::Length(1), // blank
        Constraint::Length(1), // stats
    ])
    .split(body);

    frame.render_widget(Paragraph::new(lines), rows[0]);

    stats(frame, rows[2], app);
}

/// The technical stats row from DESIGN.md 3.1.
fn stats(frame: &mut Frame, area: Rect, app: &App) {
    let line = Line::from(vec![
        text("Flutter ", app.theme.muted),
        strong(app.flutter.as_str(), app.theme.cyan),
        text("  ·  ", app.theme.muted),
        text("Dart ", app.theme.muted),
        strong(app.dart.as_str(), app.theme.purple),
        text("  ·  ", app.theme.muted),
        text("Runtime ", app.theme.muted),
        strong(format!("({})", app.toolchain.label()), app.theme.purple),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

/// Empty line when separators have been conceded, so the row indices stay put.
fn maybe_sep(w: u16, plan: &Budget, theme: &Theme) -> Line<'static> {
    if plan.separators {
        separator(w, theme)
    } else {
        Line::default()
    }
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

    let gap = u16::from(area.height >= ART_H + 2);

    let rows = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(ART_H),
        Constraint::Length(gap),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(cols[1]);

    let art_cols = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(ART_W),
        Constraint::Fill(1),
    ])
    .split(rows[1]);

    art.render(frame, art_cols[1]);

    frame.render_widget(
        Paragraph::new(Line::from(text(LABEL, theme.muted)).centered()),
        rows[3],
    );
}
