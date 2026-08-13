//! The Flutter mark, rendered as an actual image.
//!
//! Replaces the half-block chevron that stood in for it. That placeholder existed
//! because an earlier attempt to draw the real logo out of block glyphs came out
//! as an unreadable blob, which is worse than not trying: it looks like a
//! rendering fault rather than a mark.

use image::DynamicImage;
use ratatui::layout::{Rect, Size};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};

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

pub struct Logo {
    picker: Picker,
    source: Option<DynamicImage>,

    /// Built once per size. Encoding is the expensive part, so it must not
    /// happen every frame; the area only changes when the terminal is resized.
    protocol: Option<Protocol>,
    built_for: Option<Size>,
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
        }
    }

    /// Halfblocks without querying, for non-interactive rendering.
    pub fn halfblocks() -> Self {
        Self {
            picker: Picker::halfblocks(),
            source: image::load_from_memory(LOGO_PNG).ok(),
            protocol: None,
            built_for: None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let size = Size::new(area.width, area.height);

        if self.built_for != Some(size) {
            self.protocol = self.build(size);
            self.built_for = Some(size);
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
