# Cyberpunk Neon Powerline Palette Design

## Goal

Align frun's functional accent colours with the local Starship Cyberpunk Neon
Powerline prompt while keeping Ghostty's Argonaut theme unchanged.

## Scope

Only `src/theme.rs` changes. Existing UI code continues to consume the same
semantic tokens, so state meanings and layout remain unchanged.

## Token mapping

| frun token | Cyberpunk Neon Powerline colour | Meaning |
| --- | --- | --- |
| `CYAN` | `#34EDF3` | focus and informational accents |
| `EMERALD` | `#B8FF6A` | successful and healthy states |
| `AMBER` | `#FFE66D` | pending and warning states |
| `ROSE` | `#F715AB` | errors and destructive actions |
| `PURPLE` | `#9201CB` | virtual and secondary runtime tags |
| `INK` | `#070E34` | text drawn inside a filled accent badge |

Neutral `BORDER`, `SURFACE`, `TEXT`, and `MUTED` stay unchanged to preserve
legibility over Ghostty's Argonaut background. Ghostty configuration is out of
scope and must remain untouched.

## Verification

Per user instruction, skip TDD and automated tests. Verify the focused source
diff only, including that Ghostty configuration is unchanged.
