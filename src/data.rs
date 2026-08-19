//! Application state.
//!
//! Two ways in. `App::live` reads the machine and is driven by Flutter;
//! `App::new` fills every field with the mock data the design was judged
//! against and is driven by `--dump`, `--all`, `--rows`, `--hits` and `--demo`.
//!
//! The mock path is not leftover scaffolding. Layout bugs here are silent (see
//! DESIGN.md 7.4), and rendering all eleven states at a dozen sizes without a
//! device attached is the only way to catch them.

use std::time::Instant;

use ratatui::layout::Rect;

use crate::probe::{self, Boot};
use crate::theme;

// The three types the UI needs that are defined next to the command output they
// are parsed from. Re-exported so a widget imports one module, not two.
pub use crate::probe::{Device, Platform};

// ============================================================
// States
// ============================================================

/// The eleven frames from DESIGN.md section 4, in flow order, and the switch list
/// from 8.5 after them — it is reachable from six of the eleven rather than from
/// one, so it has no place in the flow order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// 1. Discovery is running (8.9).
    Detecting,
    /// 3. Booting a simulator or emulator, possibly for minutes.
    ///
    /// State 2 is gone, and the numbers of the others are left alone: they are labels
    /// this file shares with DESIGN.md, not indices into `ALL`. `NO_DEVICES` offered
    /// every launchable target under `Nothing is attached`, which is what the picker
    /// has done since the lists were merged (7.6) — with the same rows, in the same
    /// order, and with ` active ` and ` last used ` saying what its amber heading said.
    /// It was also unreachable: `devices_answered` branched on `Device::attached`, and
    /// that counts every iOS and Android row including the bootable ones, so one
    /// installed simulator was enough to make the picker the only answer.
    Booting,
    /// 4. Two or more devices attached; pick one.
    MultipleDevices,
    /// 5. Exactly one; no picker is shown.
    SingleDevice,
    /// 6. `flutter run` is building.
    Building,
    /// 7. The build died before an interactive session opened.
    BuildFailed,
    /// 8. Interactive session live, app logs streaming.
    Running,
    /// 9. `r` or `R` acknowledged, operation in progress.
    ReloadInFlight,
    /// 10. Flutter accepted the key and the operation failed.
    ReloadFailed,
    /// 11. Flutter never acknowledged the key at all.
    ReloadDropped,
    /// 13. `^S`: the run is over, frun is not.
    ///
    /// The one frame that exists with no child behind it. Every other state
    /// assumes a run is coming or here, which is why stopping used to mean leaving:
    /// there was nowhere to stand. The log stays readable, the device stays booted,
    /// and `r` builds again.
    Stopped,
    /// 12. `^D`: the target list, reopened over a run that is still alive (8.5).
    ///
    /// Not `MultipleDevices` with a flag on it. The frame differs in four places —
    /// its title, the badge on the row already running, what `Esc` means, and the
    /// three cards that stay on screen — and a state is what the `--dump` harness
    /// can reach. A field on another state is a frame nothing can render on
    /// demand, which is the same as a frame nobody checks.
    Switching,
}

impl State {
    pub const ALL: [State; 12] = [
        State::Detecting,
        State::Booting,
        State::MultipleDevices,
        State::SingleDevice,
        State::Building,
        State::BuildFailed,
        State::Running,
        State::ReloadInFlight,
        State::ReloadFailed,
        State::ReloadDropped,
        State::Switching,
        State::Stopped,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            State::Detecting => "detecting",
            State::Booting => "booting",
            State::MultipleDevices => "picker",
            State::SingleDevice => "single",
            State::Building => "building",
            State::BuildFailed => "build-failed",
            State::Running => "running",
            State::ReloadInFlight => "reload",
            State::ReloadFailed => "reload-failed",
            State::ReloadDropped => "reload-dropped",
            State::Switching => "switch",
            State::Stopped => "stopped",
        }
    }

    pub fn from_slug(s: &str) -> Option<State> {
        State::ALL.into_iter().find(|st| st.slug() == s)
    }

    /// Whether a device has been chosen, which decides if the
    /// SelectedTargetCard has anything to show.
    pub fn has_target(self) -> bool {
        !matches!(
            self,
            State::Detecting | State::Booting | State::MultipleDevices
        )
    }

    /// Whether the build tracker is on screen.
    ///
    /// Not during `Switching`. The tracker there describes a build that is about to
    /// be thrown away, and the rows are worth more to the list of devices that
    /// replaces it. Code asking what the *run* is doing asks `App::run_state()`,
    /// which is a different question from what is on screen.
    pub fn has_build(self) -> bool {
        matches!(
            self,
            State::Building
                | State::BuildFailed
                | State::Running
                | State::ReloadInFlight
                | State::ReloadFailed
                | State::ReloadDropped
                | State::Stopped
        )
    }

    /// Whether the BuildPhaseTracker occupies a block of the frame.
    ///
    /// Only while a build is happening or has just broken. Narrower than
    /// `has_build`, and the two must not be merged: `has_build` is "there is a build
    /// behind this frame", which is what the target card's `^D` hint and
    /// `Action::StopRun` ask about and which stays true throughout a run, while this
    /// is "the tracker has something to say" — true for a small part of one.
    ///
    /// Everything the tracker held after a build stopped moving has been rehoused,
    /// and each fact went to the component that owns it:
    ///
    /// * **The two totals** are in the log card's title bar (3.5). A build total is a
    ///   fact about the session, and the log window is the region it describes.
    /// * **How the run ended** — `STOPPED`, `DETACHED`, `DISCONNECTED` — is a pill on
    ///   the target card's control row (3.2). Those three words are statements about
    ///   the *device*: after `d` the app is still live on it, after `^S` it is gone,
    ///   and after a `Lost` the connection to it is what broke.
    ///
    /// Both new homes are rows that already exist, so the block's row and the blank
    /// above it go to the log window — and, more to the point, the log window no
    /// longer changes height when a run ends. It used to grow a banner and push
    /// everything in the stream down two rows, at the moment the user is most likely
    /// to be reading it.
    pub fn has_tracker(self) -> bool {
        matches!(self, State::Building | State::BuildFailed)
    }

    /// Whether the log stream is on screen.
    ///
    /// Includes `Building`. It used to be excluded on the grounds that nothing
    /// has printed yet, which is not true: Flutter prints throughout, and the
    /// eight seconds between `Launching lib/main.dart` and the first Gradle line
    /// are full of Impeller notices, daemon messages and warnings. The space was
    /// being spent instead on a placeholder reading `Waiting for the application
    /// to start...`, which said nothing the spinner above was not already saying.
    ///
    /// Still excluded on build failure, where the failure card takes the space and
    /// the compiler output is the only output that matters.
    pub fn has_logs(self) -> bool {
        matches!(
            self,
            // Not during BUILDING. The card was showing there to fill the
            // startup gap, but it arrives empty and stays nearly empty while the
            // rows would be better spent on the stage list. The elapsed clock on
            // the pending stage now marks that gap instead, which makes it
            // load-bearing rather than a nicety.
            //
            // `Stopped` included, and it is the point of that state: the log is the
            // reason to stop without leaving.
            State::Running
                | State::ReloadInFlight
                | State::ReloadFailed
                | State::ReloadDropped
                | State::Stopped
        )
    }

    /// Whether a hot reload or restart is being reported on the status row.
    pub fn reloading(self) -> bool {
        matches!(
            self,
            State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped
        )
    }

    /// Whether the interactive session is live, so a key has something to reach.
    ///
    /// Used for far more than the tracker: `q` is only forwardable once Flutter is
    /// reading keys, and a reload before that would be sent to Gradle. Which is why
    /// `Stopped` is not here — there is no child behind that frame — and why the
    /// tracker asks `has_tracker` instead.
    pub fn build_done(self) -> bool {
        matches!(
            self,
            State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped
        )
    }

    /// Whether there is still a child behind this frame — something a switch would
    /// be leaving, and something `Esc` could return to.
    ///
    /// `has_build()` is not the same question and using it for this was the bug: it
    /// includes `BuildFailed`, where the pty has already closed. A device switched
    /// away from after a failed build was still marked ` running ` in the list, with
    /// a ` ⏎ Keep ` offering to keep a session that had exited — the same falsehood
    /// `Stopped` was already excluded for, arrived at one state earlier.
    ///
    /// `Building` counts: the child is alive and mid-build, and `Esc` puts the
    /// tracker back exactly where the parser has moved it to.
    pub fn holds_session(self) -> bool {
        matches!(self, State::Building) || self.build_done()
    }

    // `build_settled()` was here — `build_done() || self == Stopped`, "the tracker's
    // rows have stopped moving, which is what collapses it". Both of its callers are
    // gone: `Budget::solve` asks `has_tracker` (is there a block at all) and
    // `ui::build` no longer draws the settled one-row summary, because its totals went
    // to the log card's title and its ending word to a pill on the target card.
    //
    // Deleted rather than left for a third caller that might want it. The distinction
    // it drew is still available — `has_build() && !has_tracker()` is the same set —
    // and a predicate nothing calls is a claim about the frame that nothing checks.
}

