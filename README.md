<div align="center">
  <img src="assets/flutter-trim.png" width="72" alt="flutter-run-tui logo">

  # flutter-run-tui

  *A fast terminal UI for `flutter run` — instant device picker, live build stages, and clean app logs.*

  ![Rust](https://img.shields.io/badge/Rust-2021-000000?logo=rust&style=flat-square)
  ![ratatui](https://img.shields.io/badge/ratatui-0.30.2-38bdf8?style=flat-square)
  ![Platform](https://img.shields.io/badge/platform-macOS-71717a?style=flat-square)
  ![Flutter](https://img.shields.io/badge/Flutter-FVM%20or%20plain%20SDK-02569B?logo=flutter&style=flat-square)

  [Features](#features) • [Requirements](#requirements) • [Install](#install) • [Usage](#usage) • [Keyboard](#keyboard) • [Screens](#screens) • [Development](#development)
</div>

`flutter run` outputs a noisy, scrolling wall of text where device selection, build stages, timing metrics, and application logs fight for the same lines. **frun** organizes everything into clean, dedicated cards: a project status card, target device details, a real-time build stage tracker, and an interactive log viewer — while proxying Flutter through a pty so all interactive shortcuts remain fully functional.

<div align="center">
  <img src="assets/screens/03-running.png" alt="frun during a live run" width="760">
  <br>
  <sub>Live run on iOS Simulator: project card, device card, build timings, and streaming app logs.</sub>
</div>


## Features

- **⚡ Fast Device Picker (<200ms):** Queries `adb`, `emulator`, and `simctl` directly to display running and bootable targets instantly, rather than waiting 6–9s on `flutter devices`.
- **🚀 Boot on Enter:** Select any shut-down simulator or AVD and press `Enter` to boot it and start the build automatically.
- **⏱️ Honest Build Timings:** Separate tracking for toolchain startup time (Dart VM & `flutter_tools`) and actual compilation time (Gradle/Xcode).
- **🔄 Smart Device Switching (`^D`):** Switch targets mid-session without exiting. Press `⇧Enter` to launch another device in a separate Ghostty or tmux tab.
- **📜 Ergonomic Log Window:** Word-wrapped logs, keyboard/mouse scrolling (`j`/`k`, arrows, wheel), fullscreen log toggle (`e`), and transcript replay into your terminal history on exit.
- **🎯 Full Interactive Forwarding:** Hot reload (`r`), hot restart (`R`), and all native Flutter shortcuts (`h`, `c`, `p`, `o`, `w`, `d`) pass through directly.

## Requirements

| Requirement | Details |
| :--- | :--- |
| **Rust** | Stable toolchain (2021 edition) |
| **Flutter** | `flutter` on `PATH` or managed via [FVM](https://fvm.app) (auto-detected) |
| **Android / iOS** | `adb` & `emulator` (Android) / Xcode CLI tools with `xcrun simctl` (iOS) |
| **Font** | Nerd Font (JetBrains Mono, Fira Code) for status glyphs and platform icons |
| **Terminal** | Any terminal (Ghostty recommended for image graphics & Kitty keyboard protocol) |

> [!NOTE]
> Tested primarily on macOS. Android emulation and tmux multi-tab workflows are cross-platform.

## Install

```bash
# Clone and build
git clone https://github.com/okasutarto/flutter-run-tui.git ~/.config/zsh/flutter-run-tui
cd ~/.config/zsh/flutter-run-tui
cargo build --release

# Symlink to your PATH
ln -s "$PWD/target/release/frun-tui" ~/.local/bin/frun
```

**Optional zsh function:**
```zsh
# ~/.zshrc
frun() {
  local bin="$HOME/.config/zsh/flutter-run-tui/target/release/frun-tui"
  [[ -x $bin ]] || { print -u2 "frun: not built yet (run cargo build --release)"; return 127 }
  "$bin" "$@"
}
```

## Usage

Run `frun` in any Flutter project containing a `pubspec.yaml`. All command-line arguments are forwarded to `flutter run`:

```bash
frun                                    # Pick device, build, and run
frun --flavor staging                   # Forward flavor flag
frun --dart-define=API=http://localhost # Forward dart-defines
```

### Exit Codes

| Code | Meaning |
| ---: | :--- |
| `0` | Normal exit or graceful quit (`q` / `^S`) |
| `1` | Fatal error (missing `pubspec.yaml`, boot failure, discovery error) |
| `130` | Device selection cancelled with `Esc` |
| *Flutter's* | Direct exit code from Flutter build failure |

## Keyboard Shortcuts

The footer cheatsheet automatically adapts to the current state.

| Key | Action | Context |
| :--- | :--- | :--- |
| `↑` `↓` / `1`-`9` | Navigate / select row | Device picker |
| `Enter` | Launch build or select target | Device picker |
| `⇧Enter` | Launch device in a new tab (Ghostty / tmux) | Device picker |
| `Esc` | Cancel (exit 130) or return to live session | Picker / Switch |
| `r` | Hot reload / retry failed build | Running / Stopped / Build Failed |
| `R` | Hot restart | Running |
| `j` `k` / `↑` `↓` | Scroll log viewer | Log view |
| `e` | Expand/collapse log viewer fullscreen | Log view |
| `^D` | Open device switcher over active run | Active run |
| `^S` | Stop run (retains frame and log) | Active run |
| `q` | Quit gracefully (shuts down Flutter process) | Active run |
| `^C` | Force quit (SIGINT) | Anywhere |
| `m` | Toggle mouse capture | Anywhere |



## Toolchain & Configuration

### SDK Detection
`frun` automatically detects your Flutter SDK without manual configuration:
1. `FRUN_FLUTTER` environment variable (if set)
2. Project-level `.fvmrc` or `.fvm/` directory with `fvm` on `PATH`
3. Global `flutter` on `PATH`

Custom runners can be supplied via `FRUN_FLUTTER`:
```bash
FRUN_FLUTTER='mise exec flutter --' frun
FRUN_FLUTTER=puro frun
```

### Environment Variables

| Variable | Description |
| :--- | :--- |
| `FRUN_FLUTTER` | Override Flutter command (e.g. custom wrapper or SDK path). |
| `FRUN_NO_QUERY` | Disable terminal capability queries (useful in bare PTYs/environments that hang on stdin). |
| `TMUX` | When present, `⇧Enter` opens a new tmux window instead of a Ghostty tab. |
| `FVM_CACHE_PATH` | Custom path to FVM cache directory. |

### Custom Palette
Themes are defined semantically in [`src/theme.rs`](src/theme.rs) (default: **Cyberpunk Neon**):
- **Cyan (`#34EDF3`):** Focus and informative highlights.
- **Lime (`#B8FF6A`):** Success and active status.
- **Yellow (`#FFE66D`):** Warning and pending operations.
- **Magenta (`#F715AB`):** Errors and build failures.
- **Purple (`#CC4DFF`):** Virtual devices and runtime badges.

## Screens

<div align="center">
  <img src="assets/screens/01-picker.png" alt="Device Picker" width="48%">
  <img src="assets/screens/02-building.png" alt="Build Tracker" width="48%">
</div>
<br>
<div align="center">
  <img src="assets/screens/05-switch.png" alt="Device Switcher" width="48%">
  <img src="assets/screens/03-running.png" alt="Log Stream" width="48%">
</div>

## Development & Testing

```bash
cargo build --release
cargo test --release            # 77 unit tests, 6 integration tests
cargo clippy --all-targets
```

### Diagnostic Flags
`frun` includes headless diagnostic and testing flags:

| Flag | Purpose |
| :--- | :--- |
| `--probe` | Output detected project metadata, SDK, and device lists. |
| `--dump <state> [WxH]` | Render a specific TUI state directly to stdout as ANSI text. |
| `--all [WxH]` | Render every visual state sequentially. |
| `--rows <state> [W]` | Test responsive layout degradation across terminal heights. |
| `--demo` | Play an automated visual demo of the full lifecycle. |

## Architecture & Further Reading

For deep implementation details, layout budget rules, parser nuances, and design rationale:
- Read [`DESIGN.md`](DESIGN.md) — complete specification and architectural guide.
