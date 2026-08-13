//! Static stand-in for the data the real frun will gather.
//!
//! Values match the attached mockup so the two can be compared directly.

/// Which screen the app is showing.
///
/// This split is the whole argument of the prototype. The dashboard is
/// correct while you are choosing a device, because every field on it is
/// something you might act on. Once `flutter run` is live, none of it
/// changes and the only thing still moving is the log stream, so the
/// metadata collapses to one line and gives its rows back.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Dashboard,
    Streaming,
}

/// Something the user can trigger, by key or by click.
///
/// Both input paths resolve to this, so a mouse click cannot drift out of
/// sync with its keyboard equivalent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Reload,
    Restart,
    Quit,
    Stop,
}

impl Action {
    pub fn key(self) -> &'static str {
        match self {
            Action::Reload => "r",
            Action::Restart => "R",
            Action::Quit => "q",
            Action::Stop => "^C",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Reload => "hot reload",
            Action::Restart => "hot restart",
            Action::Quit => "quit",
            Action::Stop => "stop",
        }
    }
}

/// A rectangle the mouse can land on.
///
/// ratatui has no hit testing: it draws into a cell buffer and forgets the
/// geometry. So anything clickable has to have its `Rect` recorded during
/// render and looked up when a click arrives. The list is rebuilt every
/// frame, which is what keeps it correct when the layout degrades and a
/// card moves or disappears.
pub struct Hit {
    pub area: ratatui::layout::Rect,
    pub action: Action,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
    Build,
}

impl Level {
    pub fn badge(self) -> &'static str {
        match self {
            Level::Info => " info ",
            Level::Warn => " warn ",
            Level::Error => " err  ",
            Level::Build => " bld  ",
        }
    }
}

pub struct LogLine {
    pub time: &'static str,
    pub level: Level,
    pub source: &'static str,
    pub message: &'static str,
}

pub struct Step {
    pub label: &'static str,
    pub duration: &'static str,
    pub done: bool,
}

pub struct App {
    pub phase: Phase,

    pub project: &'static str,
    pub version: &'static str,
    pub branch: &'static str,
    pub git_clean: bool,
    pub flutter: &'static str,
    pub dart: &'static str,

    pub device: &'static str,
    pub platform: &'static str,
    pub device_id: &'static str,
    pub device_count: usize,

    pub steps: Vec<Step>,
    pub total_build: &'static str,
    pub session: &'static str,

    pub logs: Vec<LogLine>,
    pub selected_log: usize,

    /// Set while a hot reload is in flight, so the streaming footer has
    /// something to report other than "idle".
    pub reloading: bool,

    /// Clickable regions, rebuilt on every render.
    pub hits: Vec<Hit>,

    /// What the pointer is currently over, if mouse reporting is on.
    pub hover: Option<Action>,

    /// Whether the app is capturing the mouse.
    ///
    /// Capturing takes the mouse away from the terminal, which is why this
    /// is a runtime toggle rather than a fixed setting.
    pub mouse_on: bool,

    /// Last thing triggered, echoed in the footer so a click is visibly
    /// distinguishable from a missed click.
    pub last_action: Option<&'static str>,
}

impl App {
    pub fn mock() -> Self {
        Self {
            phase: Phase::Dashboard,

            project: "cwclub",
            version: "2.1.0+32",
            branch: "refactor/cwclub-new",
            git_clean: true,
            flutter: "3.29.3",
            dart: "3.7.2",

            device: "iPhone 17 Pro Max",
            platform: "iOS",
            device_id: "8A3F91C2-4D2E",
            device_count: 1,

            steps: vec![
                Step {
                    label: "flutter started",
                    duration: "0.4s",
                    done: true,
                },
                Step {
                    label: "application prepared",
                    duration: "1.1s",
                    done: true,
                },
                Step {
                    label: "xcode build complete",
                    duration: "11.1s",
                    done: true,
                },
                Step {
                    label: "files synced",
                    duration: "0.3s",
                    done: true,
                },
                Step {
                    label: "interactive session ready",
                    duration: "",
                    done: true,
                },
            ],
            total_build: "20.2s",
            session: "04:12",

            logs: vec![
                LogLine {
                    time: "07:42:22",
                    level: Level::Warn,
                    source: "shorebird",
                    message: "Shorebird Engine not available, using no-op implementation.",
                },
                LogLine {
                    time: "07:42:24",
                    level: Level::Info,
                    source: "auth",
                    message: "Restored session for okasputra@gmail.com",
                },
                LogLine {
                    time: "07:42:26",
                    level: Level::Info,
                    source: "router",
                    message: "pushNamed(/checkout) - CheckoutScreen mounted",
                },
                LogLine {
                    time: "07:42:29",
                    level: Level::Build,
                    source: "engine",
                    message: "Reloaded 3 of 1284 libraries in 412ms",
                },
                LogLine {
                    time: "07:42:31",
                    level: Level::Error,
                    source: "network",
                    message: "SocketException: Connection refused (port 8080)",
                },
                LogLine {
                    time: "07:42:31",
                    level: Level::Error,
                    source: "network",
                    message: "  at CartRepository.sync (cart_repository.dart:88)",
                },
                LogLine {
                    time: "07:42:35",
                    level: Level::Info,
                    source: "cart",
                    message: "Retry scheduled in 2s (attempt 1 of 5)",
                },
                LogLine {
                    time: "07:42:37",
                    level: Level::Warn,
                    source: "perf",
                    message: "Frame took 34ms - 2 frames dropped",
                },
                LogLine {
                    time: "07:42:41",
                    level: Level::Info,
                    source: "cart",
                    message: "Sync recovered, 12 items reconciled",
                },
            ],
            selected_log: 0,
            reloading: false,

            hits: Vec::new(),
            hover: None,
            mouse_on: true,
            last_action: None,
        }
    }