// ============================================================
// Platform presentation
// ============================================================
// The type itself lives in `probe`, next to the `targetPlatform` string it is
// parsed from. Only the glyph and the label belong here, which is the only part
// that knows about the theme.

impl Platform {
    pub fn glyph(self) -> &'static str {
        match self {
            Platform::Ios => theme::GLYPH_APPLE,
            Platform::Android => theme::GLYPH_ANDROID,
            Platform::Desktop => theme::GLYPH_DESKTOP,
            Platform::Web => theme::GLYPH_WEB,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Ios => "iOS",
            Platform::Android => "Android",
            Platform::Desktop => "Desktop",
            Platform::Web => "Web",
        }
    }

    /// Phases the progress bar counts on this platform.
    ///
    /// The platform is known before the build starts and the trigger table in 3.4
    /// is per-platform, so the count is knowable in advance — which is what gives
    /// the bar an honest denominator.
    ///
    /// Only ever an upper bound. Flutter skips phases it does not need, and a
    /// phase that is skipped leaves the bar one short until the build ends, where
    /// the denominator collapses to what actually ran. The reverse — a floor —
    /// would let the bar reach full and keep working, which is the failure worth
    /// avoiding.
    pub fn stage_count(self) -> usize {
        match self {
            // Starting, Xcode, syncing, running.
            //
            // Four, because the first platform phase adopts the generic row rather
            // than following it: `Preparing build` *becomes* `Building with Xcode`,
            // so they are one row, not two. CocoaPods is not counted either — it is
            // skipped whenever `Podfile.lock` is current, which is most runs, and
            // counting a phase that usually does not happen would leave the bar
            // permanently one short. When pods does run it takes the adoption and
            // Xcode opens its own row, and `expected_stages` raises the total to
            // match.
            Platform::Ios | Platform::Desktop => 4,
            // Starting, Gradle, install, syncing, running. The install step is the
            // one Android has and iOS does not.
            Platform::Android => 5,
            // Nothing to adopt: no native toolchain announces itself, so the generic
            // row stays generic. Starting, preparing, syncing, running.
            Platform::Web => 4,
        }
    }
}

// ============================================================
// Build stages
// ============================================================

/// Which stage a row is, so the parser can find it again.
///
/// An enum and not a label match: the label carries the Gradle task name, so it
/// is not stable across the lines that open and close the stage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StageKey {
    /// Opened when the pty spawns, before Flutter has said anything.
    ///
    /// Covers the toolchain resolving the SDK, the Dart VM booting flutter_tools,
    /// and flutter_tools starting up. Nothing in Flutter's output brackets this
    /// span — its first line is what *ends* it — so without a row opened by frun
    /// itself these seconds have no indicator at all.
    Start,
    Launch,
    /// `pub get`, on the runs where Flutter decides it is needed.
    Pub,
    Pods,
    Xcode,
    Gradle,
    Install,
    Sync,
    Ready,
}

impl StageKey {}

/// The set is platform-dependent by construction: a stage Flutter never
/// mentions is never created, so iOS gets CocoaPods and Xcode while Android gets
/// Gradle and an install, with no branch deciding that anywhere.
pub struct Stage {
    pub key: StageKey,
    pub label: String,
    pub duration: String,
    pub done: bool,
    /// When the stage opened, so a duration Flutter never printed can still be
    /// reported honestly.
    pub started: Instant,

    /// The figure came from Flutter, so measuring must not overwrite it.
    ///
    /// Only the sync sets this. Its opening and closing lines arrive in one read,
    /// so the measured span is zero and Flutter's own `81ms` is the only evidence
    /// it took any time.
    pub pinned: bool,
}

// ============================================================
// Logs
// ============================================================

/// Only what the application itself produces.
///
/// `SYS`, `BLD` and `OK` are gone: everything they carried is a build stage, and
/// the tracker owns those. Showing them here put the same fact on one screen
/// twice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Inf,
    Wrn,
    Err,
    /// Hot reload results. Kept in the stream deliberately: they happen while
    /// the app is running, they interleave with app output, and their position
    /// relative to the surrounding lines is the information.
    Reload,
}

impl Level {
    pub fn badge(self) -> &'static str {
        match self {
            Level::Inf => "INF",
            Level::Wrn => "WRN",
            Level::Err => "ERR",
            Level::Reload => theme::GLYPH_BOLT,
        }
    }

    pub fn color(self) -> ratatui::style::Color {
        match self {
            Level::Inf => theme::CYAN,
            Level::Wrn => theme::AMBER,
            Level::Err => theme::ROSE,
            Level::Reload => theme::PURPLE,
        }
    }
}

pub struct LogLine {
    pub time: String,
    pub level: Level,
    pub message: String,
}

// ============================================================
// Build failure
// ============================================================

/// What the failure card shows.
///
/// `location` and `context` are optional because a build can die without naming
/// a source position at all — a Gradle dependency failure, a signing error, a
/// missing platform toolchain. When there is no code frame the raw tail of the
/// build output takes its place, which is the only honest thing to show.
pub struct Failure {
    pub summary: String,
    pub location: Option<(String, u32, u32)>,
    pub context: Vec<(u32, String)>,
    pub caret_col: u32,
    /// Which stage broke.
    pub note: String,
    pub output: Vec<String>,
}

// ============================================================
// Pending hot reload / restart
// ============================================================

/// A keypress Flutter has not answered yet.
///
/// The reason this is a state machine and not a boolean: Flutter drops keys it
/// cannot service and reports nothing on stdout, so "requested" and "accepted"
/// are genuinely different facts and only the second one has a guaranteed
/// ending.
pub struct Pending {
    pub kind: crate::flutter::Kind,
    pub acked: bool,
    /// When to give up waiting for an ack. `None` once acknowledged, because an
    /// accepted reload may legitimately take as long as it likes.
    pub deadline: Option<Instant>,
    pub started: Instant,
    /// Why it is going to fail, learned before the verdict is printed.
    pub reason: String,
}

// ============================================================
// Messages from worker threads
// ============================================================

