//! BuildPhaseTracker, DESIGN.md 3.4. States 6 and 7.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use crate::budget::Budget;
use crate::data::{App, Ending, State};
use crate::theme;
use crate::widgets::{alert_card, card, spread, strong, text};

/// Widest the filled bar itself may grow.
///
/// This applies to the bar graphic only, not to the row it sits on. A stage row
/// spans the card so its duration right-aligns to the border, but a progress bar
/// 118 columns long conveys nothing a 44-column one does not, and it turns a
/// glance into a scan.
///
/// The previous version capped the whole row at 66 columns, which is what left
/// this card ignoring the terminal: the border reached the window edge while
/// every duration stopped at column 66.
const BAR_MAX: u16 = 44;

/// Glyph, title and colour for the state the build is in.
///
/// One function because both the card and the collapsed row need the same answer,
/// and the collapsed row is what the card *becomes*. Two copies of this drifted
/// once already: the card said `BUILD FINISHED` while the row under it said
/// `build finished`, so a layout change also changed the wording.
///
/// The card ignores the glyph — `card()` supplies its own `◆` — and the collapsed
/// row, having no border to hang a title on, uses it.
///
/// The two glyphs for a run that has ended are Nerd Font rather than the `⏹` and
/// `⚠` that were here. Both are East Asian Ambiguous, and this row is the one place
/// where that is visible as an *indent* rather than as an overflow: the glyph is the
/// row's first cell, so a font giving it emoji presentation pushes the whole row a
/// half-cell right of the card borders above and below it. See `theme::GLYPH_WARN`.
/// Two components read this, and they split it exactly where `has_tracker` splits.
/// `BUILDING` and `BUILD FAILED` are drawn here, as the tracker card's own title.
/// The other four — `RUNNING`, `STOPPED`, `DETACHED`, `DISCONNECTED` — are drawn by
/// the target card as a pill, because in those states there is no tracker block and
/// all four are statements about the device rather than about a build (3.2).
///
/// One function for both, so the two cards cannot disagree about what a state is
/// called. They did once, when this was two copies: the card said `BUILD FINISHED`
/// while the row beneath it said `build finished`.
pub(super) fn status(app: &App) -> (&'static str, &'static str, ratatui::style::Color) {
    match app.state {
        State::BuildFailed => ("✖", "BUILD FAILED", theme::ROSE),
        // Muted rather than rose: the run ending was asked for, and colouring it
        // like a failure would make a deliberate stop read as something going wrong.
        //
        // Two words for one state, because the device is left in opposite conditions.
        // After `^S` the app is gone. After Flutter's own `d` the app is still running
        // and only the tooling let go, which is worth knowing before pressing `r` and
        // wondering why the screen you are looking at is still there.
        State::Stopped if app.ending == Some(Ending::Detached) => {
            (theme::GLYPH_STOP, "DETACHED", theme::MUTED)
        }
        // Rose, with the rest of the failures. This was amber, on the argument that
        // rose is for something broken and muted is for a stop that was asked for,
        // and this is neither — nothing frun did failed, but nothing asked for it
        // either.
        //
        // The argument was about the wrong axis. Amber is not the colour between
        // those two here, it is the colour of *in progress*: the `BUILDING` spinner,
        // the pending stage rows, the reload note, the ` in use ` chip, the `^S`
        // hint. A terminal state wearing it competes with five live ones, and losing
        // a device mid-run is the last thing that should read as still working.
        //
        // It still cannot be narrowed further, because a device switched off and an
        // app that crashed arrive as the same event. The word says what closed, not
        // why; the `ERR` line in the log is where the reason lives.
        State::Stopped if app.ending == Some(Ending::Lost) => {
            (theme::GLYPH_WARN, "DISCONNECTED", theme::ROSE)
        }
        State::Stopped => (theme::GLYPH_STOP, "STOPPED", theme::MUTED),
        // `RUNNING`, and this arm used to read `✔ BUILD FINISHED`.
        //
        // That wording had no reader left. The tracker block is not laid out once a
        // build succeeds (`has_tracker`), so nothing could draw it — a state this
        // function claims to describe with a word nothing on screen could show.
        //
        // The target card's pill is what reads it now, and there the subject is the
        // run rather than the build it came out of: the pill's other three words are
        // `STOPPED`, `DETACHED` and `DISCONNECTED`, so a fourth saying the build
        // finished would be answering a different question from its neighbours. It is
        // also the word the device list already uses for this exact fact — ` running `
        // marks the row your app is on.
        //
        // A play triangle rather than the `✔` that was here, paired with the stop
        // square on the way out. A tick means *done*, which is the build; this is the
        // one state where something is still happening.
        s if s.build_done() => (theme::GLYPH_PLAY, "RUNNING", theme::EMERALD),
        State::Switching if app.run_state().holds_session() => {
            (theme::GLYPH_PLAY, "RUNNING", theme::EMERALD)
        }
        State::Switching => (theme::GLYPH_STOP, "STOPPED", theme::MUTED),
        _ => (app.spinner(), "BUILDING", theme::AMBER),
    }
}

