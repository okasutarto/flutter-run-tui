//! The pty session and the Flutter output parser.
//!
//! DESIGN.md 7.3 item 3, ported from `frun-runner` rather than reused as a
//! subprocess: the runner's job was to *print* progress, and this one's is to
//! keep state that a renderer reads. Every marker string and every scar comment
//! below came from there.
//!
//! Two things the port deliberately drops:
//!
//! * `vte`, which DESIGN.md names. Emulating a terminal is more code than
//!   replaying backspaces, and there is no cursor addressing to honour: the UI
//!   redraws itself from state, so all that is wanted from the byte stream is
//!   its text.
//! * `FRUN_LOG_DELAY`, the five-second hold before app logs were released. It
//!   existed because the shell printed the build summary and the log stream to
//!   the same scrolling region, so a chatty app scrolled the summary away. The
//!   tracker and the log window are separate regions here, so there is nothing
//!   to protect them from.

use std::io::{Read, Write};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::data::{App, Level, Msg, Stage, StageKey, State};

/// How long Flutter gets to acknowledge an `r`/`R` before the key is presumed
/// dropped.
///
/// A keypress is a request, not a fact. Flutter discards terminal input while it
/// is busy with a previous command and says so only through a `printTrace()`
/// that never reaches stdout, and `R` returns false silently when the run mode
/// cannot hot restart. So the only honest signal is Flutter's own `Performing
/// hot reload...` progress message, and its absence needs a deadline or the
/// spinner runs forever.
///
/// Generous, because `runSourceGenerators()` goes first and a false timeout is
/// self-correcting: a late ack simply reopens the stage.
pub const ACK_TIMEOUT: Duration = Duration::from_secs(4);

// ============================================================
// Session
// ============================================================

/// A running `fvm flutter run`.
pub struct Session {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,

    /// Held for the lifetime of the session and otherwise unused.
    ///
    /// Dropping the master closes the pty, which hangs up the child's terminal
    /// and kills it. Letting this fall out of scope at the end of `spawn` is a
    /// Flutter that dies the moment it starts.
    _master: Box<dyn MasterPty + Send>,
}

// `command_line` used to live here, building `fvm flutter run -d <id>` as a string
// for the SelectedTargetCard to print. The card dropped that row (3.2) and nothing
// else read it: `Session::spawn` assembles its own argv, so the string was never
// the command, only a description of one.

impl Session {
    /// Spawn Flutter on a pty and start pumping its output into `tx`.
    ///
    /// A pty and not a pipe: `flutter run` only opens its interactive session on
    /// a tty, so on a pipe there is no hot reload to drive at all.
    pub fn spawn(device: &str, extra: &[String], tx: Sender<Msg>) -> Result<Self, String> {
        let pty = native_pty_system();

        // The size Flutter is told about is not the terminal's. Its own line
        // wrapping is undone by this module anyway, and the log window rewraps
        // to whatever columns it has, so a stable width keeps Flutter's
        // progress lines predictable across a resize.
        let pair = pty
            .openpty(PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("could not open a pty: {e}"))?;

        let mut cmd = CommandBuilder::new("fvm");
        cmd.args(["flutter", "run", "-d", device]);
        cmd.args(extra);

        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("could not start fvm flutter run: {e}"))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("could not read from the pty: {e}"))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("could not write to the pty: {e}"))?;

        // The slave handle has to go, or the reader never sees EOF: this process
        // would still be holding the other end of the child's terminal open.
        drop(pair.slave);

        std::thread::spawn(move || {
            pump(&mut reader, &tx);
            let _ = tx.send(Msg::Eof);
        });

        Ok(Self {
            child,
            writer,
            _master: pair.master,
        })
    }

    /// Forward bytes verbatim, per DESIGN.md 5.1.
    ///
    /// Flutter owns `h`, `d`, `c`, `p`, `o`, `w` and more; intercepting them
    /// would silently remove functionality that works today.
    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// `^C`: SIGINT, forwarded rather than simulated.
    ///
    /// Writing ETX into the pty is what a real terminal does — the line
    /// discipline raises SIGINT on the child's foreground process group — which
    /// is why this needs no pid handling and no libc.
    pub fn interrupt(&mut self) {
        self.send(&[0x03]);
    }

    /// Kill and reap, for `[r] Retry Build`.
    ///
    /// Reaping matters: DESIGN.md 3.4 is explicit that retry is a fresh spawn
    /// and not a keypress forwarded to Flutter, and a respawn racing an
    /// unreaped child is how two Gradle daemons end up fighting over a lock.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Exit status, once the child has finished. `None` while it still runs.
    pub fn exit_code(&mut self) -> Option<i32> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.exit_code() as i32),
            _ => None,
        }
    }
}

/// Read the pty until EOF, emitting one message per line.
///
/// Bytes are buffered rather than decoded per chunk: a 4 KiB read can split a
/// multi-byte character, and lossy-decoding each chunk would turn the braille
/// spinner frames into replacement characters at random.
fn pump(reader: &mut Box<dyn Read + Send>, tx: &Sender<Msg>) {
    let mut buf = [0u8; 4096];
    let mut pending: Vec<u8> = Vec::new();

    loop {
        let read = match reader.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };

        // CR is a line break here, not a cursor move. Flutter animates progress
        // lines in place with CR, and each redraw carries a longer elapsed time,
        // so treating them as separate lines is what lets the last one win.
        for byte in &buf[..read] {
            match byte {
                b'\r' | b'\n' => {
                    if !pending.is_empty() {
                        let line = String::from_utf8_lossy(&pending).into_owned();
                        pending.clear();

                        if tx.send(Msg::Line(line)).is_err() {
                            return;
                        }
                    }
                }

                _ => pending.push(*byte),
            }
        }

        // Flutter's progress message is written without a newline and stays open
        // for the whole operation, so the ack for an r/R lives in this tail. On
        // a quiet app it is the only thing standing between a dropped key and a
        // spinner that never resolves.
        if !pending.is_empty() {
            let tail = String::from_utf8_lossy(&pending).into_owned();

            if tx.send(Msg::Partial(tail)).is_err() {
                return;
            }
        }
    }
}

