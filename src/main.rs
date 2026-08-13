//! frun-tui — design prototype for the frun rewrite.
//!
//!   frun-tui                        interactive
//!   frun-tui --dump <state> [WxH]   render one frame to stdout
//!   frun-tui --all [WxH]            every state, in flow order
//!   frun-tui --hits <state> [WxH]   probe the clickable regions
//!   frun-tui --rows <state> [W]     degradation ladder across heights
//!   frun-tui --states               list the state slugs
//!
//! Data is static. The pty, the Flutter output parser and device discovery are
//! the next stage.

mod budget;
mod data;
mod dump;
mod theme;
mod ui;
mod widgets;

use std::io::{self, Write};

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

use data::{Action, App, State};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--states") => {
            for state in State::ALL {
                println!("{}", state.slug());
            }
            Ok(())
        }

        Some("--dump") => {
            let state = state_arg(args.get(1))?;
            let (w, h) = size(args.get(2), 106, 45);
            print!("{}", dump::dump(&mut App::new(state), w, h));
            Ok(())
        }

        Some("--all") => {
            let (w, h) = size(args.get(1), 106, 45);

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

        Some("--hits") => {
            let state = state_arg(args.get(1))?;
            let (w, h) = size(args.get(2), 106, 45);
            print!("{}", dump::hits(&mut App::new(state), w, h));
            Ok(())
        }

        Some("--rows") => {
            let state = state_arg(args.get(1))?;
            let w = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(106);
            print!("{}", dump::rows(&App::new(state), w));
            Ok(())
        }

        Some(other) if other.starts_with("--") => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown flag {other}"),
        )),

        Some("--demo") => {
            let mut app = App::new(State::Detecting);
            app.demo = true;
            run(app)
        }

        // Opens on Running rather than Detecting.
        //
        // Detecting is a state that waits for a process to finish, and in a
        // prototype no process ever does, so it sat there forever and read as a
        // hang. Running is the state with the most on screen and nothing
        // pending, which is the honest place to start when the data is static.
        _ => run(App::new(State::Running)),
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

fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;

    // Mouse capture is NOT enabled here. Capturing takes text selection away
    // from the terminal, and copying a stack trace out of the log window is a
    // large part of what that window is for. `m` turns it on when the scroll
    // wheel or a clickable control is wanted.
    execute!(io::stdout(), EnterAlternateScreen)?;

    let result = event_loop(&mut app);

    // Restore before propagating any error, so a failure inside the loop cannot
    // leave the terminal in raw mode or holding the mouse.
    disable_raw_mode()?;

    if app.mouse_on {
        execute!(io::stdout(), DisableMouseCapture)?;
    }

    execute!(io::stdout(), LeaveAlternateScreen)?;

    result?;

    // Leaving the alternate screen discards everything the app drew. Replaying
    // the log buffer here is the mitigation for that: filtering and scrolling
    // while running, a plain transcript in scrollback afterwards.
    if !app.logs.is_empty() {
        let mut out = io::stdout();

        writeln!(out)?;
        writeln!(out, "\x1b[2m── transcript ──────────────────────\x1b[0m")?;

        for log in &app.logs {
            writeln!(out, "{}  {}  {}", log.time, log.level.badge(), log.message)?;
        }
    }

    Ok(())
}

fn event_loop(app: &mut App) -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        // Timeout drives the spinner. Anything longer reads as a stutter,
        // anything shorter is redraw for its own sake.
        if !event::poll(std::time::Duration::from_millis(80))? {
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
                if app.command_mode {
                    match key.code {
                        KeyCode::Esc => {
                            app.command_mode = false;
                            app.command_input.clear();
                        }
                        KeyCode::Enter => {
                            app.command_mode = false;
                            app.command_input.clear();
                        }
                        KeyCode::Backspace => {
                            app.command_input.pop();
                        }
                        KeyCode::Char(c) => app.command_input.push(c),
                        _ => {}
                    }

                    continue;
                }

                match key.code {
                    // Routed through `apply` like everything else rather than
                    // returning here directly. `q` and `^C` are genuinely
                    // different exits — one is graceful and lets Flutter shut
                    // itself down, the other forwards SIGINT — and the single
                    // dispatch point is where that distinction will live.
                    KeyCode::Char('q') | KeyCode::Esc => {
                        apply(app, Action::Quit);
                        return Ok(());
                    }

                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        apply(app, Action::Stop);
                        return Ok(());
                    }

                    // State navigation, prototype only. In the real build the
                    // state is decided by what Flutter is doing.
                    KeyCode::Tab | KeyCode::Right => app.next_state(),
                    KeyCode::BackTab | KeyCode::Left => app.prev_state(),

                    KeyCode::Char(':') => app.command_mode = true,

                    KeyCode::Char('r') => {
                        let action = if app.state == State::BuildFailed {
                            Action::RetryBuild
                        } else {
                            Action::Reload
                        };

                        apply(app, action);
                    }

                    KeyCode::Char('R') => apply(app, Action::Restart),

                    KeyCode::Char('m') => {
                        app.mouse_on = !app.mouse_on;
                        app.hover = None;

                        if app.mouse_on {
                            execute!(io::stdout(), EnableMouseCapture)?;
                        } else {
                            execute!(io::stdout(), DisableMouseCapture)?;
                        }
                    }

                    KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                    KeyCode::Up | KeyCode::Char('k') => app.select_prev(),

                    KeyCode::Enter => {
                        if app.state == State::NoDevices {
                            app.goto(State::Booting);
                        } else if app.state == State::MultipleDevices {
                            app.goto(State::Building);
                        }
                    }

                    _ => {}
                }
            }

            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(action) = app.hit_test(mouse.column, mouse.row) {
                        apply(app, action);
                    }
                }

                MouseEventKind::Moved => {
                    app.hover = app.hit_test(mouse.column, mouse.row);
                }

                MouseEventKind::ScrollDown => app.select_next(),
                MouseEventKind::ScrollUp => app.select_prev(),

                _ => {}
            },

            _ => {}
        }
    }
}

/// One path for keys and clicks alike, so the two cannot drift apart.
///
/// In the real build this is also where the byte is forwarded to the pty, which
/// is the reason it matters that nothing bypasses it: a click on `r` and a
/// press of `r` have to reach Flutter through the same call.
fn apply(app: &mut App, action: Action) {
    app.last_action = Some(action);

    match action {
        Action::Reload | Action::Restart => app.goto(State::ReloadInFlight),
        Action::RetryBuild => app.goto(State::Building),
        Action::StartDevice => app.goto(State::Booting),

        // Graceful: Flutter receives the key and shuts itself down (⏏).
        Action::Quit => {}

        // Interrupt: SIGINT is forwarded to the child (⏹). Distinct from Quit
        // in the existing implementation, and kept distinct here.
        Action::Stop => {}
    }
}