/// Everything that reaches the event loop from somewhere other than the
/// keyboard.
///
/// One channel, polled next to the existing input poll. No async runtime: there
/// are three producers and none of them needs to be cancelled.
pub enum Msg {
    /// A complete line from the pty.
    Line(String),
    /// The unterminated tail, where an r/R acknowledgement lives.
    Partial(String),
    /// The pty closed.
    Eof,
    /// Discovery finished: one merged, ordered list of everything runnable.
    Devices(Result<Vec<Device>, String>),
    /// The same list from the tools that answer immediately, ~150ms into the run
    /// (`probe::quick_targets`).
    ///
    /// Its own message rather than the first `Devices`, because the two answers are
    /// not interchangeable: this one may be missing macOS, Chrome and a physical
    /// iPhone, so it is allowed to open the picker and not allowed to be fatal. An
    /// empty answer here means "the cheap tools saw nothing", which is a question for
    /// `Devices` rather than a verdict.
    Quick(Vec<Device>),
    /// A boot finished, carrying the id Flutter will use and whatever else the
    /// device could be asked while the connection to it was warm.
    Booted(Result<probe::Booted, String>),
    /// Which device ids another `flutter run` is on (8.4).
    ///
    /// Its own message rather than a field on `Devices`, because it is a fact about
    /// processes and not about the list: it is asked on the same worker but it stays
    /// true across a list that failed to refresh, and a device can be taken over
    /// without any row changing.
    Busy(std::collections::HashSet<String>),
    /// The slow SDK version lookup landed.
    Versions(String, String),
}

// ============================================================
// Actions
// ============================================================

/// Anything the user can trigger, by key or by click.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Reload,
    Restart,
    RetryBuild,
    /// Reopen the picker to move this run to another device, DESIGN.md 8.5.
    Switch,
    /// Launch the highlighted row in a second frun, in a new terminal tab,
    /// DESIGN.md 8.4. This tab is left exactly as it was.
    NewTab,
    /// End the run and stay in frun, DESIGN.md 8.8.
    StopRun,
    StartDevice,
    Quit,
    Stop,
}

impl Action {
    pub fn key(self) -> &'static str {
        match self {
            Action::Reload => "r",
            Action::Restart => "R",
            Action::RetryBuild => "r",
            // The first frun key that is not a plain letter, and the reason it is
            // affordable: Flutter's interactive commands are all bare single
            // bytes, so a modifier takes nothing from 5.1. `s` was the obvious
            // mnemonic and is Flutter's screenshot key.
            Action::Switch => "^D",
            // Not a letter at all. `Enter` in the three picker states is already
            // frun's, so the modifier costs nothing from 5.1 either — what it costs
            // instead is the Kitty keyboard protocol, without which every modified
            // `Enter` is plain CR on the wire. See 8.4 and `App::shift_enter`.
            Action::NewTab => "⇧⏎",
            // `s` is Flutter's screenshot key, so the mnemonic has to be a modifier
            // the same way `^D` did. Raw mode clears IXON, so `^S` is not XOFF here.
            Action::StopRun => "^S",
            Action::StartDevice => "⏎",
            Action::Quit => "q",
            Action::Stop => "^C",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            // No `Hot` on either. The word is Flutter's vocabulary, not a
            // distinction the user is choosing between on this row — there is no cold
            // reload to tell it apart from — and the two of them cost eight columns
            // on the one row that must never truncate. The reload *note* above still
            // says `Hot reload` in full, where it is quoting Flutter.
            Action::Reload => "Reload",
            Action::Restart => "Restart",
            Action::RetryBuild => "Retry Build",
            // Not "Change target": the run is killed and rebuilt, and a label
            // that reads like a live switch would leave the user thinking the
            // tool had hung through a forty-second Gradle build.
            //
            // `Switch` and not `Switch Device`, since the footer carries this in five
            // states now rather than one. The noun was never load-bearing — the
            // argument above is about `switch` versus `change`, and it survives the
            // cut — and seven columns on a row that must never truncate is what the
            // run states had left to give.
            Action::Switch => "Switch",
            // The same words in the picker and in the switch list, deliberately.
            // `⏎` changes meaning between those two frames and has to say which it
            // means; `⇧⏎` does not — it launches, in a new tab, either way.
            Action::NewTab => "Launch in new tab",
            Action::StopRun => "Stop",
            Action::StartDevice => "Start",
            // `q` and `^C` are not the same exit, so they are not merged.
            //
            // `q` is forwarded to Flutter, which shuts itself down and closes the
            // pty; frun waits. `^C` sends SIGINT, which does not need Flutter to
            // be in any state to read a key. That is the difference that matters:
            // when Flutter is wedged, only one of the two works.
            //
            // The labels have to carry that, otherwise two keys for one apparent
            // outcome look like an accident.
            Action::Quit => "Quit",
            // `Force stop`, now that `^S` is `Stop`. Three ways out of a run and
            // three different words: `^S` ends the run and stays, `q` ends the run
            // and leaves, `^C` ends the process whatever the run is doing.
            Action::Stop => "Force stop",
        }
    }
}

/// How a run ended.
///
/// Every one of these looks identical from the pty — the child closes it and goes
/// — so this field is the only thing that can tell them apart, and they differ in
/// both directions that matter: whether frun should still be here afterwards, and
/// what state the device was left in.
///
/// `None` is the fourth case and the one no key produces: the app was killed or
/// the device went away. That is the whole of the detection, for the same reason
/// build failure is not a catalogue of error strings — a device yanked mid-run can
/// close the pty without Flutter printing a word, so the *absence* of a recorded
/// ending is more reliable than any line to grep for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ending {
    /// `^S`: Flutter was asked to shut down, and the app went with it.
    Stopped,
    /// `d`/`D`, Flutter's own key: the tool let go and the app is still running.
    Detached,
    /// `q`: the same request as `^S`, aimed at frun rather than at the run.
    ///
    /// Recorded even though nothing renders it, because `child_exited` has to know
    /// this death was asked for. Before this existed, `q` and a device dying were
    /// one indistinguishable event — both arrived as "the pty closed while the
    /// session was live" — so giving the second one a frame to land on would have
    /// taken `q` with it.
    Quit,
    /// Nobody asked: the app was closed on the device, crashed, or the device
    /// itself went away.
    ///
    /// Lands on `Stopped` like the two deliberate endings, because the device is
    /// left in the same condition as after `^S` — app gone, and `r` is the way
    /// back. Only the title differs, and it has to: frun cannot tell a deliberate
    /// shutdown from a crash, so the frame says what it knows (the connection
    /// ended) and not why.
    Lost,
}

/// A clickable region, rebuilt every frame.
///
/// ratatui has no hit testing: it draws into a cell buffer and forgets the
/// geometry. Rebuilding per frame is what keeps this correct when the layout
/// degrades and a card moves or disappears.
pub struct Hit {
    pub area: Rect,
    pub action: Action,
}

// ============================================================
// App
// ============================================================

pub struct App {
    pub state: State,

    // Project card.
    pub project: String,
    pub version: String,
    pub branch: String,
    pub dirty: usize,
    pub flutter: String,
    pub dart: String,
    pub cwd: String,

    /// How Flutter is reached on this machine, for the `Runtime` column (3.1)
    /// and the command the `DETECTING` screen says it is running.
    ///
    /// Carried on the App rather than read from `probe` at render time so that
    /// mock frames stay deterministic: `--dump` is how layout is verified, and a
    /// frame that says `FVM` on one machine and `SDK` on another cannot be
    /// compared against anything.
    pub toolchain: probe::Toolchain,

    // Devices.
    pub devices: Vec<Device>,
    pub selected_device: usize,
    pub scroll: usize,

    /// Rows the log window is scrolled back from the live tail.
    ///
    /// Separate from `scroll`, which belongs to the device list. Sharing one
    /// field was the bug: `Up`/`Down` in `RUNNING` moved a device selection that
    /// was not on screen, so the arrow keys appeared to do nothing while the log
    /// window stayed pinned to the bottom.
    ///
    /// Zero means the bottom, which is why new output keeps arriving without
    /// having to be followed: the offset is measured from the end, so the tail
    /// stays put as the buffer grows.
    pub log_scroll: usize,

    /// The chosen device. Everything the SelectedTargetCard shows is derived
    /// from it rather than copied out, so the card cannot describe a device that
    /// is not the one being run.
    pub target: Option<Device>,

