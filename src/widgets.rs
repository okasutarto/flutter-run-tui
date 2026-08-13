//! Small composition helpers shared by both screens.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};

use crate::theme;

/// Filled badge with rounded caps.
///
/// A cell grid cannot round a corner, but the Nerd Font half-circles are
/// drawn to bleed to the cell edge, so a filled run between them reads as
/// a pill. `text` should carry its own padding spaces.
pub fn pill(text: &str, color: Color) -> Vec<Span<'_>> {
    vec![
        Span::styled(theme::PILL_L, Style::new().fg(color)),
        Span::styled(text, Style::new().bg(color).fg(theme::INK).bold()),
        Span::styled(theme::PILL_R, Style::new().fg(color)),
    ]
}

/// Outline badge, for keycaps.
pub fn keycap(key: &str, color: Color) -> Vec<Span<'_>> {
    vec![
        Span::styled("[", Style::new().fg(theme::BORDER)),
        Span::styled(key, Style::new().fg(color).bold()),
        Span::styled("]", Style::new().fg(theme::BORDER)),
    ]
}

/// Rounded card. DESIGN.md asks for rounded borders, and unlike badge
/// corners this one is real: U+256D..U+2570 are actual rounded joints.
pub fn card<'a>(title: &'a str, title_color: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::BORDER))
        // Leading rule segment so the title reads as an inset label in the
        // border rather than as text abutting the corner.
        .title(Line::from(vec![
            Span::styled("─ ", Style::new().fg(theme::BORDER)),
            Span::styled(title, Style::new().fg(title_color).bold()),
            Span::raw(" "),
        ]))
}

/// Push `left` and `right` to opposite edges of a `width`-wide line.
///
/// Used for every label/value pair and every status bar. Doing it here
/// rather than with hand-counted spaces is the single biggest reason the
/// Rust version cannot drift out of alignment the way the shell version
/// does: the padding is derived from measured span widths, not from a
/// literal in a format string.
pub fn spread<'a>(width: u16, left: Vec<Span<'a>>, right: Vec<Span<'a>>) -> Line<'a> {
    let used: usize =
        left.iter().map(Span::width).sum::<usize>() + right.iter().map(Span::width).sum::<usize>();

    let gap = (width as usize).saturating_sub(used).max(1);

    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);

    Line::from(spans)
}

/// Label on the left, value on the right, muted label.
pub fn field<'a>(width: u16, label: &'a str, value: Vec<Span<'a>>) -> Line<'a> {
    spread(
        width,
        vec![Span::styled(label, Style::new().fg(theme::MUTED))],
        value,
    )
}

/// Shorten to `max` columns, dropping from the middle.
///
/// Middle elision rather than a trailing cut, because both ends of a real
/// branch name carry information: the head says what kind of work it is
/// (`feature/`, `hotfix/`) and the tail says which work. Cutting the tail
/// leaves you with `feature/PROJ-4821-refac`, which is the half you could
/// have guessed.
///
/// Operates on chars, not bytes, so it cannot split a multi-byte glyph.
pub fn elide(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();

    if chars.len() <= max {
        return s.to_string();
    }

    if max <= 1 {
        return "…".into();
    }

    // One column goes to the ellipsis; the head keeps the extra column on
    // an odd budget, since prefixes are what you scan first.
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;

    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);

    out
}

/// Plain foreground text.
pub fn text(s: &str, color: Color) -> Span<'_> {
    Span::styled(s, Style::new().fg(color))
}

/// Bold foreground text.
pub fn strong(s: &str, color: Color) -> Span<'_> {
    Span::styled(s, Style::new().fg(color).bold())
}
