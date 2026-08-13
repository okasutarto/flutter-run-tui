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

/// The eleven frames from DESIGN.md section 4, in flow order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// 1. `fvm flutter devices --machine` is running.
    Detecting,
    /// 2. Nothing is attached; offer everything launchable.
    NoDevices,
    /// 3. Booting a simulator or emulator, possibly for minutes.
    Booting,
    /// 4. Two or more devices attached; pick one.
    MultipleDevices,
    /// 5. Exactly one; no picker is shown.
    SingleDevice,
    /// 6. `fvm flutter run` is building.
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
}

impl State {
    pub const ALL: [State; 11] = [
        State::Detecting,
        State::NoDevices,
        State::Booting,
        State::MultipleDevices,
        State::SingleDevice,
        State::Building,
        State::BuildFailed,
        State::Running,
        State::ReloadInFlight,
        State::ReloadFailed,
        State::ReloadDropped,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            State::Detecting => "detecting",
            State::NoDevices => "no-devices",
            State::Booting => "booting",
            State::MultipleDevices => "picker",
            State::SingleDevice => "single",
            State::Building => "building",
            State::BuildFailed => "build-failed",
            State::Running => "running",
            State::ReloadInFlight => "reload",
            State::ReloadFailed => "reload-failed",
            State::ReloadDropped => "reload-dropped",
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
            State::Detecting | State::NoDevices | State::Booting | State::MultipleDevices
        )
    }

    /// Whether the build tracker is on screen.
    pub fn has_build(self) -> bool {
        matches!(
            self,
            State::Building
                | State::BuildFailed
                | State::Running
                | State::ReloadInFlight
                | State::ReloadFailed
                | State::ReloadDropped
        )
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
            State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped
        )
    }

    /// Whether a hot reload or restart is being reported on the status row.
    pub fn reloading(self) -> bool {
        matches!(
            self,
            State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped
        )
    }

    /// Whether the build finished, which is what lets the tracker collapse.
    pub fn build_done(self) -> bool {
        matches!(
            self,
            State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped
        )
    }
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

    /// How many stages Flutter is expected to announce here.
    ///
    /// The platform is known before the build starts, and the trigger table in
    /// 3.4 is per-platform, so the count is knowable after all — which is what
    /// gives the progress bar an honest denominator.
    ///
    /// Only ever an upper bound. Flutter skips stages it does not need: no
    /// `pod install` when `Podfile.lock` is current, no install when attaching to
    /// an app that is already there. Being an upper bound is the useful
    /// direction — the bar can stall below full and then complete, which reads
    /// correctly, where the reverse would reach 100% and keep working.
    pub fn stage_count(self) -> usize {
        match self {
            // Launching, CocoaPods, Xcode, Syncing, ready. macOS builds through
            // CocoaPods and Xcode as well, so it counts the same.
            Platform::Ios | Platform::Desktop => 5,
            // Launching, Gradle, install, Syncing, ready.
            Platform::Android => 5,
            // No native toolchain in the middle: launching, Syncing, ready.
            Platform::Web => 3,
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
    Launch,
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
    /// A boot finished, carrying the id Flutter will use.
    Booted(Result<String, String>),
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
            Action::StartDevice => "⏎",
            Action::Quit => "q",
            Action::Stop => "^C",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Reload => "Hot reload",
            Action::Restart => "Hot restart",
            Action::RetryBuild => "Retry Build",
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
            Action::Stop => "Force stop",
        }
    }
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
    pub command: String,

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

            devices: Vec::new(),
            selected_device: 0,
            scroll: 0,
            log_scroll: 0,

            target: None,
            command: String::new(),

            boot_name: String::new(),
            boot_started: None,

            stages: Vec::new(),
            build_started: Instant::now(),
            build_time: "-".into(),
            sync_time: "-".into(),

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
    pub fn goto(&mut self, state: State) {
        self.state = state;
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

    /// Scroll the log window. Positive goes back in history.
    ///
    /// Not clamped here: the ceiling depends on how many visual rows the entries
    /// wrap to, which is only known at the width the window is drawn at. `logs.rs`
    /// clamps it while rendering, where that is known.
    pub fn scroll_logs(&mut self, delta: isize) {
        self.log_scroll = self.log_scroll.saturating_add_signed(delta);
    }

    /// Adopt a device as the run target.
    pub fn choose(&mut self, device: Device, extra: &[String]) {
        probe::remember_device(&device.id);

        self.command = crate::flutter::command_line(&device.id, extra);
        self.target = Some(device);
    }

    /// How the target card describes the target's kind.
    pub fn target_kind(&self) -> &'static str {
        match &self.target {
            Some(d) if d.virtual_device => "Simulator / Emulator",
            Some(_) => "Hardware",
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
    pub fn begin_build(&mut self) {
        self.stages.clear();
        self.failure = None;
        self.exit_code = 0;
        self.build_started = Instant::now();
        self.build_time = "-".into();
        self.sync_time = "-".into();
        self.pending = None;
        self.reload_note.clear();
        self.goto(State::Building);
    }

    /// Elapsed build time: counting while it builds, frozen once it stops.
    ///
    /// Frozen matters on failure as much as on success. A build that died at 11
    /// seconds whose clock keeps running says the build is still going.
    pub fn build_clock(&self) -> String {
        if self.state == State::Building {
            return crate::flutter::elapsed(self.build_started.elapsed());
        }

        self.build_time.clone()
    }

    /// Stop the clock, whatever the outcome.
    pub fn end_build(&mut self) {
        self.build_time = crate::flutter::elapsed(self.build_started.elapsed());
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
            .unwrap_or(5);

        expected.max(self.stages.len())
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
        app.exit_code = 1;
        app.command = "fvm flutter run -d ios-sim".into();

        app.target = Some(mock_device(
            "8A3F91C2-4D2E",
            "iPhone 16 Pro",
            Platform::Ios,
            "ios",
            "iOS 18.2 (arm64)",
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
        // Set on one row below rather than here, so `--dump no-devices` shows the
        // case the chip exists for: the device you always reach for is off.
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
        State::NoDevices | State::Booting => vec![
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
        key,
        label: label.into(),
        duration: duration.into(),
        done,
        started: if done { Instant::now() } else { waiting_since },
    };

    match state {
        // iOS: CocoaPods then Xcode. Gradle never appears here.
        State::Building => vec![
            stage(StageKey::Launch, "Starting Flutter", "0.4s", true),
            stage(StageKey::Pods, "Installing CocoaPods", "1.2s", true),
            stage(StageKey::Xcode, "Building with Xcode", "", false),
        ],

        State::BuildFailed => vec![
            stage(StageKey::Launch, "Starting Flutter", "0.4s", true),
            stage(StageKey::Pods, "Installing CocoaPods", "1.2s", true),
            stage(StageKey::Xcode, "Building with Xcode", "11.1s", false),
        ],

        s if s.build_done() => vec![
            stage(StageKey::Launch, "Starting Flutter", "0.4s", true),
            stage(StageKey::Pods, "Installing CocoaPods", "1.2s", true),
            stage(StageKey::Xcode, "Building with Xcode", "11.1s", true),
            stage(StageKey::Sync, "Syncing files", "240ms", true),
            stage(StageKey::Ready, "Interactive session ready", "0.1s", true),
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
