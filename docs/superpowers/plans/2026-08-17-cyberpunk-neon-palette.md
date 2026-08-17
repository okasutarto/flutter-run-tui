# Cyberpunk Neon Palette Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make frun's semantic accent palette match the local Cyberpunk Neon Powerline prompt.

**Architecture:** Keep the existing semantic token names and all UI call sites. Replace only their RGB values in `src/theme.rs`; leave neutral tokens and Ghostty configuration unchanged.

**Tech Stack:** Rust, Ratatui.

## Global Constraints

- Do not modify Ghostty's Argonaut configuration.
- Keep `BORDER`, `SURFACE`, `TEXT`, and `MUTED` unchanged.
- Per user instruction, skip TDD and automated tests.

---

### Task 1: Update semantic palette tokens

**Files:**
- Modify: `src/theme.rs:22-56`

**Interfaces:**
- Consumes: existing `ratatui::style::Color` constants.
- Produces: unchanged public token names with Cyberpunk Neon Powerline RGB values.

- [x] **Step 1: Replace the accent RGB values**

```rust
pub const INK: Color = Color::Rgb(7, 14, 52);
pub const CYAN: Color = Color::Rgb(52, 237, 243);
pub const EMERALD: Color = Color::Rgb(184, 255, 106);
pub const AMBER: Color = Color::Rgb(255, 230, 109);
pub const ROSE: Color = Color::Rgb(247, 21, 171);
pub const PURPLE: Color = Color::Rgb(146, 1, 203);
```

- [x] **Step 2: Update inline documentation**

Change each corresponding hex annotation to its new value and describe it as a
Cyberpunk Neon Powerline token.

- [x] **Step 3: Review the diff**

Run: `git diff -- src/theme.rs`

Expected: only `INK` and the five semantic accent tokens change; no Ghostty
configuration files appear in the diff.