    /// Why the run is ending, when frun asked for it or watched it be asked for
    /// (8.8).
    ///
    /// What it decides is what `child_exited` does with the death: an ending frun
    /// knows about lands on `Stopped`, anything else is the end of the process.
    /// Without it, a graceful stop, a detach and Flutter quitting on its own are one
    /// indistinguishable event — the pty closing.
    ///
    /// Kept after the child is gone rather than cleared, because the `STOPPED` frame
    /// has to say which of the two happened. `begin_build` clears it.
    pub ending: Option<Ending>,

    /// Discovery is running again behind a list that is already on screen (8.5).
    ///
    /// Only for saying so in the title. The rows are swapped when the answer lands,
    /// and a list that changes under the cursor with no explanation reads as a
    /// glitch.
    pub refreshing: bool,

    /// Where `Esc` goes back to, and the one flag that says the picker is open
    /// over a run that is still alive (DESIGN.md 8.5).
    ///
    /// `Some` only between `^D` and the pick that answers it. While it is set the
    /// child keeps streaming, so `goto` banks its transitions here instead of
    /// putting them on screen: a `Reloaded 12 libraries` arriving at the wrong
    /// moment must not close the picker mid-choice.
    pub resume: Option<State>,

    /// What is booting, and since when. The clock matters: Android waits on
    /// `sys.boot_completed` for up to three minutes and a spinner alone cannot
    /// tell a slow boot from a hung one.
    pub boot_name: String,
    pub boot_started: Option<Instant>,

    // Build.
    pub stages: Vec<Stage>,
    pub build_started: Instant,
    pub build_time: String,
    pub sync_time: String,
    pub total_time: String,

    pub failure: Option<Failure>,
    pub exit_code: i32,

    pub logs: Vec<LogLine>,
    /// True while passing through a `logger.printBox()` block.
    pub in_box: bool,

    pub pending: Option<Pending>,
    pub reload_note: String,

    /// Set when the flow cannot continue. Reported after the terminal is
    /// restored, so the message survives leaving the alternate screen.
    pub fatal: Option<String>,

    /// Spinner frame counter, advanced by the event loop tick.
    pub tick: usize,

    pub hits: Vec<Hit>,
    pub hover: Option<Action>,

    /// Off by default. Capturing the mouse takes text selection away from the
    /// terminal, and copying a stack trace out of the log window is a large part
    /// of what that window is for.
    pub mouse_on: bool,

    pub last_action: Option<Action>,

    /// Give the log window the whole frame, hiding the three static cards.
    ///
    /// 7.7 asked for more log on screen. A terminal application cannot change its
    /// font size — one font at one size covers the grid — so the app-side answer
    /// is to stop spending rows on information that is not changing. At 62 rows
    /// this takes the log window from 19 rows to 58.
    pub expanded: bool,

    /// Whether this app is talking to a real device. False for the dump and demo
    /// paths, which is what gates the prototype affordances: `Tab` must not move
    /// between states when Flutter is deciding them.
    pub live: bool,
    pub demo: bool,

    /// Whether this terminal reports the Kitty keyboard protocol, which is the
    /// only way `⇧⏎` is distinguishable from `⏎` (8.4).
    ///
    /// It gates the footer hint and nothing else: advertising a key that cannot
    /// arrive is the `[COPY]` failure of 3.1. Defaulting to `true` is what lets
    /// `--dump` draw the hint — the harness has no terminal to ask — and `run()`
    /// replaces it with the terminal's own answer before the first frame.
    pub shift_enter: bool,

    /// Device ids another `flutter run` is already on, from `probe::busy` (8.4).
    ///
    /// Read through `in_use`, never directly: the set contains this tab's own run too,
    /// and that row is `running`, not taken.
    pub busy: std::collections::HashSet<String>,

    clock: probe::Clock,
}

impl App {
    /// Live, from what the machine says.
    pub fn live(project: probe::Project) -> Self {
        let mut app = Self::empty();

        app.project = project.name;
        app.version = project.version;
        app.branch = project.branch;
        app.dirty = project.dirty;
        app.flutter = project.flutter;
        app.dart = project.dart;
        app.cwd = project.cwd;
        app.toolchain = probe::toolchain().clone();
        app.live = true;
        app.state = State::Detecting;

        app
    }

    fn empty() -> Self {
        Self {
            state: State::Detecting,

            project: "-".into(),
            version: "-".into(),
            branch: "-".into(),
            dirty: 0,
            flutter: "-".into(),
            dart: "-".into(),
            cwd: "~".into(),

            // FVM, because the mock frames were captured against it and
            // `App::live` overwrites this with the real answer. Nothing here
            // touches the machine.
            toolchain: probe::Toolchain::fvm(),

            devices: Vec::new(),
            selected_device: 0,
            scroll: 0,
            log_scroll: 0,

            target: None,
            ending: None,
            refreshing: false,
            resume: None,

            boot_name: String::new(),
            boot_started: None,

            stages: Vec::new(),
            build_started: Instant::now(),
            build_time: "-".into(),
            sync_time: "-".into(),
            total_time: "-".into(),

            failure: None,
            exit_code: 0,

            logs: Vec::new(),
            in_box: false,

            pending: None,
            reload_note: String::new(),

            fatal: None,

            tick: 0,

            hits: Vec::new(),
            hover: None,
            mouse_on: false,
            last_action: None,

            expanded: false,
            live: false,
            demo: false,
            shift_enter: true,
            busy: std::collections::HashSet::new(),

            clock: probe::Clock::new(),
        }
    }

    /// Index of the current state in the flow, for the prototype hint.
    pub fn position(&self) -> (usize, usize) {
        let i = State::ALL
            .iter()
            .position(|s| *s == self.state)
            .unwrap_or(0);

        (i + 1, State::ALL.len())
    }

