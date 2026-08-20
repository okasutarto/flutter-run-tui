//! Shared composition helpers.

use std::borrow::Cow;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;

/// Display width in cells, which is not `str::len()`.
pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Filled badge with rounded caps.
pub fn pill<'a>(text: impl Into<Cow<'a, str>>, color: Color, theme: &Theme) -> Vec<Span<'a>> {
    theme.pill(text, color)
}

/// Square badge, for log levels.
pub fn badge<'a>(text: impl Into<Cow<'a, str>>, color: Color, theme: &Theme) -> Vec<Span<'a>> {
    theme.badge(text, color)
}

/// Outline badge, for keycaps.
pub fn keycap<'a>(key: &'a str, color: Color, theme: &Theme) -> Vec<Span<'a>> {
    theme.keycap(key, color)
}

/// Card with an inset title in the border.
pub fn card<'a>(title: &'a str, title_color: Color, theme: &Theme) -> Block<'a> {
    theme.card(title, title_color)
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
pub fn field<'a>(w: u16, label: &'a str, value: Vec<Span<'a>>, theme: &Theme) -> Line<'a> {
    spread(
        w,
        vec![Span::styled(label, Style::new().fg(theme.muted))],
        value,
    )
}

/// Hairline rule spanning `w`, for the row separators in 3.1.
pub fn separator(w: u16, theme: &Theme) -> Line<'static> {
    theme.separator(w)
}

/// Foreground text.
pub fn text<'a>(s: impl Into<Cow<'a, str>>, color: Color) -> Span<'a> {
    Span::styled(s, Style::new().fg(color))
}

pub fn strong<'a>(s: impl Into<Cow<'a, str>>, color: Color) -> Span<'a> {
    Span::styled(s, Style::new().fg(color).bold())
}

/// Shorten to `max` columns, dropping from the middle.
pub fn elide(s: &str, max: usize) -> String {
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
pub fn wrap(message: &str, width: usize) -> Vec<String> {
    if width < 8 {
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