/// The build's figures, as the log card's first row carries them.
///
/// Shared with the collapsed row for the same reason `status` is: the row inherits
/// the card's right-hand group, so the numbers keep both their wording and their
/// horizontal position when the card goes.
///
/// And shared with the log card, which is where the group lives for the whole of a
/// run now that the tracker block is not on screen for one (3.4). Two call sites,
/// one wording: the figures are the same figures wherever they surface, and a
/// rename cannot reach one of them and miss the others.
///
/// **Not the full card's own title bar**, which is the one frame where the tracker
/// is on screen holding `Starting Flutter` and `Syncing files` as rows. It carries
/// `running_clock` instead — see there.
///
/// In clock order, because the figures are consecutive rather than nested:
/// `Startup` runs from the spawn to Flutter's first line, `Build time` from there
/// to the interactive session, and `Sync` is the last phase inside it. Exactly one
/// of the first two is counting at any moment, so reading them left to right is
/// reading the wait in the order it happened.
/// Labels in `EMERALD`, values in `TEXT`. The row sits above a stream where every
/// line opens with a timestamp and a level badge, so the words need a colour of their
/// own to be found at a glance, while white keeps the figures readable as values.
/// Emerald is the palette's settled-and-fine hue, which is what this summary names.
///
/// Three blanks between the groups, where a `│` rule in `BORDER` used to be. On a
/// border row that rule read as part of the frame; on a content row it read as a
/// table nobody asked for. Same three columns either way, so nothing reflows.
pub(super) fn timings(app: &App) -> Vec<Span<'static>> {
    vec![
        // Live while the toolchain boots, frozen once Flutter speaks. A dash first,
        // because until the pty says something even the startup span is unmeasured.
        text("Startup ", theme::EMERALD),
        strong(app.startup_clock(), theme::TEXT),
        // Live while building, final once it is not. A build time that only
        // appears at the end tells you nothing during the wait that matters.
        text("   Build ", theme::EMERALD),
        strong(app.build_clock(), theme::TEXT),
        text("   Sync ", theme::EMERALD),
        strong(app.sync_time.clone(), theme::TEXT),
        text("   Total ", theme::EMERALD),
        strong(app.total_clock(), theme::TEXT),
    ]
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, plan: &Budget) {
    if !plan.full_build {
        collapsed(frame, area, app);
        return;
    }

    let (_, title, color) = status(app);

    // **No figure on this border, deliberately.** Every number a build produces is a
    // row of the tracker immediately below it — `Starting Flutter`, the platform
    // phase, `Syncing files` — and the rows are measured open-to-open so they
    // partition the build and sum to it. A total on the border was a second copy of
    // that sum, and a `Startup` or `Sync` there was a second copy of one row.
    //
    // The group still exists for the two frames that have no rows to read: the
    // collapsed row this card becomes, and the log card once the build is over. See
    // `timings`.
    let block = card(title, color);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Slot model: the blank between the bar and the stage list is its own slot.
    // Charged in `Budget::build_h`, without which the Layout would keep the old
    // height and clip the last stage.
    let rows = Layout::vertical([
        Constraint::Length(1), // progress bar
        Constraint::Length(1), // blank
        Constraint::Min(1),    // stage list
    ])
    .split(inner);

    progress(frame, rows[0], app);
    stages(frame, rows[2], app);
}