    pub fn spinner(&self) -> &'static str {
        theme::SPINNER[self.tick % theme::SPINNER.len()]
    }

    /// Move to `state`. Carries no data with it: everything on screen is already
    /// in `self`, put there by whatever caused the transition.
    ///
    /// One exception, and it is the reason every transition goes through here:
    /// while the picker is open over a live run (`resume`), the child is still
    /// announcing reloads and failures. Those transitions are banked rather than
    /// drawn, so the list cannot vanish under the user's cursor, and `Esc`
    /// restores the state Flutter actually reached rather than the one it was in
    /// when `^D` was pressed.
    pub fn goto(&mut self, state: State) {
        if self.resume.is_some() {
            self.resume = Some(state);
            return;
        }

        self.state = state;
    }

    /// What the run is doing, as opposed to what the screen is showing.
    ///
    /// The two differ in exactly one place: `Switching`, where the list is on
    /// screen and the child behind it is still building or still up. Anything
    /// reasoning about the *run* has to ask this instead of `state`. The case that
    /// makes it load-bearing: if the app dies while the list is open, `child_exited`
    /// reading `state` sees no build in progress and ends the process, throwing away
    /// the failure it was called to report.
    pub fn run_state(&self) -> State {
        self.resume.unwrap_or(self.state)
    }

    pub fn log(&mut self, level: Level, message: &str) {
        self.logs.push(LogLine {
            time: self.clock.now(),
            level,
            message: message.to_string(),
        });

        // A run left open all day would otherwise grow without bound. The window
        // shows a screenful and the transcript on exit is a debugging aid, not an
        // archive; `flutter logs` exists for that.
        //
        // ponytail: flat cap, add a ring buffer if 4000 lines ever costs
        // something measurable.
        if self.logs.len() > 4000 {
            self.logs.drain(..1000);
        }
    }

    // ========================================================
    // Devices
    // ========================================================

    pub fn select_next(&mut self) {
        if !self.devices.is_empty() && self.selected_device + 1 < self.devices.len() {
            self.selected_device += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_device = self.selected_device.saturating_sub(1);
    }

    pub fn selected(&self) -> Option<&Device> {
        self.devices.get(self.selected_device)
    }

    /// Whether some *other* run holds this device (8.4).
    ///
    /// **The exclusion is what makes this readable rather than merely true.**
    /// `probe::busy` reads the process table, and this tab's own `flutter run -d`
    /// is in it — so without the second half, the device you are running on would
    /// wear ` in use ` in your own switch list, next to ` running `, and `⏎ Keep`
    /// would refuse the row it exists to offer. The target is compared rather than a
    /// pid excluded because the answer wanted here is about the device, and one
    /// device is one run: if it is yours, it is not another tab's.
    ///
    /// A heuristic, deliberately: `-d <id>` on a `flutter run` command line, nothing
    /// registered anywhere. It can only be wrong in the safe direction — an unusual
    /// invocation goes unnoticed and the device reads free, which is where frun was
    /// before this existed.
    pub fn in_use(&self, id: &str) -> bool {
        self.busy.contains(id) && self.target.as_ref().map(|t| t.id.as_str()) != Some(id)
    }

    /// Where a row belongs, top to bottom. Lower sorts higher.
    ///
    /// Computed from the row and from what is running, and from nothing else — no
    /// history, no arrival order. That is what makes `SELECT DEVICE` and
    /// `SWITCH DEVICE` show one order: they are the same list, ranked by the same
    /// function, and `probe::targets` hands both of them the same canonical slots
    /// underneath.
    ///
    /// `running` above `in use` above `last used` is the order the three facts matter
    /// in: where the run is now, where another run is, and where you usually go. Below
    /// them the canonical order takes over, grouped only by what pressing `Enter`
    /// costs — a device that is up, then one that has to boot, then the platforms that
    /// are always there and are nobody's first choice.
    ///
    /// The two rows at the top are ones `Enter` will not launch: `running` answers
    /// `⏎ Keep` and `in use` is refused outright. That is deliberate — the top of the
    /// list is the answer to "what is going on", and the cursor is placed on the first
    /// row that *can* be picked rather than on row zero.
    fn rank(&self, device: &Device) -> u8 {
        if self.target.as_ref().is_some_and(|t| t.id == device.id) {
            return 0;
        }

        if self.in_use(&device.id) {
            return 1;
        }

        if device.last_used {
            return 2;
        }

        // Before the boot test, not after: macOS and Chrome have no boot either, and
        // ranking them with a device that is up and waiting would put the host above
        // the phone in your hand.
        if !device.platform.needs_boot() {
            return 5;
        }

        if device.boot.is_none() {
            return 3;
        }

        4
    }

    /// Put the rows in that order, keeping the canonical order inside each rank.
    ///
    /// Stable, which is the whole reason `probe::targets` stopped sorting: ties fall
    /// back to the slot the machine gave the device, so a simulator that boots and
    /// shuts down again returns to the place it was rather than to the end of its
    /// group.
    pub fn sort_devices(&mut self) {
        let mut rows = std::mem::take(&mut self.devices);

        rows.sort_by_key(|device| self.rank(device));

        self.devices = rows;
    }

    /// The row the cursor should start on: the first one `Enter` can actually launch.
    ///
    /// The remembered device is the preselection worth having — 7.6 wants one keystroke
    /// per run — but it is frequently the device another tab is on, and a preselected
    /// row that refuses `Enter` is worse than no preselection at all.
    pub fn first_pickable(&self) -> usize {
        self.devices
            .iter()
            .position(|device| device.last_used && !self.in_use(&device.id))
            .or_else(|| {
                self.devices
                    .iter()
                    .position(|device| !self.in_use(&device.id))
            })
            .unwrap_or(0)
    }

    /// Scroll the log window. Positive goes back in history.
    ///
    /// Not clamped here: the ceiling depends on how many visual rows the entries
    /// wrap to, which is only known at the width the window is drawn at. `logs.rs`
    /// clamps it while rendering, where that is known.
    pub fn scroll_logs(&mut self, delta: isize) {
        self.log_scroll = self.log_scroll.saturating_add_signed(delta);
    }

    /// Adopt a device as the run target.
    ///
    /// The Flutter arguments are not taken here any more. They were used to build
    /// a display string for the SelectedTargetCard's `❯ fvm flutter run -d ...`
    /// row, which 3.2 no longer has; `Session::spawn` builds the argv it runs from
    /// `ctx.extra` directly, so nothing else read it.
    pub fn choose(&mut self, device: Device) {
        probe::remember_device(&device.id);

        // The row that was picked is replaced by the device that is now running,
        // and this is not bookkeeping — it is what makes the switch list true for
        // Android.
        //
        // An AVD is offered under its AVD name (`Pixel_10_Pro_XL`) and runs as a
        // serial (`emulator-5554`). Adopting the target without touching the list
        // left the old row in place, so `^D` showed the emulator frun was running
        // as a row offering to boot it, with no ` running ` badge — the badge
        // matches on `target.id` — and no ` last used ` either, since that compares
        // ids too. iOS hid the bug: a simulator keeps its UDID booted or not.
        //
        // Matched on name as well as id because the name is the join `targets()`
        // already relies on to de-duplicate a running emulator against its AVD row.
        let existing = self
            .devices
            .iter()
            .position(|d| d.id == device.id || d.name == device.name);

        match existing {
            Some(i) => self.devices[i] = device.clone(),
            None => self.devices.insert(0, device.clone()),
        }

        // The chip follows the pick rather than waiting for the next full scan to
        // read `.frun-last-device` back.
        for row in &mut self.devices {
            row.last_used = row.id == device.id;
        }

        let id = device.id.clone();

        self.target = Some(device);

        // Ranked now rather than at the next recheck, because `^D` puts the list up
        // from the cache immediately and the answer behind it takes a moment: without
        // this the switch list opened in the old order and rearranged itself under the
        // cursor a fraction of a second later.
        self.sort_devices();

        // The cursor follows the run, which is where the switch list wants it: `^D` then
        // a reflexive `Enter` means "stay here", and 8.5 leans on that.
        self.selected_device = self
            .devices
            .iter()
            .position(|row| row.id == id)
            .unwrap_or(0);
    }

    /// How the target card describes the target's kind.
    pub fn target_kind(&self) -> &'static str {
        match &self.target {
            Some(d) => match (&d.platform, d.virtual_device) {
                (Platform::Ios, true) => "Simulator",
                (Platform::Android, true) => "Emulator",

                (Platform::Ios, false) |
                (Platform::Android, false) => "Hardware",

                (Platform::Desktop, _) => "Desktop",
                (Platform::Web, _) => "Web",
            },
            None => "-",
        }
    }

    // ========================================================
    // Hot reload / restart
    // ========================================================

    /// A keypress. Only a request: Flutter decides whether to honour it.
    pub fn request_reload(&mut self, kind: crate::flutter::Kind) {
        // Already in flight and confirmed: Flutter will ignore the extra key, so
        // keep the stage that is actually running rather than resetting its ack
        // window.
        if let Some(pending) = &self.pending {
            if pending.kind == kind && pending.acked {
                return;
            }
        }

        self.begin_reload(kind, false);
    }

    /// Flutter's own progress message arrived, so the key was taken.
    pub fn ack_reload(&mut self, kind: crate::flutter::Kind) {
        match &mut self.pending {
            // Adopt what Flutter is actually doing: a reload can be triggered
            // from outside frun, or be one Flutter had queued.
            Some(pending) if pending.kind != kind => self.begin_reload(kind, true),

            Some(pending) => {
                pending.acked = true;
                pending.deadline = None;
            }

            None => self.begin_reload(kind, true),
        }
    }

    fn begin_reload(&mut self, kind: crate::flutter::Kind, acked: bool) {
        self.pending = Some(Pending {
            kind,
            acked,
            deadline: (!acked).then(|| Instant::now() + crate::flutter::ACK_TIMEOUT),
            started: Instant::now(),
            reason: String::new(),
        });

        self.reload_note = match kind {
            crate::flutter::Kind::Reload => "Syncing updated Dart libraries".into(),
            crate::flutter::Kind::Restart => "Restarting the application".into(),
        };

        if let Some(name) = self.target.as_ref().map(|d| d.name.clone()) {
            self.reload_note = format!("{} to {name}...", self.reload_note);
        }

        self.goto(State::ReloadInFlight);
    }

    pub fn finish_reload(&mut self, kind: crate::flutter::Kind, text: &str) {
        // The result line stays in the log stream rather than being consumed
        // here: it happens while the app is running, it interleaves with app
        // output, and its position is the information.
        self.log(Level::Reload, text);

        self.pending = None;
        self.reload_note.clear();
        self.goto(State::Running);

        let _ = kind;
    }

    pub fn note_reload_failure(&mut self, reason: &str) {
        if let Some(pending) = &mut self.pending {
            if pending.reason.is_empty() {
                pending.reason = reason.to_string();
            }
        }
    }

    pub fn fail_reload(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };

        let reason = if pending.reason.is_empty() {
            "see the errors above".to_string()
        } else {
            pending.reason
        };

        self.reload_note = format!("{} — {reason}", pending.kind.name());
        self.goto(State::ReloadFailed);
    }

    /// Flutter never took the key.
    pub fn drop_reload(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };

        self.reload_note = format!(
            "{} not picked up by Flutter — press {} again",
            pending.kind.name(),
            pending.kind.key()
        );

        self.goto(State::ReloadDropped);
    }

    /// Surface why a reload is slow, once per distinct message.
    pub fn reload_notice(&mut self, text: &str) {
        if self.reload_note != text {
            self.reload_note = text.to_string();
        }
    }

    /// Elapsed time on whatever is currently in flight.
    pub fn pending_clock(&self) -> String {
        match &self.pending {
            Some(pending) => crate::flutter::clock(pending.started.elapsed()),
            None => String::new(),
        }
    }

    /// Give up on an unacknowledged keypress whose deadline has passed.
    pub fn tick_pending(&mut self) {
        let expired = self
            .pending
            .as_ref()
            .and_then(|p| p.deadline)
            .is_some_and(|deadline| Instant::now() >= deadline);

        if expired {
            self.drop_reload();
        }
    }

    /// How long the current boot has been running.
    pub fn boot_clock(&self) -> String {
        match self.boot_started {
            Some(started) => crate::flutter::clock(started.elapsed()),
            None => String::new(),
        }
    }

    // ========================================================
    // Build
    // ========================================================

    /// Start, or restart, a build.
    ///
    /// Opens the first stage immediately. The build clock and the first row have
    /// to start together, or the seconds before Flutter's first line belong to
    /// nothing on screen.
    pub fn begin_build(&mut self) {
        self.stages.clear();
        self.failure = None;
        // Whatever ended the last run is history now, and the tracker's title is
        // about to be about this one.
        self.ending = None;
        self.exit_code = 0;
        self.build_started = Instant::now();
        self.build_time = "-".into();
        self.sync_time = "-".into();
        self.total_time = "-".into();
        self.pending = None;
        self.reload_note.clear();
        self.goto(State::Building);

        self.start_stage(StageKey::Start, "Starting Flutter".into());
    }

    /// Elapsed build time: counting while it builds, frozen once it stops.
    ///
    /// Frozen matters on failure as much as on success. A build that died at 11
    /// seconds whose clock keeps running says the build is still going.
    pub fn build_clock(&self) -> String {
        // A dash while `Starting Flutter` is still open, because nothing is building
        // yet: the toolchain is booting and `startup_clock` is the figure counting
        // those seconds. Two live clocks over one span would charge the same wait
        // twice and make the pair stop adding up to what was waited.
        if self.stage_open(StageKey::Start) {
            return "-".into();
        }

        if self.state == State::Building {
            return crate::flutter::elapsed(self.build_started.elapsed());
        }

        self.build_time.clone()
    }

    /// What the toolchain costs before Flutter prints anything: the `Starting
    /// Flutter` row, promoted to the title bar.
    ///
    /// The span no Flutter output brackets — the fvm hop, the Dart VM, flutter_tools
    /// and device resolution — measured at 3.2s of a 9.0s run here. The tracker that
    /// holds the row is not laid out once the build is done, so without this the
    /// header carried a total whose largest unexplained part had nowhere to surface.
    ///
    /// Live while the row is open and frozen once it closes, which is the opposite
    /// phase to `build_clock`: the two run in sequence, never together, so exactly
    /// one of them is counting at any moment of a build.
    pub fn startup_clock(&self) -> String {
        let Some(stage) = self.stages.iter().find(|stage| stage.key == StageKey::Start) else {
            return "-".into();
        };

        if !stage.done {
            return crate::flutter::elapsed(stage.started.elapsed());
        }

        match stage.duration.is_empty() {
            true => "-".into(),
            false => stage.duration.clone(),
        }
    }

    /// Combined total time across startup and build phases.
    pub fn total_clock(&self) -> String {
        if let Some(stage) = self.stages.iter().find(|stage| stage.key == StageKey::Start) {
            if self.state == State::Building {
                return crate::flutter::elapsed(stage.started.elapsed());
            }
        }

        if !self.total_time.is_empty() && self.total_time != "-" {
            return self.total_time.clone();
        }

        let startup_d = parse_time_str(&self.startup_clock());
        let build_d = parse_time_str(&self.build_clock());

        let total = startup_d + build_d;
        if total == 0.0 {
            "-".into()
        } else if total < 1.0 {
            format!("{}ms", (total * 1000.0).round() as u64)
        } else if total < 60.0 {
            format!("{total:.1}s")
        } else {
            format!("{}m {:.1}s", (total as u64) / 60, total % 60.0)
        }
    }

    /// Stop the clock, whatever the outcome.
    pub fn end_build(&mut self) {
        // A build that died before Flutter announced anything never left startup, and
        // `build_started` still points at the spawn. Charging that span as a build
        // time as well as a startup would show one wait as two.
        if let Some(stage) = self
            .stages
            .iter_mut()
            .find(|stage| stage.key == StageKey::Start && !stage.done)
        {
            stage.duration = crate::flutter::elapsed(stage.started.elapsed());
            stage.done = true;

            self.build_time = "-".into();
            self.total_time = stage.duration.clone();

            return;
        }

        self.build_time = crate::flutter::elapsed(self.build_started.elapsed());
        if let Some(stage) = self.stages.iter().find(|stage| stage.key == StageKey::Start) {
            self.total_time = crate::flutter::elapsed(stage.started.elapsed());
        }
    }

    /// Stages this build is expected to announce, for the progress denominator.
    ///
    /// Never below what has already been seen, so the bar cannot exceed full even
    /// if a platform turns out to announce more than the table predicts.
    pub fn expected_stages(&self) -> usize {
        let expected = self
            .target
            .as_ref()
            .map(|d| d.platform.stage_count())
            .unwrap_or(4);

        expected.max(self.stages.len()).max(1)
    }

    pub fn hit_test(&self, col: u16, row: u16) -> Option<Action> {
        self.hits
            .iter()
            .find(|h| {
                col >= h.area.x
                    && col < h.area.x + h.area.width
                    && row >= h.area.y
                    && row < h.area.y + h.area.height
            })
            .map(|h| h.action)
    }
}

