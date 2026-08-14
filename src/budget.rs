//! Responsive degradation, implementing DESIGN.md 6.2.
//!
//! The interesting part of this module is that the log window is not what is
//! left over after the cards have taken what they want. It is a floor the
//! cards have to yield to.
//!
//! Row separators and roomy device rows push full chrome to 41 rows, and at
//! the design's own 106x45 target that leaves the log window 4 rows while a
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
/// Border and padding take four; the rest is summary, location, blank, three
/// lines of code frame, the caret, blank, and the closing note. Without a floor
/// of its own this card was clipped at the design's target size, losing the final
/// error line — on the one screen where reading the whole message is the entire
/// point.
///
/// Unchanged at 14 now that the in-card action row is gone. The two rows it held
/// go to the message rather than back to the layout: what was being cut is the
/// oldest build output, which on a Gradle failure is usually where the cause is.
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
    /// 1. Hairline rules between metadata rows. 6 rows across both cards.
    pub separators: bool,
    /// 2. Device rows collapse from two cell-rows to one.
    pub roomy_devices: bool,
    /// 3. Build tracker collapses to a single summary line.
    ///
    /// State-dependent as much as size-dependent: once the build has
    /// succeeded, every row the tracker holds is static, so it has no claim on
    /// nine rows while the only changing region is starved.
    pub full_build: bool,
    /// 4. Both cards collapse to one metadata row each.
    pub full_cards: bool,
}

impl Budget {
    /// Everything on.
    fn full() -> Self {
        Self {
            separators: true,
            roomy_devices: true,
            full_build: true,
            full_cards: true,
        }
    }

