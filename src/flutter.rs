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

/// The invocation, as shown on the SelectedTargetCard.
pub fn command_line(device: &str, extra: &[String]) -> String {
    let mut parts = vec![
        "fvm".to_string(),
        "flutter".into(),
        "run".into(),
        "-d".into(),
        device.into(),
    ];

    parts.extend(extra.iter().cloned());
    parts.join(" ")
}

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
fn stage_line(app: &mut App, text: &str) -> bool {
    if text.starts_with("Launching ") && text.contains(".dart") {
        app.start_stage(StageKey::Launch, "Starting Flutter".into());
        app.finish_stage(StageKey::Launch, String::new());
        return true;
    }

    // --------------------------------------------------------
    // CocoaPods
    // --------------------------------------------------------

    if text.contains("Running pod install") {
        // Flutter re-emits this line through CR with the elapsed time appended
        // once pods finish, so a duration on it is the completion signal.
        let d = duration(text);

        app.start_stage(StageKey::Pods, "Installing CocoaPods".into());

        if !d.is_empty() {
            app.finish_stage(StageKey::Pods, d);
        }

        return true;
    }

    // --------------------------------------------------------
    // Xcode
    // --------------------------------------------------------

    if text.contains("Running Xcode build") {
        app.finish_stage(StageKey::Pods, String::new());
        app.start_stage(StageKey::Xcode, "Building with Xcode".into());
        return true;
    }

    if text.contains("Xcode build done") {
        app.finish_stage(StageKey::Xcode, duration(text));
        return true;
    }

    if text.contains("Compiling, linking and signing") {
        return true;
    }

    // --------------------------------------------------------
    // Gradle
    // --------------------------------------------------------

    if text.contains("Running Gradle task") {
        let task = gradle_task(text);

        let label = if task.is_empty() {
            "Building with Gradle".to_string()
        } else {
            format!("Gradle task {task}")
        };

        app.start_stage(StageKey::Gradle, label);

        // Flutter animates this line and every redraw carries a longer elapsed
        // time, so the value is recorded without closing the stage. Treating the
        // first one seen as completion reported a far too short build.
        let d = duration(text);

        if !d.is_empty() {
            app.stage_duration(StageKey::Gradle, d);
        }

        return true;
    }

    if text.contains("Built build/") || text.starts_with("✓ Built") {
        // The unambiguous "Gradle finished" signal, so whatever elapsed time the
        // Gradle line last carried is final.
        app.finish_stage(StageKey::Gradle, String::new());
        return true;
    }

    // --------------------------------------------------------
    // Install
    // --------------------------------------------------------

    if text.starts_with("Installing build/") {
        app.finish_stage(StageKey::Gradle, String::new());
        app.start_stage(StageKey::Install, "Installing app".into());

        let d = duration(text);

        if !d.is_empty() {
            app.stage_duration(StageKey::Install, d);
        }

        return true;
    }

    // Some versions emit the install duration on a line of its own.
    if app.stage_open(StageKey::Install) && is_bare_duration(text) {
        app.finish_stage(StageKey::Install, text.to_string());
        return true;
    }

    // --------------------------------------------------------
    // Sync
    // --------------------------------------------------------

    if text.contains("Syncing files to device") {
        // Safety net for builds that skip both `Built build/...` and the install
        // step, as happens when attaching to an already-installed app.
        app.finish_stage(StageKey::Gradle, String::new());
        app.finish_stage(StageKey::Install, String::new());
        app.start_stage(StageKey::Sync, "Syncing files".into());
        return true;
    }

    // --------------------------------------------------------
    // Interactive session
    // --------------------------------------------------------

    if text.contains("Flutter run key commands") {
        app.finish_stage(StageKey::Sync, String::new());
        app.start_stage(StageKey::Ready, "Interactive session ready".into());
        app.finish_stage(StageKey::Ready, String::new());
        app.session_ready();
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
        if let Some(previous) = self.stages.last_mut() {
            previous.duration = elapsed(now.duration_since(previous.started));
        }

        self.stages.push(Stage {
            key,
            label,
            duration: String::new(),
            done: false,
            started: now,
        });
    }

    /// Record an elapsed time without closing the stage.
    pub fn stage_duration(&mut self, key: StageKey, duration: String) {
        if let Some(stage) = self.stages.iter_mut().find(|s| s.key == key && !s.done) {
            stage.duration = duration;
        }
    }

    /// Close a stage. An empty `duration` keeps whatever the stage already had,
    /// and falls back to wall time when Flutter reported nothing.
    ///
    /// Measuring it ourselves is what gives `Syncing files` and `Interactive
    /// session ready` an honest number: Flutter prints no duration for either.
    pub fn finish_stage(&mut self, key: StageKey, duration: String) {
        let Some(stage) = self.stages.iter_mut().find(|s| s.key == key && !s.done) else {
            return;
        };

        // Flutter's reported figure is deliberately ignored, see `start_stage`:
        // its timers start before its announcements arrive, so mixing it with
        // measured gaps double-counts. Kept as a parameter because the call sites
        // read better naming what Flutter said, even where we do not use it.
        let _ = duration;

        // Every stage that completes without a figure gets the time from when it
        // opened until now, markers included. The comment that used to sit here
        // said markers are left empty on purpose because start_stage fills them
        // with the gap to the next stage, and the last row keeps a blank because
        // nothing follows it.
        //
        // That was the defect. The last row is `Interactive session ready`, and
        // leaving it blank lost the time from that announcement until the build is
        // declared done. A measured run showed 11.3s + 7.1s + 0ms + blank = 18.4s
        // against a 21.6s total, 3.2s unaccounted for.
        if stage.duration.is_empty() {
            stage.duration = elapsed(stage.started.elapsed());
        }

        stage.done = true;
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
            (StageKey::Ready, "Interactive session ready"),
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
            app.finish_stage(key, "1s".into());
        }
    }

    /// A marker row shows the gap to the next stage, which is where the
    /// eight-second startup delay after `Launching lib/main.dart` becomes visible.
    #[test]
    fn a_marker_stage_carries_the_gap_to_the_next_one() {
        let mut app = App::new(State::Building);
        app.stages.clear();

        app.start_stage(StageKey::Launch, "Starting Flutter".into());
        app.finish_stage(StageKey::Launch, String::new());

        assert!(
            app.stages[0].duration.is_empty(),
            "a marker must not time itself, that is the 0ms defect"
        );

        std::thread::sleep(Duration::from_millis(20));
        app.start_stage(StageKey::Gradle, "Gradle".into());

        assert!(
            !app.stages[0].duration.is_empty(),
            "the gap to the next stage should have filled it"
        );

        // The last row has nothing after it, but leaving it blank loses the time
        // from its announcement until the build finishes. A measured run showed
        // 11.3s + 7.1s + 0ms + blank = 18.4s against 21.6s, 3.2s unaccounted for.
        app.start_stage(StageKey::Ready, "Interactive session ready".into());
        std::thread::sleep(Duration::from_millis(20));
        app.finish_stage(StageKey::Ready, String::new());

        assert!(
            !app.stages.last().expect("a stage").duration.is_empty(),
            "the last stage must account for the gap to build completion"
        );
    }

    #[test]
    fn a_paused_isolate_notice_is_recognised() {
        assert!(is_paused_notice(
            "The application is paused in the debugger"
        ));
        assert!(is_paused_notice("2 isolates are paused"));
        assert!(!is_paused_notice("Reloaded 1 of 2 libraries"));
    }
}