fn parse_time_str(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() || s == "-" {
        return 0.0;
    }
    if let Some(ms) = s.strip_suffix("ms") {
        ms.parse::<f64>().unwrap_or(0.0) / 1000.0
    } else if let Some(secs) = s.strip_suffix('s') {
        if let Some((m, rest)) = secs.split_once('m') {
            let mins = m.trim().parse::<f64>().unwrap_or(0.0);
            let s = rest.trim().parse::<f64>().unwrap_or(0.0);
            mins * 60.0 + s
        } else {
            secs.trim().parse::<f64>().unwrap_or(0.0)
        }
    } else {
        0.0
    }
}

// ============================================================
// Mock data
// ============================================================
// Reached only through --dump / --all / --rows / --hits / --demo.

impl App {
    pub fn new(state: State) -> Self {
        let mut app = Self::empty();

        app.project = "cwclub".into();
        app.version = "2.1.0+32".into();
        app.branch = "refactor/cwclub-new".into();
        app.flutter = "3.29.3".into();
        app.dart = "3.7.2".into();
        app.cwd = "~/cwclub".into();
        app.build_time = "3.4s".into();
        app.sync_time = "240ms".into();
        app.total_time = "7.0s".into();
        app.exit_code = 1;

        // `iOS-26-5`, not `iOS 18.2 (arm64)`. The mock is what the layout is
        // judged against, so a string Flutter cannot produce judges it against
        // the wrong thing: an iOS simulator reports the runtime `simctl` filed it
        // under, and the parenthesised arch was never in this field on any
        // platform.
        app.target = Some(mock_device(
            "8A3F91C2-4D2E",
            "iPhone 16 Pro",
            Platform::Ios,
            "ios",
            "iOS-26-5",
            true,
        ));

        app.boot_name = "Pixel 10 Pro XL".into();
        app.boot_started = Some(Instant::now());

        app.mock_goto(state);

        app
    }

