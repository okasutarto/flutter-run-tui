//! frun — a TUI front end for `fvm flutter run`.
//!
//!   frun-tui [flutter args...]       run, in the current Flutter project
//!   frun-tui --dump <state> [WxH]    render one mock frame to stdout
//!   frun-tui --all [WxH]             every state, in flow order
//!   frun-tui --hits <state> [WxH]    probe the clickable regions
//!   frun-tui --rows <state> [W]      degradation ladder across heights
//!   frun-tui --states                list the state slugs
//!   frun-tui --demo                  walk the flow on a timer
//!
//! The flags render mock data and need no device. They are how the layout gets
//! verified, because a row that overflows its budget here is clipped in silence.

mod budget;
mod data;
mod dump;
mod flutter;
mod probe;
mod theme;
mod ui;
mod widgets;

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use data::{Action, App, Msg, State};
use flutter::Session;
use ui::logo::Logo;

/// What the command line asked for.
///
/// Separated from `main` so the routing can be tested. The bug that prompted
/// this was pure arm ordering: a `starts_with("--")` catch-all sitting above a
/// literal arm swallowed `--demo`, and nothing in the test suite looked at
/// argument parsing at all.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    States,
    Dump(State, u16, u16),
    All(u16, u16),
    Hits(State, u16, u16),
    Rows(State, u16),
    Demo,
    /// Report what the machine actually answered, and exit.
    Probe,
    /// A real run, carrying whatever else was on the command line for Flutter.
    Run(Vec<String>),
}

fn parse(args: &[String]) -> io::Result<Command> {
    let flag = args.first().map(String::as_str);

    Ok(match flag {
        Some("--states") => Command::States,

        Some("--dump") => {
            let (w, h) = size(args.get(2), 106, 45);
            Command::Dump(state_arg(args.get(1))?, w, h)
        }

        Some("--all") => {
            let (w, h) = size(args.get(1), 106, 45);
            Command::All(w, h)
        }

        Some("--hits") => {
            let (w, h) = size(args.get(2), 106, 45);
            Command::Hits(state_arg(args.get(1))?, w, h)
        }

        Some("--rows") => Command::Rows(
            state_arg(args.get(1))?,
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(106),
        ),

        Some("--demo") => Command::Demo,

        Some("--probe") => Command::Probe,

        // Catch-all for typos. Must stay last among the flag arms: arms are
        // tried in order, so a guard this broad placed above a literal swallows
        // it.
        //
        // Only frun's own flags are caught. Flutter has plenty of its own
        // (`--flavor`, `--dart-define`), and those are forwarded, which is why
        // this is limited to the `--` names frun actually claims.
        Some(other) if FRUN_FLAGS.contains(&other) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown flag {other}"),
            ))
        }

        _ => Command::Run(args.to_vec()),
    })
}

/// Flags frun owns. Anything else beginning with `--` belongs to Flutter.
const FRUN_FLAGS: [&str; 8] = [
    "--states", "--dump", "--all", "--hits", "--rows", "--demo", "--probe", "--help",
];

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match parse(&args)? {
        Command::States => {
            for state in State::ALL {
                println!("{}", state.slug());
            }

            Ok(())
        }

        Command::Dump(state, w, h) => {
            print!("{}", dump::dump(&mut App::new(state), w, h));
            Ok(())
        }

        Command::All(w, h) => {
            for state in State::ALL {
                println!(
                    "\x1b[2m── {} ─────────────────────────\x1b[0m",
                    state.slug()
                );

                print!("{}", dump::dump(&mut App::new(state), w, h));
                println!();
            }

            Ok(())
        }

        Command::Hits(state, w, h) => {
            print!("{}", dump::hits(&mut App::new(state), w, h));
            Ok(())
        }

        Command::Rows(state, w) => {
            print!("{}", dump::rows(&App::new(state), w));
            Ok(())
        }

        Command::Demo => {
            let mut app = App::new(State::Detecting);
            app.demo = true;

            let (tx, rx) = mpsc::channel();
            run(app, tx, rx, Vec::new()).map(exit)
        }

        Command::Probe => {
            probe_report();
            Ok(())
        }

        Command::Run(extra) => live(extra).map(exit),
    }
}

