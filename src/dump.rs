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
use crate::ui::logo::Logo;

fn draw(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))
        .expect("test backend never fails to construct");

    // Halfblocks rather than a terminal query: there is no terminal here, and
    // halfblocks still render the real artwork, so a dump shows what the design
    // actually looks like.
    let mut art = Logo::halfblocks();

    terminal
        .draw(|frame| ui::render(frame, app, &mut art))
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

    out.push_str(&format!("state {}   width {width}\n", app.state.slug()));

    // Said once, here, and not in the per-height column. Both of these are decided
    // by state at every height, so listing either as something the size gave up
    // would put a fixed fact in a column about the ladder.
    //
    // Two lines, not one, because "collapsed" and "gone" are different rows in the
    // arithmetic underneath: a collapsed tracker still costs its row and the gap
    // above it, and an absent one costs neither.
    if !app.state.has_tracker() {
        out.push_str("  tracker absent: no build is in progress in this frame\n");
    }

    out.push('\n');

    // `mid`, not `log`: the flexible region is the log window in some states, the
    // failure card in one and the switch list in another, and each has its own
    // floor. Naming the column after one of them made the other two look wrong.
    out.push_str("  rows   mid   given up\n");
    out.push_str("  ────   ───   ────────\n");

    for h in [20u16, 24, 29, 33, 37, 40, 45, 50, 56, 62] {
        let plan = Budget::solve(Rect::new(0, 0, width, h), app.state, app.stages.len());
        let chrome = plan.chrome(app.state, app.stages.len());
        let mid = h.saturating_sub(chrome);

        out.push_str(&format!(
            "  {h:>4}   {mid:>3}   {}\n",
            plan.describe(app.state)
        ));
    }

    out.push_str(&format!(
        "\n  full chrome = {} rows   ·   floor = {} rows\n",
        Budget::solve(Rect::new(0, 0, width, 200), app.state, app.stages.len())
            .chrome(app.state, app.stages.len()),
        Budget::floor(app.state),
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

    /// 8.4: a device another run holds says so, and stops claiming to be free.
    ///
    /// Both halves matter. ` in use ` is the warning, and `active` going away is what
    /// keeps the row honest: it means "press Enter and it launches now", which on this
    /// row is the one thing that will not happen.
    ///
    /// Measured at the floor as well, because a fourth chip is a fourth claim on a row
    /// that already clipped once (7.5).
    #[test]
    fn a_device_another_run_holds_reads_in_use_instead_of_active() {
        for (w, h) in [(106, 45), (60, 14)] {
            let mut app = App::new(State::MultipleDevices);

            // The one attached row in the mock, and so the only one that carries
            // ` active ` for this to take away.
            app.busy.insert("emulator-5554".to_string());

            let frame = dump(&mut app, w, h);

            for (i, row) in frame.lines().enumerate() {
                let plain: String = strip_sgr(row);

                assert!(
                    width(&plain) <= w as usize,
                    "at {w}x{h}: row {i} overflows with the chip on it\n{plain}"
                );
            }

            if w < 106 {
                continue;
            }

            assert!(
                frame.contains("in use"),
                "a taken device has to say so:\n{frame}"
            );

            assert!(
                !frame.contains("active"),
                "a taken device is not one Enter can launch:\n{frame}"
            );
        }
    }

    /// The switch list draws the same rows as the first pick and means something
    /// else by them, so the three places that say so are checked together.
    ///
    /// They are one frame's worth of difference: a title, a badge on the row the
    /// run is on, and a footer where `Esc` goes back instead of cancelling out with
    /// 130. Any one of them silently reverting to the `SELECT DEVICE` wording would
    /// leave a frame that reads like a first launch and is not one.
    #[test]
    fn the_switch_list_says_it_is_replacing_a_run() {
        let mut app = App::new(State::Switching);
        let frame = dump(&mut app, 106, 45);

        assert!(
            frame.contains("SWITCH DEVICE") && !frame.contains("SELECT DEVICE"),
            "the title has to distinguish a switch from a first pick:\n{frame}"
        );

        assert!(
            frame.contains("running"),
            "the row the run is on has to be marked:\n{frame}"
        );

        assert!(
            frame.contains("Back") && !frame.contains("Cancel"),
            "Esc returns to the run here, it does not exit with 130:\n{frame}"
        );

        // The target card and the tracker are off screen here, so the list has the
        // frame to itself as the first picker does.
        assert!(
            !frame.contains("DEVICE INFO") && !frame.contains("Stage "),
            "the cards describing the outgoing run should be gone:\n{frame}"
        );
    }

    /// Scrolling back is bounded by the oldest row, and the bound is in visual
    /// rows rather than entries: one wrapped Dart exception is eight rows, so
    /// counting entries would stop in the wrong place.
    #[test]
    fn log_scrolling_stops_at_the_oldest_row() {
        let mut app = App::new(State::Running);
        app.log_scroll = 9_999;

        dump(&mut app, 106, 45);

        assert!(
            app.log_scroll < 9_999,
            "scroll was never clamped: {}",
            app.log_scroll
        );

        // Zero means the live tail, and must survive being asked for.
        app.log_scroll = 0;
        dump(&mut app, 106, 45);
        assert_eq!(app.log_scroll, 0);
    }

    /// A stage that has been running a while shows a clock.
    ///
    /// `frun-runner` had this and the port dropped it. The reason it matters is
    /// that the spinner cycles identically whether a stage is working or wedged,
    /// so the elapsed time is the only thing distinguishing the two — and it is
    /// exactly the kind of row that can vanish without anything failing.
    #[test]
    fn a_slow_stage_shows_its_elapsed_time() {
        let mut app = App::new(State::Building);
        let frame = strip_sgr(&dump(&mut app, 106, 45));

        // `6.0s`, not `6s`: the running row uses the same formatter as the frozen
        // one, so the number does not change units when the stage closes.
        assert!(
            frame.contains("6.0s") || frame.contains("6.1s"),
            "no clock on the running stage:\n{frame}"
        );
    }

    /// Scrolling back must actually change what is on screen.
    ///
    /// Clamping alone is not evidence of that: a window taller than its content
    /// clamps every offset to zero, which looks identical to a scroll that does
    /// nothing. A small frame is used deliberately so the content overflows.
    #[test]
    fn scrolling_back_shows_older_rows() {
        let mut app = App::new(State::Running);

        let at_bottom = dump(&mut app, 80, 24);

        app.log_scroll = 3;
        let scrolled = dump(&mut app, 80, 24);

        assert!(
            app.log_scroll > 0,
            "the window was not full, so this proves nothing"
        );

        assert_ne!(at_bottom, scrolled, "scrolling back changed no rows");
    }

    /// Expanding hands the whole frame to the log window, so it must still fill the
    /// height exactly and stay inside its width.
    #[test]
    fn expanding_fills_the_frame() {
        let mut app = App::new(State::Running);
        app.expanded = true;

        let frame = dump(&mut app, 106, 45);

        assert_eq!(frame.lines().count(), 45);

        for row in frame.lines() {
            assert!(width(&strip_sgr(row)) <= 106, "{row}");
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