    /// Prototype navigation: swap in the mock data for `state`.
    pub fn mock_goto(&mut self, state: State) {
        self.state = state;
        self.devices = mock_devices(state);
        self.stages = mock_stages(state);
        self.failure = mock_failure(state);
        self.logs = mock_logs(state, &self.clock);
        self.reload_note = mock_reload_note(state).to_string();
        self.selected_device = 0;
        self.scroll = 0;

        // The switch list is the one frame whose contents depend on each other: the
        // badge marks the row whose id matches the target, so the mock target has to
        // *be* a row in the mock list or the frame shows a switch away from a device
        // that is not there. `resume` is what the live flow sets on `^D`, and `Esc`
        // reads it back.
        if state == State::Switching {
            self.target = self.devices.first().cloned();
            self.resume = Some(State::Running);
        }

        // No run, no target, and this became load-bearing when the list started being
        // ranked. `App::new` sets a target for the frames that draw the target card, and
        // the picker frames inherited it — harmless while nothing read it, and wrong the
        // moment `rank` did: `--dump picker` lifted a row to the top as the running one
        // in a frame that exists precisely because nothing is running yet.
        if !state.has_build() && state != State::Switching {
            self.target = None;
        }

        // Ranked like the live list, so `--dump picker` and `--dump switch` are
        // comparable frames rather than two hand-written orders. The mock is what the
        // layout is judged against; an order the live flow cannot produce judges it
        // against the wrong thing.
        self.sort_devices();
    }

    pub fn next_state(&mut self) {
        let i = State::ALL
            .iter()
            .position(|s| *s == self.state)
            .unwrap_or(0);

        self.mock_goto(State::ALL[(i + 1) % State::ALL.len()]);
    }

    pub fn prev_state(&mut self) {
        let i = State::ALL
            .iter()
            .position(|s| *s == self.state)
            .unwrap_or(0);

        self.mock_goto(State::ALL[(i + State::ALL.len() - 1) % State::ALL.len()]);
    }
}

fn mock_device(
    id: &str,
    name: &str,
    platform: Platform,
    target_platform: &str,
    sdk: &str,
    virtual_device: bool,
) -> Device {
    Device {
        id: id.into(),
        name: name.into(),
        platform,
        target_platform: target_platform.into(),
        sdk: sdk.into(),
        virtual_device,
        last_used: false,
        boot: None,
    }
}

fn mock_devices(state: State) -> Vec<Device> {
    let bootable = |id: &str, name: &str, platform: Platform, boot: Boot| Device {
        id: id.into(),
        name: name.into(),
        platform,
        target_platform: String::new(),
        sdk: String::new(),
        virtual_device: platform.needs_boot(),
        // Set on one row below rather than here, so `--dump picker` shows the case the
        // chip exists for: the device you always reach for is off.
        last_used: false,
        boot: Some(boot),
    };

    let remembered = |mut d: Device| {
        d.last_used = true;
        d
    };

    match state {
        // Every launchable target, not just phones. Desktop and web are always
        // available and need no boot.
        State::Booting => vec![
            bootable(
                "Pixel_10_Pro_XL",
                "Pixel 10 Pro XL",
                Platform::Android,
                Boot::Avd("Pixel_10_Pro_XL".into()),
            ),
            bootable(
                "Pixel_8",
                "Pixel 8",
                Platform::Android,
                Boot::Avd("Pixel_8".into()),
            ),
            remembered(bootable(
                "8A3F91C2-4D2E",
                "iPhone 17 Pro",
                Platform::Ios,
                Boot::Sim("8A3F91C2-4D2E".into()),
            )),
            bootable(
                "1B7C22E9-90AF",
                "iPhone 17 Pro Max",
                Platform::Ios,
                Boot::Sim("1B7C22E9-90AF".into()),
            ),
            bootable(
                "C4D1099A-71B3",
                "iPhone Air",
                Platform::Ios,
                Boot::Sim("C4D1099A-71B3".into()),
            ),
            bootable(
                "77E0C1B4-2A6D",
                "iPad Pro 13-inch (M5)",
                Platform::Ios,
                Boot::Sim("77E0C1B4-2A6D".into()),
            ),
            // No boot step: already available, so picking one launches it.
            mock_device("macos", "macOS", Platform::Desktop, "darwin", "", false),
            mock_device(
                "chrome",
                "Chrome",
                Platform::Web,
                "web-javascript",
                "",
                false,
            ),
        ],

        // One merged list, ordered the way `probe::targets` orders it: running
        // first, then things that need starting, then the platforms that are
        // always there. The mock has to show this shape or the dumps verify a
        // layout the live flow no longer produces.
        State::MultipleDevices => {
            let mut devices = vec![
                mock_device(
                    "emulator-5554",
                    "Pixel 10 Pro XL",
                    Platform::Android,
                    "android-arm64",
                    "Android 17 (API 37)",
                    true,
                ),
                bootable(
                    "Pixel_8",
                    "Pixel 8",
                    Platform::Android,
                    Boot::Avd("Pixel_8".into()),
                ),
                bootable(
                    "8A3F91C2-4D2E",
                    "iPhone 17 Pro",
                    Platform::Ios,
                    Boot::Sim("8A3F91C2-4D2E".into()),
                ),
                mock_device("macos", "macOS", Platform::Desktop, "darwin", "", false),
                mock_device(
                    "chrome",
                    "Chrome",
                    Platform::Web,
                    "web-javascript",
                    "",
                    false,
                ),
            ];

            devices[0].last_used = true;
            devices
        }

        // A run always arrives from that list, and the list is kept afterwards so
        // `^D` can reopen it without re-running discovery (8.5). The mock has to
        // hold one too: without it the target card's border cannot advertise the
        // key that is there live, and `--dump switch` is an empty list.
        s if s.has_build() || s == State::Switching => mock_devices(State::MultipleDevices),

        _ => Vec::new(),
    }
}

