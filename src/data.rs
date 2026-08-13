//! Application state and the mock data behind each frame.
//!
//! Still static: no pty, no Flutter, no device discovery. What this proves is
//! that the design survives all eleven states and every terminal size, which
//! is a different question from whether the parser works.

use ratatui::layout::Rect;

use crate::theme;

// ============================================================
// States
// ============================================================

/// The eleven frames from DESIGN.md section 4, in flow order.
///
/// Derived from `frun.zsh` + `frun-runner` rather than designed alongside
/// them, so each variant corresponds to a branch that already exists.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// 1. `fvm flutter devices --machine` is running.
    Detecting,
    /// 2. Zero devices answered; offer everything launchable.
    NoDevices,
    /// 3. Booting a simulator or emulator, possibly for minutes.
    Booting,
    /// 4. Two or more devices answered; pick one.
    MultipleDevices,
    /// 5. Exactly one device answered; no picker is shown.
    SingleDevice,
    /// 6. `fvm flutter run` is building.
    Building,
    /// 7. The build died. No implementation behind this yet.
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
    /// Not during a build: nothing has printed yet. Not on build failure
    /// either, where the failure card takes the space and the compiler output
    /// is the only output that matters.
    pub fn has_logs(self) -> bool {
        matches!(
            self,
            State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped
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
// Devices
// ============================================================

/// Which family a target belongs to, for the glyph and the badge.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Ios,
    Android,
    Desktop,
    Web,
}

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
            Platform::Desktop => "macOS",
            Platform::Web => "Web",
        }
    }

    /// Desktop and web are always available, so they never pass through
    /// `State::Booting`. This asymmetry is the practical consequence of
    /// lifting the mobile-only restriction.
    pub fn needs_boot(self) -> bool {
        matches!(self, Platform::Ios | Platform::Android)
    }
}

pub struct Device {
    pub name: &'static str,
    pub platform: Platform,
    pub id: &'static str,
    /// Simulator or emulator rather than hardware.
    pub virtual_device: bool,
    /// Promoted to the top of the picker, and labelled as such.
    pub last_used: bool,
}

// ============================================================
// Build stages
// ============================================================

/// Stage names are the ones Flutter emits, and the set is platform-dependent:
/// iOS runs CocoaPods then Xcode, Android runs Gradle then an install. There
/// is no fixed pipeline.
pub struct Stage {
    pub label: &'static str,
    pub duration: &'static str,
    pub done: bool,
}

// ============================================================
// Logs
// ============================================================

/// Only what the application itself produces.
///
/// `SYS`, `BLD` and `OK` are gone: everything they carried was a build stage,
/// and the build tracker already owns those. Showing them here put the same
/// two facts on one screen twice.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    pub time: &'static str,
    pub level: Level,
    pub message: &'static str,
}

// ============================================================
// Compiler failure
// ============================================================

/// A code frame around the reported error position.
///
/// Dart emits the offending line and a caret itself, so that much is free
/// passthrough. The lines either side require reading the file at the reported
/// line number, which is worth it: one line of context is usually the
/// difference between recognising the mistake and opening the editor.
pub struct CodeFrame {
    pub summary: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
    pub context: &'static [(u32, &'static str)],
    pub caret_pad: usize,
    pub caret: &'static str,
    pub caret_note: &'static str,
    pub tail: &'static str,
}

// ============================================================
// Actions
// ============================================================