// ============================================================
// Cleaning
// ============================================================

/// Strip escape sequences and replay backspaces.
pub fn clean(raw: &str) -> String {
    let text = strip_ansi(raw);
    let text = apply_backspaces(&text);

    // A progress line interrupted by a log line leaves its last animation frame
    // glued to the front of that line, and a status line flushed mid-animation
    // keeps one at the end.
    text.trim_matches(|c: char| is_braille(c) || c == ' ' || c == '\t')
        .to_string()
}

fn is_braille(c: char) -> bool {
    ('\u{2800}'..='\u{28FF}').contains(&c)
}

/// Remove CSI, OSC and two-character escape sequences.
///
/// Hand-rolled rather than a regex: the grammar is "ESC, then one of three
/// shapes", which is a shorter `match` than the pattern describing it would be.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }

        match chars.next() {
            // CSI: parameters, then a final byte in @..~
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }

            // OSC: terminated by BEL or ST.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }

                    if c == '\x1b' {
                        chars.next();
                        break;
                    }
                }
            }

            // Two-character sequence; the second character is already consumed.
            _ => {}
        }
    }

    out
}

/// Replay `\b` destructively.
///
/// Flutter animates a progress line with `\b` + next frame, and erases with
/// `\b`*n + ' '*n + `\b`*n (see `AnonymousSpinnerStatus`). Those bytes arrive
/// verbatim, so without replaying them the raw frames end up in the log stream.
fn apply_backspaces(text: &str) -> String {
    if !text.contains('\u{8}') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());

    for c in text.chars() {
        if c == '\u{8}' {
            out.pop();
            continue;
        }

        out.push(c);
    }

    out
}

/// Whether a cleaned line carries no information.
fn is_artifact(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }

    // Nothing but animation frames.
    if text.chars().all(|c| is_braille(c) || c.is_whitespace()) {
        return true;
    }

    // A short fragment left behind by a CR-based redraw.
    text.chars().count() <= 8 && !text.chars().any(char::is_alphanumeric)
}

/// Flutter's own interactive help, which the footer already carries.
fn is_help(text: &str) -> bool {
    const HEADS: [&str; 6] = [
        "r Hot reload",
        "R Hot restart",
        "h List all",
        "d Detach",
        "c Clear",
        "q Quit",
    ];

    HEADS.iter().any(|head| text.starts_with(head))
}

// ============================================================
// Durations
// ============================================================

/// First duration token in `text`, as Flutter wrote it.
///
/// Flutter formats elapsed milliseconds through `NumberFormat`, so the value
/// carries a group separator: `Restarted application in 1,234ms.` Matching only
/// digits used to clip that to `234ms`, which is why the separator is part of
/// the token here.
pub fn duration(text: &str) -> String {
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        let start = i;

        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == ',' || bytes[i] == '.') {
            i += 1;
        }

        // Trailing separators are punctuation, not part of the number.
        let mut end = i;

        while end > start && !bytes[end - 1].is_ascii_digit() {
            end -= 1;
        }

        let unit: String = bytes[i..].iter().take(2).collect();

        if unit.starts_with("ms") {
            return format!("{}ms", bytes[start..end].iter().collect::<String>());
        }

        if unit.starts_with('s') && !unit.starts_with("st") {
            return format!("{}s", bytes[start..end].iter().collect::<String>());
        }
    }

    String::new()
}

/// The Gradle task name out of `Running Gradle task 'assembleDebug'...`.
fn gradle_task(text: &str) -> String {
    text.split('\'').nth(1).unwrap_or_default().to_string()
}

/// Elapsed time as the tracker shows it.
///
/// Milliseconds below a second, matching how Flutter reports its own timings.
/// `Syncing files` really does finish in tens of milliseconds, and rendering
/// that as `0.0s` reads as a stage that was skipped or a clock that is broken.
pub fn elapsed(d: Duration) -> String {
    let secs = d.as_secs_f64();

    if secs < 1.0 {
        return format!("{}ms", d.as_millis());
    }

    if secs < 60.0 {
        return format!("{secs:.1}s");
    }

    format!("{}m {:.1}s", (secs as u64) / 60, secs % 60.0)
}

/// Whole seconds, for a clock that redraws several times a second.
pub fn clock(d: Duration) -> String {
    let secs = d.as_secs();

    if secs < 60 {
        return format!("{secs}s");
    }

    format!("{}m {:02}s", secs / 60, secs % 60)
}

// ============================================================
// Parser
// ============================================================

/// Feed one raw pty line to the app.
///
/// Keyed off Flutter's output rather than off keypresses, so a reload triggered
/// from outside frun still reports properly instead of spilling raw progress
/// text into the log stream.
pub fn feed(app: &mut App, raw: &str) {
    let text = clean(raw);
    let text = text.trim();

    if is_artifact(text) {
        return;
    }

    // --------------------------------------------------------
    // Flutter's boxed notices
    // --------------------------------------------------------
    // `logger.printBox()`: the new-version warning, Flutter Fix hints. Plain
    // `flutter run` prints these inline before the build, so they pass straight
    // through rather than being dropped.

    if text.starts_with('┌') {
        app.in_box = true;
        app.log(Level::Wrn, text);
        return;
    }

    if app.in_box {
        app.log(Level::Wrn, text);

        if text.starts_with('└') {
            app.in_box = false;
        }

        return;
    }

    if stage_line(app, text) {
        return;
    }

    if is_help(text) {
        return;
    }

    if reload_line(app, text) {
        return;
    }

    app.log(classify(text), text);
}