fn mock_stages(state: State) -> Vec<Stage> {
    // A stage that has been running for a while, so `--dump building` shows the
    // elapsed clock rather than a blank. Charging it to the mock is what makes
    // that row verifiable at all: a clock that only appears after three real
    // seconds cannot be seen in a single rendered frame otherwise.
    let waiting_since = Instant::now()
        .checked_sub(std::time::Duration::from_secs(6))
        .unwrap_or_else(Instant::now);

    let stage = |key, label: &str, duration: &str, done| Stage {
        pinned: false,
        key,
        label: label.into(),
        duration: duration.into(),
        done,
        started: if done { Instant::now() } else { waiting_since },
    };

    // Exactly one row open in every building state, because that is the invariant
    // the parser now guarantees and the mock has to show the same shape or the
    // dumps verify a layout the live flow cannot produce.
    //
    // `Starting Flutter` is charged the startup gap: the toolchain, the Dart VM and
    // flutter_tools, which is the span no Flutter output brackets.
    match state {
        // iOS: CocoaPods then Xcode. Gradle never appears here.
        // No `Preparing build` row alongside `Installing CocoaPods`: that pairing
        // cannot happen, because pods adopts the generic row rather than following
        // it. Showing it would have the dumps verifying a shape the parser cannot
        // produce.
        State::Building => vec![
            stage(StageKey::Start, "Starting Flutter", "3.6s", true),
            stage(StageKey::Pods, "Installing CocoaPods", "1.2s", true),
            stage(StageKey::Xcode, "Building with Xcode", "", false),
        ],

        State::BuildFailed => vec![
            stage(StageKey::Start, "Starting Flutter", "3.6s", true),
            stage(StageKey::Pods, "Installing CocoaPods", "1.2s", true),
            stage(StageKey::Xcode, "Building with Xcode", "11.1s", false),
        ],

        // Five rows, not six: on the runs where CocoaPods appears it adopts the
        // generic row, so `Preparing build` is not a row of its own.
        // `Stopped` with them: the run it is the remains of had finished building,
        // and a tracker with no rows would describe a build that never happened.
        s if s.build_done() || s == State::Stopped => vec![
            stage(StageKey::Start, "Starting Flutter", "3.6s", true),
            stage(StageKey::Pods, "Installing CocoaPods", "1.2s", true),
            stage(StageKey::Xcode, "Building with Xcode", "14.5s", true),
            stage(StageKey::Sync, "Syncing files", "240ms", true),
            stage(StageKey::Ready, "Application Running", "", true),
        ],

        _ => Vec::new(),
    }
}

fn mock_failure(state: State) -> Option<Failure> {
    if state != State::BuildFailed {
        return None;
    }

    Some(Failure {
        summary: "lib/main.dart:42:18: Error: The argument type 'int' can't be assigned to \
                  the parameter type 'String'."
            .into(),
        location: Some(("lib/main.dart".into(), 42, 18)),
        context: vec![
            (41, "  @override".into()),
            (42, "  Widget build(BuildContext context) {".into()),
            (43, "    return const MaterialApp(title: 1234);".into()),
        ],
        caret_col: 18,
        note: "failed during: Building with Xcode".into(),
        output: vec![
            "Error: Compilation failed.".into(),
            "Target kernel_snapshot_program failed: Exception".into(),
            "Encountered error while building for device.".into(),
        ],
    })
}

fn mock_logs(state: State, clock: &probe::Clock) -> Vec<LogLine> {
    if !state.has_logs() {
        return Vec::new();
    }

    let line = |level, message: &str| LogLine {
        time: clock.now(),
        level,
        message: message.to_string(),
    };

    let mut logs = vec![
        line(
            Level::Wrn,
            "flutter: [ShorebirdCodePush]: Shorebird Engine not available, using no-op implementation. This occurs when using package:shorebird_code_push in an app that does not contain the Shorebird Engine.",
        ),
        line(
            Level::Inf,
            "flutter: [CWClub state] Auth token restored from secure storage (user: okasputra@gmail.com)",
        ),
        line(
            Level::Inf,
            "flutter: [CWClub UI] Initializing homepage feed with 24 widget components...",
        ),
        line(
            Level::Err,
            "flutter: The following assertion was thrown building CheckoutScreen(dirty, dependencies: [_InheritedProviderScope<CartBloc?>]):",
        ),
        line(
            Level::Err,
            "flutter: 'package:flutter/src/widgets/framework.dart': Failed assertion: line 4795 pos 12: '_debugCurrentBuildTarget == null': is not true.",
        ),
        line(
            Level::Err,
            "flutter: #0      _AssertionError._doThrowNew (dart:core-patch/errors_patch.dart:51:61)",
        ),
    ];

    if state == State::ReloadFailed || state == State::ReloadDropped {
        return logs;
    }

    logs.push(line(
        Level::Reload,
        "Reloaded 125 of 1824 libraries in 148ms.",
    ));

    logs
}

fn mock_reload_note(state: State) -> &'static str {
    match state {
        State::ReloadInFlight => "Syncing updated Dart libraries to iPhone 16 Pro...",
        State::ReloadFailed => {
            "Hot reload — lib/screens/checkout.dart:88:14 Expected ';' after this."
        }
        State::ReloadDropped => "Hot reload not picked up by Flutter — press r again",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picker opened by `^D` sits over a child that is still talking, and
    /// every one of those announcements arrives as a `goto`. Without the banking
    /// in `goto` a reload landing at the wrong moment closes the list under the
    /// cursor, and `Esc` afterwards restores a state Flutter has already left.
    #[test]
    fn a_live_transition_does_not_close_the_picker() {
        let mut app = App::empty();
        app.state = State::Running;

        // `^D`: the list goes up, then the state behind it is banked.
        app.goto(State::MultipleDevices);
        app.resume = Some(State::Running);

        // Flutter keeps going while the list is up.
        app.goto(State::ReloadFailed);

        assert_eq!(
            app.state,
            State::MultipleDevices,
            "the list has to survive a transition from the live child"
        );

        // `Esc`.
        let back = app.resume.take().expect("a state to go back to");
        app.goto(back);

        assert_eq!(
            app.state,
            State::ReloadFailed,
            "what Flutter reached while the list was up is what comes back"
        );
    }

    /// The picker and the switch list are one order.
    ///
    /// They draw the same rows through the same widget, and they used to disagree about
    /// where those rows went — measured on one session, `iPhone 17 Pro Max` was the
    /// fourth row of the picker and the fifteenth of the switch list. Nothing about the
    /// device had changed but its state, and a list that rearranges itself between two
    /// frames makes `1`-`9` mean something different in each.
    #[test]
    fn both_frames_put_the_devices_in_the_same_order() {
        let order = |state| {
            App::new(state)
                .devices
                .iter()
                .map(|device| device.name.clone())
                .collect::<Vec<String>>()
        };

        assert_eq!(order(State::MultipleDevices), order(State::Switching));
    }

    /// `running` above `in use` above `last used`, and the canonical order below them.
    ///
    /// Asserted on one list rather than two frames because that is the fix: there is no
    /// per-frame ordering left to disagree with itself. The rows go in deliberately
    /// scrambled, so a rank that quietly stopped applying would show up as the input
    /// order surviving.
    #[test]
    fn the_list_is_ranked_by_what_is_going_on_and_then_by_the_machine() {
        let row = |id: &str, boot: Option<Boot>, platform: Platform| Device {
            id: id.into(),
            name: id.into(),
            platform,
            target_platform: String::new(),
            sdk: String::new(),
            virtual_device: platform.needs_boot(),
            last_used: false,
            boot,
        };

        let mut app = App::empty();

        let asleep = |id: &str| row(id, Some(Boot::Sim(id.into())), Platform::Ios);

        app.devices = vec![
            row("chrome", None, Platform::Web),
            asleep("sleeping-b"),
            row("mine", None, Platform::Android),
            asleep("remembered"),
            row("awake", None, Platform::Ios),
            row("theirs", None, Platform::Ios),
            asleep("sleeping-a"),
        ];

        app.target = Some(row("mine", None, Platform::Android));
        app.busy.insert("theirs".to_string());
        app.busy.insert("mine".to_string());

        if let Some(device) = app.devices.iter_mut().find(|d| d.id == "remembered") {
            device.last_used = true;
        }

        app.sort_devices();

        let order: Vec<&str> = app.devices.iter().map(|d| d.id.as_str()).collect();

        assert_eq!(
            order,
            [
                // The run, and `mine` is in `busy` too — this tab put it there. Ranked
                // as the run, not as somebody else's.
                "mine",
                "theirs",
                "remembered",
                // Up and free, then the two that need booting in the order they
                // arrived, then the host.
                "awake",
                "sleeping-b",
                "sleeping-a",
                "chrome",
            ]
        );

        // A row `Enter` refuses must not be the row the cursor opens on, and here the
        // remembered device is free while the top two rows are not.
        assert_eq!(app.devices[app.first_pickable()].id, "remembered");
    }
}