    /// Which action, if any, sits under a screen cell.
    pub fn hit_test(&self, col: u16, row: u16) -> Option<Action> {
        self.hits
            .iter()
            .find(|hit| {
                col >= hit.area.x
                    && col < hit.area.x + hit.area.width
                    && row >= hit.area.y
                    && row < hit.area.y + hit.area.height
            })
            .map(|hit| hit.action)
    }

    /// Run an action. Returns true when the app should exit.
    ///
    /// Deliberately shared by the key handler and the click handler. In the
    /// real build this is also where the byte gets forwarded to the pty, so
    /// clicking `r` and pressing `r` reach Flutter through one path.
    pub fn apply(&mut self, action: Action) -> bool {
        self.last_action = Some(action.label());

        match action {
            Action::Reload | Action::Restart => {
                self.reloading = !self.reloading;
                false
            }
            Action::Quit | Action::Stop => true,
        }
    }

    /// The same screen, fed data shaped like what the tools actually emit.
    ///
    /// Every value here is realistic rather than tidy: the device name is
    /// what `flutter devices --machine` reports for an Android emulator, the
    /// Gradle task is a real flavoured variant, the branch carries a ticket
    /// prefix, and the log lines are genuine Flutter output lengths
    /// including a framework exception banner.
    ///
    /// A layout that only looks right against curated sample data is not a
    /// layout, it is a picture.
    pub fn stress() -> Self {
        let mut app = Self::mock();

        app.project = "cwclub_mobile_app";
        app.version = "2.1.0+3241";
        app.branch = "feature/PROJ-4821-refactor-checkout-payment-sheet";

        app.device = "sdk gphone64 arm64";
        app.platform = "Android";
        app.device_id = "emulator-5554";
        app.device_count = 3;

        app.total_build = "3m 12.4s";

        app.steps = vec![
            Step {
                label: "flutter started",
                duration: "0.4s",
                done: true,
            },
            Step {
                label: "application prepared",
                duration: "1.1s",
                done: true,
            },
            Step {
                label: "running gradle task 'app:assembleDevelopmentDebug'",
                duration: "1,847ms",
                done: true,
            },
            Step {
                label: "app installed",
                duration: "693ms",
                done: true,
            },
            Step {
                label: "syncing files to device sdk gphone64 arm64",
                duration: "",
                done: false,
            },
        ];

        app.logs = vec![
            LogLine {
                time: "07:42:22",
                level: Level::Warn,
                source: "shorebird",
                message: "[ShorebirdCodePush]: Shorebird Engine not available, using no-op implementation.",
            },
            LogLine {
                time: "07:42:23",
                level: Level::Error,
                source: "flutter",
                message: "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════════════════════════════════",
            },
            LogLine {
                time: "07:42:23",
                level: Level::Error,
                source: "flutter",
                message: "The following assertion was thrown building CheckoutScreen(dirty, dependencies: [_InheritedProviderScope<CartBloc?>]):",
            },
            LogLine {
                time: "07:42:23",
                level: Level::Error,
                source: "flutter",
                message: "'package:flutter/src/widgets/framework.dart': Failed assertion: line 4795 pos 12: '_debugCurrentBuildTarget == null': is not true.",
            },
            LogLine {
                time: "07:42:23",
                level: Level::Error,
                source: "flutter",
                message: "#0      _AssertionError._doThrowNew (dart:core-patch/errors_patch.dart:51:61)",
            },
            LogLine {
                time: "07:42:24",
                level: Level::Info,
                source: "auth",
                message: "Restored session for okasputra@gmail.com",
            },
            LogLine {
                time: "07:42:29",
                level: Level::Build,
                source: "engine",
                message: "Reloaded 3 of 1284 libraries in 1,412ms",
            },
        ];

        app
    }

    pub fn toggle_phase(&mut self) {
        self.phase = match self.phase {
            Phase::Dashboard => Phase::Streaming,
            Phase::Streaming => Phase::Dashboard,
        };
    }

    pub fn scroll_down(&mut self) {
        if self.selected_log + 1 < self.logs.len() {
            self.selected_log += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        self.selected_log = self.selected_log.saturating_sub(1);
    }
}
