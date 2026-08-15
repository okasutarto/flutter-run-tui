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
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    KeyboardEnhancementFlags, MouseButton, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
    LeaveAlternateScreen, SetTitle,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use data::{Action, App, Ending, Level, Msg, State};
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

        // Sent first, and from here rather than left to `recheck`. The picker can be
        // answered inside four seconds — sooner than the first recheck — and both the
        // ` in use ` chip and the guard behind `Enter` are worthless if they arrive
        // after the pick. 25ms against the six seconds this thread is already spending.
        let _ = tx.send(Msg::Busy(probe::busy()));

        // The bootable scan is no longer conditional. It used to run only when
        // nothing was attached, which is exactly why booting was unreachable the
        // rest of the time. Two extra spawns, both of which answer immediately.
        let msg = probe::devices(&last).map(|d| probe::targets(d, &last));

        let _ = tx.send(Msg::Devices(msg));
    });
}

/// How often a list on screen is rechecked against the machine.
///
/// Device lists go stale with nobody touching them. Measured on this machine: two
/// booted simulators shut themselves down inside a minute, unasked — and a simulator
/// booted *after* detection sat in the list as a row offering to boot it. The chips
/// are claims about right now, so they have to be re-earned while they are on screen.
///
/// Four seconds against ~265ms of `adb`/`simctl` on a worker thread. The full scan
/// this replaces is 6s and would be unusable at any interval.
const RECHECK: Duration = Duration::from_secs(4);

/// Recheck the cached list, without paying for discovery again.
///
/// `detect()` cannot be reused here. It runs `fvm flutter devices --machine`, which
/// is six seconds of Dart VM startup on this machine against 145ms for `adb devices`
/// plus `simctl list -j`, and six seconds is long enough that the list would settle
/// after the user had already chosen a row from it.
///
/// So this asks the cheap question instead. The rows are already known; what is not
/// known is which of them are still real. Anything virtual that was up and is no
/// longer answering is dropped, and `probe::targets` then rebuilds the bootable rows
/// around what survived — so a shut-down emulator comes back as a row that offers to
/// boot it rather than one that claims to be ready.
///
/// Physical devices are kept whether or not they answered. `adb` covers Android, but
/// a physical iPhone is only ever visible to Flutter's own scan, and dropping a row
/// this cannot see would be worse than keeping one that has gone.
fn recheck(app: &App, ctx: &Ctx) {
    let cached = app.devices.clone();
    let tx = ctx.tx.clone();

    std::thread::spawn(move || {
        let alive = probe::alive();
        let last = probe::last_device();

        // Same worker, because it is the same question at the same cadence: what is
        // true of these rows right now.
        let _ = tx.send(Msg::Busy(probe::busy()));

        let mut real: Vec<probe::Device> = cached
            .into_iter()
            .filter_map(|mut device| {
                if !device.attached() || !device.virtual_device {
                    return Some(device);
                }

                let up = alive.contains(&device.id);

                match (device.boot.is_some(), up) {
                    // Booted since the list was built, by frun or by hand. Clearing
                    // `boot` is what turns the row from "press Enter to start this"
                    // into a device that is ready, which is the whole of the ` active `
                    // chip. Without this a simulator brought up after detection sat in
                    // the list offering to boot something already running.
                    (true, true) => {
                        device.boot = None;
                        Some(device)
                    }

                    // Gone. Dropped rather than rewritten, because `targets()` puts it
                    // back as a bootable row from `simctl`/`-list-avds` with the
                    // spelling those tools use — an AVD comes back under its AVD name,
                    // not the serial it was running as.
                    (false, false) => None,

                    _ => Some(device),
                }
            })
            .collect();

        // **Rows can appear, not only vanish, and this half was missing.** The filter
        // above can confirm or drop what it was handed and nothing else, so a device
        // that came up after detection could only be noticed if one of the cached rows
        // already carried its id. iOS satisfies that — a simulator keeps one UDID —
        // and Android never does: the row said `Pixel_8` and the machine said
        // `emulator-5554`. So an emulator booted by the tab a `⇧⏎` spawned (8.4), or
        // by hand, stayed a row offering to boot it, ` active ` never appeared, and
        // `Enter` on it would have booted a second copy of something already up.
        for serial in &alive {
            if real.iter().any(|row| row.id == *serial) {
                continue;
            }

            if let Some(device) = probe::adopted(serial) {
                adopt(&mut real, device);
            }
        }

        let _ = tx.send(Msg::Devices(Ok(probe::targets(real, &last))));
    });
}

