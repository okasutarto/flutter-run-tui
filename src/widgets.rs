//! Shared composition helpers.

use std::borrow::Cow;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding};
use unicode_width::UnicodeWidthStr;

use crate::theme;

/// Display width in cells, which is not `str::len()`.
pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Filled badge with rounded caps.
///
/// A cell grid cannot round a corner, but the Nerd Font half-circles are drawn
/// to bleed to the cell edge, so a filled run between them reads as a pill.
/// `text` carries its own padding.
pub fn pill<'a>(text: impl Into<Cow<'a, str>>, color: Color) -> Vec<Span<'a>> {
    vec![
        Span::styled(theme::PILL_L, Style::new().fg(color)),
        Span::styled(text, Style::new().bg(color).fg(theme::INK).bold()),
        Span::styled(theme::PILL_R, Style::new().fg(color)),
    ]
}

/// Square badge, for log levels. Deliberately not a pill: these repeat on
/// every row and the caps would add visual noise at that density.
///
/// Owned or borrowed, like `pill` and `text`, so a caller can pad a badge to a
/// fixed width without hoisting a binding above the `vec![]` that uses it.
pub fn badge<'a>(text: impl Into<Cow<'a, str>>, color: Color) -> Vec<Span<'a>> {
    vec![Span::styled(
        text,
        Style::new().bg(color).fg(theme::INK).bold(),
    )]
}

/// Outline badge, for keycaps.
pub fn keycap(key: &str, color: Color) -> Vec<Span<'_>> {
    vec![
        Span::styled("[", Style::new().fg(theme::BORDER)),
        Span::styled(key, Style::new().fg(color).bold()),
        Span::styled("]", Style::new().fg(theme::BORDER)),
    ]
}

/// Card with an inset title in the border.
///
/// Rounded corners here are real: `U+256D`..`U+2570` are genuine rounded box
/// joints, unlike the badge corners which have to be faked.
pub fn card<'a>(title: &'a str, title_color: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::BORDER))
        // One blank row under the title, baked in here rather than left to each
        // call site. Content butting straight against the title read as a
        // continuation of it, and a rule that has to be remembered six times is
        // a rule that will be forgotten once.
        .padding(Padding::new(1, 1, 1, 0))
        .title(Line::from(vec![
            Span::styled("─ ", Style::new().fg(theme::BORDER)),
            Span::styled("◆ ", Style::new().fg(title_color)),
            Span::styled(title, Style::new().fg(title_color).bold()),
            Span::raw(" "),
        ]))
}

/// Card whose whole border carries a state colour, for failures.
pub fn alert_card<'a>(title: &'a str, color: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(Line::from(vec![
            Span::styled("─ ", Style::new().fg(color)),
            Span::styled("✖ ", Style::new().fg(color).bold()),
            Span::styled(title, Style::new().fg(color).bold()),
            Span::raw(" "),
        ]))
}

/// Push `left` and `right` to opposite edges of a `w`-wide line.
///
/// Padding is derived from measured span widths rather than counted by hand,
/// which is the single reason this cannot drift out of alignment the way the
/// shell version's `printf "%-28s"` literals do.
pub fn spread<'a>(w: u16, left: Vec<Span<'a>>, right: Vec<Span<'a>>) -> Line<'a> {
    let used: usize =
        left.iter().map(Span::width).sum::<usize>() + right.iter().map(Span::width).sum::<usize>();

    let gap = (w as usize).saturating_sub(used).max(1);

    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);

    Line::from(spans)
}

/// Muted label on the left, value on the right.
pub fn field<'a>(w: u16, label: &'a str, value: Vec<Span<'a>>) -> Line<'a> {
    spread(
        w,
        vec![Span::styled(label, Style::new().fg(theme::MUTED))],
        value,
    )
}

/// Hairline rule spanning `w`, for the row separators in 3.1.
pub fn separator(w: u16) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(w as usize),
        Style::new().fg(theme::BORDER),
    ))
}

/// Foreground text.
///
/// Takes `impl Into<Cow<str>>` rather than `&str` so a caller can hand over an
/// owned `String` from `format!` without it dying before the frame is drawn.
/// With a `&str` signature every computed label needs a `let` binding hoisted
/// above the `vec![]` that uses it, which is noise that hides the layout.
pub fn text<'a>(s: impl Into<Cow<'a, str>>, color: Color) -> Span<'a> {
    Span::styled(s, Style::new().fg(color))
}

pub fn strong<'a>(s: impl Into<Cow<'a, str>>, color: Color) -> Span<'a> {
    Span::styled(s, Style::new().fg(color).bold())
}

/// Shorten to `max` columns, dropping from the middle.
///
/// Middle elision rather than a trailing cut, because both ends of a real
/// branch name carry information: the head says what kind of work it is
/// (`feature/`, `hotfix/`) and the tail says which work. Cutting the tail
/// leaves `feature/PROJ-4821-refac`, which is the half you could have guessed.
pub fn elide(s: &str, max: usize) -> String {
    // Measured in display columns, not chars: a single char can occupy two
    // cells, and the whole point of this function is fitting a column budget.
    if width(s) <= max {
        return s.to_string();
    }

    let chars: Vec<char> = s.chars().collect();

    if max <= 1 {
        return "…".into();
    }

    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;

    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);

    out
}

/// Wrap `message` to `width` columns, per DESIGN.md 6.1.
///
/// Returns the visual rows. The caller draws the gutter beside row 0 and leaves
/// it empty for the rest, which is what stops an eighteen-column gutter being
/// repeated for every line of a stack trace that shares one timestamp.
///
/// `textwrap` handles the parts worth not writing by hand: breaking on word
/// boundaries, and measuring in display columns rather than bytes so a line
/// containing a glyph does not overflow.
pub fn wrap(message: &str, width: usize) -> Vec<String> {
    if width < 8 {
        // Too narrow to wrap sensibly; the caller will clip.
        return vec![message.to_string()];
    }

    textwrap::wrap(message, width)
        .into_iter()
        .map(|c| c.into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_counts_cells_not_bytes() {
        assert_eq!(width("abc"), 3);
        // Multi-byte but single-column.
        assert_eq!(width("─"), 1);
    }

    #[test]
    fn elide_keeps_both_ends() {
        let out = elide("feature/PROJ-4821-refactor-checkout-payment-sheet", 30);

        assert_eq!(width(&out), 30);
        assert!(out.starts_with("feature/"), "head survives: {out}");
        assert!(out.ends_with("sheet"), "tail survives: {out}");
        assert!(out.contains('…'));
    }

    #[test]
    fn elide_leaves_short_input_alone() {
        assert_eq!(elide("main", 30), "main");
    }

    #[test]
    fn wrap_respects_the_column_budget() {
        let msg = "'package:flutter/src/widgets/framework.dart': Failed assertion: \
                   line 4795 pos 12: '_debugCurrentBuildTarget == null': is not true.";

        let rows = wrap(msg, 84);

        assert!(rows.len() > 1, "a 130-column line must wrap at 84");

        for row in &rows {
            assert!(
                width(row) <= 84,
                "row overflows the budget: {} cols in {row:?}",
                width(row)
            );
        }
    }

    #[test]
    fn wrap_does_not_lose_words() {
        let msg = "Reloaded 125 of 1824 libraries in 148ms.";
        let joined = wrap(msg, 20).join(" ");

        for word in msg.split_whitespace() {
            assert!(joined.contains(word), "lost {word:?}");
        }
    }
}