/// Hand the exit code to the shell.
///
/// Declared as returning `()` rather than `!` so it can be passed straight to
/// `Result::map`, which cannot unify `Result<!, _>` with `main`'s return type.
fn exit(code: i32) {
    std::process::exit(code)
}

/// Print what the machine answered.
///
/// The counterpart to `--dump`: that verifies the layout without a device, this
/// verifies discovery without a tty. Both exist because the alternative is
/// judging a full-screen application by eye and hoping.
fn probe_report() {
    let mut project = probe::project();

    let source = match probe::sdk_versions() {
        Some((flutter, dart)) => {
            project.flutter = flutter;
            project.dart = dart;
            "flutter.version.json"
        }

        None => match probe::sdk_versions_slow() {
            Some((flutter, dart)) => {
                project.flutter = flutter;
                project.dart = dart;
                "fvm flutter --version --machine (slow path)"
            }

            None => "unresolved",
        },
    };

    println!("project   {}  {}", project.name, project.version);
    println!("branch    {}  ({} changed)", project.branch, project.dirty);
    println!(
        "sdk       flutter {}  dart {}",
        project.flutter, project.dart
    );
    println!("versions  {source}");
    println!("cwd       {}", project.cwd);

    let last = probe::last_device();
    println!("last used {}", if last.is_empty() { "-" } else { &last });

    println!();

    match probe::devices(&last) {
        Err(reason) => println!("devices   FATAL  {reason}"),

        Ok(reported) => {
            println!(
                "reported  {} from flutter devices --machine",
                reported.len()
            );

            for d in &reported {
                println!(
                    "  {} {:<24} {:<16} {:<14} {}",
                    d.platform.glyph(),
                    d.name,
                    d.id,
                    d.target_platform,
                    d.sdk,
                );
            }

            let targets = probe::targets(reported, &probe::last_device());
            let attached = targets.iter().any(probe::Device::attached);

            println!();
            println!(
                "picker    {} rows, frame {}",
                targets.len(),
                if attached { "picker" } else { "no-devices" }
            );

            for (i, t) in targets.iter().enumerate() {
                println!(
                    "  {}{} {:<24} {:<16} {}{}",
                    if t.last_used { "*" } else { " " },
                    t.platform.glyph(),
                    t.name,
                    t.id,
                    match &t.boot {
                        None => "run now".to_string(),
                        Some(boot) => format!("{boot:?}"),
                    },
                    if i == 0 { "   <- Enter" } else { "" },
                );
            }
        }
    }
}

/// A real run.
///
/// The pubspec check happens before the terminal is touched: DESIGN.md 4 makes
/// this fatal, and a fatal error is worth more as a line in the shell's
/// scrollback than as a frame in an alternate screen that is about to be torn
/// down.
fn live(extra: Vec<String>) -> io::Result<i32> {
    if !std::path::Path::new("pubspec.yaml").exists() {
        eprintln!("\x1b[38;5;203m✖ FATAL  pubspec.yaml not found\x1b[0m");
        eprintln!("\x1b[2mRun frun from a Flutter project directory.\x1b[0m");

        // Exit rather than returning an error. `main` returning `Err` makes Rust
        // print the `Debug` form of it, so the shell saw a tidy FATAL line
        // followed by `Error: Custom { kind: NotFound, .. }`.
        std::process::exit(1);
    }

    let mut project = probe::project();

    // One channel for every worker, created before any of them starts. Two
    // channels was the first attempt and it was silently broken: the loop held
    // the receiver of one and handed threads the sender of the other, so a boot
    // completing had nowhere to report to.
    let (tx, rx) = mpsc::channel();

    // The SDK manifest is a file read, so it costs nothing and the card can
    // paint immediately. Only its absence is expensive.
    match probe::sdk_versions() {
        Some((flutter, dart)) => {
            project.flutter = flutter;
            project.dart = dart;
        }

        None => {
            // FVM has not materialised an SDK for this project yet, so the
            // versions have to come from the Flutter tool, which costs 3-4
            // seconds of Dart VM startup. Off the main thread, so the card fills
            // in when it lands rather than holding up device detection.
            let versions = tx.clone();

            std::thread::spawn(move || {
                if let Some((flutter, dart)) = probe::sdk_versions_slow() {
                    let _ = versions.send(Msg::Versions(flutter, dart));
                }
            });
        }
    }

    detect(&tx);

    run(App::live(project), tx, rx, extra)
}