/// Anything the user can trigger, by key or by click.
///
/// Both input paths resolve here, so a click cannot drift out of sync with its
/// keyboard equivalent. In the real build this is also where the byte is
/// forwarded to the pty.
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
            Action::Quit => "Quit",
            Action::Stop => "Stop",
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

    pub project: &'static str,
    pub version: &'static str,
    pub branch: &'static str,
    pub git_clean: bool,
    pub flutter: &'static str,
    pub dart: &'static str,
    pub cwd: &'static str,

    pub devices: Vec<Device>,
    pub selected_device: usize,
    pub scroll: usize,

    pub target_name: &'static str,
    pub target_platform_id: &'static str,
    pub target_os: &'static str,
    pub target_kind: &'static str,
    pub command: &'static str,

    pub stages: Vec<Stage>,
    pub build_time: &'static str,
    pub sync_time: &'static str,

    pub failure: Option<CodeFrame>,
    pub exit_code: i32,

    pub logs: Vec<LogLine>,

    pub reload_note: &'static str,

    /// Spinner frame counter, advanced by the event loop tick.
    pub tick: usize,

    pub hits: Vec<Hit>,
    pub hover: Option<Action>,

    /// Off by default. Capturing the mouse takes text selection away from the
    /// terminal, and copying a stack trace out of the log window is a large
    /// part of what that window is for.
    pub mouse_on: bool,

    /// Last action dispatched, so a click that landed is distinguishable from
    /// one that missed.
    pub last_action: Option<Action>,

    /// True while the command line has focus. In NORMAL mode every unbound
    /// key is forwarded to Flutter, which has its own interactive commands
    /// (`h`, `d`, `c`, `p`, `o`, `w`); a prompt that captures keys at all
    /// times cannot coexist with that.
    pub command_mode: bool,
    pub command_input: String,
}

impl App {
    pub fn new(state: State) -> Self {
        Self {
            state,

            project: "cwclub",
            version: "2.1.0+32",
            branch: "refactor/cwclub-new",
            git_clean: true,
            flutter: "3.29.3",
            dart: "3.7.2",
            cwd: "~/cwclub",

            devices: devices_for(state),
            selected_device: 0,
            scroll: 0,

            target_name: "iPhone 16 Pro (emulator)",
            target_platform_id: "ios-sim (ios)",
            target_os: "iOS 18.2 (arm64)",
            target_kind: "Simulator / Emulator",
            command: "fvm flutter run -d ios-sim",

            stages: stages_for(state),
            build_time: "3.4s",
            sync_time: "240ms",

            failure: failure_for(state),
            exit_code: 1,

            logs: logs_for(state),

            reload_note: reload_note_for(state),

            tick: 0,

            hits: Vec::new(),
            hover: None,
            mouse_on: false,
            last_action: None,

            command_mode: false,
            command_input: String::new(),
        }
    }