    /// Turn off the next cheapest element. Returns false when nothing is left
    /// to give up.
    ///
    /// The order is 6.2's table, with the one swap that table's own footnote
    /// asks for: rung 3 is state-dependent, and once the build has finished it
    /// is no longer the third-cheapest thing on screen but the first. Every row
    /// the tracker then holds is static — five stage labels and their final
    /// durations — while the separators are structure on two cards that are
    /// still being read, and the log window below is the only region changing.
    ///
    /// Getting this wrong was visible rather than theoretical. At 46 to 51 rows,
    /// which is an ordinary Ghostty window at 12px, the fixed order paid for the
    /// log floor out of the separators and kept nine rows of finished timings, so
    /// both cards lost their rules the instant the app started logging.
    ///
    /// During the build the original order stands, and for the same reason: the
    /// stage list is the thing moving, so it is the last thing to give up.
    fn concede(&mut self, state: State) -> bool {
        let order = if state.build_done() {
            [
                &mut self.full_build,
                &mut self.separators,
                &mut self.roomy_devices,
                &mut self.full_cards,
            ]
        } else {
            [
                &mut self.separators,
                &mut self.roomy_devices,
                &mut self.full_build,
                &mut self.full_cards,
            ]
        };

        for flag in order {
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

    /// Ceiling on tracker rows, so an unexpected flood of stages cannot push the
    /// log window off the screen. Android's six plus `pub get` and CocoaPods is
    /// the most that has ever been observed.
    const MAX_STAGES: u16 = 8;

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
    /// The logo is not charged here and is not in the ladder. It sits in a
    /// parallel column five rows tall, and the metadata beside it is never
    /// shorter than six, so the body height is the metadata's either way.
    ///
    /// It used to be the first concession, described as reclaiming four rows. It
    /// reclaimed none: `meta.max(5)` is `meta` for every value `meta` can take.
    /// So the ladder's cheapest rung spent the mark and bought nothing, and then
    /// took the separators as well on the next pass. The logo now survives
    /// exactly as long as the card does, which is what `full_cards` already
    /// decides.
    pub fn project_h(&self) -> u16 {
        if !self.full_cards {
            return 1;
        }

        let mut meta = 6;

        if self.separators {
            meta += 3;
        }

        meta + 2 + Self::TITLE_GAP
    }

    /// SelectedTargetCard.
    ///
    /// ```text
    ///   Device Target / Platform ID / OS / Type   4
    ///   separators between the four fields        3   (optional)
    /// ```
    ///
    /// Four rows of content, down from eight. The active-status banner and the
    /// `❯ fvm flutter run -d ...` row went, with the blank each of them needed:
    /// every fact on the banner is in the table under it and the count it added is
    /// one by construction, and the command string was a description of an argv
    /// that `Session::spawn` builds for itself. Four rows, handed to the log
    /// window.
    pub fn target_h(&self) -> u16 {
        if !self.full_cards {
            return 1;
        }

        let mut body = 4;

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
    /// Height of the tracker, for `stages` rows.
    ///
    /// The count is passed in rather than assumed. It was a fixed six, which left
    /// blank rows below `Starting Flutter` for phases that had not been announced
    /// yet — reserving space for work nobody had asked for. The cost of taking it
    /// from the live list is that the card grows a row as each phase opens, and the
    /// log window below gives one up; that is honest, and cheaper than dead space.
    ///
    /// Charging exactly what is drawn is not optional: a row drawn past the
    /// charged height is clipped with no error at all (7.5).
    pub fn build_h(&self, stages: usize) -> u16 {
        if !self.full_build {
            return 1;
        }

        // One row minimum. `begin_build` opens `Starting Flutter` before Flutter
        // has printed anything, so a build always has at least one row — but a
        // zero here would collapse the card into its own borders.
        let stages = (stages.max(1) as u16).min(Self::MAX_STAGES);

        1 + 1 + stages + 2 + Self::TITLE_GAP
    }

    /// How many stacked blocks the frame has, including the flexible middle.
    ///
    /// Needed on its own because `Layout::spacing(1)` inserts a blank row
    /// *between* blocks, so the gap count is `blocks - 1` and not a constant.
    /// Hardcoding it at 4 undercounted the running view by one row, which the
    /// spec-arithmetic test caught.
    fn blocks(&self, state: State) -> u16 {
        // ProjectCard and the flexible middle are always present.
        //
        // The footer is not counted. It is split off before these are laid out so
        // that no gap precedes it, so it contributes its row to `chrome` but no
        // spacing.
        let mut n = 2;

        if state.has_target() {
            n += 1;
        }

        if state.has_build() {
            n += 1;
        }

        n
    }

    /// Rows of chrome this configuration costs in `state`.
    ///
    /// Excludes the flexible middle, which is what the log window or the device
    /// list expands into. Includes the blank rows between blocks, because those
    /// are just as unavailable to the log window as a border is.
    pub fn chrome(&self, state: State, stages: usize) -> u16 {
        let mut rows = self.project_h();

        if state.has_target() {
            rows += self.target_h();
        }

        if state.has_build() {
            rows += self.build_h(stages);
        }

        // Footer, always present.
        rows += 1;

        rows + self.blocks(state).saturating_sub(1)
    }

    /// Solve for the largest configuration that still leaves the log window
    /// its floor.
    pub fn solve(area: Rect, state: State, stages: usize) -> Self {
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

        while budget.chrome(state, stages) + floor > area.height {
            if !budget.concede(state) {
                break;
            }
        }

        budget
    }

    /// Human-readable report, used by `--rows`.
    pub fn describe(&self) -> String {
        let mut given_up = Vec::new();

        if !self.separators {
            given_up.push("separators");
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

    /// Stage rows a finished Android build leaves on the tracker: starting, Gradle,
    /// install, syncing, running.
    ///
    /// Five, not six — the Gradle phase adopts the generic `Preparing build` row
    /// rather than following it.
    ///
    /// The tracker's height follows this now, so it has to be stated rather than
    /// assumed; that is the whole point of passing it in.
    const DONE: usize = 5;

    fn area(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn full_chrome_matches_the_spec_arithmetic() {
        // Enumerated from the rows the cards actually draw, not estimated:
        // project 12, target 10, build 11, footer 1, three gaps.
        let chrome = Budget::full().chrome(State::Running, DONE);

        assert_eq!(
            chrome, 36,
            "enumerated from the rows each card actually draws"
        );
    }

    /// Everything cut from the static cards lands in the log window, and this is
    /// the arithmetic that says so.
    ///
    /// Three removals, eight rows: the prompt bar and its gap (four), and the
    /// target card's status banner and command string with the blank each needed
    /// (four).
    #[test]
    fn the_log_window_keeps_what_the_static_cards_gave_up() {
        let log_rows = |h: u16| {
            let plan = Budget::solve(area(106, h), State::Running, DONE);
            h - plan.chrome(State::Running, DONE)
        };

        // At the design target, full chrome now leaves 8 rows where it once left
        // none at all.
        assert_eq!(45 - Budget::full().chrome(State::Running, DONE), 9);
        assert!(log_rows(45) >= LOG_MIN, "{} rows", log_rows(45));

        // And a window tall enough to keep everything gains the same eight.
        let plan = Budget::solve(area(106, 60), State::Running, DONE);
        assert_eq!(plan, Budget::full());
        assert_eq!(60 - plan.chrome(State::Running, DONE), 24);
    }

    /// Guards the failure mode that adding the title gap caused: the Layout is
    /// split by these heights, so if `card()` grows and the budget does not, the
    /// bottom of every card is clipped in silence.
    #[test]
    fn card_heights_account_for_the_title_gap() {
        let full = Budget::full();
        let mut flat = full;
        flat.separators = false;

        // 6 content + 2 border + 1 title gap.
        assert_eq!(flat.project_h(), 9);

        // 4 content + 2 border + 1 title gap.
        assert_eq!(flat.target_h(), 7);

        // Separators add three rows to each.
        assert_eq!(full.project_h(), 12);
        assert_eq!(full.target_h(), 10);
    }

    #[test]
    fn log_floor_is_defended_at_the_design_target() {
        // At 106x45 the full layout would leave 5 rows, so it must concede.
        let budget = Budget::solve(area(106, 45), State::Running, DONE);
        let remaining = 45 - budget.chrome(State::Running, DONE);

        assert!(
            remaining >= LOG_MIN,
            "log window got {remaining} rows, floor is {LOG_MIN}, budget: {}",
            budget.describe()
        );
    }

    #[test]
    fn a_tall_window_keeps_everything() {
        let budget = Budget::solve(area(106, 60), State::Running, DONE);
        assert_eq!(budget, Budget::full());
    }

    #[test]
    fn concessions_happen_in_the_documented_order() {
        let mut b = Budget::full();

        b.concede(State::Building);
        assert!(!b.separators, "separators go first");

        b.concede(State::Building);
        assert!(!b.roomy_devices, "device rows go second");

        b.concede(State::Building);
        assert!(!b.full_build, "the build tracker collapses third");

        b.concede(State::Building);
        assert!(!b.full_cards, "the cards collapse last");

        assert!(!b.concede(State::Building), "nothing left to give up");
    }

    /// 6.2's footnote to the table: rung 3 is state-dependent.
    #[test]
    fn a_finished_build_pays_before_the_separators_do() {
        let mut b = Budget::full();

        b.concede(State::Running);
        assert!(
            !b.full_build,
            "a finished tracker is static, so it goes first"
        );
        assert!(b.separators, "and the separators survive it");

        b.concede(State::Running);
        assert!(!b.separators, "separators are next");
    }

    /// The regression this order exists for: an ordinary Ghostty window at 12px
    /// is 46 to 51 rows, and both cards used to lose their rules there the moment
    /// the app started logging.
    #[test]
    fn separators_survive_a_short_window_once_the_app_is_logging() {
        // Five stages: a finished iOS build, which is what the screenshot showed.
        for h in 46..=51 {
            let plan = Budget::solve(area(106, h), State::Running, 5);

            assert!(
                plan.separators,
                "{h} rows dropped the separators: {}",
                plan.describe()
            );

            assert!(
                plan.full_cards,
                "{h} rows collapsed the cards: {}",
                plan.describe()
            );

            let log = h - plan.chrome(State::Running, 5);
            assert!(log >= LOG_MIN, "{h} rows left the log window {log}");
        }
    }

    /// The build is the exception: while it runs, its stage list outranks them.
    ///
    /// 38 rows rather than something shorter because `BUILDING` has no log window
    /// yet, so it defends a floor of 3 and full chrome (36) fits until 39.
    #[test]
    fn a_running_build_keeps_its_stage_list_instead() {
        let plan = Budget::solve(area(106, 38), State::Building, 5);

        assert!(plan.full_build, "the stage list is what is moving");
        assert!(!plan.separators, "so the separators pay for the floor");
    }

    /// Every rung must actually reclaim rows in the state it applies to.
    ///
    /// This is the check the logo rung would have failed. It was documented as
    /// worth four rows and worth giving up first; it was worth none, because the
    /// artwork shares a row range with the metadata beside it.
    #[test]
    fn every_concession_reclaims_rows_where_it_applies() {
        // Running: no device list on screen, so the device-row rung is inert here
        // by design and is checked against the picker instead.
        let mut b = Budget::full();
        let before = b.chrome(State::Running, DONE);

        b.separators = false;
        assert!(
            b.chrome(State::Running, DONE) < before,
            "separators must reclaim rows"
        );

        let before = b.chrome(State::Running, DONE);
        b.full_build = false;
        assert!(
            b.chrome(State::Running, DONE) < before,
            "collapsing the build tracker must reclaim rows"
        );

        let before = b.chrome(State::Running, DONE);
        b.full_cards = false;
        assert!(
            b.chrome(State::Running, DONE) < before,
            "collapsing the cards must reclaim rows"
        );
    }

    #[test]
    fn states_without_logs_do_not_defend_a_floor() {
        // The picker has no log window, so it should keep its detail at a
        // height where the running view would already be conceding.
        let budget = Budget::solve(area(106, 34), State::MultipleDevices, DONE);
        assert!(budget.separators, "picker has no log floor to protect");
    }
}
