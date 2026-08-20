//! Palette and glyph vocabulary, from DESIGN.md v1.3.0 section 2.

use std::borrow::Cow;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding};

// ============================================================
// Theme & Palettes
// ============================================================

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    CyberpunkNeon,
    MidnightTeal,
    SunsetHorizon,
    VintageForest,
    CyberCrimson,
    ObsidianGold,
    CatppuccinMocha,
    TokyoNight,
    Dracula,
    Nord,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 10] = [
        ThemeKind::CyberpunkNeon,
        ThemeKind::MidnightTeal,
        ThemeKind::SunsetHorizon,
        ThemeKind::VintageForest,
        ThemeKind::CyberCrimson,
        ThemeKind::ObsidianGold,
        ThemeKind::CatppuccinMocha,
        ThemeKind::TokyoNight,
        ThemeKind::Dracula,
        ThemeKind::Nord,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ThemeKind::CyberpunkNeon => "Cyberpunk Neon",
            ThemeKind::MidnightTeal => "Midnight Teal",
            ThemeKind::SunsetHorizon => "Sunset Horizon",
            ThemeKind::VintageForest => "Vintage Forest",
            ThemeKind::CyberCrimson => "Cyber Crimson",
            ThemeKind::ObsidianGold => "Obsidian Gold",
            ThemeKind::CatppuccinMocha => "Catppuccin Mocha",
            ThemeKind::TokyoNight => "Tokyo Night",
            ThemeKind::Dracula => "Dracula",
            ThemeKind::Nord => "Nord Dark",
        }
    }

    #[allow(dead_code)]
    pub fn slug(self) -> &'static str {
        match self {
            ThemeKind::CyberpunkNeon => "cyberpunk",
            ThemeKind::MidnightTeal => "midnight-teal",
            ThemeKind::SunsetHorizon => "sunset-horizon",
            ThemeKind::VintageForest => "vintage-forest",
            ThemeKind::CyberCrimson => "cyber-crimson",
            ThemeKind::ObsidianGold => "obsidian-gold",
            ThemeKind::CatppuccinMocha => "catppuccin",
            ThemeKind::TokyoNight => "tokyonight",
            ThemeKind::Dracula => "dracula",
            ThemeKind::Nord => "nord",
        }
    }

    pub fn from_slug(s: &str) -> Option<ThemeKind> {
        let clean = s.trim().to_lowercase();
        match clean.as_str() {
            "cyberpunk" | "cyberpunk-neon" => Some(ThemeKind::CyberpunkNeon),
            "midnight-teal" | "midnight_teal" => Some(ThemeKind::MidnightTeal),
            "sunset-horizon" | "sunset_horizon" => Some(ThemeKind::SunsetHorizon),
            "vintage-forest" | "vintage_forest" => Some(ThemeKind::VintageForest),
            "cyber-crimson" | "cyber_crimson" => Some(ThemeKind::CyberCrimson),
            "obsidian-gold" | "obsidian_gold" => Some(ThemeKind::ObsidianGold),
            "catppuccin" | "catppuccin-mocha" | "catppuccin_mocha" => Some(ThemeKind::CatppuccinMocha),
            "tokyonight" | "tokyo-night" | "tokyo_night" => Some(ThemeKind::TokyoNight),
            "dracula" => Some(ThemeKind::Dracula),
            "nord" | "nord-dark" | "nord_dark" => Some(ThemeKind::Nord),
            _ => None,
        }
    }

    pub fn palette(self) -> Theme {
        match self {
            // 1. Cyberpunk Neon (High-Voltage Neon Noir)
            ThemeKind::CyberpunkNeon => Theme {
                kind: self,
                border: Color::Rgb(52, 237, 243),  // Neon Cyan #34edf3
                surface: Color::Rgb(24, 24, 27),
                ink: Color::Rgb(7, 14, 52),
                text: Color::Rgb(228, 228, 231),
                muted: Color::Rgb(113, 113, 122),
                cyan: Color::Rgb(52, 237, 243),    // Neon Cyan
                emerald: Color::Rgb(184, 255, 106),// Neon Lime
                amber: Color::Rgb(255, 230, 109),  // Neon Yellow
                rose: Color::Rgb(247, 21, 171),    // Neon Magenta
                purple: Color::Rgb(204, 77, 255),  // Neon Purple
            },

            // 2. Midnight Teal (Color Hunt All-Time Top #1 Dark: #222831, #393E46, #00ADB5, #EEEEEE)
            ThemeKind::MidnightTeal => Theme {
                kind: self,
                border: Color::Rgb(0, 173, 181),   // Vibrant Teal #00adb5
                surface: Color::Rgb(57, 62, 70),   // Dark Slate #393e46
                ink: Color::Rgb(34, 40, 49),       // Deep Base #222831
                text: Color::Rgb(238, 238, 238),   // Clean White #eeeeee
                muted: Color::Rgb(146, 154, 158),  // Muted Slate #929a9e
                cyan: Color::Rgb(0, 173, 181),     // Teal #00adb5
                emerald: Color::Rgb(0, 255, 245),  // Bright Aqua #00fff5
                amber: Color::Rgb(255, 211, 105),  // Warm Amber #ffd369
                rose: Color::Rgb(255, 87, 34),     // Deep Flame Red #ff5722
                purple: Color::Rgb(166, 227, 233), // Soft Frost Blue #a6e3e9
            },

            // 3. Sunset Horizon (Color Hunt Top Warm Dark: #2D4059, #EA5455, #F07B3F, #FFD460)
            ThemeKind::SunsetHorizon => Theme {
                kind: self,
                border: Color::Rgb(234, 84, 85),   // Coral Red #ea5455
                surface: Color::Rgb(45, 64, 89),   // Slate Navy #2d4059
                ink: Color::Rgb(29, 45, 68),       // Night Navy #1d2d44
                text: Color::Rgb(249, 249, 249),   // Warm White #f9f9f9
                muted: Color::Rgb(139, 155, 174),  // Slate Muted #8b9bae
                cyan: Color::Rgb(0, 206, 201),     // Ocean Teal #00cec9
                emerald: Color::Rgb(85, 230, 193), // Mint Marine #55e6c1
                amber: Color::Rgb(240, 123, 63),   // Tangerine #f07b3f
                rose: Color::Rgb(234, 84, 85),     // Coral Red #ea5455
                purple: Color::Rgb(255, 212, 96),  // Sun Gold #ffd460
            },

            // 4. Vintage Forest (Earthy Dark Green & Warm Leather: #2C3930, #3F4F44, #A27B5C, #DCD7C9)
            ThemeKind::VintageForest => Theme {
                kind: self,
                border: Color::Rgb(162, 123, 92),  // Warm Leather #a27b5c
                surface: Color::Rgb(63, 79, 68),   // Forest Green #3f4f44
                ink: Color::Rgb(44, 57, 48),       // Dark Olive #2c3930
                text: Color::Rgb(220, 215, 201),   // Parchment Cream #dcd7c9
                muted: Color::Rgb(117, 138, 124),  // Sage Muted #758a7c
                cyan: Color::Rgb(123, 158, 168),   // Slate Blue #7b9ea8
                emerald: Color::Rgb(143, 162, 138),// Sage Green #8fa28a
                amber: Color::Rgb(208, 169, 107),  // Golden Sand #d0a96b
                rose: Color::Rgb(194, 89, 83),     // Terracotta Red #c25953
                purple: Color::Rgb(162, 123, 92),  // Leather Bronze #a27b5c
            },

            // 5. Cyber Crimson (Neon Noir / Navy & Crimson: #1A1A2E, #16213E, #0F3460, #E94560)
            ThemeKind::CyberCrimson => Theme {
                kind: self,
                border: Color::Rgb(233, 69, 96),   // Neon Crimson #e94560
                surface: Color::Rgb(22, 33, 62),   // Dark Navy #16213e
                ink: Color::Rgb(26, 26, 46),       // Deep Night #1a1a2e
                text: Color::Rgb(234, 234, 234),   // Off White #eaeaea
                muted: Color::Rgb(108, 122, 137),  // Steel Muted #6c7a89
                cyan: Color::Rgb(0, 210, 211),     // Cyber Cyan #00d2d3
                emerald: Color::Rgb(78, 204, 163), // Mint Neon #4ecca3
                amber: Color::Rgb(243, 156, 18),   // Amber Gold #f39c12
                rose: Color::Rgb(233, 69, 96),     // Crimson Red #e94560
                purple: Color::Rgb(155, 89, 182),  // Royal Violet #9b59b6
            },

            // 6. Obsidian Gold (High-Contrast Minimalist: #101820, #2B2D42, #FEE715, #F2F2F2)
            ThemeKind::ObsidianGold => Theme {
                kind: self,
                border: Color::Rgb(254, 231, 21),  // Electric Gold #fee715
                surface: Color::Rgb(43, 45, 66),   // Surface Charcoal #2b2d42
                ink: Color::Rgb(16, 24, 32),       // Obsidian Black #101820
                text: Color::Rgb(242, 242, 242),   // Crisp White #f2f2f2
                muted: Color::Rgb(141, 153, 174),  // Slate Muted #8d99ae
                cyan: Color::Rgb(0, 245, 212),     // Neon Mint Cyan #00f5d4
                emerald: Color::Rgb(112, 224, 0),  // Electric Lime #70e000
                amber: Color::Rgb(254, 231, 21),   // Electric Gold #fee715
                rose: Color::Rgb(255, 0, 84),      // Neon Rose Red #ff0054
                purple: Color::Rgb(157, 78, 221),  // Vivid Purple #9d4edd
            },

            // 7. Catppuccin Mocha (Harmonious Pastel Dark)
            ThemeKind::CatppuccinMocha => Theme {
                kind: self,
                border: Color::Rgb(203, 166, 247), // Signature Mauve #cba6f7
                surface: Color::Rgb(49, 50, 68),   // Surface0 #313244
                ink: Color::Rgb(17, 17, 27),       // Crust #11111b
                text: Color::Rgb(205, 214, 244),   // Text #cdd6f4
                muted: Color::Rgb(147, 153, 178),  // Overlay1 #9399b2
                cyan: Color::Rgb(137, 220, 235),   // Sky Blue #89dceb
                emerald: Color::Rgb(166, 227, 161),// Green #a6e3a1
                amber: Color::Rgb(250, 179, 135),  // Peach #fab387
                rose: Color::Rgb(243, 139, 168),   // Red #f38ba8
                purple: Color::Rgb(203, 166, 247), // Mauve #cba6f7
            },

            // 8. Tokyo Night (Glowing Indigo & Night Lights)
            ThemeKind::TokyoNight => Theme {
                kind: self,
                border: Color::Rgb(122, 162, 247), // Signature Blue #7aa2f7
                surface: Color::Rgb(41, 46, 66),   // Surface #292e42
                ink: Color::Rgb(22, 22, 30),       // Darker bg #16161e
                text: Color::Rgb(192, 202, 245),   // Fg #c0caf5
                muted: Color::Rgb(120, 124, 153),  // Comment #787c99
                cyan: Color::Rgb(125, 207, 255),   // Cyan #7dcfff
                emerald: Color::Rgb(115, 218, 202),// Teal #73daca
                amber: Color::Rgb(255, 158, 100),  // Orange #ff9e64
                rose: Color::Rgb(247, 118, 142),   // Red #f7768e
                purple: Color::Rgb(187, 154, 247), // Purple #bb9af7
            },

            // 9. Dracula (High-Contrast Gothic Purple & Pink)
            ThemeKind::Dracula => Theme {
                kind: self,
                border: Color::Rgb(189, 147, 249), // Signature Purple #bd93f9
                surface: Color::Rgb(68, 71, 90),   // Selection #44475a
                ink: Color::Rgb(33, 34, 44),       // Darker #21222c
                text: Color::Rgb(248, 248, 242),   // Fg #f8f8f2
                muted: Color::Rgb(98, 114, 164),   // Comment #6272a4
                cyan: Color::Rgb(139, 233, 253),   // Cyan #8be9fd
                emerald: Color::Rgb(80, 250, 123), // Green #50fa7b
                amber: Color::Rgb(255, 184, 108),  // Orange #ffb86c
                rose: Color::Rgb(255, 85, 85),     // Red #ff5555
                purple: Color::Rgb(255, 121, 198), // Pink #ff79c6
            },

            // 10. Nord Dark (Cool Arctic Frost & Aurora)
            ThemeKind::Nord => Theme {
                kind: self,
                border: Color::Rgb(136, 192, 208), // Frost Cyan #88c0d0
                surface: Color::Rgb(59, 66, 82),   // Polar Night 1 #3b4252
                ink: Color::Rgb(36, 41, 51),       // Deep night #242933
                text: Color::Rgb(236, 239, 244),   // Snow Storm #eceff4
                muted: Color::Rgb(123, 136, 161),  // Gray #7b88a1
                cyan: Color::Rgb(136, 192, 208),   // Frost Cyan
                emerald: Color::Rgb(163, 190, 140),// Aurora Green #a3be8c
                amber: Color::Rgb(235, 203, 139),  // Aurora Yellow #ebcb8b
                rose: Color::Rgb(191, 97, 106),    // Aurora Red #bf616a
                purple: Color::Rgb(180, 142, 173), // Aurora Purple #b48ead
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub kind: ThemeKind,
    pub border: Color,
    pub surface: Color,
    pub ink: Color,
    pub text: Color,
    pub muted: Color,
    pub cyan: Color,
    pub emerald: Color,
    pub amber: Color,
    pub rose: Color,
    pub purple: Color,
}

impl Default for Theme {
    fn default() -> Self {
        ThemeKind::CyberpunkNeon.palette()
    }
}

#[allow(dead_code)]
impl Theme {
    /// Optimal text color for badge/pill backgrounds based on perceived luminance.
    pub fn badge_fg(&self, bg: Color) -> Color {
        match bg {
            Color::Rgb(r, g, b) => {
                let lum = (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) as u8;
                if lum > 140 {
                    self.ink
                } else {
                    self.text
                }
            }
            _ => self.ink,
        }
    }

    /// Card with an inset title in the border.
    pub fn card<'a>(&self, title: &'a str, title_color: Color) -> Block<'a> {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(self.border))
            .padding(Padding::new(1, 1, 1, 0))
            .title(Line::from(vec![
                Span::styled("─ ", Style::new().fg(self.border)),
                Span::styled("◆ ", Style::new().fg(title_color)),
                Span::styled(title, Style::new().fg(title_color).bold()),
                Span::raw(" "),
            ]))
    }

    /// Card whose whole border carries a state colour, for failures.
    pub fn alert_card<'a>(&self, title: &'a str, color: Color) -> Block<'a> {
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

    /// Filled badge with rounded caps.
    pub fn pill<'a>(&self, text: impl Into<Cow<'a, str>>, color: Color) -> Vec<Span<'a>> {
        let fg = self.badge_fg(color);
        vec![
            Span::styled(PILL_L, Style::new().fg(color)),
            Span::styled(text, Style::new().bg(color).fg(fg).bold()),
            Span::styled(PILL_R, Style::new().fg(color)),
        ]
    }

    /// Square badge, for log levels.
    pub fn badge<'a>(&self, text: impl Into<Cow<'a, str>>, color: Color) -> Vec<Span<'a>> {
        let fg = self.badge_fg(color);
        vec![Span::styled(
            text,
            Style::new().bg(color).fg(fg).bold(),
        )]
    }

    /// Outline badge, for keycaps.
    pub fn keycap<'a>(&self, key: &'a str, color: Color) -> Vec<Span<'a>> {
        vec![
            Span::styled("[", Style::new().fg(self.border)),
            Span::styled(key, Style::new().fg(color).bold()),
            Span::styled("]", Style::new().fg(self.border)),
        ]
    }

    /// Hairline rule spanning `w`.
    pub fn separator(&self, w: u16) -> Line<'static> {
        Line::from(Span::styled(
            "─".repeat(w as usize),
            Style::new().fg(self.border),
        ))
    }
}

