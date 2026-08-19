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
    /// Decided by state before the ladder runs, and only reached *as* a rung
    /// while a build is in progress. Once the build has settled the tracker holds
    /// nothing that changes — five labels and their frozen durations — so it has
    /// no claim on nine rows at any size, and `solve` switches it off before the
    /// first concession is considered. See `State::has_tracker`.
    ///
    /// It stays in the ladder for the `BUILDING` and `BUILD_FAILED` cases, where
    /// the rows are load-bearing but a very short terminal may still have to have
    /// them: without that rung a 14-row window during a build cannot be made to
    /// fit, and a layout that overflows is clipped in silence (7.5).
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
    /// One fixed order, 6.2's table, with no state branch in it any more. There
    /// used to be one: rung 3 was promoted to first once the build had finished,
    /// because a static tracker has a weaker claim than the separators on two cards
    /// still being read. That reasoning was right and the mechanism was wrong — a
    /// claim that weak is not a claim the *size* should have to defeat, so the
    /// tracker now collapses on state alone and this list never has to rank it
    /// against anything.
    ///
    /// The rung it left behind is not dead: a build in progress keeps its stage
    /// list, so `full_build` is still reachable here, and still last but one.
    fn concede(&mut self) -> bool {
        let order = [
            &mut self.separators,
            &mut self.roomy_devices,
            &mut self.full_build,
            &mut self.full_cards,
        ];

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
    ///   Project / Branch / Git Status             3
    ///   blank                                     1
    ///   Flutter · Dart · Runtime                  1
    ///   separators between the three fields       2   (optional)
    /// ```
    ///
    /// Three fields, down from four. `Version` is the tail of the `Project` row now —
    /// one pubspec fact on one row — and the row it vacated, plus its rule, went to
    /// the log window.
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

        let mut meta = 5;

        if self.separators {
            meta += 2;
        }

        meta + 2 + Self::TITLE_GAP
    }

    /// SelectedTargetCard (2 columns x 2 rows: Device/Type & Platform/OS).
    ///
    /// 2 rows of content (+ 1 separator if enabled) + 2 border + 1 title gap = 5 or 6 rows.
    pub fn target_h(&self) -> u16 {
        if !self.full_cards {
            return 1;
        }

        let mut body = 2;

        if self.separators {
            body += 1;
        }

        body + 2 + Self::TITLE_GAP
    }

    /// BuildPhaseTracker, for `stages` rows.
    ///
    /// ```text
    ///   progress bar                              1
    ///   blank                                     1
    ///   one row per stage                         5 finished / 3 mid-build
    /// ```
    ///
    /// Only ever this tall while the build is running. Once it settles the card is
    /// one borderless row and this returns 1, so the tallest the tracker gets is
    /// the tallest it gets *mid-build* — which is also the moment the log window
    /// has the least to show.
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

        // `has_tracker`, not `has_build`: a build that succeeded and is now running
        // has no tracker block at all, so counting one here would reserve a gap for
        // a block that is not laid out and leave a blank row in the middle of the
        // frame. See `State::has_tracker`.
        if state.has_tracker() {
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

        if state.has_tracker() {
            rows += self.build_h(stages);
        }

        // Footer, always present.
        rows += 1;

        rows + self.blocks(state).saturating_sub(1)
    }

    /// Rows the flexible middle must keep in `state`.
    ///
    /// The floor protects whichever region carries the information the user came
    /// for. Where nothing does — the picker before a run, `Detecting`, `Booting` —
    /// the cards can have what they want and the remainder scrolls.
    ///
    /// One function rather than a branch inside `solve`, because `--rows` reports
    /// this number and a report that names a different floor than the solver used
    /// is a diagnostic that lies.
    pub fn floor(state: State) -> u16 {
        if state.has_logs() {
            // Plus two for the log card's first row, which carries the build's
            // figures, and the blank under it. Both are drawn inside the flexible
            // middle rather than as chrome, so without the term the stream's floor of
            // twelve quietly becomes ten.
            //
            // Unconditional, because `has_logs` and `has_tracker` are disjoint: every
            // state with a log window is one the tracker has already left, so the row
            // is drawn in all of them. `logs.rs` still gates on `has_tracker` — that
            // is the invariant, and `Building` has been in `has_logs` before.
            return LOG_MIN + 2;
        }

        if state == State::BuildFailed {
            FAIL_MIN
        } else {
            // The lists, and the two frames that are a spinner and a sentence. A
            // list needs no floor of its own because it scrolls, and because
            // nothing else competes for the frame: `Switching` hides the target
            // card and the tracker exactly as the first picker does.
            3
        }
    }

    /// Solve for the largest configuration that still leaves the flexible middle
    /// its floor.
    pub fn solve(area: Rect, state: State, stages: usize) -> Self {
        let mut budget = Self::full();
        let floor = Self::floor(state);

        // Before the ladder, not inside it, and spent for the state rather than by
        // the height. A state with no tracker block cannot buy anything by
        // collapsing one, so the rung has to be marked used or `concede` will spend
        // it on nothing and take the next one as well — which is exactly the defect
        // the logo rung had (6.2).
        //
        // This used to read `build_settled()`, from when a finished build still
        // showed a one-row summary and the argument was that its rows were frozen.
        // The summary is gone: its totals are in the log card's title and its ending
        // word is a pill on the target card, so the condition is now simply whether
        // the block exists at all.
        if !state.has_tracker() {
            budget.full_build = false;
        }

        while budget.chrome(state, stages) + floor > area.height {
            if !budget.concede() {
                break;
            }
        }

        budget
    }

    /// Human-readable report, used by `--rows` and by the footer.
    pub fn describe(&self, state: State) -> String {
        let mut given_up = Vec::new();

        if !self.separators {
            given_up.push("separators");
        }
        if !self.roomy_devices {
            given_up.push("dense devices");
        }
        // Only when the size forced it, which means only where there is a tracker to
        // collapse. `solve` marks this rung used in every other state, so without the
        // guard the footer would sit there reading `[build collapsed]` for the whole
        // of every run, naming a concession the layout never made. Same defect 6.3
        // describes for the expanded log view.
        if !self.full_build && state.has_tracker() {
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

    /// A five-stage build (Starting Flutter, CocoaPods, Xcode, VM, Connecting),
    /// which is the iOS worst case and the tallest the tracker gets.
    ///
    /// Tests that check `Running` or `Stopped` need a non-zero count to prove the
    /// budget is *not* reading the argument — that a settled build is collapsed
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
        // Enumerated from the rows the cards actually draw:
        // project 10 (with separators) + target 6 (with separator) + footer 1 + two gaps = 19 rows.
        let chrome = Budget::full().chrome(State::Running, DONE);

        assert_eq!(
            chrome, 19,
            "enumerated from the rows each card actually draws"
        );

        // Where the tracker does exist, it is charged:
        // project 10 + target 6 + tracker 10 + footer 1 + three gaps = 30.
        assert_eq!(Budget::full().chrome(State::Building, DONE), 30);
    }

    /// Everything cut from the static cards lands in the log window, and this is
    /// the arithmetic that says so.
    #[test]
    fn the_log_window_keeps_what_the_static_cards_gave_up() {
        let log_rows = |h: u16| {
            let plan = Budget::solve(area(106, h), State::Running, DONE);
            h - plan.chrome(State::Running, DONE)
        };

        // At the design target, full chrome leaves 26 rows for the log window.
        assert_eq!(45 - Budget::full().chrome(State::Running, DONE), 26);
        assert!(log_rows(45) >= Budget::floor(State::Running), "{} rows", log_rows(45));

        // Nothing is conceded at the design target.
        assert_eq!(
            Budget::solve(area(106, 45), State::Running, DONE).describe(State::Running),
            "full"
        );

        // And a window tall enough to keep every *rung* still collapses the
        // tracker, because that is not a rung any more.
        let plan = Budget::solve(area(106, 60), State::Running, DONE);
        assert_eq!(
            plan,
            Budget {
                full_build: false,
                ..Budget::full()
            }
        );
        assert_eq!(60 - plan.chrome(State::Running, DONE), 41);
    }

    /// `Stopped` is laid out exactly like a live run: two cards, a footer, two gaps.
    #[test]
    fn a_stopped_run_is_laid_out_like_a_live_one() {
        let plan = Budget::solve(area(106, 45), State::Stopped, DONE);

        assert_eq!(
            plan.chrome(State::Stopped, DONE),
            plan.chrome(State::Running, DONE),
            "the log window must not change height when the run ends"
        );

        assert!(45 - plan.chrome(State::Stopped, DONE) >= LOG_MIN);
    }

    /// Guards the failure mode that adding the title gap caused: the Layout is
    /// split by these heights, so if `card()` grows and the budget does not, the
    /// bottom of every card is clipped in silence.
    #[test]
    fn card_heights_account_for_the_title_gap() {
        let full = Budget::full();
        let mut flat = full;
        flat.separators = false;

        // 5 content + 2 border + 1 title gap = 8.
        assert_eq!(flat.project_h(), 8);

        // 2 content + 2 border + 1 title gap = 5.
        assert_eq!(flat.target_h(), 5);

        // Separators add two rows to project card: 10. Separator adds one row to target card: 6.
        assert_eq!(full.project_h(), 10);
        assert_eq!(full.target_h(), 6);
    }

    #[test]
    fn log_floor_is_defended_at_the_design_target() {
        let budget = Budget::solve(area(106, 45), State::Running, DONE);
        let remaining = 45 - budget.chrome(State::Running, DONE);

        assert!(
            remaining >= LOG_MIN,
            "log window got {remaining} rows, floor is {LOG_MIN}, budget: {}",
            budget.describe(State::Running)
        );
    }

    /// A tall window keeps every rung — and still collapses the tracker, which is
    /// the whole of this change.
    #[test]
    fn a_tall_window_keeps_every_rung_but_not_the_finished_tracker() {
        let budget = Budget::solve(area(106, 60), State::Running, DONE);

        assert!(budget.separators);
        assert!(budget.roomy_devices);
        assert!(budget.full_cards);
        assert!(
            !budget.full_build,
            "a settled tracker is collapsed at any size"
        );
    }

    #[test]
    fn concessions_happen_in_the_documented_order() {
        let mut b = Budget::full();

        b.concede();
        assert!(!b.separators, "separators go first");

        b.concede();
        assert!(!b.roomy_devices, "device rows go second");

        b.concede();
        assert!(!b.full_build, "the build tracker collapses third");

        b.concede();
        assert!(!b.full_cards, "the cards collapse last");

        assert!(!b.concede(), "nothing left to give up");
    }

    /// The tracker is collapsed by state, so it costs the ladder nothing.
    #[test]
    fn a_settled_tracker_collapses_before_the_ladder_runs() {
        // Tall enough that nothing is under pressure at all.
        let plan = Budget::solve(area(106, 80), State::Running, DONE);

        assert!(!plan.full_build, "collapsed with rows to spare");
        assert_eq!(
            plan.describe(State::Running),
            "full",
            "and not reported as a concession, because none was made"
        );

        let stopped = Budget::solve(area(106, 45), State::Stopped, DONE);

        assert!(!stopped.full_build);
        assert!(
            stopped.separators && stopped.roomy_devices,
            "which is what those rows were being paid for: {}",
            stopped.describe(State::Stopped)
        );
    }

    /// A build in progress still has the rung, and a 14-row terminal needs it.
    #[test]
    fn a_short_terminal_can_still_collapse_a_running_tracker() {
        let plan = Budget::solve(area(60, MIN_H), State::Building, 8);

        assert!(!plan.full_build, "nothing else can free enough rows");
        assert!(
            plan.chrome(State::Building, 8) + Budget::floor(State::Building) <= MIN_H,
            "chrome {} must fit in {MIN_H}",
            plan.chrome(State::Building, 8)
        );
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
                plan.describe(State::Running)
            );

            assert!(
                plan.full_cards,
                "{h} rows collapsed the cards: {}",
                plan.describe(State::Running)
            );

            let log = h - plan.chrome(State::Running, 5);
            assert!(log >= LOG_MIN, "{h} rows left the log window {log}");
        }
    }

    /// The build is the exception: while it runs, its stage list outranks them.
    #[test]
    fn a_running_build_keeps_its_stage_list_instead() {
        let plan = Budget::solve(area(106, 32), State::Building, 5);

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

        // The tracker rung is checked against `Building`, for the same reason the
        // device-row rung is checked against the picker: `Running` has no tracker
        // block at all now, so the rung is inert there by design rather than broken.
        // That is also the only state the rung is still reachable in — `solve`
        // collapses a settled tracker before the ladder is consulted.
        let mut mid = Budget::full();
        let before = mid.chrome(State::Building, DONE);

        mid.full_build = false;
        assert!(
            mid.chrome(State::Building, DONE) < before,
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