/// The build pipeline. Returns true when the line was a stage transition.
///
/// Platform-dependent by construction rather than by branching on the target:
/// iOS emits the CocoaPods and Xcode lines, Android the Gradle and install
/// ones, and a stage that never appears is simply never created.
/// Every arm here opens a stage and nothing closes one.
///
/// A row is closed by the arrival of its successor, inside `start_stage`, which
/// is the same moment it is charged its duration. That is the whole rule, and it
/// is what guarantees a spinner is on screen for every second of the build:
/// closing a row *is* opening the next one, so there is no instant in between.
///
/// What this replaced: triggers that only closed a row — `Xcode build done`,
/// `Built build/...`, a bare duration line, and the `finish_stage` that used to
/// sit on `Launching` and `pod install`. Each of them stopped the spinner while
/// the build carried on, and because the charge still happened later, at the next
/// open, the row's number kept moving after it had been marked done: measured on
/// an iOS transcript, `0ms → 125ms`, `121ms → 241ms`, `124ms → 249ms`.
///
/// Their safety nets went too — every `if gradle_started && !gradle_completed`
/// existed because a close could be missed, and a close that cannot happen
/// separately cannot be missed.
///
/// Platform-dependence needs no branch: iOS emits the CocoaPods and Xcode lines,
/// Android the Gradle and install ones, and a stage nobody announces is never
/// created.
fn stage_line(app: &mut App, text: &str) -> bool {
    if text.starts_with("Launching ") && text.contains(".dart") {
        // Closes `Starting Flutter`, which was opened when the pty spawned and so
        // measures fvm, the Dart VM boot and flutter_tools startup — the one span
        // no Flutter output brackets.
        // Not labelled `Launching lib/main.dart`, which is what Flutter prints
        // here. That is the announcement, and this row is the span that follows
        // it — measured at 3.5s on Android and 20s on iOS, all of it unannounced.
        // Naming a row after the line that opens it made an 18.7s figure look like
        // a fault rather than like the toolchain working.
        app.start_stage(StageKey::Launch, "Preparing build".into());
        return true;
    }

    // Dependency resolution, when Flutter decides it is needed. Absent from most
    // runs, which is why it is not counted in `Platform::stage_count`.
    if text.contains("flutter pub get") || text.starts_with("Resolving dependencies") {
        app.start_stage(StageKey::Pub, "Resolving dependencies".into());
        return true;
    }

    // The first platform phase adopts the generic row rather than opening a new
    // one, so no boundary is claimed between "preparing" and work Flutter had
    // already started without saying so.
    if text.contains("Running pod install") {
        app.adopt_or_open(StageKey::Pods, "Installing CocoaPods".into());
        return true;
    }

    if text.contains("Running Xcode build") {
        app.adopt_or_open(StageKey::Xcode, "Building with Xcode".into());
        return true;
    }

    if text.contains("Running Gradle task") {
        let task = gradle_task(text);

        let label = if task.is_empty() {
            "Building with Gradle".to_string()
        } else {
            format!("Gradle task {task}")
        };

        app.adopt_or_open(StageKey::Gradle, label);
        return true;
    }

    if text.starts_with("Installing build/") {
        app.start_stage(StageKey::Install, "Installing app".into());
        return true;
    }

    if text.contains("Syncing files to device") {
        app.start_stage(StageKey::Sync, "Syncing files".into());

        // Flutter states this one on the line that opens the row, and the line that
        // closes it arrives in the same read, so the measured span is zero. Its own
        // figure is the only evidence the sync took any time at all — and taking it
        // touches no other row, so nothing can appear to swap.
        if let Some(token) = Some(duration(text)).filter(|t| !t.is_empty()) {
            app.set_stage_duration(StageKey::Sync, token);
        }

        return true;
    }

    if text.contains("Flutter run key commands") {
        app.start_stage(StageKey::Ready, "Application Running".into());
        app.session_ready();
        return true;
    }

    // Swallowed rather than logged: these are Flutter closing a phase it already
    // announced, so they carry nothing the row does not already say. They used to
    // be where a stage got closed, which is exactly what left the tracker idle.
    if text.contains("Xcode build done")
        || text.contains("Built build/")
        || text.starts_with("✓ Built")
        || text.contains("Compiling, linking and signing")
        || (app.stage_open(StageKey::Install) && is_bare_duration(text))
    {
        return true;
    }

    false
}

fn is_bare_duration(text: &str) -> bool {
    !text.is_empty() && duration(text) == text
}

/// Hot reload and hot restart. Returns true when the line belonged to one.
fn reload_line(app: &mut App, text: &str) -> bool {
    for (marker, kind) in ACK_MARKERS {
        if text.starts_with(marker) {
            app.ack_reload(kind);
            return true;
        }
    }

    if text.contains("Restarted application in") || text.contains("Hot restart performed") {
        app.finish_reload(Kind::Restart, text);
        return true;
    }

    if text.starts_with("Reloaded ") || text.contains("Hot reload performed") {
        app.finish_reload(Kind::Reload, text);
        return true;
    }

    if app.pending.is_none() {
        return false;
    }

    // Why a reload is taking its time. `run_hot.dart` cancels the "Performing
    // hot reload..." progress and opens a new one carrying this instead, and
    // because it is a progress message every redraw arrives as another line.
    if is_paused_notice(text) {
        app.reload_notice(text);
        return true;
    }

    // Flutter accepted the key and the operation did not land. It reports the
    // cause over several lines and closes with "Try again after fixing the above
    // error(s)", so the reason is noted, Flutter's own lines are let through, and
    // the verdict goes last — after the errors it refers to, not before.
    let reason = if text.starts_with("Hot reload was rejected")
        || text.starts_with("Hot restart was rejected")
    {
        Some("rejected")
    } else if text.contains("Unable to hot reload application") {
        Some("unrecoverable error, use R")
    } else if text.contains("hot restart failed to complete")
        || text.contains("hot reload failed to complete")
    {
        Some("failed to complete")
    } else {
        None
    };

    if let Some(reason) = reason {
        app.note_reload_failure(reason);
        app.log(Level::Err, text);
        return true;
    }

    if text.contains("Try again after fixing the above error") {
        // Superseded by the failure row, which carries the same advice.
        app.fail_reload();
        return true;
    }

    false
}

