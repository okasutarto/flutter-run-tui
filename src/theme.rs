//! Palette and glyph vocabulary, from DESIGN.md v1.3.0 section 2.

use ratatui::style::Color;

// ============================================================
// Surfaces
// ============================================================

/// Card borders. `border-zinc-700` #3F3F46.
///
/// DESIGN.md v1 specified #2A2A2C, which rendered as effectively invisible in
/// Ghostty: the cards stopped reading as containers and the screen became
/// floating text with coloured headings. The v1 value assumes a CSS border on
/// an opaque panel, but a terminal draws box glyphs against the user's own
/// background, which is rarely that dark and is often translucent. v1.3
/// moved to zinc-700, which is the same conclusion.
pub const BORDER: Color = Color::Rgb(63, 63, 70);

/// Selected-row background. One step up from the canvas, nothing more.
pub const SURFACE: Color = Color::Rgb(24, 24, 27);

/// Ink for text drawn on top of a filled badge.
///
/// The specified canvas colour is never used to paint the background: filling
/// cells would destroy terminal transparency and blur. It exists only here,
/// where a real background does exist.
pub const INK: Color = Color::Rgb(12, 14, 20);

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

/// `text-cyan-400` #38BDF8. Section headers, selection, active focus.
pub const CYAN: Color = Color::Rgb(56, 189, 248);

/// `text-emerald-400` #34D399. Success, completed stages, clean git.
pub const EMERALD: Color = Color::Rgb(52, 211, 153);

/// `text-amber-300` #FBBF24. Branch tags, pending stages, warnings.
pub const AMBER: Color = Color::Rgb(251, 191, 36);

/// `text-rose-400` #F87171. Failures, stack traces.
pub const ROSE: Color = Color::Rgb(248, 113, 113);

/// `text-purple-300` #C084FC. Simulator and emulator badges only.
pub const PURPLE: Color = Color::Rgb(192, 132, 252);

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