/// Start device discovery.
///
/// DESIGN.md 4: always entered, always first. Nothing below it is reachable
/// without passing through here.
fn detect(tx: &Sender<Msg>) {
    let tx = tx.clone();

    std::thread::spawn(move || {
        let last = probe::last_device();

        // The bootable scan is no longer conditional. It used to run only when
        // nothing was attached, which is exactly why booting was unreachable the
        // rest of the time. Two extra spawns, both of which answer immediately.
        let msg = probe::devices(&last).map(|d| probe::targets(d, &last));

        let _ = tx.send(Msg::Devices(msg));
    });
}

fn state_arg(arg: Option<&String>) -> io::Result<State> {
    let Some(slug) = arg else {
        return Ok(State::Running);
    };

    State::from_slug(slug).ok_or_else(|| {
        let known: Vec<&str> = State::ALL.iter().map(|s| s.slug()).collect();

        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unknown state {slug:?}, expected one of: {}",
                known.join(", ")
            ),
        )
    })
}

fn size(arg: Option<&String>, dw: u16, dh: u16) -> (u16, u16) {
    let Some(spec) = arg else {
        return (dw, dh);
    };

    let Some((w, h)) = spec.split_once('x') else {
        return (dw, dh);
    };

    (w.parse().unwrap_or(dw), h.parse().unwrap_or(dh))
}

// ============================================================
// Event loop
// ============================================================

/// Everything the loop owns beyond the app itself.
struct Ctx {
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    session: Option<Session>,
    /// Extra arguments to pass through to Flutter, kept for a retried build.
    extra: Vec<String>,
    /// Set when the loop should return.
    done: bool,
}

/// Runs the UI and returns the process exit code.
///
/// The code is part of the interface, not an afterthought: DESIGN.md 4 specifies
/// 130 for a cancelled pick, and a failed build has to be non-zero or a script
/// wrapping frun cannot tell it apart from a successful run.
fn run(mut app: App, tx: Sender<Msg>, rx: Receiver<Msg>, extra: Vec<String>) -> io::Result<i32> {
    // Before raw mode and before the alternate screen: the capability query
    // writes control sequences to stdout and reads the reply, which needs an
    // uncontended terminal.
    let mut art = Logo::detect();

    let mut ctx = Ctx {
        rx,
        tx,
        session: None,
        extra,
        done: false,
    };

    enable_raw_mode()?;

    // Mouse capture is NOT enabled here. Capturing takes text selection away
    // from the terminal, and copying a stack trace out of the log window is a
    // large part of what that window is for. `m` turns it on when the scroll
    // wheel or a clickable control is wanted.
    execute!(io::stdout(), EnterAlternateScreen)?;

    let result = event_loop(&mut app, &mut ctx, &mut art);

    // Restore before propagating any error, so a failure inside the loop cannot
    // leave the terminal in raw mode or holding the mouse.
    disable_raw_mode()?;

    if app.mouse_on {
        execute!(io::stdout(), DisableMouseCapture)?;
    }

    execute!(io::stdout(), LeaveAlternateScreen)?;

    // The child outlives the screen otherwise: leaving the alternate screen does
    // not stop Flutter, and an orphaned `flutter run` holds the device.
    if let Some(session) = &mut ctx.session {
        session.kill();
    }

    result?;

    if let Some(fatal) = &app.fatal {
        eprintln!("\x1b[38;5;203m✖ FATAL  {fatal}\x1b[0m");

        return Ok(1);
    }

    // Leaving the alternate screen discards everything the app drew. Replaying
    // the log buffer here is the mitigation: filtering and scrolling while
    // running, a plain transcript in scrollback afterwards.
    if !app.logs.is_empty() {
        let mut out = io::stdout();

        writeln!(out)?;
        writeln!(out, "\x1b[2m── transcript ──────────────────────\x1b[0m")?;

        for log in &app.logs {
            writeln!(out, "{}  {}  {}", log.time, log.level.badge(), log.message)?;
        }
    }

    Ok(app.exit_code)
}

