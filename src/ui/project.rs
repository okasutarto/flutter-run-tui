//! ProjectCard, DESIGN.md 3.1.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::App;
use crate::theme;
use crate::widgets::{card, elide, field, pill, separator, spread, strong, text};

/// Widest the branch pill may grow before it is elided.
///
/// A width cap on the metadata block itself used to live here, which is what
/// stopped this card responding to the terminal: the border stretched to the
/// window while the values stopped at column 44, and the row separators stopped
/// with them, hanging in the middle of the card. Values now right-align to the
/// card border, which is also what the design frames show.
const BRANCH_MAX: usize = 30;

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
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
        let split = Layout::horizontal([Constraint::Length(22), Constraint::Min(30)]).split(inner);

        logo(frame, split[0]);
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

    // The three-column stats row from 3.1, kept as one left-aligned group.
    //
    // Not spread: the fields above are label/value pairs, where the gap is the
    // alignment. These three are a single fact about the toolchain, so pushing
    // `Runtime (FVM)` to the far border would separate it from the two values it
    // belongs with.
    lines.push(Line::default());
    lines.push(Line::from(vec![
        text("Flutter ", theme::MUTED),
        strong(app.flutter, theme::CYAN),
        text("   Dart ", theme::MUTED),
        strong(app.dart, theme::PURPLE),
        text("   Runtime ", theme::MUTED),
        strong("(FVM)", theme::PURPLE),
    ]));

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
    // Exactly nine rows, which is what `Budget::LOGO_H` charges for. The
    // leading blank the earlier version had is now supplied by the card's title
    // gap, and keeping both pushed the two label lines off the bottom.
    let art = vec![
        Line::from(text("      ▄▄██", theme::CYAN)),
        Line::from(text("    ▄▄██▀", theme::CYAN)),
        Line::from(text("  ▄▄██▀", theme::CYAN)),
        Line::from(text(" ▄██▀", theme::CYAN)),
        Line::from(text(" ▀██▄", theme::CYAN)),
        Line::from(text("   ▀▀██▄▄", theme::CYAN)),
        Line::from(text("      ▀▀██", theme::CYAN)),
        Line::default(),
        Line::from(strong("  Flutter Engine", theme::CYAN)),
        Line::default(),
        Line::from(text("      ▀▀██", theme::CYAN)),
    ];

    frame.render_widget(Paragraph::new(art), area);
}
