//! The Flutter mark, rendered as an actual image.
//!
//! Replaces the half-block chevron that stood in for it. That placeholder existed
//! because an earlier attempt to draw the real logo out of block glyphs came out
//! as an unreadable blob, which is worse than not trying: it looks like a
//! rendering fault rather than a mark.

use std::time::{Duration, Instant};

use image::DynamicImage;
use ratatui::layout::{Rect, Size};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{FontSize, Image, Resize};

use crate::theme;
use crate::widgets::text;

/// The trimmed asset, embedded rather than read from disk.
///
/// `frun` runs from whichever Flutter project directory you happen to be in, so
/// a relative path is useless and an absolute one is fragile. Embedding also
/// means there is no missing-file case to handle at runtime.
///
/// `flutter-trim.png` and not `flutter.png`: the latter is a 400x300 canvas with
/// roughly 79px of transparent padding per side, which renders as dead columns
/// and throws off the gutter. The trimmed asset is the same artwork cropped to
/// its content at 242x300. The existing `frun.zsh` learned this the hard way and
/// says so in a comment.
const LOGO_PNG: &[u8] = include_bytes!("../../assets/flutter-trim.png");

/// How often the cell is re-measured. See `follow_cell_size`.
const REMEASURE: Duration = Duration::from_millis(200);

pub struct Logo {
    picker: Picker,
    source: Option<DynamicImage>,

    /// Built once per size. Encoding is the expensive part, so it must not
    /// happen every frame; the area only changes when the terminal is resized.
    ///
    /// Keyed on the cell's pixel size as well as the cell box, which is the fix
    /// for `Cmd -` doing nothing to the mark. The box is a constant 11x5 cells
    /// (`ui::project::ART_W`), so on the key alone this encoded exactly once per
    /// process and never again, no matter what the terminal did afterwards.
    protocol: Option<Protocol>,
    built_for: Option<(Size, (u16, u16))>,

    /// When the cell was last measured, so that is a syscall five times a second
    /// rather than once per frame.
    checked: Instant,
}

impl Logo {
    /// Ask the terminal what it can do, and fall back to halfblocks.
    ///
    /// Must be called before the alternate screen is entered: the query writes
    /// control sequences to stdout and reads the reply, which needs an
    /// uncontended terminal.
    ///
    /// The fallback is not a failure mode. Halfblocks work everywhere, including
    /// under `TestBackend`, which is what keeps `--dump` showing the real
    /// artwork rather than a placeholder.
    ///
    /// `FRUN_NO_QUERY=1` skips the query outright. This is not a debugging knob.
    /// The query reads a reply from stdin, and against a terminal that never
    /// answers — a bare pty, some task runners — it returns but leaves stdin
    /// unreadable, so the UI renders and then ignores every key including `q`.
    /// Measured, not guessed: 63 frames drawn, zero key events, and the same
    /// behaviour on the commit before this one. Every real terminal answers, so
    /// the default stays; the valve is for when one does not.
    pub fn detect() -> Self {
        if std::env::var_os("FRUN_NO_QUERY").is_some() {
            return Self::halfblocks();
        }

        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        Self {
            picker,
            source: image::load_from_memory(LOGO_PNG).ok(),
            protocol: None,
            built_for: None,
            checked: Instant::now(),
        }
    }