fn event_loop(app: &mut App, ctx: &mut Ctx, art: &mut Logo) -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        terminal.draw(|frame| ui::render(frame, app, art))?;

        drain(app, ctx);

        // An unacknowledged r/R has a deadline, and nothing arriving on the
        // channel can be relied on to notice it has passed.
        app.tick_pending();

        if ctx.done {
            return Ok(());
        }

        // Timeout drives the spinner and the elapsed clocks. Anything longer
        // reads as a stutter, anything shorter is redraw for its own sake.
        if !event::poll(Duration::from_millis(80))? {
            app.tick += 1;

            // ~2.4s per frame in demo mode, which is long enough to read a
            // screen and short enough not to feel stalled.
            if app.demo && app.tick.is_multiple_of(30) {
                app.next_state();
            }

            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key_press(app, ctx, key)? {
                    return Ok(());
                }
            }

            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(action) = app.hit_test(mouse.column, mouse.row) {
                        if apply(app, ctx, action) {
                            return Ok(());
                        }
                    }
                }

                MouseEventKind::Moved => {
                    app.hover = app.hit_test(mouse.column, mouse.row);
                }

                // Three rows a notch, which is what a wheel click means in every
                // other scrolling view. One row per notch reads as a stuck wheel.
                MouseEventKind::ScrollDown if app.state.has_logs() => app.scroll_logs(-3),
                MouseEventKind::ScrollUp if app.state.has_logs() => app.scroll_logs(3),

                MouseEventKind::ScrollDown => app.select_next(),
                MouseEventKind::ScrollUp => app.select_prev(),

                _ => {}
            },

            _ => {}
        }
    }
}

/// Handle everything waiting on the channel.
///
/// Non-blocking, so the loop keeps its own cadence: the spinner and the boot
/// clock have to advance whether or not a worker has anything to say.
fn drain(app: &mut App, ctx: &mut Ctx) {
    loop {
        match ctx.rx.try_recv() {
            Ok(msg) => handle(app, ctx, msg),
            Err(TryRecvError::Empty) => return,

            // Every sender is gone. In demo mode there never was one.
            Err(TryRecvError::Disconnected) => return,
        }
    }
}

fn handle(app: &mut App, ctx: &mut Ctx, msg: Msg) {
    match msg {
        Msg::Versions(flutter, dart) => {
            app.flutter = flutter;
            app.dart = dart;
        }

        Msg::Devices(Err(reason)) => {
            app.fatal = Some(reason);
            ctx.done = true;
        }

        Msg::Devices(Ok(targets)) => devices_answered(app, ctx, targets),

        Msg::Booted(Ok(id)) => {
            app.boot_started = None;

            // Straight to the run. The device is already known, so the picker is
            // skipped: 3.3 is explicit that asking again would name the same
            // device a third time in a row.
            let device = booted_device(app, &id);

            app.devices = Vec::new();
            launch(app, ctx, device);
        }

        Msg::Booted(Err(reason)) => {
            app.boot_started = None;
            app.fatal = Some(format!("{} {reason}", app.boot_name));
            ctx.done = true;
        }

        Msg::Line(line) => flutter::feed(app, &line),

        // The unterminated tail. Only the acknowledgement is read out of it:
        // Flutter's progress message carries no newline and stays open for the
        // whole operation, so waiting for a complete line would mean waiting
        // until the reload had already finished.
        Msg::Partial(tail) => {
            let pending = app.pending.as_ref().is_some_and(|p| !p.acked);

            if pending {
                let text = flutter::clean(&tail);

                for (marker, kind) in flutter::ACK_MARKERS {
                    if text.contains(marker) {
                        app.ack_reload(kind);
                        break;
                    }
                }
            }
        }

        Msg::Eof => child_exited(app, ctx),
    }
}