/// `The application is paused...` / `2 isolates are paused...`
fn is_paused_notice(text: &str) -> bool {
    if !text.contains("paused") {
        return false;
    }

    text.starts_with("The application is")
        || text
            .split_whitespace()
            .next()
            .is_some_and(|word| word.chars().all(|c| c.is_ascii_digit()))
}

/// Which operation is in flight.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Reload,
    Restart,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Reload => "Hot reload",
            Kind::Restart => "Hot restart",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Kind::Reload => "r",
            Kind::Restart => "R",
        }
    }
}

/// Flutter's own acknowledgement that it took the key.
///
/// Restart is listed first so a future `Performing hot restart` that also
/// matched a shorter reload prefix could not be misread as a reload.
pub const ACK_MARKERS: [(&str, Kind); 2] = [
    ("Performing hot restart", Kind::Restart),
    ("Performing hot reload", Kind::Reload),
];

// ============================================================
// Log levels
// ============================================================

/// Which badge a line gets, per DESIGN.md 3.5.
///
/// Only what the application itself produces: `SYS`, `BLD` and `OK` are gone
/// because everything they carried is a build stage and the tracker owns those.
fn classify(text: &str) -> Level {
    // Android logcat, as Flutter forwards it: `W/ActivityThread( 1234):`.
    if let Some(rest) = text.strip_prefix(|c| matches!(c, 'I' | 'W' | 'E' | 'D' | 'V')) {
        if rest.starts_with('/') {
            return match text.as_bytes()[0] {
                b'E' => Level::Err,
                b'W' => Level::Wrn,
                _ => Level::Inf,
            };
        }
    }

    const ERRORS: [&str; 8] = [
        "Exception",
        "Error:",
        "error:",
        "Failed assertion",
        "was thrown",
        "FAILURE:",
        "Unhandled exception",
        "══╡",
    ];

    // A Dart stack frame: `#0      _AssertionError._doThrowNew (...)`.
    let is_frame =
        text.starts_with('#') && text[1..].chars().next().is_some_and(|c| c.is_ascii_digit());

    if is_frame || ERRORS.iter().any(|needle| text.contains(needle)) {
        return Level::Err;
    }

    if text.starts_with("Warning:") || text.contains("warning:") {
        return Level::Wrn;
    }

    Level::Inf
}

// ============================================================
// Build failure
// ============================================================

/// Build a failure report from what the run printed before it died.
///
/// `frun-runner` has no build-failure detection at all: if Gradle dies the
/// output falls through to raw passthrough and the spinner simply stops. The
/// trigger here is not a catalogue of error strings but a single fact — the child
/// exited without ever opening an interactive session — which catches Gradle,
/// Xcode, `pub get`, a missing entrypoint, and whatever fails next.
pub fn failure(app: &App, code: i32) -> crate::data::Failure {
    // The tail of what was printed. Everything Flutter said that was not a stage
    // transition is already in the log buffer, which during a build is not on
    // screen — this is where it becomes visible.
    let tail: Vec<String> = app
        .logs
        .iter()
        .rev()
        .take(40)
        .map(|l| l.message.clone())
        .collect();

    let summary = tail
        .iter()
        .find(|l| l.contains("Error:") || l.contains("error:") || l.contains("FAILURE:"))
        .or_else(|| tail.first())
        .cloned()
        .unwrap_or_else(|| format!("Build failed with exit code {code}"));

    let location = tail.iter().find_map(|l| dart_location(l));

    let (context, caret_col) = match &location {
        Some((file, line, column)) => (source_context(file, *line), *column),
        None => (Vec::new(), 0),
    };

    // Reversed back into the order Flutter printed them, and without the line
    // already shown as the summary: a one-line failure like `Target file ... not
    // found.` is both the summary and the entire output, so it appeared twice.
    let mut output: Vec<String> = tail
        .into_iter()
        .filter(|line| line.trim() != summary)
        .take(8)
        .collect();

    output.reverse();

    crate::data::Failure {
        summary: summary.trim().to_string(),
        location,
        context,
        caret_col,
        note: which_stage(app),
        output,
    }
}

/// Which stage broke, which DESIGN.md 3.4 asks the failure state to name.
fn which_stage(app: &App) -> String {
    match app.stages.iter().find(|s| !s.done) {
        Some(stage) => format!("failed during: {}", stage.label),
        None => "failed before any stage completed".to_string(),
    }
}

