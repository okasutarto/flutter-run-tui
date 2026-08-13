//! Responsive degradation, implementing DESIGN.md 6.2.
//!
//! The interesting part of this module is that the log window is not what is
//! left over after the cards have taken what they want. It is a floor the
//! cards have to yield to.
//!
//! Row separators and roomy device rows push full chrome to 40 rows, and at
//! the design's own 106x45 target that leaves the log window 5 rows while a
//! wrapped five-line Dart exception needs 8. Handing the log window the
//! remainder produces a layout that cannot show a single error at the size it
//! was designed for.

use ratatui::layout::Rect;

use crate::data::State;

/// Rows the log window must keep: enough for one wrapped Dart exception plus a
/// little context.
pub const LOG_MIN: u16 = 12;

/// Rows the compiler-error card must keep.
///
/// Summary, location, three lines of code frame, the caret note, the closing
/// error line and the action row. Without a floor of its own this card was
/// clipped at the design's target size, losing both the final error line and
/// the in-card Retry action — on the one screen where reading the whole message
/// is the entire point.
pub const FAIL_MIN: u16 = 14;

/// Cards stop widening here so a very wide window cannot stretch a label to
/// one edge and its value to the other. The log window is exempt, because more
/// columns means fewer wrapped rows per entry.
pub const MAX_W: u16 = 142;

/// Below this nothing is drawable and the app says so rather than rendering a
/// broken grid.
pub const MIN_W: u16 = 60;
pub const MIN_H: u16 = 14;

/// Which optional elements survive at the current size.
///
/// Fields are in the order they are given up, cheapest first, matching the
/// table in 6.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// 1. Flutter logo in the ProjectCard. 4 rows, no information.
    pub logo: bool,
    /// 2. Hairline rules between metadata rows. 6 rows across both cards.
    pub separators: bool,
    /// 3. Interactive prompt bar. 4 rows; its keys remain in the footer.
    pub prompt: bool,
    /// 4. Device rows collapse from two cell-rows to one.
    pub roomy_devices: bool,
    /// 5. Build tracker collapses to a single summary line.
    ///
    /// State-dependent as much as size-dependent: once the build has
    /// succeeded, every row the tracker holds is static, so it has no claim on
    /// nine rows while the only changing region is starved.
    pub full_build: bool,
    /// 6. Both cards collapse to one metadata row each.
    pub full_cards: bool,
}

impl Budget {
    /// Everything on.
    fn full() -> Self {
        Self {
            logo: true,
            separators: true,
            prompt: true,
            roomy_devices: true,
            full_build: true,
            full_cards: true,
        }
    }

    /// Turn off the next cheapest element. Returns false when nothing is left
    /// to give up.
    fn concede(&mut self) -> bool {
        for flag in [
            &mut self.logo,
            &mut self.separators,
            &mut self.prompt,
            &mut self.roomy_devices,
            &mut self.full_build,
            &mut self.full_cards,
        ] {
            if *flag {
                *flag = false;
                return true;
            }
        }

        false
    }

    // ========================================================
    // Component heights
    // ========================================================
    // These are the single source of truth: `chrome` sums them and the layout
    // in `ui` splits by them. Keeping two copies is how a degradation ladder
    // ends up deciding one thing and drawing another.

    /// Blank row under every card title.
    ///
    /// Charged separately because it is easy to add to `card()` and forget
    /// here, and the failure mode is silent: the Layout keeps the old height,
    /// the padding eats into the inner area, and the card's last rows are
    /// clipped without any warning. That is exactly how the SelectedTargetCard
    /// lost its `Type` field and its command string.
    const TITLE_GAP: u16 = 1;

    /// Rows the logo block needs: 5 of artwork, a blank, and the wordmark.
    ///
    /// Charging only what is drawn matters here. The column ends up as tall as
    /// the metadata beside it anyway, and the difference is what `logo` uses to
    /// centre the mark vertically.
    const LOGO_H: u16 = 7;

    /// ProjectCard.
    ///
    /// Content rows, enumerated so the number can be checked against the code
    /// that draws them rather than estimated:
    ///
    /// ```text
    ///   Project / Version / Branch / Git Status   4
    ///   blank                                     1
    ///   Flutter · Dart · Runtime                  1
    ///   separators between the four fields        3   (optional)
    /// ```
    ///
    /// The logo sits in a parallel column, so the body is the taller of the two
    /// rather than their sum.
    pub fn project_h(&self) -> u16 {
        if !self.full_cards {
            return 1;
        }

        let mut meta = 6;

        if self.separators {
            meta += 3;
        }

        let body = if self.logo {
            meta.max(Self::LOGO_H)
        } else {
            meta
        };

        body + 2 + Self::TITLE_GAP
    }

    /// SelectedTargetCard.
    ///
    /// ```text
    ///   ✔ 1 device active: ...                    1
    ///   blank                                     1
    ///   Device Target / Platform ID / OS / Type   4
    ///   blank                                     1
    ///   ❯ fvm flutter run -d ...                  1
    ///   separators between the four fields        3   (optional)
    /// ```
    pub fn target_h(&self) -> u16 {
        if !self.full_cards {
            return 1;
        }

        let mut body = 8;

        if self.separators {
            body += 3;
        }

        body + 2 + Self::TITLE_GAP
    }

