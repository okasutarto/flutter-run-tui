//! ProjectCard, DESIGN.md 3.1.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme;
use crate::widgets::{card, elide, field, pill, separator, spread, strong, text};

/// Widest a label/value pair may span.
///
/// Right-aligning a value against the full card width only reads as alignment
/// while the card is narrow. Past this the gap becomes two unrelated columns:
/// `project` on the far left and `cwclub` sixty columns away.
const META_W: u16 = 44;

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
    if !plan.full_cards {
        collapsed(frame, area, app);
        return;
    }

    let block = card("PROJECT INFO", theme::CYAN)
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                text(app.cwd, theme::MUTED),
                Span::raw("  "),
                text("[COPY]", theme::MUTED),
                Span::raw(" "),
            ])
            .right_aligned(),
        )
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let body = if plan.logo {
        let split = Layout::horizontal([Constraint::Length(22), Constraint::Min(30)]).split(inner);

        logo(frame, split[0]);
        split[1]
    } else {
        inner
    };

    let w = body.width.min(META_W);

    // Bound before the lines so the pill can borrow it. Elided because a real
    // branch name is not the tidy one in the mockup:
    // `feature/PROJ-4821-refactor-checkout-payment-sheet` is 48 characters and
    // ran the pill past the card, taking its closing cap with it.
    let branch = format!(" {} ", elide(app.branch, META_W as usize - 14));

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

    // The three-column stats row from 3.1, spread across the same width so it
    // lines up with the fields above rather than floating free.
    lines.push(Line::default());
    lines.push(spread(
        w,
        vec![
            text("Flutter ", theme::MUTED),
            strong(app.flutter, theme::CYAN),
            text("   Dart ", theme::MUTED),
            strong(app.dart, theme::PURPLE),
        ],
        vec![
            text("Runtime ", theme::MUTED),
            strong("(FVM)", theme::PURPLE),
        ],
    ));

    frame.render_widget(Paragraph::new(lines), body);
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

/// Placeholder mark, not the Flutter logo.
///
/// An earlier attempt drew the real logo in half-blocks and came out as an
/// unreadable blob, which is worse than not trying because it looks like a
/// rendering fault. The actual fix is `ratatui-image` pointed at
/// `assets/flutter-trim.png` through the kitty graphics protocol, which needs
/// the `image` crate and a terminal capability query.
fn logo(frame: &mut Frame, area: Rect) {
    let art = vec![
        Line::default(),
        Line::from(text("      ▄▄██", theme::CYAN)),
        Line::from(text("    ▄▄██▀", theme::CYAN)),
        Line::from(text("  ▄▄██▀", theme::CYAN)),
        Line::from(text(" ▄██▀", theme::CYAN)),
        Line::from(text(" ▀██▄", theme::CYAN)),
        Line::from(text("   ▀▀██▄▄", theme::CYAN)),
        Line::from(text("      ▀▀██", theme::CYAN)),
        Line::default(),
        Line::from(strong("  Flutter Engine", theme::CYAN)),
        Line::from(text("  Cross-Platform CLI", theme::MUTED)),
    ];

    frame.render_widget(Paragraph::new(art), area);
}