    /// Halfblocks without querying, for non-interactive rendering.
    pub fn halfblocks() -> Self {
        Self {
            picker: Picker::halfblocks(),
            source: image::load_from_memory(LOGO_PNG).ok(),
            protocol: None,
            built_for: None,
            checked: Instant::now(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.follow_cell_size();

        let cell = self.picker.font_size();
        let key = (
            Size::new(area.width, area.height),
            (cell.width, cell.height),
        );

        if self.built_for != Some(key) {
            self.protocol = self.build(key.0);
            self.built_for = Some(key);
        }

        match &self.protocol {
            Some(protocol) => frame.render_widget(Image::new(protocol), area),

            // Decode or encode failed. Say so rather than leaving a blank
            // rectangle that reads as a layout bug.
            None => frame.render_widget(
                Paragraph::new(Line::from(text("[logo]", theme::BORDER))),
                area,
            ),
        }
    }

    /// Keep the picker's idea of a cell in step with the terminal's.
    ///
    /// `Picker::from_query_stdio` measures the cell once, before the alternate
    /// screen is entered, and there is no setter for it afterwards. That is the
    /// whole bug behind `Cmd -` leaving the mark alone: pressing it changes the
    /// cell's pixel size, the artwork is encoded against the cell size from
    /// startup, and nothing in the frame ever asks again. The text reflows around
    /// a mark that is still drawn for the old font.
    ///
    /// Measured from `TIOCGWINSZ` rather than by re-running the query. The query
    /// writes an escape sequence and reads the reply off stdin, which is the
    /// input loop's stdin — the same contention `FRUN_NO_QUERY` exists for. The
    /// ioctl asks the kernel and touches neither stream.
    ///
    /// Two guards, both load-bearing:
    ///
    /// * Halfblocks are exempt. Their 10x20 "cell" is a fiction that sets the
    ///   aspect ratio of a block-glyph render, not a measurement, and `--dump`
    ///   runs on halfblocks against a `TestBackend` with no tty at all.
    /// * A terminal that reports no pixel size is left alone. `ws_xpixel` is
    ///   optional and plenty of ptys return 0; deriving a cell from that would
    ///   divide the artwork by zero-ish and replace a stale mark with no mark.
    fn follow_cell_size(&mut self) {
        if self.picker.protocol_type() == ProtocolType::Halfblocks {
            return;
        }

        if self.checked.elapsed() < REMEASURE {
            return;
        }

        self.checked = Instant::now();

        let Some(cell) = cell_size() else {
            return;
        };

        let current = self.picker.font_size();

        if cell.width == current.width && cell.height == current.height {
            return;
        }

        // No setter exists, so the picker is rebuilt around the new measurement
        // and the detected protocol is carried over. `from_fontsize` is
        // deprecated for the case this is not: it warns against *guessing* a
        // cell size instead of querying for one. This is a measurement, and the
        // protocol type it cannot detect is supplied from the query that did.
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(cell);

        picker.set_protocol_type(self.picker.protocol_type());

        self.picker = picker;

        // The protocol goes with it. Under kitty this matters twice over: the
        // image bytes are transmitted once per protocol object and tracked by a
        // flag inside it, so re-encoding is also what re-transmits the artwork at
        // its new pixel size. `render` rebuilds it, because the cell is part of
        // the key.
    }

    fn build(&self, size: Size) -> Option<Protocol> {
        let source = self.source.clone()?;

        // Fit rather than crop: the mark has to stay recognisable, and letting
        // it overflow the reserved columns would paint over the metadata beside
        // it, which no amount of cell-skipping fixes for every protocol.
        self.picker
            .new_protocol(source, size, Resize::Fit(None))
            .ok()
    }
}

/// The terminal's cell in pixels, from the window size the kernel holds.
///
/// `None` when the terminal does not report a pixel size, which is a real and
/// common answer rather than an error: `ws_xpixel` and `ws_ypixel` are optional
/// and a bare pty leaves them at zero.
fn cell_size() -> Option<FontSize> {
    let window = ratatui::crossterm::terminal::window_size().ok()?;

    if window.columns == 0 || window.rows == 0 || window.width == 0 || window.height == 0 {
        return None;
    }

    Some(FontSize::new(
        window.width / window.columns,
        window.height / window.rows,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dump path must not consult the terminal at all: it renders against a
    /// `TestBackend` with no tty, and its 10x20 halfblock cell is a fiction that
    /// the snapshot tests measure rows against.
    #[test]
    fn halfblocks_keep_their_nominal_cell() {
        let mut logo = Logo::halfblocks();
        let before = logo.picker.font_size();

        logo.follow_cell_size();

        let after = logo.picker.font_size();

        assert_eq!((before.width, before.height), (after.width, after.height));
    }

    /// A cell measured as zero pixels is not a cell. Dividing the artwork into it
    /// would trade a stale mark for no mark at all.
    #[test]
    fn a_terminal_without_a_pixel_size_is_left_alone() {
        // `cell_size` is the only thing that can answer here, and under `cargo
        // test` there is either no tty or one that reports no pixels. Either way
        // the contract is the same: it must not invent a cell.
        if let Some(cell) = cell_size() {
            assert!(cell.width > 0 && cell.height > 0, "{cell:?}");
        }
    }
}