/// State 1 answered.
///
/// The picker is always shown, per DESIGN.md 7.6. It used to auto-launch when
/// exactly one device was attached, on the reasoning that one option is not a
/// choice. Using it disproved that: with only the iPhone simulator up there was
/// no way to ask for Android, because booting is a choice and auto-launch removed
/// it. One keystroke per run buys back the ability to say what you meant.
///
/// The remembered device is preselected, so that keystroke is a bare `Enter`.
///
/// Two frames, one list. Which frame depends only on whether anything is
/// attached, because "nothing is attached, these can be started" is the honest
/// heading in that case and a poor one in the other.
fn devices_answered(app: &mut App, ctx: &mut Ctx, targets: Vec<probe::Device>) {
    if targets.is_empty() {
        app.fatal = Some("No device(s) detected".into());
        ctx.done = true;

        return;
    }

    let attached = targets.iter().any(probe::Device::attached);

    // `targets` is already ordered running-first, so this is the top row unless
    // the remembered device is further down.
    app.selected_device = targets.iter().position(|d| d.last_used).unwrap_or(0);

    app.devices = targets;
    app.scroll = 0;

    app.goto(if attached {
        State::MultipleDevices
    } else {
        State::NoDevices
    });
}

/// A device that was booted but is not in any list yet.
fn booted_device(app: &App, id: &str) -> probe::Device {
    let selected = app.selected();

    probe::Device {
        id: id.to_string(),
        name: selected
            .map(|d| d.name.clone())
            .unwrap_or_else(|| id.to_string()),
        platform: selected
            .map(|d| d.platform)
            .unwrap_or(probe::Platform::Android),
        target_platform: String::new(),
        sdk: String::new(),
        virtual_device: true,
        last_used: false,
        boot: None,
    }
}

/// Take a device as the target and start the build.
fn launch(app: &mut App, ctx: &mut Ctx, device: probe::Device) {
    app.choose(device, &ctx.extra);
    spawn_session(app, ctx);
}

fn spawn_session(app: &mut App, ctx: &mut Ctx) {
    let Some(device) = app.target.as_ref().map(|d| d.id.clone()) else {
        return;
    };

    app.begin_build();

    match Session::spawn(&device, &ctx.extra, ctx.tx.clone()) {
        Ok(session) => ctx.session = Some(session),

        Err(reason) => {
            app.fatal = Some(reason);
            ctx.done = true;
        }
    }
}

/// The pty closed.
///
/// This is the whole of build-failure detection, and it is deliberately not a
/// catalogue of error strings: a run that never opened an interactive session
/// did not succeed, whatever killed it. Gradle, Xcode, `pub get`, a missing
/// entrypoint and whatever breaks next all arrive here.
fn child_exited(app: &mut App, ctx: &mut Ctx) {
    let code = ctx
        .session
        .as_mut()
        .and_then(Session::exit_code)
        .unwrap_or(1);

    if app.state.build_done() {
        // Flutter shut itself down, which is the graceful exit `q` asks for.
        ctx.done = true;
        return;
    }

    if app.state.has_build() {
        app.end_build();
        app.exit_code = code;
        app.failure = Some(flutter::failure(app, code));
        app.goto(State::BuildFailed);

        return;
    }

    ctx.done = true;
}

// ============================================================
// Keys
// ============================================================