/// Put an adopted device where its bootable row was.
///
/// In place, not appended: the row under the cursor is the one changing identity, and
/// pushing the new one to the end would leave the old `Pixel 8` sitting above a second
/// `Pixel 8` until `targets` deduplicated by name — which it only does for rows it
/// adds itself, so the pair would have survived the merge.
fn adopt(rows: &mut Vec<probe::Device>, device: probe::Device) {
    match rows
        .iter()
        .position(|row| row.name == device.name && row.boot.is_some())
    {
        Some(index) => rows[index] = device,
        None => rows.push(device),
    }
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
    /// When the device list was last checked against the machine.
    rechecked: Instant,
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

    // The same reason, and so the same slot: this query also writes a sequence and
    // reads the reply off stdin. Raw mode is not the constraint — crossterm enables
    // it for the query itself when it is off — an uncontended stdin is, which is what
    // `FRUN_NO_QUERY` exists for (see `Logo::detect`).
    //
    // The answer only decides whether `⇧⏎` is advertised (8.4). Under `FRUN_NO_QUERY`
    // the hint is withheld and the key still works if the terminal happens to support
    // it, which is the safe way round: a key that works unannounced costs nothing, and
    // one announced but unreachable is the `[COPY]` failure of 3.1.
    app.shift_enter = std::env::var_os("FRUN_NO_QUERY").is_none()
        && supports_keyboard_enhancement().unwrap_or(false);

    let mut ctx = Ctx {
        rx,
        tx,
        session: None,
        extra,
        // Detection has just been fired and its answer is the first list, so the
        // clock starts here rather than at the epoch.
        rechecked: Instant::now(),
        done: false,
    };

    enable_raw_mode()?;

    // Mouse capture is NOT enabled here. Capturing takes text selection away
    // from the terminal, and copying a stack trace out of the log window is a
    // large part of what that window is for. `m` turns it on when the scroll
    // wheel or a clickable control is wanted.
    execute!(io::stdout(), EnterAlternateScreen)?;

    // `⇧⏎` (8.4). Without this, `Enter`, `⇧Enter` and `^Enter` are all CR on the
    // wire and crossterm reports `KeyCode::Enter` with an empty modifier set, so the
    // arm in `key_press` could never fire.
    //
    // **After the alternate screen, and that ordering is the whole bug this comment
    // exists for.** The Kitty protocol keeps a separate flag stack per screen, so a
    // push on the main screen is not in effect on the alternate one. Pushed first,
    // measured: `⇧⏎` produced no event in frun at all, while the same push in a
    // program that never leaves the main screen reported `Enter + SHIFT` correctly.
    // The pop is paired with it and runs *before* `LeaveAlternateScreen` for the same
    // reason.
    //
    // Pushed whether or not the query above said yes: a terminal that does not
    // understand the sequence ignores it, and asking again here would be a second
    // read from a stdin the input loop now owns. Only `DISAMBIGUATE_ESCAPE_CODES` —
    // event types are deliberately not requested, so no release or repeat events
    // start arriving at a loop that filters for `Press`.
    //
    // It changes what frun *reads* and nothing it writes: the bytes forwarded to
    // Flutter are ones frun composes itself (5.1).
    execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;

    // Before the first frame: a tab spawned by `⇧⏎` starts its build now, not after a
    // discovery it does not need (8.4). Discovery is already running behind this.
    handed_over(&mut app, &mut ctx);

    let result = event_loop(&mut app, &mut ctx, &mut art);

    // Restore before propagating any error, so a failure inside the loop cannot
    // leave the terminal in raw mode or holding the mouse.
    disable_raw_mode()?;

    // Paired with the push above. Left on, the shell inherits a keyboard mode it
    // never asked for, and `⏎` at the prompt would arrive as an escape sequence.
    execute!(io::stdout(), PopKeyboardEnhancementFlags)?;

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

        // Whichever list is up, keep it true. Nothing else on screen makes a claim
        // about the world that the world can invalidate on its own.
        if app.live
            && !app.refreshing
            && !app.devices.is_empty()
            && ctx.rechecked.elapsed() >= RECHECK
            && matches!(
                app.state,
                State::NoDevices | State::MultipleDevices | State::Switching
            )
        {
            ctx.rechecked = Instant::now();
            app.refreshing = true;

            recheck(app, ctx);
        }

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

        // Only the *first* answer decides the flow. Discovery runs again whenever
        // `^D` opens the switch list, and by then there is a run on screen: a
        // second answer that reset the state would drag the user out of their
        // session, and a second failure that set `fatal` would end it.
        Msg::Devices(Err(reason)) => {
            app.refreshing = false;

            if app.state == State::Detecting {
                app.fatal = Some(reason);
                ctx.done = true;
            } else {
                app.log(Level::Wrn, &format!("device refresh failed — {reason}"));
            }
        }

        Msg::Busy(ids) => app.busy = ids,

        Msg::Devices(Ok(targets)) => {
            app.refreshing = false;

            if app.state == State::Detecting {
                devices_answered(app, ctx, targets);
            } else {
                devices_refreshed(app, targets);
            }
        }

        Msg::Booted(Ok(booted)) => {
            app.boot_started = None;

            // Straight to the run. The device is already known, so the picker is
            // skipped: 3.3 is explicit that asking again would name the same
            // device a third time in a row.
            let device = booted_device(app, &booted);

            // The list is kept rather than emptied. `^D` reopens it without
            // re-running discovery, and nothing renders it outside the two picker
            // states, so holding it costs a vector and no rows.
            launch(app, ctx, device);
        }

        // Fatal only before the first run. Once a target exists this boot was a
        // retry or a switch, and ending the process over it would throw away a
        // session the user still has — the failure card and its log are the whole
        // reason they are looking at the screen.
        Msg::Booted(Err(reason)) => {
            app.boot_started = None;

            if app.target.is_none() {
                app.fatal = Some(format!("{} {reason}", app.boot_name));
                ctx.done = true;

                return;
            }

            app.log(
                Level::Err,
                &format!("{} did not start — {reason}", app.boot_name),
            );

            app.goto(State::BuildFailed);
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

/// The device this process was handed by the tab that spawned it (8.4).
///
/// **Read before the first frame, and acted on there.** The earlier version resolved
/// the handoff inside `devices_answered`, which meant the new tab sat on `DETECTING`
/// through its own `fvm flutter devices --machine` — six seconds — before starting a
/// build that never needed the answer. `flutter run -d <id>` resolves its own device,
/// and everything the frame shows in the meantime came over with the id.
///
/// Discovery still runs, in the background, because `^D` needs a list to open and
/// `devices_refreshed` is already built to swap rows under a live run. It also fills
/// in `Platform ID` and `OS Version`, which are the two fields deliberately left out
/// of the handoff.
///
/// A value that is not a full handoff is logged and ignored, and the picker comes up
/// as usual. Five fields or nothing: the platform decides whether a boot is even
/// possible and `virtual_device` decides whether frun may shut the device down, so a
/// guess there is worse than a keystroke.
fn handed_over(app: &mut App, ctx: &mut Ctx) {
    let Some(line) = std::env::var(HANDOFF).ok().filter(|l| !l.is_empty()) else {
        return;
    };

    match probe::Device::from_handoff(&line) {
        Some(device) => {
            // The handed row becomes this tab's list, of one, so everything downstream
            // sees the same shape a pick produces. `booted_device` reads the selected
            // row to recover the name and platform of an AVD whose serial it has just
            // learned, and with an empty list it would fall through to a bare shell
            // that hardcodes Android — an iOS simulator handed over would have come
            // back wearing the wrong glyph. It also gives `^D` something to open
            // before the background scan lands, and `devices_refreshed` replaces it
            // when that happens.
            app.devices = vec![device.clone()];
            app.selected_device = 0;

            start(app, ctx, device);
        }

        None => app.log(
            Level::Wrn,
            &format!("{HANDOFF} is not a device this frun can start"),
        ),
    }
}

/// A later answer from discovery: swap the rows, leave the frame alone.
///
/// This is what makes the chips on the switch list true. They are facts about a
/// device at the moment it was scanned — ` active ` means no boot is needed,
/// ` last used ` means it is the one in `.frun-last-device` — and the cache they
/// came from is a snapshot taken before the run started. A device that has stopped
/// since, including the one frun itself shut down on the way out, still read as
/// ready, and picking it failed the build.
///
/// The selection follows the device rather than the row number: the list is
/// reordered by what is running, so keeping the index would move the cursor to a
/// different device while the user was looking at it.
fn devices_refreshed(app: &mut App, targets: Vec<probe::Device>) {
    // Nothing answered. The run is untouched and the old rows stay: a stale list is
    // a better answer here than an empty frame, and `enter()` reports a dead device
    // loudly if one is picked.
    if targets.is_empty() {
        return;
    }

    let anchor = app
        .selected()
        .map(|device| (device.id.clone(), device.name.clone()));

    app.devices = targets;

    // By name when the id is gone, which is not a fallback for a rare case: an
    // emulator that boots between two rechecks changes the id of its own row from the
    // AVD name to a serial (`adopt`), so the row the user is looking at would have
    // dropped the cursor back to the top of the list at the moment it became runnable.
    app.selected_device = anchor
        .and_then(|(id, name)| {
            app.devices
                .iter()
                .position(|device| device.id == id)
                .or_else(|| app.devices.iter().position(|device| device.name == name))
        })
        .unwrap_or(0);

    app.scroll = 0;

    fill_target(app);
}

/// Fill the two target-card fields a handoff cannot carry (8.4).
///
/// `Platform ID` and `OS Version` are Flutter's `targetPlatform` and
/// `sdkNameAndVersion`, so they only exist once a scan has answered. A handed-over
/// device starts building before that, deliberately, and this is where the card stops
/// reading blank — from the scan that was running behind it the whole time.
///
/// Only ever fills a blank, which is the same rule `booted_device` follows and for the
/// same reason: a device frun booted describes itself better than a stale list does.
fn fill_target(app: &mut App) {
    let Some(target) = app.target.as_ref() else {
        return;
    };

    if !target.target_platform.is_empty() && !target.sdk.is_empty() {
        return;
    }

    let Some(row) = app.devices.iter().find(|d| d.id == target.id).cloned() else {
        return;
    };

    if let Some(target) = app.target.as_mut() {
        if target.target_platform.is_empty() {
            target.target_platform = row.target_platform;
        }

        if target.sdk.is_empty() {
            target.sdk = row.sdk;
        }
    }
}

/// A device that was booted but is not in any list yet.
///
/// Assembled from two sources, because neither knows everything. The picked row
/// has the name the user chose and, for a simulator, the runtime `simctl` filed
/// it under; the boot has the serial Flutter will address and, for Android, what
/// the running system says about itself. Discovery does not run again, so this is
/// the last chance to combine them.
fn booted_device(app: &App, booted: &probe::Booted) -> probe::Device {
    // Which row this boot belongs to, in falling order of certainty.
    //
    // The selected row is right for a first pick — an AVD boots as a serial that is
    // in no list yet — and wrong for a retry, where the boot is of the device already
    // running and the cursor may be sitting anywhere after a recheck reordered the
    // list. Taking the selection there would keep the correct id and inherit another
    // device's name and platform.
    let base = app
        .devices
        .iter()
        .find(|d| d.id == booted.id)
        .or_else(|| app.target.as_ref().filter(|d| d.id == booted.id))
        .or_else(|| app.selected());

    // Or a bare shell, if the list is somehow gone.
    let mut device = base.cloned().unwrap_or_else(|| probe::Device {
        id: booted.id.clone(),
        name: booted.id.clone(),
        platform: probe::Platform::Android,
        target_platform: String::new(),
        sdk: String::new(),
        virtual_device: true,
        last_used: false,
        boot: None,
    });

    // An AVD row is keyed by the AVD name, and Flutter cannot run on that.
    device.id = booted.id.clone();

    // It is running now, so there is nothing left to start. Leaving `boot` set
    // would make the run target look like a bootable row.
    device.boot = None;
    device.virtual_device = true;

    // Only ever fills a blank. An Android boot answers both fields and the row it
    // came from has neither; a simulator answers neither and the row it came from
    // has its runtime. Overwriting unconditionally would trade the second for an
    // empty string, which is the dash this change exists to remove.
    if !booted.target_platform.is_empty() {
        device.target_platform = booted.target_platform.clone();
    }

    if !booted.sdk.is_empty() {
        device.sdk = booted.sdk.clone();
    }

    device
}

/// Take a device as the target and start the build.
fn launch(app: &mut App, ctx: &mut Ctx, device: probe::Device) {
    app.choose(device);
    name_tab(app);
    spawn_session(app, ctx);
}

/// Name the terminal tab after what it is running, `<device> · <project>` (8.4).
///
/// Called from `launch` and nowhere else, which is enough because every path that
/// sets a target ends here: the first pick, a device that had to be booted, and a
/// switch. The one path that skips it is the one that changes nothing — `Enter` on
/// the row already running returns before `launch` — and the title it would write is
/// the title already on the tab.
///
/// `OSC 2`, so no AppleScript and no automation permission. Nothing restores it on
/// exit: the string it replaced came from the shell reporting its running command,
/// and the next prompt writes that again.
///
/// Failure is ignored deliberately. A terminal that will not take a title is not a
/// reason to interrupt a build, and there is no second way to ask.
fn name_tab(app: &App) {
    let Some(device) = app.target.as_ref() else {
        return;
    };

    let _ = execute!(
        io::stdout(),
        SetTitle(format!("{} · {}", device.name, app.project))
    );
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

    // `q`, and the only ending that is about frun rather than about the run. It has
    // to be checked before the two below, which both keep frun on screen.
    if app.ending == Some(Ending::Quit) {
        ctx.done = true;
        return;
    }

    // An ending frun knows about (8.8): `^S`, or Flutter's own detach. The child is
    // gone, frun is not — the log stays readable, the device stays booted, and `r`
    // builds again.
    if app.ending.is_some() {
        ctx.session = None;

        app.end_build();
        app.goto(State::Stopped);

        return;
    }

    // The run's state, not the screen's: with the switch list open the child is
    // still the run, and reading `state` here would treat its death as a process
    // with nothing left to do and exit without reporting anything.
    if app.run_state().build_done() {
        // A live session died and no key asked for it: the app was closed on the
        // device, it crashed, or the device went away. This used to be read as
        // Flutter shutting itself down — the graceful exit `q` asks for — so
        // switching an emulator off took frun with it and reported nothing.
        //
        // The same landing as `^S`, because the device is in the same condition:
        // the app is gone and `r` is the way back, which for a virtual target
        // boots it again first. Only the title differs.
        ctx.session = None;

        app.ending = Some(Ending::Lost);
        app.end_build();

        // frun's own line, not a reclassified one of Flutter's. `Lost connection to
        // device.` is what Flutter usually prints and it classifies as `INF`, but a
        // device that goes away abruptly closes the pty without printing anything —
        // so the one thing that is always true has to be the thing that is said.
        app.log(Level::Err, "lost connection to the device");

        app.goto(State::Stopped);

        return;
    }

    if app.run_state().has_build() {
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

        // The first frun key that is not a plain letter, and that is why it is
        // affordable: Flutter's interactive commands are all bare single bytes, so
        // a modifier takes nothing from 5.1. `s` was the obvious mnemonic and is
        // Flutter's screenshot key.
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(apply(app, ctx, Action::Switch));
        }

        // End the run without ending frun (8.8). A modifier for the same reason as
        // `^D`: bare `s` is Flutter's screenshot.
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(apply(app, ctx, Action::StopRun));
        }

        KeyCode::Char('q') => return Ok(apply(app, ctx, Action::Quit)),

        // Cancel, only where there is something to cancel. During a run `Esc` is
        // Flutter's, and quitting on it would be a surprise.
        KeyCode::Esc => {
            // The picker was opened over a run that is still alive, so this is a
            // return and not a cancel: nothing has been killed yet, and the state
            // restored is whatever Flutter reached while the list was up.
            if let Some(state) = app.resume.take() {
                app.goto(state);

                return Ok(false);
            }

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

        // Two verbs behind one letter, decided by whether there is a session to
        // reload. `Stopped` belongs with `BuildFailed` here: the footer advertised
        // `[r] Build again` and the key was still resolving to a hot reload, which
        // then declined itself because `build_done()` is false with no child. The
        // click on the same hint worked, which is exactly the drift `apply()` exists
        // to prevent.
        KeyCode::Char('r') => {
            let action = if matches!(app.state, State::BuildFailed | State::Stopped) {
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

        // 8.4: the same verb as `⏎`, aimed at a new terminal tab instead of this
        // one. Scoped to the three states that show a list, so everywhere else a
        // shifted `Enter` still falls through to `enter()` and reaches Flutter as CR
        // — which is what it did before the keyboard protocol made the modifier
        // visible at all.
        KeyCode::Enter
            if key.modifiers.contains(KeyModifiers::SHIFT)
                && matches!(
                    app.state,
                    State::NoDevices | State::MultipleDevices | State::Switching
                ) =>
        {
            return Ok(apply(app, ctx, Action::NewTab));
        }

        KeyCode::Enter => return Ok(enter(app, ctx)),

        // Number hotkeys, per DESIGN.md 3.3 mode 4. Only where a list is on
        // screen: everywhere else a digit is Flutter's, and it has to arrive
        // unchanged.
        KeyCode::Char(c @ '1'..='9')
            if matches!(
                app.state,
                State::NoDevices | State::MultipleDevices | State::Switching
            ) =>
        {
            let index = c as usize - '1' as usize;

            if index < app.devices.len() {
                app.selected_device = index;
                return Ok(enter(app, ctx));
            }
        }

        // Flutter's detach, forwarded like any other key of its own — and recorded on
        // the way past.
        //
        // Not an interception: `d` still reaches Flutter and still detaches. What frun
        // gains is knowing *why* the pty is about to close. Without it, detach and
        // Flutter quitting are the same event, so `D` read as a graceful shutdown and
        // took frun with it while the app carried on running on the device.
        //
        // Only once the interactive session is live, because that is the only time
        // Flutter can read the key. Setting it during a build would leave a flag that
        // outlives the keypress and turn the next crash into a false detach.
        KeyCode::Char(c @ ('d' | 'D')) if app.state.build_done() => {
            app.ending = Some(Ending::Detached);

            let mut buf = [0u8; 4];
            forward(ctx, c.encode_utf8(&mut buf).as_bytes());
        }

        // Every key not claimed above is Flutter's, per DESIGN.md 5.1: `h` help,
        // `c` clear, `p` debug paint, `o` platform toggle, `w` widget tree, and more.
        // Intercepting them would silently remove functionality that works today.
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
    if !matches!(
        app.state,
        State::NoDevices | State::MultipleDevices | State::Switching
    ) {
        forward(ctx, b"\r");
        return false;
    }

    let Some(device) = app.selected().cloned() else {
        return false;
    };

    if refuse_busy(app, &device) {
        return false;
    }

    // Switching to the device that is already running is a return, not a rebuild.
    // The row for the current target is the highlighted one when the list opens, so a
    // reflexive `Enter` lands on it, and the label promises another device — not forty
    // seconds of Gradle to arrive back where it started.
    //
    // Only while something is actually running. Opened from `Stopped` there is no
    // session to keep, so the same row means "start this one again", and returning
    // silently was a dead end: the frame came back with nothing rebuilt.
    if let Some(state) = app.resume {
        let same = app.target.as_ref().is_some_and(|t| t.id == device.id);

        if same && ctx.session.is_some() {
            app.resume = None;
            app.goto(state);

            return false;
        }
    }

    // The pick is committed from here on. The banked state goes with it, or the
    // transitions below would be banked too and never reach the screen.
    let switching = app.resume.take().is_some();

    // The outgoing child goes now rather than at the respawn. A boot can take
    // three minutes, and an old app still running on a device that has already
    // been replaced is a second run nobody asked for.
    stop_session(ctx);

    if switching {
        release_target(app);
    }

    start(app, ctx, device);

    false
}

/// Refuse a device another run already holds, and say so (8.4).
///
/// **One guard, both verbs.** `⏎` and `⇧⏎` are the two ways a row becomes a run, here
/// and in another tab, and a device can only carry one: the second `flutter run`
/// reinstalls over the first and takes its VM service down, so the tab that was
/// working is the one that breaks. Guarding only the key that happened to be reported
/// would have left the other spelling of the same mistake open.
///
/// Logged rather than silent, because a key that does nothing reads as a broken key.
/// The chip on the row already says which device it is; this says the press was heard
/// and refused.
fn refuse_busy(app: &mut App, device: &probe::Device) -> bool {
    if !app.in_use(&device.id) {
        return false;
    }

    app.log(
        Level::Wrn,
        &format!(
            "{} is in use by another run — pick another device",
            device.name
        ),
    );

    true
}

/// Take a device from wherever it was chosen and get it running.
///
/// One list, so one branch: either the target has to be started first or it is ready
/// to run. Which frame the row was on does not matter, and neither does whether a
/// person picked it here or another tab handed it over (8.4) — which is the second
/// caller this exists for.
fn start(app: &mut App, ctx: &mut Ctx, device: probe::Device) {
    match device.boot.clone() {
        None => launch(app, ctx, device),

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
}

fn forward(ctx: &mut Ctx, bytes: &[u8]) {
    if let Some(session) = &mut ctx.session {
        session.send(bytes);
    }
}

// ============================================================
// New tab (8.4)
// ============================================================

/// The variable the new process is handed its device in.
///
/// An environment variable and not a flag: `FRUN_FLAGS` is a closed list and every
/// other `--name` belongs to Flutter (5.1), so `--device` would both collide with
/// Flutter's own `--device-id` and open a hole in that rule. `FRUN_NO_QUERY` is the
/// precedent, and an env var rides Ghostty's surface configuration and `tmux -e`
/// without any quoting.
const HANDOFF: &str = "FRUN_DEVICE";

/// Open a terminal tab running a second frun on `device`.
///
/// Blocking, and that is a deliberate trade. The alternative is a thread, which
/// cannot report back without a new `Msg` variant, and the whole point of this shape
/// is that `Msg` is untouched. `tmux` answers in milliseconds; `osascript` does too,
/// except on the first call of a session, where macOS may put its automation-consent
/// dialog up first. The frame is frozen while that dialog is open, and the user is
/// looking at the dialog.
fn new_tab(device: &probe::Device, extra: &[String]) -> Result<(), String> {
    // The binary, not `frun`: that name is a zsh function from `.zshrc` and
    // `~/.cargo/bin` is deliberately off `PATH` (see `frun.zsh`), so it does not
    // exist for anything but that shell. `current_exe` also survives the crate being
    // moved or renamed.
    let exe = std::env::current_exe().map_err(|why| format!("cannot find frun — {why}"))?;
    let cwd = std::env::current_dir().map_err(|why| format!("cannot read cwd — {why}"))?;

    let (program, args) = tab_command(
        &exe.to_string_lossy(),
        &cwd.to_string_lossy(),
        &handoff_env(device),
        extra,
        std::env::var_os("TMUX").is_some(),
    );

    let out = std::process::Command::new(&program)
        .args(&args)
        .output()
        .map_err(|why| format!("{program} — {why}"))?;

    if out.status.success() {
        return Ok(());
    }

    // The shortest decisive line. AppleScript errors are one line and useful
    // (`Application isn't running`, `Not authorized to send Apple events`); a wall of
    // them in a log window is not.
    let stderr = String::from_utf8_lossy(&out.stderr);

    Err(stderr
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("no reason given")
        .to_string())
}

/// What the new process is given beyond its arguments.
///
/// **`PATH` is not optional here, and finding that out cost a measurement.** A
/// Ghostty surface created with a `command` runs that command directly instead of a
/// shell, so no `.zshrc` runs and the environment is the one the app itself was
/// launched with: measured on this machine, `PATH` is
/// `/usr/bin:/bin:/usr/sbin:/sbin:/Applications/Ghostty.app/Contents/MacOS` — where
/// `fvm`, `flutter`, `adb` and `emulator` are all invisible. So the new tab would
/// have failed on its first command with `fvm` not found, and it would have looked
/// like frun's bug rather than a missing variable. This process's `PATH` is the one
/// that works, because it came from the shell that started it.
///
/// `FVM_CACHE_PATH` for the same reason and only when set: `probe::fvm_cache` honours
/// it, so a non-default cache would otherwise resolve differently in the two tabs.
/// Nothing else is copied. This is a handoff, not a session transfer.
fn handoff_env(device: &probe::Device) -> Vec<String> {
    let mut env = vec![format!("{HANDOFF}={}", device.to_handoff())];

    for name in ["PATH", "FVM_CACHE_PATH"] {
        if let Some(value) = std::env::var_os(name) {
            env.push(format!("{name}={}", value.to_string_lossy()));
        }
    }

    env
}

/// What to run, split out so it can be asserted without a terminal.
///
/// Two mechanisms because there is no portable one. Inside tmux the multiplexer owns
/// the tabs, so asking the terminal would open a tab tmux does not know about;
/// outside it, Ghostty's AppleScript dictionary is the only way in, and it is also
/// the one that can set the working directory, the command and the environment in a
/// single call.
fn tab_command(
    exe: &str,
    cwd: &str,
    env: &[String],
    extra: &[String],
    tmux: bool,
) -> (String, Vec<String>) {
    if tmux {
        let mut args = vec!["new-window".to_string(), "-c".to_string(), cwd.to_string()];

        for entry in env {
            args.extend(["-e".to_string(), entry.clone()]);
        }

        args.push(exe.to_string());
        args.extend(extra.iter().cloned());

        return ("tmux".to_string(), args);
    }

    // Values travel as `argv` rather than being interpolated into the script, so a
    // device id or a path can contain anything without breaking the quoting.
    let script = [
        "on run argv",
        "tell application \"Ghostty\"",
        "set cfg to new surface configuration",
        "set initial working directory of cfg to item 1 of argv",
        "set command of cfg to item 2 of argv",
        // Everything from the third argument on is one `KEY=VALUE`, so the count can
        // grow without the script changing.
        "set envs to {}",
        "repeat with i from 3 to count of argv",
        "set end of envs to item i of argv",
        "end repeat",
        "set environment variables of cfg to envs",
        // So the tab survives frun exiting and its transcript can still be read.
        "set wait after command of cfg to true",
        "new tab in front window with configuration cfg",
        "end tell",
        "end run",
    ];

    let mut args: Vec<String> = script
        .iter()
        .flat_map(|line| ["-e".to_string(), line.to_string()])
        .collect();

    // `command` is one string that Ghostty hands to a shell, so this half does need
    // quoting — and it is the only half that does.
    let command = std::iter::once(exe.to_string())
        .chain(extra.iter().cloned())
        .map(|part| format!("'{}'", part.replace('\'', r"'\''")))
        .collect::<Vec<_>>()
        .join(" ");

    args.extend([cwd.to_string(), command]);
    args.extend(env.iter().cloned());

    ("osascript".to_string(), args)
}

/// Shut the outgoing device down when the run moves off it.
///
/// Off the main thread: `simctl shutdown` and `adb emu kill` both take about a
/// second, and the frame after this one is the new build starting. Nothing is
/// reported back — there is no answer worth waiting for and nothing the user could
/// do with one.
///
/// Every virtual device, not only the ones frun booted. The ownership test was the
/// first rule here and it silently did nothing in the commonest case: `boot_avd`
/// starts the emulator under `nohup` precisely so it outlives frun, so on the next
/// run the emulator is already attached, nothing was booted, and switching away left
/// it running. One rule that always holds beats a rule that holds only in the
/// session that started the device.
///
/// Physical devices are untouched — there is nothing here that could — and so are
/// macOS and Chrome, where the nearest equivalent is closing the user's browser.
fn release_target(app: &App) {
    let Some(device) = app.target.as_ref().filter(|d| d.virtual_device) else {
        return;
    };

    let id = device.id.clone();
    let platform = device.platform;

    std::thread::spawn(move || probe::shutdown(&id, platform));
}

/// Kill and reap the current child, if there is one.
///
/// One place, because two callers want it for the same reason: a retry and a
/// device switch both respawn, and a respawn racing an unreaped child is how two
/// Gradle daemons end up fighting over a lock.
fn stop_session(ctx: &mut Ctx) {
    if let Some(session) = &mut ctx.session {
        session.kill();
    }

    ctx.session = None;
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
        // Retry is not only a respawn any more: the device is often *why* the build
        // failed. Shut a simulator down mid-build and Flutter reports `No supported
        // devices found with name or id matching '<udid>'`, and respawning
        // `flutter run -d <udid>` fails identically for the same reason, as often as
        // it is pressed. So a virtual target is brought back up first.
        Action::RetryBuild => {
            stop_session(ctx);

            let Some(boot) = app.target.as_ref().and_then(probe::boot_target) else {
                // A physical device, macOS or Chrome. Nothing frun can start, so the
                // respawn is the whole of the retry.
                spawn_session(app, ctx);

                return false;
            };

            let id = app.target.as_ref().map(|d| d.id.clone()).unwrap_or_default();

            app.boot_name = app
                .target
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_default();

            app.boot_started = Some(std::time::Instant::now());
            app.goto(State::Booting);

            let tx = ctx.tx.clone();

            std::thread::spawn(move || {
                // Liveness first, and it decides whether anything is booted at all.
                // `simctl bootstatus -b` is idempotent, but `boot_avd` is not: it
                // spawns `emulator -avd`, so calling it on a device that is already
                // running leaves two.
                let msg = if probe::alive().contains(&id) {
                    Ok(probe::Booted::bare(id))
                } else {
                    probe::boot(&boot)
                };

                let _ = tx.send(Msg::Booted(msg));
            });

            false
        }

        // Reopen the picker over the live run, DESIGN.md 8.5. Nothing is killed
        // here: the child keeps running and keeps streaming while the list is up,
        // which is what makes `Esc` a free return rather than a lost session.
        //
        // Only where there is a run to move. Before one, the picker is the screen
        // already, and `^D` there would reopen what is open.
        Action::Switch => {
            // Nothing to move before a run, nowhere to move it with an empty cache,
            // and nothing to do when the list is already up.
            if !app.state.has_build() || app.devices.is_empty() {
                return false;
            }

            if app.state == State::Switching {
                return false;
            }

            app.resume = Some(app.state);

            // Assigned rather than `goto`: `resume` is now set, and `goto` banks
            // every transition while it is.
            app.state = State::Switching;

            // The cached rows go up immediately and are rechecked behind them.
            app.refreshing = true;
            recheck(app, ctx);

            false
        }

        // 8.4: a second frun, in a new terminal tab, on the highlighted row.
        //
        // This tab is untouched. Nothing is killed, no state changes, and the list
        // stays open — the pick was about the other tab, so closing the list here
        // would answer a question nobody asked, and leaving it open turns one scan
        // into a dispatcher for several devices.
        Action::NewTab => {
            let Some(device) = app.selected().cloned() else {
                return false;
            };

            if refuse_busy(app, &device) {
                return false;
            }

            match new_tab(&device, &ctx.extra) {
                Ok(()) => app.log(Level::Inf, &format!("new tab — {}", device.name)),

                // Reported, never fatal. A tab that did not open must not take a
                // live run with it.
                Err(reason) => app.log(Level::Err, &format!("new tab failed — {reason}")),
            }

            false
        }

        // Graceful, and deliberately the same mechanism as `q`: Flutter is asked to
        // shut itself down and frun waits for the pty to close. The only difference
        // is where it lands afterwards, which is what `app.stopping` records.
        //
        // No escalation ladder behind this. If Flutter is wedged and ignores the
        // request, `^C` still ends the process — that is what keeping `^C` as force
        // stop buys, and a timeout that killed the child would be a second answer to
        // a question the user already has a key for.
        Action::StopRun => {
            if ctx.session.is_none() || !app.state.has_build() {
                return false;
            }

            app.ending = Some(Ending::Stopped);

            // Before the interactive session opens there is no `q` to read: a build
            // is Gradle or Xcode, and only a signal reaches it.
            if app.state.build_done() {
                forward(ctx, b"q");
            } else if let Some(session) = &mut ctx.session {
                session.interrupt();
            }

            false
        }

        Action::StartDevice => enter(app, ctx),

        // Graceful: Flutter receives the key and shuts itself down (⏏). The loop
        // keeps running until the child closes the pty, so the exit stays
        // Flutter's to make.
        Action::Quit => {
            if ctx.session.is_some() && app.state.build_done() {
                // Recorded before the key goes, because the pty closing is all
                // `child_exited` will have to go on and every other way a live
                // session can end now keeps frun on screen. Without this the exit
                // asked for here would land on the `DISCONNECTED` frame instead.
                app.ending = Some(Ending::Quit);

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

    /// 8.4: what the new tab is told, in both mechanisms.
    ///
    /// The spawn itself cannot be reached from here — it needs a real terminal — so
    /// this asserts the two things that would break silently: the device has to
    /// travel in `FRUN_DEVICE`, and the flags this run was given have to travel with
    /// it, or the second tab quietly builds a different flavour.
    /// 8.4: what the new tab is told, in both mechanisms.
    ///
    /// The spawn itself cannot be reached from here — it needs a real terminal — so
    /// this asserts the three things that would break silently: the device has to
    /// travel in `FRUN_DEVICE`, the flags this run was given have to travel with it
    /// or the second tab quietly builds a different flavour, and `PATH` has to travel
    /// or the second tab cannot find `fvm` at all.
    #[test]
    fn a_new_tab_is_handed_the_device_the_flags_and_a_usable_path() {
        let extra = vec!["--flavor".to_string(), "staging".to_string()];
        let env = vec![
            "FRUN_DEVICE=emulator-5554".to_string(),
            "PATH=/opt/homebrew/bin".to_string(),
        ];

        let (program, args) = tab_command("/bin/frun", "/p", &env, &extra, true);

        assert_eq!(program, "tmux");
        assert_eq!(
            args,
            [
                "new-window",
                "-c",
                "/p",
                "-e",
                "FRUN_DEVICE=emulator-5554",
                "-e",
                "PATH=/opt/homebrew/bin",
                "/bin/frun",
                "--flavor",
                "staging",
            ]
        );

        let (program, args) = tab_command("/bin/frun", "/p", &env, &extra, false);

        assert_eq!(program, "osascript");

        // Positional, after the script: everything the AppleScript reads out of
        // `argv`, in the order it reads it.
        assert_eq!(
            args[args.len() - 4..],
            [
                "/p".to_string(),
                "'/bin/frun' '--flavor' 'staging'".to_string(),
                "FRUN_DEVICE=emulator-5554".to_string(),
                "PATH=/opt/homebrew/bin".to_string(),
            ]
        );

        assert!(
            args.iter()
                .any(|arg| arg == "new tab in front window with configuration cfg"),
            "the script must actually open a tab: {args:?}"
        );
    }

    /// `PATH` is the one variable the new tab cannot do without: a Ghostty surface
    /// started with a command runs no shell, so nothing else would set it.
    #[test]
    fn the_handoff_carries_this_processs_path() {
        let device = probe::Device {
            id: "emulator-5554".into(),
            name: "Pixel 10 Pro XL".into(),
            platform: probe::Platform::Android,
            target_platform: "android-arm64".into(),
            sdk: "Android 17 (API 37)".into(),
            virtual_device: true,
            last_used: false,
            boot: None,
        };

        let env = handoff_env(&device);

        assert!(
            env[0].starts_with("FRUN_DEVICE=emulator-5554\t"),
            "the device leads the handoff: {:?}",
            env[0]
        );

        assert!(
            env.iter().any(|entry| entry.starts_with("PATH=")),
            "PATH must be handed over: {env:?}"
        );
    }

    /// An emulator that comes up takes the place of the row that offered to boot it.
    ///
    /// The identity of the row changes here — `Pixel_8` becomes `emulator-5554` — which
    /// is why this cannot be an append. Two rows called `Pixel 8`, one claiming to be
    /// bootable, is what the user would have been asked to choose between, and
    /// `targets` deduplicates only the rows it adds itself, so the pair would have
    /// survived the merge.
    #[test]
    fn an_adopted_emulator_replaces_the_row_that_offered_to_boot_it() {
        let bootable = |id: &str, name: &str| probe::Device {
            id: id.into(),
            name: name.into(),
            platform: probe::Platform::Android,
            target_platform: String::new(),
            sdk: String::new(),
            virtual_device: true,
            last_used: false,
            boot: Some(probe::Boot::Avd(id.into())),
        };

        let mut rows = vec![
            bootable("Pixel_10_Pro_XL", "Pixel 10 Pro XL"),
            bootable("Pixel_8", "Pixel 8"),
        ];

        let mut up = bootable("emulator-5554", "Pixel 8");
        up.boot = None;

        adopt(&mut rows, up);

        assert_eq!(rows.len(), 2, "the AVD row is replaced, not joined");

        // In place: the cursor is on the row the user was looking at, and this is the
        // moment it becomes runnable.
        assert_eq!(rows[1].id, "emulator-5554");
        assert!(rows[1].boot.is_none(), "an emulator that is up needs no boot");
        assert_eq!(rows[0].id, "Pixel_10_Pro_XL", "the other AVD is untouched");

        // Nothing to replace: a device with no bootable row of its own is still news.
        let mut fresh = bootable("emulator-5556", "Pixel 9");
        fresh.boot = None;

        adopt(&mut rows, fresh);

        assert_eq!(rows.len(), 3);
    }

    /// A single quote in a path or a flag must not end the quoting.
    #[test]
    fn a_quote_in_an_argument_cannot_break_out() {
        let extra = vec!["--dart-define=NAME=o'brien".to_string()];
        let env = vec!["FRUN_DEVICE=x".to_string()];
        let (_, args) = tab_command("/bin/frun", "/p", &env, &extra, false);

        let command = &args[args.len() - 2];

        assert_eq!(
            command, r"'/bin/frun' '--dart-define=NAME=o'\''brien'",
            "quoting escaped: {command}"
        );
    }

    /// A live session can end four ways, and one branch used to serve all of them.
    ///
    /// `q` is the regression this exists for. It forwards the key and keeps the loop
    /// running, so it leaves through the same branch a dying device now lands on:
    /// pointing that branch at `Stopped` without recording the key first would have
    /// left `q` unable to quit.
    #[test]
    fn a_live_session_dying_only_ends_frun_when_a_key_asked_for_it() {
        fn eof(ending: Option<Ending>) -> (App, bool) {
            let (tx, rx) = std::sync::mpsc::channel();

            let mut ctx = Ctx {
                rx,
                tx,
                session: None,
                extra: Vec::new(),
                rechecked: Instant::now(),
                done: false,
            };

            let mut app = App::new(State::Running);
            app.ending = ending;

            child_exited(&mut app, &mut ctx);

            (app, ctx.done)
        }

        // Nobody asked: the app was closed, or the device went away.
        let (app, done) = eof(None);

        assert!(!done, "a device going away must not take frun with it");
        assert_eq!(app.state, State::Stopped);
        assert_eq!(app.ending, Some(Ending::Lost));
        assert_eq!(
            app.logs.last().map(|line| line.level),
            Some(Level::Err),
            "the reason has to be in the log, which is what survives in the transcript"
        );

        // `q`: the one ending that is about frun rather than about the run.
        let (_, done) = eof(Some(Ending::Quit));
        assert!(done, "q asked to leave");

        // `^S` and `d`/`D` keep the frame, as they did before.
        for ending in [Ending::Stopped, Ending::Detached] {
            let (app, done) = eof(Some(ending));

            assert!(!done, "{ending:?} ends the run, not frun");
            assert_eq!(app.state, State::Stopped);
            assert_eq!(app.ending, Some(ending), "the frame has to name which");
        }
    }
}
