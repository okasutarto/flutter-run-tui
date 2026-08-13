//! Palette, taken verbatim from DESIGN.md.
//!
//! Kept unmodified on purpose so the spec can be judged as written. The
//! audit notes live in comments next to the colours they concern rather
//! than being silently applied.

use ratatui::style::Color;

/// Card borders.
///
/// DESIGN.md specifies #2A2A2C. Rendered in Ghostty that turned out to be
/// effectively invisible: the cards stopped reading as containers and the
/// screen became floating text with coloured headings. The spec value
/// assumes a CSS border on an opaque #0A0A0B panel, but a terminal draws
/// box glyphs against the user's own background, which is rarely that dark
/// and is often translucent.
///
/// Raised to #3E3E44. First deliberate deviation from the spec, and the
/// reason is contrast rather than taste.
pub const BORDER: Color = Color::Rgb(62, 62, 68);

/// Primary accent. #8A63D2
pub const VIOLET: Color = Color::Rgb(138, 99, 210);

/// Success / status. #27C93F
pub const EMERALD: Color = Color::Rgb(39, 201, 63);

/// Warning / hot reload. #FFBD2E  (saturation 100%)
pub const AMBER: Color = Color::Rgb(255, 189, 46);

/// Error / danger. #FF5F56  (saturation 100%)
pub const ROSE: Color = Color::Rgb(255, 95, 86);

/// Info. #50A1FF  (saturation 100%)
pub const CYAN: Color = Color::Rgb(80, 161, 255);

/// Text primary. #E1E1E1
pub const TEXT: Color = Color::Rgb(225, 225, 225);

/// Text secondary. #666666
pub const MUTED: Color = Color::Rgb(102, 102, 102);

/// Text drawn on top of a filled badge.
///
/// DESIGN.md specifies a #0A0A0B background. That colour is deliberately
/// never used to paint the app background: filling cells would destroy
/// terminal transparency and blur. It is only used as ink on filled
/// pills, where a real background does exist.
pub const INK: Color = Color::Rgb(10, 10, 11);

/// Nerd Font half-circle caps, used to fake `border-radius` on a filled
/// badge. Confirmed safe: frun already ships U+F179 in its device list.
pub const PILL_L: &str = "\u{e0b6}";
pub const PILL_R: &str = "\u{e0b4}";
