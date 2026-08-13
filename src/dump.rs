//! Render one frame through `TestBackend` and serialise it back to ANSI.
//!
//! This exists so the layout can be verified without a TTY: CI, a pipe, or
//! an agent session can all check that the grid is intact. It is also how
//! the design gets reviewed in a chat log rather than only on a screen.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use crate::data::App;
use crate::ui;

pub fn dump(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height))
        .expect("test backend never fails to construct");

    terminal
        .draw(|frame| ui::render(frame, app))
        .expect("draw into an in-memory buffer");

    buffer_to_ansi(terminal.backend().buffer())
}

/// Render one frame and report the clickable regions it registered.
///
/// Hit testing is the one part of a mouse UI that cannot be checked by
/// looking at the screen: a button can be drawn perfectly and still be
/// unclickable if the recorded Rect is wrong. This renders a frame, then
/// probes the centre of every registered region and confirms the lookup
/// returns the action that region was registered for.
pub fn hits(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height))
        .expect("test backend never fails to construct");

    terminal
        .draw(|frame| ui::render(frame, app))
        .expect("draw into an in-memory buffer");

    let mut out = format!(
        "{} clickable regions at {width}x{height}\n\n",
        app.hits.len()
    );

    let probes: Vec<(u16, u16, String)> = app
        .hits
        .iter()
        .map(|hit| {
            (
                hit.area.x + hit.area.width / 2,
                hit.area.y,
                format!("{:?}", hit.action),
            )
        })
        .collect();

    for hit in &app.hits {
        out.push_str(&format!(
            "  {:<9} x {:>3}..{:<3} y {:>2}  w {:>2}\n",
            format!("{:?}", hit.action),
            hit.area.x,
            hit.area.x + hit.area.width,
            hit.area.y,
            hit.area.width,
        ));
    }

    out.push_str("\nprobe centre of each region:\n");

    for (col, row, expected) in probes {
        let got = app.hit_test(col, row);

        let verdict = match &got {
            Some(action) if format!("{action:?}") == expected => "ok",
            _ => "MISMATCH",
        };

        out.push_str(&format!(
            "  ({col:>3},{row:>2}) -> {:<12} {verdict}\n",
            got.map(|a| format!("{a:?}"))
                .unwrap_or_else(|| "none".into()),
        ));
    }

    // A click on empty space must resolve to nothing, otherwise a stray
    // click anywhere would fire the nearest button.
    out.push_str(&format!(
        "\n  (0, 0) -> {:?}  (must be None)\n",
        app.hit_test(0, 0)
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

        for x in 0..buf.area.width {
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