/// Filled bar with a step counter, against the stage count the platform implies.
///
/// The denominator used to be `app.stages.len()`, which is the number of stages
/// *announced so far*. That runs the bar backwards: at `Flutter started` it is 1
/// of 1 and shows full, and when Gradle appears it becomes 1 of 2 and drops to
/// half.
///
/// The fix is that the total was knowable all along. The platform is chosen
/// before the build starts and 3.4's trigger table is per-platform, so
/// `Platform::stage_count` is the answer. It is an upper bound, never a floor,
/// which is the direction that keeps the bar honest: a skipped stage leaves it
/// short of full until completion fills it, rather than reaching 100% early and
/// carrying on.
fn progress(frame: &mut Frame, area: Rect, app: &App) {
    let done = app.stages.iter().filter(|s| s.done).count();
    let total = app.expected_stages();
    let finished = app.state.build_done();

    // The bar is bounded; the row it sits on is not.
    let bar_w = area.width.saturating_sub(24).min(BAR_MAX) as usize;

    // Which stage we are *on*, not how many have closed.
    //
    // It used to be the closed count, which read `Stage 0/6` for the whole of the
    // first stage — a build that has been running for four seconds reporting that
    // nothing has happened. The row that is currently spinning is the stage you
    // are on, so it counts.
    //
    // On completion the denominator collapses to what actually ran, so the row
    // reads `6/6` rather than `5/6`. The estimate is an upper bound and has done
    // its job by then: iOS skips CocoaPods when `Podfile.lock` is current and
    // finishes in five, and ending on `5/6` would report a build that stopped
    // short of itself.
    let (reached, denominator) = if finished {
        let ran = app.stages.len();
        (ran, ran)
    } else {
        ((done + 1).min(total), total)
    };

    // Same two numbers the label shows, so the bar cannot disagree with it.
    let filled = bar_w * reached / denominator.max(1);

    let colour = if finished {
        theme::EMERALD
    } else {
        theme::AMBER
    };

    let right = vec![
        text("Stage ", theme::MUTED),
        strong(format!("{reached}"), colour),
        text(format!("/{denominator}"), theme::MUTED),
    ];

    let line = spread(
        area.width,
        vec![
            text("[", theme::BORDER),
            strong(
                "▓".repeat(filled),
                if finished {
                    theme::EMERALD
                } else {
                    theme::AMBER
                },
            ),
            text("░".repeat(bar_w.saturating_sub(filled)), theme::BORDER),
            text("]", theme::BORDER),
        ],
        right,
    );

    frame.render_widget(Paragraph::new(line), area);
}