    pub fn spinner(&self) -> &'static str {
        theme::SPINNER[self.tick % theme::SPINNER.len()]
    }

    pub fn goto(&mut self, state: State) {
        self.state = state;
        self.devices = devices_for(state);
        self.stages = stages_for(state);
        self.failure = failure_for(state);
        self.logs = logs_for(state);
        self.reload_note = reload_note_for(state);
        self.selected_device = 0;
        self.scroll = 0;
    }

    pub fn next_state(&mut self) {
        let i = State::ALL
            .iter()
            .position(|s| *s == self.state)
            .unwrap_or(0);
        self.goto(State::ALL[(i + 1) % State::ALL.len()]);
    }

    pub fn prev_state(&mut self) {
        let i = State::ALL
            .iter()
            .position(|s| *s == self.state)
            .unwrap_or(0);
        self.goto(State::ALL[(i + State::ALL.len() - 1) % State::ALL.len()]);
    }

    pub fn select_next(&mut self) {
        if !self.devices.is_empty() && self.selected_device + 1 < self.devices.len() {
            self.selected_device += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_device = self.selected_device.saturating_sub(1);
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
// Mock data per state
// ============================================================

fn devices_for(state: State) -> Vec<Device> {
    let d = |name, platform, id, virtual_device, last_used| Device {
        name,
        platform,
        id,
        virtual_device,
        last_used,
    };

    match state {
        // Every launchable target, not just phones. Desktop and web are
        // always available and need no boot.
        State::NoDevices | State::Booting => vec![
            d(
                "Pixel 10 Pro XL",
                Platform::Android,
                "Pixel_10_Pro_XL",
                true,
                false,
            ),
            d("Pixel 8", Platform::Android, "Pixel_8", true, false),
            d("iPhone 17 Pro", Platform::Ios, "8A3F91C2-4D2E", true, false),
            d(
                "iPhone 17 Pro Max",
                Platform::Ios,
                "1B7C22E9-90AF",
                true,
                false,
            ),
            d("iPhone Air", Platform::Ios, "C4D1099A-71B3", true, false),
            d(
                "iPad Pro 13-inch (M5)",
                Platform::Ios,
                "77E0C1B4-2A6D",
                true,
                false,
            ),
            d("macOS Desktop", Platform::Desktop, "macos", false, false),
            d("Chrome", Platform::Web, "chrome", false, false),
        ],

        State::MultipleDevices => vec![
            d("iPhone 16 Pro", Platform::Ios, "8A3F91C2-4D2E", true, true),
            d(
                "sdk gphone64 arm64",
                Platform::Android,
                "emulator-5554",
                true,
                false,
            ),
            d("macOS Desktop", Platform::Desktop, "macos", false, false),
            d("Chrome", Platform::Web, "chrome", false, false),
        ],

        _ => Vec::new(),
    }
}

fn stages_for(state: State) -> Vec<Stage> {
    let s = |label, duration, done| Stage {
        label,
        duration,
        done,
    };

    match state {
        // iOS: CocoaPods then Xcode. Gradle never appears here.
        State::Building => vec![
            s("Launching lib/main.dart", "0.4s", true),
            s("Running pod install", "1.2s", true),
            s("Running Xcode build", "", false),
        ],

        State::BuildFailed => vec![
            s("Launching lib/main.dart", "0.4s", true),
            s("Running pod install", "1.2s", true),
            s("Running Xcode build", "11.1s", false),
        ],

        State::Running | State::ReloadInFlight | State::ReloadFailed | State::ReloadDropped => {
            vec![
                s("Launching lib/main.dart", "0.4s", true),
                s("Running pod install", "1.2s", true),
                s("Xcode build done", "11.1s", true),
                s("Syncing files to device", "240ms", true),
                s("Interactive session ready", "", true),
            ]
        }

        _ => Vec::new(),
    }
}

fn failure_for(state: State) -> Option<CodeFrame> {
    if state != State::BuildFailed {
        return None;
    }

    Some(CodeFrame {
        summary: "Compiler Exception: Type mismatch",
        file: "lib/main.dart",
        line: 42,
        column: 18,
        context: &[
            (41, "@override"),
            (42, "Widget build(BuildContext context) {"),
            (43, "  return const MaterialApp(title: 1234);"),
        ],
        caret_pad: 20,
        caret: "^^^^",
        caret_note: "The argument type 'int' can't be assigned to parameter 'String'",
        tail: "Error: Target //lib:main failed to compile.",
    })
}

fn logs_for(state: State) -> Vec<LogLine> {
    if !state.has_logs() {
        return Vec::new();
    }

    let l = |time, level, message| LogLine {
        time,
        level,
        message,
    };

    let mut logs = vec![
        l(
            "07:42:22",
            Level::Wrn,
            "flutter: [ShorebirdCodePush]: Shorebird Engine not available, using no-op implementation. This occurs when using package:shorebird_code_push in an app that does not contain the Shorebird Engine.",
        ),
        l(
            "07:42:25",
            Level::Inf,
            "flutter: [CWClub state] Auth token restored from secure storage (user: okasputra@gmail.com)",
        ),
        l(
            "07:42:28",
            Level::Inf,
            "flutter: [CWClub UI] Initializing homepage feed with 24 widget components...",
        ),
        l(
            "07:42:31",
            Level::Err,
            "flutter: The following assertion was thrown building CheckoutScreen(dirty, dependencies: [_InheritedProviderScope<CartBloc?>]):",
        ),
        l(
            "07:42:31",
            Level::Err,
            "flutter: 'package:flutter/src/widgets/framework.dart': Failed assertion: line 4795 pos 12: '_debugCurrentBuildTarget == null': is not true.",
        ),
        l(
            "07:42:31",
            Level::Err,
            "flutter: #0      _AssertionError._doThrowNew (dart:core-patch/errors_patch.dart:51:61)",
        ),
    ];

    if state == State::ReloadFailed || state == State::ReloadDropped {
        return logs;
    }

    logs.push(l(
        "07:42:34",
        Level::Reload,
        "Reloaded 125 of 1824 libraries in 148ms.",
    ));

    logs
}

fn reload_note_for(state: State) -> &'static str {
    match state {
        State::ReloadInFlight => "Syncing updated Dart libraries to iPhone 16 Pro...",
        State::ReloadFailed => {
            "lib/screens/checkout.dart:88:14 — Expected ';' after this. Hot reload was rejected."
        }
        State::ReloadDropped => "not picked up by Flutter — press r again",
        _ => "",
    }
}