/// Returns true when the loop should end.
fn key_press(
    app: &mut App,
    ctx: &mut Ctx,
    key: ratatui::crossterm::event::KeyEvent,
) -> io::Result<bool> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(apply(app, ctx, Action::Stop));
        }

        KeyCode::Char('q') => return Ok(apply(app, ctx, Action::Quit)),

        // Cancel, only where there is something to cancel. During a run `Esc` is
        // Flutter's, and quitting on it would be a surprise.
        KeyCode::Esc => {
            if matches!(
                app.state,
                State::NoDevices | State::MultipleDevices | State::Detecting
            ) {
                // 130, the shell's convention for "cancelled at a prompt", which
                // is what the implementation being replaced returned.
                app.exit_code = 130;

                return Ok(true);
            }

            forward(ctx, b"\x1b");
        }

        // Prototype navigation. Live, the state is decided by what Flutter is
        // doing, so these keys belong to Flutter instead.
        KeyCode::Tab | KeyCode::Right if !app.live => app.next_state(),
        KeyCode::BackTab | KeyCode::Left if !app.live => app.prev_state(),

        KeyCode::Char('m') => {
            app.mouse_on = !app.mouse_on;
            app.hover = None;

            if app.mouse_on {
                execute!(io::stdout(), EnableMouseCapture)?;
            } else {
                execute!(io::stdout(), DisableMouseCapture)?;
            }
        }

        KeyCode::Char('r') => {
            let action = if app.state == State::BuildFailed {
                Action::RetryBuild
            } else {
                Action::Reload
            };

            return Ok(apply(app, ctx, action));
        }

        KeyCode::Char('R') => return Ok(apply(app, ctx, Action::Restart)),

        // Arrows and `j`/`k` mean the log window wherever one is on screen, and
        // the device list otherwise. There is never both.
        //
        // `j`/`k` are taken from Flutter here, which 5.1 otherwise forbids. Flutter
        // binds neither, and reaching a stack trace eight rows tall in a window
        // twelve rows deep is worth the two letters.
        KeyCode::Down | KeyCode::Char('j') if app.state.has_logs() => app.scroll_logs(-1),
        KeyCode::Up | KeyCode::Char('k') if app.state.has_logs() => app.scroll_logs(1),

        KeyCode::Down => app.select_next(),
        KeyCode::Up => app.select_prev(),

        // 7.7: hand the whole window to the log stream. The three cards above are
        // static once a run is under way, so on a short terminal they are 26 rows
        // describing things that are not changing while the only region that is
        // gets twelve.
        KeyCode::Char('e') => app.expanded = !app.expanded,

        KeyCode::Enter => return Ok(enter(app, ctx)),

        // Number hotkeys, per DESIGN.md 3.3 mode 4. Only where a list is on
        // screen: everywhere else a digit is Flutter's, and it has to arrive
        // unchanged.
        KeyCode::Char(c @ '1'..='9')
            if matches!(app.state, State::NoDevices | State::MultipleDevices) =>
        {
            let index = c as usize - '1' as usize;

            if index < app.devices.len() {
                app.selected_device = index;
                return Ok(enter(app, ctx));
            }
        }

        // Every key not claimed above is Flutter's, per DESIGN.md 5.1: `h` help,
        // `d` detach, `c` clear, `p` debug paint, `o` platform toggle, `w` widget
        // tree, and more. Intercepting them would silently remove functionality
        // that works today.
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            forward(ctx, c.encode_utf8(&mut buf).as_bytes());
        }

        _ => {}
    }

    Ok(false)
}

/// `Enter` means "select" in the two states that offer a list, and nothing
/// anywhere else.
fn enter(app: &mut App, ctx: &mut Ctx) -> bool {
    if !matches!(app.state, State::NoDevices | State::MultipleDevices) {
        forward(ctx, b"\r");
        return false;
    }

    let Some(device) = app.selected().cloned() else {
        return false;
    };

    // One list, so one branch: either the target has to be started first or it is
    // ready to run. Which frame the row was on does not matter.
    match device.boot.clone() {
        None => {
            app.devices = Vec::new();
            launch(app, ctx, device);
        }

        Some(boot) => {
            app.boot_name = device.name.clone();
            app.boot_started = Some(std::time::Instant::now());
            app.goto(State::Booting);

            let tx = ctx.tx.clone();

            std::thread::spawn(move || {
                let _ = tx.send(Msg::Booted(probe::boot(&boot)));
            });
        }
    }

    false
}

fn forward(ctx: &mut Ctx, bytes: &[u8]) {
    if let Some(session) = &mut ctx.session {
        session.send(bytes);
    }
}