/// `lib/main.dart:42:18` out of a compiler line.
fn dart_location(text: &str) -> Option<(String, u32, u32)> {
    let at = text.find(".dart:")?;
    let rest = &text[at + ".dart:".len()..];

    let mut numbers = rest.split(':');
    let line: u32 = numbers.next()?.trim().parse().ok()?;

    let column: u32 = numbers
        .next()
        .and_then(|c| {
            c.trim()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or(1);

    // Walk back to the start of the path, which is bounded by whitespace or the
    // quotes and parentheses Dart wraps locations in.
    let head = &text[..at + ".dart".len()];
    let start = head
        .rfind(|c: char| c.is_whitespace() || "'\"(<".contains(c))
        .map(|i| i + 1)
        .unwrap_or(0);

    Some((head[start..].to_string(), line, column))
}

/// The reported line and one either side, read from disk.
///
/// Dart emits the offending line and a caret itself, so that much would be free
/// passthrough — but it arrives interleaved with everything else, and one line of
/// context either side is usually the difference between recognising the mistake
/// and opening the editor.
fn source_context(file: &str, line: u32) -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(file) else {
        // Relative to the wrong directory, or a path inside a package rather
        // than this project. The failure card drops the frame and shows the
        // build output instead.
        return Vec::new();
    };

    let first = line.saturating_sub(1).max(1);

    text.lines()
        .enumerate()
        .map(|(i, l)| (i as u32 + 1, l))
        .filter(|(n, _)| *n >= first && *n <= line + 1)
        .map(|(n, l)| (n, l.trim_end().to_string()))
        .collect()
}

// ============================================================
// Stage bookkeeping
// ============================================================

impl App {
    /// Open a stage, or leave it alone if it is already open.
    ///
    /// Idempotent, which is what all of `frun-runner`'s `if not
    /// pods_started` guards were doing by hand. Flutter re-emits most of these
    /// lines several times through CR.
    pub fn start_stage(&mut self, key: StageKey, label: String) {
        if self.stages.iter().any(|s| s.key == key) {
            return;
        }

        let now = Instant::now();

        // Every stage is measured open-to-open, so the column partitions the build
        // and therefore sums to it.
        //
        // Mixing sources is what broke this: markers got our measured gap while
        // real stages kept Flutter's own figure, and Flutter's timers start before
        // its announcements reach us. A measured run showed 11.0s + 11.2s + 0ms
        // against a 20.1s total, 2.1s counted twice.
        //
        // The cost is that `Building with Xcode` now reads ~9.1s where raw
        // `flutter run` says 11.2s. Flutter's figure is the truer measure of the
        // Xcode build alone; ours is the truer measure of where the wall clock
        // went, and only one of the two can add up.
        // Closing the previous row and charging it happen here, together, and this
        // is the only place either happens. Splitting them is what produced both
        // defects: a row closed by its own trigger stopped spinning while the
        // build carried on, and was then still charged later at the next open, so
        // its number moved after it had been marked done.
        if let Some(previous) = self.stages.last_mut() {
            if !previous.pinned {
                previous.duration = elapsed(now.duration_since(previous.started));
            }

            previous.done = true;
        }

        self.stages.push(Stage {
            pinned: false,
            key,
            label,
            duration: String::new(),
            done: false,
            started: now,
        });
    }

    // `finish_stage` and `stage_duration` used to live here and are gone.
    //
    // Closing a row is not a separate operation any more: `start_stage` closes the
    // previous row as it opens the next, which is the only way to guarantee a
    // spinner is on screen for every second of the build. A public method that can
    // close a row without opening one is exactly how the idle gaps got in, so
    // there no longer is one.
    //
    // `stage_duration` recorded Flutter's own figure without closing the row. It
    // has no caller now that every row is measured open-to-open; keeping Flutter's
    // number alongside measured gaps double-counted, because its timers start
    // before its announcements reach us.

    /// Adopt the open row instead of opening a new one, when the open row is the
    /// generic one.
    ///
    /// This is what replaced `rewind_stage`, and the reasoning is worth keeping.
    ///
    /// Flutter's timers start before its announcements reach us. On a cold iOS
    /// build it began the Xcode build **16.3 seconds** before printing `Running
    /// Xcode build...`, then closed with `Xcode build done. 22.6s`. Timing the row
    /// from the announcement gave `Building with Xcode 5.6s` for work Flutter said
    /// took 22.6s, with the missing 16 seconds parked on the row above.
    ///
    /// The first fix believed Flutter's figure and moved the boundary between the
    /// two rows back to where the work began. It was accurate and it was wrong: a
    /// row already shown as `✔ Preparing build 10s` became `3.5s` seconds later,
    /// and the two figures looked like they had swapped. Measured: `405ms → 60ms`.
    ///
    /// The real problem is that the two phases **overlap**. Xcode was already
    /// building while frun still thought it was preparing, so two consecutive rows
    /// were claiming a boundary that cannot be observed. Rather than correcting a
    /// boundary that does not exist, the claim is dropped: the row opened by
    /// `Launching lib/main.dart` is the same row that becomes `Building with
    /// Xcode` once Flutter says so. One row, one span, one figure, and nothing to
    /// correct afterwards.
    ///
    /// A label changing while a row spins is a far smaller surprise than a settled
    /// number moving. It is also honest about what happened: *I am working, and now
    /// I can tell you what this is.*
    fn adopt_or_open(&mut self, key: StageKey, label: String) {
        match self.stages.last_mut() {
            // The generic row is still open: rename it in place.
            Some(open) if !open.done && open.key == StageKey::Launch => {
                open.key = key;
                open.label = label;
            }

            _ => self.start_stage(key, label),
        }
    }

    /// Pin a row's figure to what Flutter said, without touching its neighbours.
    ///
    /// Only used where Flutter's number is the only evidence a phase took any time,
    /// which is the sync. It does not move a boundary, so no other row changes and
    /// nothing can look like it swapped.
    fn set_stage_duration(&mut self, key: StageKey, duration: String) {
        if let Some(stage) = self.stages.iter_mut().find(|s| s.key == key) {
            stage.duration = duration;
            stage.pinned = true;
        }
    }

    pub fn stage_open(&self, key: StageKey) -> bool {
        self.stages.iter().any(|s| s.key == key && !s.done)
    }

    /// The interactive session is live: the build is over and logs start.
    pub fn session_ready(&mut self) {
        if self.state.build_done() {
            return;
        }

        self.end_build();

        // The last row has no successor to close it, so the end of the build does.
        // Without this it would spin forever behind a finished build.
        if let Some(last) = self.stages.last_mut() {
            if !last.done {
                // `Application Running` carries no figure. It is the destination,
                // not a leg of the journey: it opens and closes on the same line
                // Flutter prints, so any measurement of it is `0ms` — a number that
                // looks like a broken clock rather than like the end of a build.
                //
                // Leaving it blank costs the column nothing, since zero adds
                // nothing to a sum.
                if last.key != StageKey::Ready {
                    last.duration = elapsed(last.started.elapsed());
                }

                last.done = true;
            }
        }

        self.sync_time = self
            .stages
            .iter()
            .find(|s| s.key == StageKey::Sync)
            .map(|s| s.duration.clone())
            .unwrap_or_default();

        self.goto(State::Running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_sequences_and_backspaces_are_replayed() {
        assert_eq!(clean("\x1b[32mBuilt\x1b[0m"), "Built");
        assert_eq!(clean("\x1b]0;title\x07Built"), "Built");

        // The erase pattern Flutter uses: backspaces, spaces, backspaces.
        assert_eq!(clean("ab\u{8}\u{8}cd"), "cd");

        // A leading animation frame glued to a log line.
        assert_eq!(clean("⠋ Running Gradle task"), "Running Gradle task");
    }

    #[test]
    fn durations_keep_their_group_separator() {
        // The scar this guards: matching only digits clipped `1,847ms` to
        // `847ms`, because Flutter formats through NumberFormat.
        assert_eq!(duration("Restarted application in 1,847ms."), "1,847ms");
        assert_eq!(duration("Xcode build done.                 11.1s"), "11.1s");
        assert_eq!(
            duration("Reloaded 125 of 1824 libraries in 148ms."),
            "148ms"
        );
        assert_eq!(duration("Running Gradle task 'assembleDebug'..."), "");
    }

    #[test]
    fn gradle_task_names_come_out_of_the_quotes() {
        assert_eq!(
            gradle_task("Running Gradle task 'assembleDebug'..."),
            "assembleDebug"
        );
    }

    #[test]
    fn artifacts_are_dropped_but_real_lines_survive() {
        assert!(is_artifact(""));
        assert!(is_artifact("⠹"));
        assert!(is_artifact("⠋ ⠙"));
        assert!(is_artifact("..."));

        assert!(!is_artifact("Built build/app.apk"));
        // Short, but alphanumeric: a real log line.
        assert!(!is_artifact("693ms"));
    }

    #[test]
    fn levels_follow_the_source_of_the_line() {
        assert_eq!(classify("I/flutter ( 1234): hello"), Level::Inf);
        assert_eq!(classify("W/ActivityThread( 1234): slow"), Level::Wrn);
        assert_eq!(classify("E/AndroidRuntime( 1234): boom"), Level::Err);
        assert_eq!(
            classify("#0      _AssertionError._doThrowNew (errors_patch.dart:51)"),
            Level::Err
        );
        assert_eq!(
            classify("The following assertion was thrown building Checkout:"),
            Level::Err
        );
        assert_eq!(classify("flutter: just a print"), Level::Inf);
    }

    #[test]
    fn dart_locations_are_found_inside_prose() {
        assert_eq!(
            dart_location("lib/main.dart:42:18: Error: type mismatch"),
            Some(("lib/main.dart".to_string(), 42, 18))
        );

        assert_eq!(
            dart_location("Error at 'lib/screens/checkout.dart:88:14'"),
            Some(("lib/screens/checkout.dart".to_string(), 88, 14))
        );

        assert_eq!(dart_location("no location here"), None);
    }

    /// The reported defect: the bar filled to 100% at the first stage and then
    /// dropped as more stages were announced, because the denominator was the
    /// number announced so far.
    #[test]
    fn the_progress_fraction_never_goes_backwards() {
        let mut app = App::new(State::Building);
        app.stages.clear();

        let mut previous = 0.0;

        for (key, label) in [
            (StageKey::Launch, "Starting Flutter"),
            (StageKey::Gradle, "Gradle task assembleDebug"),
            (StageKey::Install, "Installing app"),
            (StageKey::Sync, "Syncing files"),
            (StageKey::Ready, "Application Running"),
        ] {
            app.start_stage(key, label.into());

            let done = app.stages.iter().filter(|s| s.done).count();
            let fraction = done as f64 / app.expected_stages() as f64;

            assert!(
                fraction >= previous,
                "fraction fell from {previous} to {fraction} at {label}"
            );

            assert!(fraction <= 1.0, "{fraction} exceeds full at {label}");

            previous = fraction;
        }
    }

    /// A build that skips a stage still ends complete.
    ///
    /// iOS runs five when `Podfile.lock` is current, against an estimate of six.
    /// The estimate is an upper bound, so the numerator can never reach it — which
    /// is correct while building and wrong the moment the build is over. Ending on
    /// `5/6` reports a build that stopped short of itself.
    #[test]
    fn a_skipped_stage_still_ends_at_full() {
        let mut app = App::new(State::Building);
        app.begin_build();

        // Neither `Running pod install` nor `Syncing files`, so this run comes in
        // under the estimate: starting, launching, Xcode, running.
        for line in [
            "Launching lib/main.dart on iPhone 17 Pro in debug mode...",
            "Running Xcode build...",
            "Flutter run key commands.",
        ] {
            feed(&mut app, line);
        }

        // Three rows, not four: `Running Xcode build` adopts the generic row rather
        // than opening its own, so `Preparing build` and `Building with Xcode` are
        // one row that changed its name.
        assert_eq!(app.stages.len(), 3, "the skipped path should run three");
        assert_eq!(app.expected_stages(), 4, "the estimate stays an upper bound");

        assert!(
            app.state.build_done(),
            "the session should be live, which is what collapses the denominator"
        );

        // What the row draws once finished: both numbers taken from what ran, so
        // the bar reads full instead of stopping short of itself.
        let ran = app.stages.len();
        assert_eq!((ran, ran), (3, 3), "should read 3/3, not 3/5");
    }

    /// No figure ever changes once it has been shown.
    ///
    /// This is the check the previous attempt failed. It believed Flutter's own
    /// `Xcode build done. 22.6s` and moved the boundary back to where the work
    /// really began, which was accurate and unusable: a row already reading
    /// `✔ Preparing build 10s` became `3.5s` seconds later, so the two figures
    /// looked like they had swapped. Measured on a transcript: `405ms → 60ms`.
    ///
    /// The old test for this passed only because its timings were too short for the
    /// correction to land between two rows — a false pass, which is exactly why it
    /// is rebuilt here with a gap wide enough for the correction to have applied.
    #[test]
    fn no_figure_changes_once_it_has_been_shown() {
        let mut app = App::new(State::Building);
        app.begin_build();

        let mut shown: Vec<(String, String)> = Vec::new();

        // The real sequence, with a gap where Flutter goes silent while already
        // building. On a cold iOS build that stretch is 7 to 16 seconds.
        let script: [(&str, u64); 5] = [
            ("Launching lib/main.dart on iPhone 17 Pro in debug mode...", 400),
            ("Running Xcode build...", 100),
            ("Xcode build done.                      0.450s", 0),
            ("Syncing files to device iPhone 17 Pro...     81ms", 0),
            ("Flutter run key commands.", 0),
        ];

        for (line, pause) in script {
            feed(&mut app, line);
            std::thread::sleep(Duration::from_millis(pause));

            for (label, figure) in &shown {
                let now = app
                    .stages
                    .iter()
                    .find(|s| s.label == *label)
                    .map(|s| s.duration.clone())
                    .unwrap_or_default();

                assert_eq!(
                    &now, figure,
                    "{label:?} changed from {figure:?} to {now:?} after {line:?}"
                );
            }

            // Remember every figure that is now on screen.
            shown = app
                .stages
                .iter()
                .filter(|s| !s.duration.is_empty())
                .map(|s| (s.label.clone(), s.duration.clone()))
                .collect();
        }
    }

    /// The generic row becomes the platform phase rather than being followed by it.
    ///
    /// That is what removes the boundary nobody can observe: Flutter is already
    /// building when it finally says `Running Xcode build...`, so a separate row
    /// starting at that line would be claiming a split that did not happen.
    #[test]
    fn the_first_platform_phase_adopts_the_generic_row() {
        let mut app = App::new(State::Building);
        app.begin_build();

        feed(&mut app, "Launching lib/main.dart on iPhone 17 Pro in debug mode...");

        assert_eq!(app.stages.len(), 2);
        assert_eq!(app.stages[1].label, "Preparing build");

        feed(&mut app, "Running Xcode build...");

        assert_eq!(
            app.stages.len(),
            2,
            "a new row was opened instead of adopting the generic one"
        );

        assert_eq!(app.stages[1].label, "Building with Xcode");
        assert!(!app.stages[1].done, "the adopted row must keep running");

        // A second, genuinely distinct phase still gets its own row.
        feed(&mut app, "Syncing files to device iPhone 17 Pro...");
        assert_eq!(app.stages.len(), 3);
    }

    /// State 11: a keypress Flutter never acknowledged.
    ///
    /// This is the one state that cannot be provoked on a real device on demand.
    /// Its trigger is Flutter silently discarding a keypress while busy — reported
    /// only through `printTrace()`, which never reaches stdout — and an attempt to
    /// force it by sending `r` during a hot restart failed: Flutter queued the key
    /// and serviced it afterwards rather than dropping it.
    ///
    /// So what is checked here is the half that belongs to frun: a request with no
    /// acknowledgement resolves, rather than spinning forever. That was the whole
    /// reason the state exists.
    #[test]
    fn an_unacknowledged_keypress_resolves_instead_of_spinning_forever() {
        let mut app = App::new(State::Running);

        app.request_reload(Kind::Reload);

        assert_eq!(app.state, State::ReloadInFlight);
        assert!(
            app.pending.as_ref().is_some_and(|p| !p.acked),
            "a keypress starts unacknowledged: it is a request, not a fact"
        );

        // Nothing acknowledges it. Wind the deadline into the past rather than
        // sleeping out ACK_TIMEOUT.
        app.pending
            .as_mut()
            .expect("a pending reload")
            .deadline = Some(Instant::now() - Duration::from_millis(1));

        app.tick_pending();

        assert_eq!(app.state, State::ReloadDropped);
        assert!(app.pending.is_none(), "the stage has to be cleared");
        assert!(
            app.reload_note.contains("not picked up"),
            "the row must say why: {:?}",
            app.reload_note
        );
    }

    /// An acknowledged operation has no deadline, because it may legitimately
    /// take as long as it likes. Applying one would report a failure that has not
    /// happened.
    #[test]
    fn an_acknowledged_reload_is_never_dropped() {
        let mut app = App::new(State::Running);

        app.request_reload(Kind::Reload);
        feed(&mut app, "Performing hot reload...");

        assert!(
            app.pending.as_ref().is_some_and(|p| p.acked),
            "Flutter's own progress line is the acknowledgement"
        );

        assert!(
            app.pending.as_ref().is_some_and(|p| p.deadline.is_none()),
            "an acknowledged stage must not carry a deadline"
        );

        app.tick_pending();
        assert_eq!(app.state, State::ReloadInFlight, "it should still be running");

        feed(&mut app, "Reloaded 3 of 1824 libraries in 212ms.");
        assert_eq!(app.state, State::Running);
    }

    /// A late acknowledgement after a drop reopens the stage.
    ///
    /// This is why the timeout can afford to be wrong: `runSourceGenerators()`
    /// runs before Flutter's progress line appears, so a slow accept looks
    /// identical to a drop for a moment. Being self-correcting is what makes a
    /// false timeout harmless.
    #[test]
    fn a_late_acknowledgement_reopens_a_dropped_stage() {
        let mut app = App::new(State::Running);

        app.request_reload(Kind::Reload);
        app.pending
            .as_mut()
            .expect("a pending reload")
            .deadline = Some(Instant::now() - Duration::from_millis(1));
        app.tick_pending();

        assert_eq!(app.state, State::ReloadDropped);

        // Flutter took it after all.
        feed(&mut app, "Performing hot reload...");

        assert_eq!(app.state, State::ReloadInFlight);
        assert!(app.pending.as_ref().is_some_and(|p| p.acked));
    }

    /// The invariant: something is always visibly happening.
    ///
    /// A build must never reach a moment where every row is `✔` and none is
    /// spinning, because that is a tracker that looks finished while Flutter is
    /// still working. Replaying real transcripts is the only way to check it —
    /// the failure is a *gap between* two lines, so it cannot be seen by testing
    /// either line alone.
    ///
    /// Both platforms, because the close-only triggers that used to break this
    /// were platform-specific: `Xcode build done` on iOS, `Built build/...` and a
    /// bare duration line on Android.
    #[test]
    fn exactly_one_stage_is_open_at_every_point_in_a_build() {
        let transcripts: [(&str, &[&str]); 2] = [
            (
                "ios",
                &[
                    "Launching lib/main.dart on iPhone 17 Pro in debug mode...",
                    "Running pod install...",
                    "Running pod install...                              1,204ms",
                    "Running Xcode build...",
                    "Xcode build done.                                   11.1s",
                    "Compiling, linking and signing...",
                    "Syncing files to device iPhone 17 Pro...",
                ],
            ),
            (
                "android",
                &[
                    "Launching lib/main.dart on Pixel 10 Pro XL in debug mode...",
                    "Running Gradle task 'assembleDebug'...",
                    "Running Gradle task 'assembleDebug'...              2,834ms",
                    "✓ Built build/app/outputs/flutter-apk/app-debug.apk",
                    "Installing build/app/outputs/flutter-apk/app-debug.apk...",
                    "693ms",
                    "Syncing files to device Pixel 10 Pro XL...",
                ],
            ),
        ];

        for (platform, lines) in transcripts {
            let mut app = App::new(State::Building);
            app.begin_build();

            let open = |app: &App| app.stages.iter().filter(|s| !s.done).count();

            assert_eq!(
                open(&app),
                1,
                "{platform}: nothing was spinning before Flutter printed anything"
            );

            for line in lines {
                feed(&mut app, line);

                assert_eq!(
                    open(&app),
                    1,
                    "{platform}: {} rows open after {line:?}",
                    open(&app)
                );
            }

            // Only the end of the build closes the final row.
            feed(&mut app, "Flutter run key commands.");

            assert_eq!(
                open(&app),
                0,
                "{platform}: a row is still spinning after the build finished"
            );

            assert!(
                app.stages
                    .iter()
                    .filter(|s| s.key != StageKey::Ready)
                    .all(|s| !s.duration.is_empty()),
                "{platform}: a completed phase has no duration"
            );

            // The destination is the exception, and deliberately so: it opens and
            // closes on the same line, so a figure there could only ever be `0ms`.
            assert!(
                app.stages
                    .last()
                    .is_some_and(|s| s.key == StageKey::Ready && s.duration.is_empty()),
                "{platform}: the last row should carry no figure"
            );
        }
    }

    /// Durations must not move once a row is marked done.
    ///
    /// The old split — closed by its own trigger, charged at the next open — let a
    /// row show `✔ Building with Xcode 11.1s` and then silently become `14.5s`
    /// seconds later. Measured on an iOS transcript: `0ms → 125ms`,
    /// `121ms → 241ms`, `124ms → 249ms`.
    #[test]
    fn a_closed_stage_never_changes_its_duration() {
        let mut app = App::new(State::Building);
        app.begin_build();

        feed(&mut app, "Launching lib/main.dart on iPhone 17 Pro in debug mode...");
        feed(&mut app, "Running Xcode build...");

        let settled: Vec<(String, String)> = app
            .stages
            .iter()
            .filter(|s| s.done)
            .map(|s| (s.label.clone(), s.duration.clone()))
            .collect();

        assert!(!settled.is_empty(), "nothing had closed yet");

        std::thread::sleep(Duration::from_millis(30));
        feed(&mut app, "Xcode build done.            11.1s");
        feed(&mut app, "Syncing files to device iPhone 17 Pro...");

        for (label, duration) in settled {
            let now = app
                .stages
                .iter()
                .find(|s| s.label == label)
                .map(|s| s.duration.clone())
                .expect("the row should still exist");

            assert_eq!(now, duration, "{label} changed after being marked done");
        }
    }

    // The marker test that used to sit here is gone with the concept. Every row
    // is now measured from its own announcement to the next one, so there is no
    // class of row that needs a special rule for its duration — which is what the
    // marker was. `exactly_one_stage_is_open_at_every_point_in_a_build` covers
    // what it was really protecting: that no span of the build goes unaccounted.

    #[test]
    fn a_paused_isolate_notice_is_recognised() {
        assert!(is_paused_notice(
            "The application is paused in the debugger"
        ));
        assert!(is_paused_notice("2 isolates are paused"));
        assert!(!is_paused_notice("Reloaded 1 of 2 libraries"));
    }
}