fn stages(frame: &mut Frame, area: Rect, app: &App) {
    // Full width, so durations right-align to the card border.
    let w = area.width;

    let lines: Vec<Line> = app
        .stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let last = i + 1 == app.stages.len();
            let failed = app.state == State::BuildFailed && last;

            let (glyph, color) = if failed {
                ("✖", theme::ROSE)
            } else if stage.done {
                ("✔", theme::EMERALD)
            } else {
                (app.spinner(), theme::AMBER)
            };

            // A clock on the stage still running, once it has been running long
            // enough to wonder about. `frun-runner` had this and the reason it
            // gave still holds: the spinner cycles the same frames whether the
            // stage is working or wedged, and the elapsed time is the difference.
            //
            // Withheld for the first few seconds so a normal fast stage does not
            // flash a number on its way past.
            // Between stages there is no pending row at all: `Flutter started`
            // gets its tick and the next stage has not been announced yet, so
            // the clock had nothing to hang on and the gap passed unmarked. On
            // iOS that gap is the Xcode build and it is long.
            //
            // So the last completed row keeps counting until the next stage
            // opens, then freezes at the gap it measured. That is the same rule
            // 7.7 sets for marker stages, applied while it is still running.
            // Ticks from zero while the row is open, freezes at its measured
            // figure once the next stage closes it. One formatter for both, so
            // `1.8s` running becomes `1.9s` frozen without switching units
            // mid-life.
            //
            // The `waiting` case this replaces — a completed bottom row that kept
            // counting — is unreachable now: a row is not closed until its
            // successor opens, so the bottom row of a running build is never
            // `done`. The three-second delay before a clock appeared went with it;
            // a stage that has just opened reads `0ms` and starts moving, which is
            // what "the timer runs while the phase runs" means.
            let right = if stage.done || failed {
                stage.duration.clone()
            } else {
                crate::flutter::elapsed(stage.started.elapsed())
            };

            spread(
                w,
                vec![
                    strong(glyph, color),
                    Span::raw(" "),
                    text(stage.label.as_str(), theme::TEXT),
                ],
                vec![text(right, theme::MUTED)],
            )
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

/// What the tracker becomes once the build has settled, and rung 3 of the ladder
/// while one is still running.
///
/// ```text
/// ✔ BUILD FINISHED           Startup 3.6s   Build time 20.2s   Sync 68ms
/// ```
///
/// Reached by state rather than by size in the case that matters. Every row the
/// full card holds after the build is over is frozen — five labels, five
/// durations, and a progress bar reading `Stage 5/5` in emerald directly under a
/// title already saying `BUILD FINISHED` — so it cannot justify nine rows while
/// the log window is the only region still changing. `Budget::solve` switches
/// `full_build` off before the ladder is consulted.
///
/// Same words and same colour as the card's own title, from `status`, and the
/// same right-hand group, from `timings`. The collapse then reads as the card
/// closing rather than as a different component appearing: the two numbers stay
/// where they were, give or take the border and its padding.
///
/// **What this gives up is the per-stage breakdown.** The tracker is the only
/// place it exists — the `BLD` and `OK` log levels were removed on the grounds
/// that the tracker owned those facts — so `Building with Xcode 14.5s` is
/// unreadable for the rest of the session. Accepted: that figure is watched while
/// the row is spinning, which is when the full card is on screen, and what is kept
/// here is the total it rolls up into.
fn collapsed(frame: &mut Frame, area: Rect, app: &App) {
    let (glyph, label, color) = status(app);

    let line = spread(
        area.width,
        vec![strong(glyph, color), Span::raw(" "), strong(label, color)],
        timings(app),
    );

    frame.render_widget(Paragraph::new(line), area);
}

/// State 7 detail: the compiler output, with a code frame when there is one.
pub fn render_failure(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(failure) = &app.failure else {
        return;
    };

    // "COMPILER ERROR" only when it is one. A Gradle dependency failure, a
    // signing error or a missing toolchain names no source position, and calling
    // those a compiler error sends you looking in the wrong place.
    let title = if failure.location.is_some() {
        "COMPILER ERROR"
    } else {
        "BUILD ERROR"
    };

    let block = alert_card(title, theme::ROSE)
        .title_top(
            Line::from(vec![
                Span::raw(" "),
                text("Exit code ", theme::MUTED),
                strong(app.exit_code.to_string(), theme::ROSE),
                Span::raw(" "),
            ])
            .right_aligned(),
        )
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    for chunk in crate::widgets::wrap(&failure.summary, inner.width as usize) {
        lines.push(Line::from(strong(chunk, theme::ROSE)));
    }

    // The position, split out from the summary so it is machine-shaped rather
    // than prose: this is what was read to build the code frame, and what an
    // editor jump would consume.
    if let Some((file, line, column)) = &failure.location {
        lines.push(Line::from(vec![
            text(file.as_str(), theme::MUTED),
            text(":", theme::BORDER),
            strong(line.to_string(), theme::TEXT),
            text(":", theme::BORDER),
            strong(column.to_string(), theme::TEXT),
        ]));
    }

    lines.push(Line::default());

    // The lines either side of the reported position, read from the file. One
    // line of context is usually the difference between recognising the mistake
    // and opening the editor.
    let hot_line = failure.location.as_ref().map(|(_, line, _)| *line);

    for (number, source) in &failure.context {
        let hot = Some(*number) == hot_line;

        lines.push(Line::from(vec![
            text(
                format!("{number:>4} "),
                if hot { theme::ROSE } else { theme::MUTED },
            ),
            if hot {
                strong(source.as_str(), theme::TEXT)
            } else {
                text(source.as_str(), theme::MUTED)
            },
        ]));
    }

    if !failure.context.is_empty() && failure.caret_col > 0 {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(5 + failure.caret_col.saturating_sub(1) as usize)),
            strong("^", theme::ROSE),
        ]));
    }

    // No code frame: the tail of what the build printed takes its place. Showing
    // nothing here would leave the one screen whose whole job is explaining the
    // failure with nothing but an exit code.
    if failure.context.is_empty() {
        for out in &failure.output {
            for chunk in crate::widgets::wrap(out, inner.width as usize) {
                lines.push(Line::from(text(chunk, theme::MUTED)));
            }
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(text(failure.note.as_str(), theme::AMBER)));

    // No in-card action row. `[r] Retry Build  [q] Quit` used to close this card,
    // costing two rows — the row itself and the blank above it — and reserving a
    // third by truncating the message to leave room for it. The footer already
    // carries both keys in `BUILD_FAILED`, with the same clickable region behind
    // `[r]`, so the card was competing with the cheatsheet for the one screen
    // whose whole job is showing as much of the compiler's output as will fit.
    //
    // The message now gets the full card. What used to be cut was the oldest build
    // output, which on a Gradle failure is where the actual cause usually is.
    lines.truncate(inner.height as usize);

    frame.render_widget(Paragraph::new(lines), inner);
}