/// One path for keys and clicks alike, so the two cannot drift apart.
///
/// Returns true when the loop should end.
fn apply(app: &mut App, ctx: &mut Ctx, action: Action) -> bool {
    app.last_action = Some(action);

    match action {
        Action::Reload | Action::Restart => {
            let kind = if action == Action::Reload {
                flutter::Kind::Reload
            } else {
                flutter::Kind::Restart
            };

            // Only meaningful once Flutter is listening. Before that the key
            // would reach a build that has no reload to perform.
            if !app.state.build_done() {
                return false;
            }

            app.request_reload(kind);
            forward(ctx, kind.key().as_bytes());

            false
        }

        // A pty restart, not a keypress: kill the child, reap it, respawn with
        // stage state reset. Hot restart is not a build retry and cannot
        // substitute for one.
        //
        // The failed build's log is kept rather than cleared, because comparing
        // the two runs is the point.
        Action::RetryBuild => {
            if let Some(session) = &mut ctx.session {
                session.kill();
            }

            ctx.session = None;
            spawn_session(app, ctx);

            false
        }

        Action::StartDevice => enter(app, ctx),

        // Graceful: Flutter receives the key and shuts itself down (⏏). The loop
        // keeps running until the child closes the pty, so the exit stays
        // Flutter's to make.
        Action::Quit => {
            if ctx.session.is_some() && app.state.build_done() {
                forward(ctx, b"q");
                return false;
            }

            true
        }

        // Interrupt: SIGINT forwarded to the child (⏹), which is a different
        // exit from `q` and the existing implementation keeps them distinct.
        Action::Stop => match &mut ctx.session {
            Some(session) => {
                session.interrupt();
                true
            }

            None => true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Command {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse(&owned).expect("should parse")
    }

    /// Every documented flag must reach its own arm.
    ///
    /// The bug this guards against was ordering, not logic: a
    /// `starts_with("--")` catch-all placed above the literal arms swallowed
    /// `--demo`, and it shipped because nothing tested argument parsing.
    #[test]
    fn every_flag_routes_to_its_own_arm() {
        assert_eq!(parsed(&["--states"]), Command::States);
        assert_eq!(parsed(&["--demo"]), Command::Demo);
        assert_eq!(parsed(&[]), Command::Run(Vec::new()));

        assert_eq!(
            parsed(&["--dump", "running"]),
            Command::Dump(State::Running, 106, 45)
        );

        assert_eq!(parsed(&["--all"]), Command::All(106, 45));

        assert_eq!(
            parsed(&["--hits", "picker"]),
            Command::Hits(State::MultipleDevices, 106, 45)
        );

        assert_eq!(
            parsed(&["--rows", "building"]),
            Command::Rows(State::Building, 106)
        );
    }

    #[test]
    fn size_argument_is_honoured() {
        assert_eq!(
            parsed(&["--dump", "running", "142x56"]),
            Command::Dump(State::Running, 142, 56)
        );

        assert_eq!(parsed(&["--all", "80x30"]), Command::All(80, 30));
        assert_eq!(
            parsed(&["--rows", "running", "80"]),
            Command::Rows(State::Running, 80)
        );
    }

    /// Flutter's own flags have to survive being passed through, or every
    /// `--flavor` and `--dart-define` would be rejected as a typo.
    #[test]
    fn flutter_flags_are_forwarded_not_rejected() {
        assert_eq!(
            parsed(&["--flavor", "staging"]),
            Command::Run(vec!["--flavor".into(), "staging".into()])
        );

        assert_eq!(
            parsed(&["--dart-define=API=prod"]),
            Command::Run(vec!["--dart-define=API=prod".into()])
        );
    }

    #[test]
    fn a_typo_in_a_frun_flag_is_reported() {
        let owned = vec!["--help".to_string()];
        let err = parse(&owned).expect_err("should reject");

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    /// Every slug printed by `--states` must be accepted back.
    #[test]
    fn state_slugs_round_trip() {
        for state in State::ALL {
            let owned = vec!["--dump".to_string(), state.slug().to_string()];

            assert_eq!(
                parse(&owned).expect("slug should parse"),
                Command::Dump(state, 106, 45),
                "{} did not round-trip",
                state.slug()
            );
        }
    }

    #[test]
    fn an_unknown_state_names_the_valid_ones() {
        let owned = vec!["--dump".to_string(), "nope".to_string()];
        let err = parse(&owned).expect_err("should reject");

        assert!(err.to_string().contains("running"), "{err}");
    }
}