// ============================================================
// Default Constants (Cyberpunk Neon) for Backwards Compatibility
// ============================================================

#[allow(dead_code)]
pub const BORDER: Color = Color::Rgb(52, 237, 243);
#[allow(dead_code)]
pub const SURFACE: Color = Color::Rgb(24, 24, 27);
#[allow(dead_code)]
pub const INK: Color = Color::Rgb(7, 14, 52);
#[allow(dead_code)]
pub const TEXT: Color = Color::Rgb(228, 228, 231);
#[allow(dead_code)]
pub const MUTED: Color = Color::Rgb(113, 113, 122);
#[allow(dead_code)]
pub const CYAN: Color = Color::Rgb(52, 237, 243);
#[allow(dead_code)]
pub const EMERALD: Color = Color::Rgb(184, 255, 106);
#[allow(dead_code)]
pub const AMBER: Color = Color::Rgb(255, 230, 109);
#[allow(dead_code)]
pub const ROSE: Color = Color::Rgb(247, 21, 171);
#[allow(dead_code)]
pub const PURPLE: Color = Color::Rgb(204, 77, 255);

// ============================================================
// Nerd Font glyphs, DESIGN.md 2.2.
// ============================================================

#[allow(dead_code)]
pub const GLYPH_APPLE: &str = "";
#[allow(dead_code)]
pub const GLYPH_ANDROID: &str = "";
#[allow(dead_code)]
pub const GLYPH_DESKTOP: &str = "";
#[allow(dead_code)]
pub const GLYPH_WEB: &str = "";
#[allow(dead_code)]
pub const GLYPH_CHROME: &str = "";
#[allow(dead_code)]
pub const GLYPH_FLUTTER: &str = "";
#[allow(dead_code)]
pub const GLYPH_DART: &str = "";
#[allow(dead_code)]
pub const GLYPH_GIT_BRANCH: &str = "";
#[allow(dead_code)]
pub const GLYPH_GIT_CLEAN: &str = "✔";
#[allow(dead_code)]
pub const GLYPH_GIT_DIRTY: &str = "✗";
#[allow(dead_code)]
pub const GLYPH_BOLT: &str = "";
#[allow(dead_code)]
pub const GLYPH_GEAR: &str = "";
#[allow(dead_code)]
pub const GLYPH_WARN: &str = "";
#[allow(dead_code)]
pub const GLYPH_PLAY: &str = "";
#[allow(dead_code)]
pub const GLYPH_STOP: &str = "⏹";
#[allow(dead_code)]
pub const GLYPH_RELOAD: &str = "勒";
#[allow(dead_code)]
pub const GLYPH_INFO: &str = "";
#[allow(dead_code)]
pub const GLYPH_SUCCESS: &str = "";

