//! Palette and glyph vocabulary, from DESIGN.md v1.3.0 section 2.

use ratatui::style::Color;

// ============================================================
// Surfaces
// ============================================================

/// Cyberpunk Neon Powerline cyan, `#34EDF3`. Card borders, separators, and
/// keycap outlines.
pub const BORDER: Color = Color::Rgb(52, 237, 243);

/// Selected-row background. One step up from the canvas, nothing more.
pub const SURFACE: Color = Color::Rgb(24, 24, 27);

/// Ink for text drawn on top of a filled badge.
///
/// Cyberpunk Neon Powerline navy, `#070E34`. The specified canvas colour is
/// never used to paint the background: filling
/// cells would destroy terminal transparency and blur. It exists only here,
/// where a real background does exist.
pub const INK: Color = Color::Rgb(7, 14, 52);

// ============================================================
// Text
// ============================================================

/// `text-zinc-200` #E4E4E7.
pub const TEXT: Color = Color::Rgb(228, 228, 231);

/// `text-zinc-500` #71717A. Labels, key hints, timestamps.
pub const MUTED: Color = Color::Rgb(113, 113, 122);

// ============================================================
// Functional
// ============================================================

/// Cyberpunk Neon Powerline cyan, `#34EDF3`. Section headers, selection,
/// active focus.
pub const CYAN: Color = Color::Rgb(52, 237, 243);

/// Cyberpunk Neon Powerline lime, `#B8FF6A`. Success, completed stages,
/// clean git.
pub const EMERALD: Color = Color::Rgb(184, 255, 106);

/// Cyberpunk Neon Powerline yellow, `#FFE66D`. Branch tags, pending stages,
/// warnings.
pub const AMBER: Color = Color::Rgb(255, 230, 109);

/// Cyberpunk Neon Powerline magenta, `#F715AB`. Failures, stack traces.
pub const ROSE: Color = Color::Rgb(247, 21, 171);

/// Cyberpunk Neon Powerline purple, brightened to `#CC4DFF`. Simulator and
/// emulator badges, and the card borders that carry them.
///
/// The prompt's own purple is `#9201CB` — HSL(283°, 99%, 40%) — and at that
/// lightness it is the one accent in this palette that does not survive being
/// text: 2.75:1 against `INK`, against a `CYAN` at 12:1 and a `ROSE` at 4.7:1.
/// The value here holds the hue and the saturation and lifts lightness to 65%,
/// which lands at 5.4:1. That number is doing two jobs, because `PURPLE` is the
/// only accent used at both polarities: foreground for the `virtual` tag and the
/// Dart version, and background under `INK` for the `last used` pill. A hue
/// rotation would have bought the same contrast, but 283° is what separates this
/// token from `ROSE` at 320°, and the two appear on the same device row.
pub const PURPLE: Color = Color::Rgb(204, 77, 255);

// ============================================================
// Glyphs
// ============================================================
// Nerd Font, not emoji. 🍎 and 🤖 are East Asian Width Wide, so they occupy
// two cells and break the column grid, and terminal emoji rendering is
// inconsistent between fonts. frun.zsh already ships U+F179 today.

/// Apple. `U+F179`.
pub const GLYPH_APPLE: &str = "\u{f179}";

/// Android. `U+F17B`.
pub const GLYPH_ANDROID: &str = "\u{f17b}";

/// Desktop / display. `U+F108`.
pub const GLYPH_DESKTOP: &str = "\u{f108}";

/// Web / globe. `U+F0AC`.
pub const GLYPH_WEB: &str = "\u{f0ac}";

/// Bolt, for hot reload. `U+F0E7`.
///
/// Not `⚡` U+26A1, which the design frames used: that is East Asian Width Wide,
/// so it occupies two cells and pushed every line containing it one column past
/// the border. `frun.zsh` already carries a scar from this class of bug — see
/// its note about `×` being Ambiguous width and shifting the HOT CONTROLS rows.
pub const GLYPH_BOLT: &str = "\u{f0e7}";

/// Warning triangle, for a run that ended without being asked to. `U+F071`.
///
/// Not `⚠` U+26A0, which is the same defect as `⚡` above and one code point away
/// from it: East Asian Ambiguous, so `unicode-width` measures one cell and a font
/// with emoji presentation draws two. The visible symptom is not the overflow the
/// bolt caused, because this glyph opens a line rather than sitting inside one —
/// it is the *first* cell of the collapsed tracker row, so the extra half-cell of
/// advance reads as an indent and the row stops lining up with the card borders
/// above and below it.
pub const GLYPH_WARN: &str = "\u{f071}";

/// Play triangle, for a run that is live. `U+F04B`.
///
/// Deliberately the same icon family as `GLYPH_STOP` below it, because the two are
/// the ends of one axis and a device card shows one or the other. Mixing families
/// there — a play triangle against a `⏹` from the Unicode symbols block — would make
/// two states of one thing look like two unrelated facts.
pub const GLYPH_PLAY: &str = "\u{f04b}";

/// Stop square, for a run that was ended deliberately. `U+F04D`.
///
/// Not `⏹` U+23F9, for the reason `GLYPH_WARN` is not `⚠`. It shares the same
/// first cell on the same row, so it would arrive at the same misalignment the
/// moment a font gave it emoji presentation.
pub const GLYPH_STOP: &str = "\u{f04d}";

/// Filled-badge caps, giving a pill the closest thing a cell grid has to
/// `border-radius`. Half-circles drawn to bleed to the cell edge.
pub const PILL_L: &str = "\u{e0b6}";
pub const PILL_R: &str = "\u{e0b4}";

/// Braille spinner, matching the frames `frun-runner` already animates.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