    /// BuildPhaseTracker. Taller once finished, because the summary row and the
    /// full stage list are both present.
    /// BuildPhaseTracker.
    ///
    /// ```text
    ///   progress bar                              1
    ///   blank                                     1
    ///   one row per stage                         5 finished / 3 mid-build
    /// ```
    pub fn build_h(&self, state: State) -> u16 {
        if !self.full_build {
            return 1;
        }

        let stages = if state.build_done() { 5 } else { 3 };

        1 + 1 + stages + 2 + Self::TITLE_GAP
    }

    pub fn prompt_h(&self) -> u16 {
        if self.prompt {
            3
        } else {
            0
        }
    }

    /// How many stacked blocks the frame has, including the flexible middle.
    ///
    /// Needed on its own because `Layout::spacing(1)` inserts a blank row
    /// *between* blocks, so the gap count is `blocks - 1` and not a constant.
    /// Hardcoding it at 4 undercounted the running view by one row, which the
    /// spec-arithmetic test caught.
    fn blocks(&self, state: State) -> u16 {
        // ProjectCard, the flexible middle, and the footer are always present.
        let mut n = 3;

        if state.has_target() {
            n += 1;
        }

        if state.has_build() {
            n += 1;
        }

        if state.has_logs() && self.prompt {
            n += 1;
        }

        n
    }

    /// Rows of chrome this configuration costs in `state`.
    ///
    /// Excludes the flexible middle, which is what the log window or the device
    /// list expands into. Includes the blank rows between blocks, because those
    /// are just as unavailable to the log window as a border is.
    pub fn chrome(&self, state: State) -> u16 {
        let mut rows = self.project_h();

        if state.has_target() {
            rows += self.target_h();
        }

        if state.has_build() {
            rows += self.build_h(state);
        }

        if state.has_logs() {
            rows += self.prompt_h();
        }

        // Footer, always present.
        rows += 1;

        rows + self.blocks(state).saturating_sub(1)
    }

    /// Solve for the largest configuration that still leaves the log window
    /// its floor.
    pub fn solve(area: Rect, state: State) -> Self {
        let mut budget = Self::full();

        // The floor protects whichever region carries the information the user
        // came for. Elsewhere the cards can have the window and the picker
        // takes the remainder, which it can scroll.
        let floor = if state.has_logs() {
            LOG_MIN
        } else if state == State::BuildFailed {
            FAIL_MIN
        } else {
            3
        };

        while budget.chrome(state) + floor > area.height {
            if !budget.concede() {
                break;
            }
        }

        budget
    }

    /// Human-readable report, used by `--rows`.
    pub fn describe(&self) -> String {
        let mut given_up = Vec::new();

        if !self.logo {
            given_up.push("logo");
        }
        if !self.separators {
            given_up.push("separators");
        }
        if !self.prompt {
            given_up.push("prompt");
        }
        if !self.roomy_devices {
            given_up.push("dense devices");
        }
        if !self.full_build {
            given_up.push("build collapsed");
        }
        if !self.full_cards {
            given_up.push("cards collapsed");
        }

        if given_up.is_empty() {
            "full".into()
        } else {
            given_up.join(", ")
        }
    }
}

/// Clamp the drawing area to the maximum card width.
pub fn clamp_width(area: Rect) -> Rect {
    Rect {
        width: area.width.min(MAX_W),
        ..area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn full_chrome_matches_the_spec_arithmetic() {
        // DESIGN.md 6.2 quotes 40, which was an estimate written before the
        // cards existed. 44 is enumerated from the rows they actually draw:
        // project 12, target 14, build 10, prompt 3, footer 1, five gaps.
        let chrome = Budget::full().chrome(State::Running);

        assert_eq!(
            chrome, 45,
            "enumerated from the rows each card actually draws"
        );
    }

    /// Guards the failure mode that adding the title gap caused: the Layout is
    /// split by these heights, so if `card()` grows and the budget does not, the
    /// bottom of every card is clipped in silence.
    #[test]
    fn card_heights_account_for_the_title_gap() {
        let full = Budget::full();
        let mut flat = full;
        flat.separators = false;
        flat.logo = false;

        // 6 content + 2 border + 1 title gap.
        assert_eq!(flat.project_h(), 9);

        // 8 content + 2 border + 1 title gap.
        assert_eq!(flat.target_h(), 11);

        // Separators add three rows to each.
        assert_eq!(full.project_h(), 12, "logo needs 9, metadata needs 9");
        assert_eq!(full.target_h(), 14);
    }

    #[test]
    fn log_floor_is_defended_at_the_design_target() {
        // At 106x45 the full layout would leave 5 rows, so it must concede.
        let budget = Budget::solve(area(106, 45), State::Running);
        let remaining = 45 - budget.chrome(State::Running);

        assert!(
            remaining >= LOG_MIN,
            "log window got {remaining} rows, floor is {LOG_MIN}, budget: {}",
            budget.describe()
        );
    }

    #[test]
    fn a_tall_window_keeps_everything() {
        let budget = Budget::solve(area(106, 60), State::Running);
        assert_eq!(budget, Budget::full());
    }

    #[test]
    fn concessions_happen_in_the_documented_order() {
        let mut b = Budget::full();

        b.concede();
        assert!(!b.logo, "logo goes first");

        b.concede();
        assert!(!b.separators, "separators go second");

        b.concede();
        assert!(!b.prompt, "prompt goes third");
    }

    #[test]
    fn states_without_logs_do_not_defend_a_floor() {
        // The picker has no log window, so it should keep its detail at a
        // height where the running view would already be conceding.
        let budget = Budget::solve(area(106, 34), State::MultipleDevices);
        assert!(budget.logo, "picker has no log floor to protect");
    }
}
