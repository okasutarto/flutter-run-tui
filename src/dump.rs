//! Render a frame through `TestBackend` and serialise it back to ANSI.
//!
//! This exists so the layout can be verified without a TTY: a pipe, CI, or an
//! agent session can all check the grid is intact. A padding error shows up as
//! clipped text rather than as a vague sense that something looks off.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use crate::budget::Budget;
use crate::data::App;
use crate::ui;

fn draw(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))
        .expect("test backend never fails to construct");

    terminal
        .draw(|frame| ui::render(frame, app))
        .expect("draw into an in-memory buffer");

    terminal
}

pub fn dump(app: &mut App, width: u16, height: u16) -> String {
    let terminal = draw(app, width, height);
    buffer_to_ansi(terminal.backend().buffer())
}

/// Probe every clickable region and confirm the lookup agrees with what was
/// registered.
///
/// Hit testing is the one part of a mouse UI that cannot be checked by looking
/// at the screen: a button can be drawn perfectly and still be unclickable if
/// the recorded rectangle is wrong.
pub fn hits(app: &mut App, width: u16, height: u16) -> String {
    draw(app, width, height);

    let mut out = format!(
        "{} clickable regions in {} at {width}x{height}\n\n",
        app.hits.len(),
        app.state.slug()
    );

    let probes: Vec<(u16, u16, String)> = app
        .hits
        .iter()
        .map(|h| {
            (
                h.area.x + h.area.width / 2,
                h.area.y,
                format!("{:?}", h.action),
            )
        })
        .collect();

    for h in &app.hits {
        out.push_str(&format!(
            "  {:<12} x {:>3}..{:<3} y {:>2}\n",
            format!("{:?}", h.action),
            h.area.x,
            h.area.x + h.area.width,
            h.area.y,
        ));
    }

    if !probes.is_empty() {
        out.push('\n');
    }

    for (col, row, expected) in probes {
        let got = app.hit_test(col, row);

        let verdict = match &got {
            Some(a) if format!("{a:?}") == expected => "ok",
            _ => "MISMATCH",
        };

        out.push_str(&format!(
            "  ({col:>3},{row:>2}) -> {:<12} {verdict}\n",
            got.map(|a| format!("{a:?}"))
                .unwrap_or_else(|| "none".into()),
        ));
    }

    // A click on empty space must resolve to nothing, or a stray click
    // anywhere would fire the nearest button.
    out.push_str(&format!(
        "\n  (0, 0) -> {:?}   (must be None)\n",
        app.hit_test(0, 0)
    ));

    out
}

/// Report the degradation ladder across heights, per DESIGN.md 6.2.
pub fn rows(app: &App, width: u16) -> String {
    use ratatui::layout::Rect;

    let mut out = String::new();

    out.push_str(&format!("state {}   width {width}\n\n", app.state.slug()));

    out.push_str("  rows   log   given up\n");
    out.push_str("  ────   ───   ────────\n");

    for h in [20u16, 24, 29, 33, 37, 40, 45, 50, 56, 62] {
        let plan = Budget::solve(Rect::new(0, 0, width, h), app.state);
        let chrome = plan.chrome(app.state);
        let log = h.saturating_sub(chrome);

        out.push_str(&format!("  {h:>4}   {log:>3}   {}\n", plan.describe()));
    }

    out.push_str(&format!(
        "\n  full chrome = {} rows   ·   log floor = {} rows\n",
        Budget::solve(Rect::new(0, 0, width, 200), app.state).chrome(app.state),
        crate::budget::LOG_MIN,
    ));

    out
}

fn buffer_to_ansi(buf: &Buffer) -> String {
    let mut out = String::new();

    for y in 0..buf.area.height {
        // Style is tracked per row and reset at the end of each one, so a
        // truncated dump cannot leak colour into the surrounding shell.
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut modifier = Modifier::empty();

        // Cells consumed by the tail of a wide glyph, which must not be emitted
        // again or the dumped row comes out wider than the buffer.
        let mut skip = 0usize;

        for x in 0..buf.area.width {
            if skip > 0 {
                skip -= 1;
                continue;
            }

            let cell = &buf[(x, y)];

            if cell.fg != fg || cell.bg != bg || cell.modifier != modifier {
                out.push_str("\x1b[0m");

                let mut codes: Vec<String> = Vec::new();

                if cell.modifier.contains(Modifier::BOLD) {
                    codes.push("1".into());
                }

                if cell.modifier.contains(Modifier::DIM) {
                    codes.push("2".into());
                }

                if let Some(code) = sgr(cell.fg, true) {
                    codes.push(code);
                }

                if let Some(code) = sgr(cell.bg, false) {
                    codes.push(code);
                }

                if !codes.is_empty() {
                    out.push_str(&format!("\x1b[{}m", codes.join(";")));
                }

                fg = cell.fg;
                bg = cell.bg;
                modifier = cell.modifier;
            }

            out.push_str(cell.symbol());

            // A double-width glyph occupies one cell and reserves the next.
            // ratatui leaves that reserved cell holding a space, so without
            // skipping it the row gains a column per wide glyph.
            skip = crate::widgets::width(cell.symbol()).saturating_sub(1);
        }

        out.push_str("\x1b[0m");
        out.push('\n');
    }

    out
}

fn sgr(color: Color, foreground: bool) -> Option<String> {
    let base = if foreground { 38 } else { 48 };

    match color {
        Color::Reset => None,
        Color::Rgb(r, g, b) => Some(format!("{base};2;{r};{g};{b}")),
        Color::Indexed(i) => Some(format!("{base};5;{i}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::State;
    use crate::widgets::width;

    /// No rendered row may exceed the terminal width, in any state, at any size.
    ///
    /// This is the guard for a whole class of bug rather than one instance of
    /// it. `⚡` U+26A1 is East Asian Width Wide, so every line carrying it ran
    /// one column past the border; the design frames used it and it went
    /// unnoticed until the grid was measured in cells instead of bytes.
    #[test]
    fn no_state_overflows_its_grid() {
        let sizes = [(106, 45), (142, 56), (80, 30), (70, 20), (60, 14)];

        for state in State::ALL {
            for (w, h) in sizes {
                let mut app = App::new(state);
                let frame = dump(&mut app, w, h);

                for (i, row) in frame.lines().enumerate() {
                    let plain: String = strip_sgr(row);
                    let cells = width(&plain);

                    assert!(
                        cells <= w as usize,
                        "{} at {w}x{h}: row {i} is {cells} cells, budget {w}\n{plain}",
                        state.slug(),
                    );
                }
            }
        }
    }

    /// Every state must fill exactly the height it was given.
    #[test]
    fn every_state_fills_its_height() {
        for state in State::ALL {
            let mut app = App::new(state);
            let frame = dump(&mut app, 106, 45);

            assert_eq!(
                frame.lines().count(),
                45,
                "{} did not fill 45 rows",
                state.slug()
            );
        }
    }

    fn strip_sgr(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();

        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
                continue;
            }

            out.push(c);
        }

        out
    }
}
