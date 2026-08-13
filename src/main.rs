//! frun-tui — design prototype for the frun rewrite.
//!
//!   frun-tui                    interactive, alternate screen
//!   frun-tui --dump 100x46      render the dashboard to stdout
//!   frun-tui --dump-stream 100x24
//!   frun-tui --rows             row budget report
//!
//! Static data. The pty, the Flutter parser and device discovery are not
//! part of this prototype.

mod data;
mod dump;
mod theme;
mod ui;
mod widgets;

use std::io::{self, Write};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use data::{Action, App, Phase};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = App::mock();

    match args.first().map(String::as_str) {
        Some("--rows") => {
            report_rows();
            Ok(())
        }

        Some("--dump") => {
            let (w, h) = size(args.get(1), 100, 46);
            print!("{}", dump::dump(&mut app, w, h));
            Ok(())
        }

        Some("--dump-stream") => {
            let (w, h) = size(args.get(1), 100, 24);
            app.phase = Phase::Streaming;
            print!("{}", dump::dump(&mut app, w, h));
            Ok(())
        }

        Some("--hits") => {
            let (w, h) = size(args.get(1), 100, 46);
            print!("{}", dump::hits(&mut app, w, h));
            Ok(())
        }

        // Same layout, data shaped like the real tools emit. This is the
        // difference between a design that works and a design that
        // photographs well.
        Some("--stress") => {
            let (w, h) = size(args.get(1), 100, 46);
            let mut app = App::stress();
            print!("{}", dump::dump(&mut app, w, h));
            Ok(())
        }

        Some("--stress-stream") => {
            let (w, h) = size(args.get(1), 100, 24);
            let mut app = App::stress();
            app.phase = Phase::Streaming;
            print!("{}", dump::dump(&mut app, w, h));
            Ok(())
        }

        _ => run(app),
    }
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

fn report_rows() {
    println!(
        "dashboard, full          {} rows of chrome",
        ui::DASHBOARD_CHROME
    );
    println!("  project card             9   (logo)");
    println!("  target + controls        9");
    println!("  build phase              7");
    println!("  prompt bar               3");
    println!("  footer                   1");
    println!("  gaps                     5");
    println!();
    println!("degradation, cheapest first");
    println!("  below 40 rows            logo dropped        -4");
    println!("  below 33 rows            prompt bar dropped  -4");
    println!("  below 29 rows            falls back to the streaming view");
    println!();
    println!("resulting log window");

    for h in [20u16, 24, 29, 33, 40, 46, 52] {
        let (logs, note) = if h < 29 {
            (h - 5, "streaming fallback")
        } else if h < 33 {
            (h - 26, "no logo, no prompt")
        } else if h < 40 {
            (h - 30, "no logo")
        } else {
            (h - ui::DASHBOARD_CHROME, "full")
        };

        println!("  at {h:>2} rows -> {logs:>2} rows   {note}");
    }

    println!();
    println!("streaming chrome         5 rows (meta, 2 rules, status, footer)");
    println!("  at 46 rows -> 41 rows   8.2x the full dashboard");
}

fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let result = event_loop(&mut app);

    // Restore before propagating any error, so a failure inside the loop
    // cannot leave the terminal in raw mode or holding the mouse.
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;

    result?;

    // Leaving the alternate screen normally discards everything the app
    // drew. Replaying the log buffer here is the mitigation discussed for
    // the real build: filtering and scrolling while running, and a plain
    // transcript in scrollback afterwards.
    let mut out = io::stdout();

    writeln!(out)?;
    writeln!(
        out,
        "\x1b[2m── transcript ─────────────────────────────\x1b[0m"
    )?;

    for log in &app.logs {
        writeln!(out, "{}  {:<10}{}", log.time, log.source, log.message)?;
    }

    Ok(())
}

fn event_loop(app: &mut App) -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        if !event::poll(std::time::Duration::from_millis(120))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Tab => app.toggle_phase(),

                KeyCode::Char('r') => {
                    if app.apply(Action::Reload) {
                        return Ok(());
                    }
                }

                KeyCode::Char('R') => {
                    if app.apply(Action::Restart) {
                        return Ok(());
                    }
                }

                // Hand the mouse back to the terminal.
                //
                // Capturing the mouse takes native text selection away: the
                // terminal forwards drags to the app instead of painting a
                // selection, so copying a stack trace out of the log stops
                // working. That matters more for frun than for most TUIs,
                // because copying error text is a large part of what the log
                // window is for. So it is a toggle, not a setting.
                KeyCode::Char('m') => {
                    app.mouse_on = !app.mouse_on;
                    app.hover = None;

                    if app.mouse_on {
                        execute!(io::stdout(), EnableMouseCapture)?;
                    } else {
                        execute!(io::stdout(), DisableMouseCapture)?;
                    }
                }

                KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
                KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
                _ => {}
            },

            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(action) = app.hit_test(mouse.column, mouse.row) {
                        if app.apply(action) {
                            return Ok(());
                        }
                    }
                }

                MouseEventKind::Moved => {
                    app.hover = app.hit_test(mouse.column, mouse.row);
                }

                // The most useful thing the mouse buys us, and the one the
                // design did not ask for: a log window you can wheel
                // through without leaving the keyboard home row behind.
                MouseEventKind::ScrollDown => app.scroll_down(),
                MouseEventKind::ScrollUp => app.scroll_up(),

                _ => {}
            },

            _ => {}
        }
    }
}