#[allow(dead_code)]
pub const PILL_L: &str = "";
#[allow(dead_code)]
pub const PILL_R: &str = "";

#[allow(dead_code)]
pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ============================================================
// Theme Persistence
// ============================================================

fn theme_file() -> std::path::PathBuf {
    crate::probe::home().join(".config/zsh/.frun-theme")
}

pub fn load_saved_theme() -> ThemeKind {
    let path = theme_file();
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Some(kind) = ThemeKind::from_slug(&content) {
            return kind;
        }
    }
    ThemeKind::CyberpunkNeon
}

pub fn save_theme(kind: ThemeKind) {
    let path = theme_file();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, kind.slug());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_slug_round_trips() {
        for kind in ThemeKind::ALL {
            let slug = kind.slug();
            let parsed = ThemeKind::from_slug(slug);
            assert_eq!(parsed, Some(kind));
        }
    }

    #[test]
    fn theme_slug_handles_aliases() {
        assert_eq!(ThemeKind::from_slug("tokyo_night"), Some(ThemeKind::TokyoNight));
        assert_eq!(ThemeKind::from_slug("midnight_teal"), Some(ThemeKind::MidnightTeal));
        assert_eq!(ThemeKind::from_slug("obsidian-gold"), Some(ThemeKind::ObsidianGold));
        assert_eq!(ThemeKind::from_slug("nonexistent"), None);
    }
}
