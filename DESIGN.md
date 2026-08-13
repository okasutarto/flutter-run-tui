# 🖥️ Flutter CLI Terminal UI (TUI) - Architecture & Design Specification (`design.md`)

Document Version: `1.2.2`  
Last Updated: `2026-08-13`  
Design System: **Monospace Character Grid TUI (Terminal User Interface)**  
Target Font: `JetBrains Mono`, `Fira Code`, or `ui-monospace` (12px base)  
Canvas Dimensions: Character matrix bounding box (`max-w-3xl` / 768px default, responsive up to 1024px)

---

## 📐 1. Design Philosophy & Aesthetic Principles

### 1.1 The High-Density Terminal Grid
Application design follows strict Text User Interface (TUI) paradigms popularized by classic terminal environments (e.g., `htop`, `lazygit`, `k9s`, `fvm`).

* **Zero Ambient FX**: No drop shadows (`shadow-*`), backdrop blurs (`backdrop-blur-*`), or decorative radial gradients.
* **Pixel-Sharp Borders**: Structural separation relies entirely on 1px crisp borders (`border-zinc-800`, `border-zinc-700`) and solid background boxes (`bg-[#0c0e14]`, `bg-black/80`).
* **Strict Monospace Hierarchy**: Typography utilizes fixed-width character alignment across all columns, headers, tables, and gutter blocks.
* **Symbol & ASCII Art Accents**: Uses UTF-8 box characters and glyphs (`◆`, `✔`, `✖`, `❯`, `▶`, `⚡`, `⠋`) for structural guidance and status indication.

---

## 🎨 2. Color Palette & Visual Tokens

The color architecture uses a dark noir base with high-contrast functional color coding to instantly signal state transitions:

| Visual Token | Hex / Tailwind Class | Primary Usage |
| :--- | :--- | :--- |
| **Canvas Background** | `#0c0e14` (`bg-[#0c0e14]`) | Outer application container & dark grid frame |
| **Box Background** | `#000000` (`bg-black/80`) | Inset cards, logs container, tables background |
| **Default Text** | `#e4e4e7` (`text-zinc-200`) | Standard body copy, output logs, table keys |
| **Subdued Text** | `#71717a` (`text-zinc-500`) | Table labels, key hints, timestamps, borders |
| **Cyan Accent** | `#38bdf8` (`text-cyan-400`) | Main section headers (`◆`), selected items, active focus |
| **Emerald Success** | `#34d399` (`text-emerald-400`) | Active device status (`✔`), build success, completed steps |
| **Amber Warning** | `#fbbf24` (`text-amber-300`) | Git branch tags, no-device alert banner, pending steps |
| **Rose Error** | `#f87171` (`text-rose-400`) | Build/Hot Reload failures (`✖`), stack traces, pointers |
| **Purple Virtual** | `#c084fc` (`text-purple-300`) | Simulator/emulator badges, secondary runtime tags |

---

## 🧩 3. Core Component Architecture

The interface is organized into modular TUI blocks:

```
┌────────────────────────────────────────────────────────────────────────────┐
│ ProjectCard: 2-Column Info (Left: Graphic Logo | Right: Metadata Table)     │
├────────────────────────────────────────────────────────────────────────────┤
│ SelectedTargetCard: Active Device Banner & Target Details (States 6-11)     │
├────────────────────────────────────────────────────────────────────────────┤
│ DYNAMIC STATE FRAME CONTENT                                                 │
│                                                                             │
│  Discovery                                                                  │
│  ├─ State 1:  FlutterDeviceManager (DETECTING)                              │
│  ├─ State 2:  FlutterDeviceManager (NO_DEVICES - Launchable Targets)        │
│  ├─ State 3:  FlutterDeviceManager (BOOTING)                                │
│  ├─ State 4:  FlutterDeviceManager (MULTIPLE_DEVICES - picker)              │
│  └─ State 5:  FlutterDeviceManager (SINGLE_DEVICE - auto-selected)          │
│                                                                             │
│  Build                                                                      │
│  ├─ State 6:  BuildPhaseTracker (BUILDING)                                  │
│  └─ State 7:  BuildPhaseTracker (BUILD_FAILED + Stack Trace)                │
│                                                                             │
│  Run                                                                        │
│  ├─ State 8:  TerminalLogsView (RUNNING) + CLI Prompt                       │
│  ├─ State 9:  HotReloadView (IN_FLIGHT)                                     │
│  ├─ State 10: HotReloadView (FAILED + Quick Actions)                        │
│  └─ State 11: HotReloadView (DROPPED - keypress never acknowledged)         │
├────────────────────────────────────────────────────────────────────────────┤
│ Terminal Footer: Hotkey Cheatsheet only                                     │
└────────────────────────────────────────────────────────────────────────────┘
```

There is no top header bar. The command string it used to carry
(`fvm flutter run -d <id>`) belongs in the SelectedTargetCard, which is
already the component describing what is being run and where.

### 3.1 `ProjectCard` (2-Column Layout)
* **Left Column (4 Cols)**: Features a high-contrast Flutter Engine graphic logo badge (`my-2` vertical margin), version text (`Flutter Engine`), and `Cross-Platform CLI` subtitle without outer card borders.
* **Right Column (8 Cols)**: Key-value table showing:
  * `Project`: `cwclub`
  * `Version`: `1.4.0+12`
  * `Branch`: `main`
  * `Git Status`: `✔ clean`
  * **3-Column Technical Stats Row**: `Flutter` (`3.27.1`), `Dart` (`3.7.2`), `Runtime` (`(FVM)`).
* **Header Bar**: Includes path tag `~/cwclub` and a metadata `[COPY]` button.

### 3.2 `SelectedTargetCard`
* **Active Status Banner**: green box header
  `✔ 1 device active: iPhone 16 Pro (emulator) (iOS)`.
* **Target Details Table**: `Device Target`, `Platform ID`,
  `OS Version / Arch`, `Type` (Simulator / Hardware).
* **Command String**: the invocation being run, e.g.
  `fvm flutter run -d 8A3F91C2-4D2E`. This lands here now that the header bar
  is gone; this card is already the component describing what runs and where.

  Implementation note: `flutter devices --machine` does supply
  `sdkNameAndVersion`, `targetPlatform` and an `emulator` flag, so every field
  above is available. The existing `_frun_device_list` currently discards all
  of them, keeping only id, name, platform and icon.

### 3.3 `FlutterDeviceManager` (Device Selector)

Five operation modes. Discovery is not one screen with three variants, it is
a short sequence with branches, and the branch taken is decided by how many
devices answered.

1. **`DETECTING`** (State 1): animated Braille spinner (`⠋ ⠙ ⠹ ...`) while
   `fvm flutter devices --machine` runs. Always entered, always first.
   Nothing below it is reachable without passing through here.

2. **`NO_DEVICES`** (State 2): zero devices answered.
   * Header banner: `◆ NO DEVICE RUNNING` with subtitle
     `Nothing is attached. These can be started:`.
   * Title section: `Start a Device`, no category filter tabs, listing every
     launchable target: Android AVDs from `emulator -list-avds`, shut-down
     iOS simulators from `xcrun simctl list devices available -j`, plus the
     desktop and web targets Flutter reports.
   * Action trigger: `▶ Start` per row, transitioning to State 3.

3. **`BOOTING`** (State 3): `⠋ Booting <name>...`.
   * Android waits on `sys.boot_completed` and can legitimately take
     minutes; the existing implementation gives up at 180 seconds.
   * iOS uses `xcrun simctl bootstatus -b`, which blocks until ready.
   * This state needs an elapsed clock. A spinner alone cannot distinguish
     a slow boot from a hung one, and three minutes is long enough that the
     difference matters.

4. **`MULTIPLE_DEVICES`** (State 4): two or more devices answered.
   Interactive list, arrow keys (`↑↓`) and number hotkeys (`[1-N]`).
   The device from `.frun-last-device` is promoted to the top and carries a
   visible `last used` marker, so the reordering is explained rather than
   silent.

5. **`SINGLE_DEVICE`** (State 5): exactly one device answered.
   No picker is shown. The device is selected automatically and the flow
   goes straight to the SelectedTargetCard. There is nothing to choose, so
   asking would be a keystroke spent on a foregone conclusion.

   Same for a device that was just booted through State 3: it is already
   known, so it skips the picker and also suppresses the
   `N devices detected` line, which would otherwise name the same device a
   third time in a row.

### 3.4 `BuildPhaseTracker`
* **Progress Pipeline**: stage list built from what Flutter actually emits,
  and **platform-dependent** rather than a fixed sequence:

  | Trigger in Flutter output | Stage shown | Platform |
  | :--- | :--- | :--- |
  | `Launching lib/main.dart` | flutter started | both |
  | `Running pod install` | cocoapods | iOS |
  | `Running Xcode build` / `Xcode build done` | xcode build | iOS |
  | `Running Gradle task '<task>'` | gradle, with the task name | Android |
  | `Built build/...` | gradle complete | Android |
  | `Installing build/...` | app installed | Android |
  | `Syncing files to device` | files synced | both |
  | `Flutter run key commands` | interactive session ready | both |

  Durations are parsed from the same lines. Note Flutter formats elapsed time
  through `NumberFormat`, so values carry a group separator (`1,847ms`) and
  must not be matched with `[0-9]+`.

* **Timing Bar**: `Build Time` and `Sync Time` only. CPU and memory gauges are
  removed. Whose CPU is ambiguous, and reporting system-wide load would be
  decorative rather than informative; making it honest would mean resolving
  the Dart VM, Gradle daemon, Xcode and simulator process tree for a number
  that is rarely acted on.

* **Failure State**: error summary, stack trace, elapsed total, and which
  stage broke. Single action: `[Retry Build]`, which kills the pty and
  respawns `fvm flutter run` — hot restart is not a build retry.

  `[Fix with AI]` is removed. There is no such capability in the project.

### 3.5 `TerminalLogsView`
* **18-Column Strict Gutter System**:
  ```
  01 14:32:01 [INF] Log content message line...
  02 14:32:02 [ OK] ⚡ Reloaded 125 of 1824 libraries in 148ms.
  03 14:32:04 [ERR] The following assertion was thrown building
                    CheckoutScreen(dirty, dependencies: [_Inherited…
  ```
  `01␣14:32:01␣[INF]␣` measures 18 columns, not 16. Continuation rows leave
  the gutter empty and indent to the message column (see 6.1).

* **Clean Header**: section title (`◆ APP LOGS STREAM`) and a count
  (`[N entries]`). No search bar and no filter toggles.

  Dropping the filters also resolves a data problem: `FLUTTER` is detectable
  from the `I/flutter (12345):` prefix, but `SYSTEM` and `NET` have no source
  in Flutter's output and would have to be guessed.

### 3.6 `InteractivePrompt`
* **CLI Input Box**: `➜ ~/cwclub ❯` interactive text input with `[Enter]` submit button and popup autocomplete command suggestions.
* **Hotkey legend**: none here. Owned by the `TerminalFooter` (3.7), which
  keeps a single source of truth and survives this prompt being dropped on a
  short window.

### 3.7 `TerminalFooter`

Hotkey cheatsheet only. One row, always the last row, never scrolls.

No status bar and no grid metrics. Session time, mode indicator and grid
dimensions were all decoration: none of them change a decision you are about
to make, and the row is more useful spent on the keys that do.

Contents adapt to the active state, since a key that does nothing here should
not be advertised: `↑↓ Enter Esc` during discovery, `r R q ^C` while running.
Mouse state (`mouse on` / `mouse off`) sits at the right edge.

Note: this replaces the hotkey legend that 3.6 places under the prompt.
Listing the same keys twice on one screen is redundant, and the footer
survives the prompt being dropped on a short window (see 6.2).

### 3.8 `HotReloadView`
* **`HOT_RELOAD_IN_FLIGHT`**: Live progress indicator (`⚡ Syncing updated Dart libraries to iPhone 16 Pro...`).
* **`HOT_RELOAD_FAILED`**: Dart compilation error, affected file and line,
  and the reason the stage failed — Flutter reports the cause several lines
  before it prints its closing `Try again after fixing the above error(s)`,
  so the verdict is placed after the errors it refers to, not before.

  Recovery actions: `[Retry Hot Reload]`, `[Hot Restart]`.

  `[Undo Changes]` is removed. It would mean running `git checkout` or
  `git stash` against the working tree from inside a log viewer, where one
  misclick discards uncommitted work.

* **`HOT_RELOAD_DROPPED`** (State 11): `⚠ not picked up by Flutter — press r
  again`. Distinct from failure: the operation never started.

---

## 🔄 4. State Frames & User Interaction Flows

Derived from the existing implementation (`frun.zsh` + `frun-runner`), not
designed independently of it. Every box below already exists as code today,
with one exception noted at the end.

```
  frun invoked
       │
       ▼
  pubspec.yaml present? ──── no ──► ✖ FATAL  pubspec.yaml not found
       │ yes                        "Run frun from a Flutter project directory."
       ▼
  read project metadata          name, version (pubspec) · branch, dirty count (git)
       │
       ▼
  read SDK versions
    ├─ fast   .fvm/flutter_sdk or .fvmrc → bin/cache/flutter.version.json
    └─ slow   fvm flutter --version --machine   ⠋ 3-4s, boots the Dart VM
       │
       ▼
  ╔═══════════════════════╗
  ║ render ProjectCard    ║   needs the SDK versions, so it cannot paint
  ╚═══════════╤═══════════╝   before the step above resolves
              ▼
      ┌───────────────────┐
      │ 1  DETECTING      │  fvm flutter devices --machine
      └─────────┬─────────┘
                │
                ├── exits non-zero ──► ✖ FATAL  Failed to detect Flutter devices
                │
                ▼  how many answered?
    ┌───────────┼────────────────────────────┐
    │ 0         │ 1                          │ 2+
    ▼           ▼                            ▼
┌─────────┐ ┌─────────────────┐   ┌────────────────────┐
│ 2  NO_  │ │ 5  SINGLE_      │   │ 4  MULTIPLE_       │
│ DEVICES │ │ DEVICE          │   │ DEVICES  (picker)  │
└────┬────┘ │ auto-selected,  │   │ last used on top   │
     │      │ no picker       │   └─────┬─────────┬────┘
     │      └────────┬────────┘         │         │
     │ no bootable            ┌─ Esc ───┘         │ Enter
     │ targets at all         ▼                   │
     ├──► ✖ FATAL      ✖ CANCELLED (130)          │
     │    No device(s) detected                   │
     ▼                                            │
┌─────────────────┐                               │
│ 3  BOOTING      │  avd: poll sys.boot_completed, 180s cap
│ ⠋ + elapsed     │  sim: open -a Simulator, simctl bootstatus -b
└────┬───────┬────┘                               │
     │       └── timeout / failure ──► ✖ FATAL  did not finish booting
     │                                           │
     └───────────────┬───────────────────────────┘
                     ▼
              save .frun-last-device
                     │
                     ▼
        ╔════════════════════════╗
        ║ SelectedTargetCard     ║
        ╚════════════╤═══════════╝
                     ▼
      ┌──────────────────────────┐
      │ 6  BUILDING              │  pty spawn: fvm flutter run -d <id>
      │                          │
      │  stages are platform-    │  iOS      pod install → Xcode build
      │  dependent, not a fixed  │  Android  Gradle task → install APK
      │  list                    │  both     syncing files
      └────────┬─────────────────┘
               │
               ├── failure ──► ┌──────────────────────┐
               │               │ 7  BUILD_FAILED      │
               │               │ summary + stack trace│
               │               │ [Retry Build]        │
               │               └──────────────────────┘
               │
               ▼  "Flutter run key commands" → interactive session ready
      ┌──────────────────────────┐
      │ 8  RUNNING + LOGS        │◄──────────────────────────┐
      └────┬──────┬──────┬───────┘                           │
           │      │      │                                   │
        r/R│    q │   ^C │                                   │
           ▼      ▼      ▼                                   │
      ┌─────────────────┐  ⏏ QUIT        ⏹ STOP              │
      │ 9  IN_FLIGHT    │  graceful,     SIGINT              │
      │ ⚡ + elapsed     │  Flutter       forwarded           │
      └────┬───────┬────┘  exits itself                      │
           │       │                                         │
           │       ├── "Reloaded N of M in Xms" ─────────────┤ success
           │       │                                         │
           │       ├── "Try again after fixing" ──► ┌────────┴──────────┐
           │       │                                │ 10 RELOAD_FAILED  │
           │       │                                │ [Retry] [Restart] │
           │       │                                └───────────────────┘
           │       │
           │       └── no acknowledgement within 4s ──► ┌──────────────────┐
           │                                            │ 11 DROPPED       │
           └────────────────────────────────────────────┤ ⚠ press r again  │
                                                        └──────────────────┘
```

### 4.1 Notes on the branches

**State 11 exists because Flutter lies by omission.** A keypress is a
request, not a fact. When Flutter is busy with a previous command it drops
the key with an internal trace that never reaches stdout, and when the run
mode cannot hot restart, `R` silently returns false. So the only honest
signal is Flutter's own `Performing hot reload...` progress message. If that
never arrives, the operation was never accepted, and the spinner needs a way
out. Without State 11 it runs forever.

**State 7 is the one box that does not exist yet.** `frun-runner` handles
hot reload failure but has no build-failure detection at all: if Gradle
dies, the output falls through to raw passthrough, the spinner just stops,
and there is no verdict, no elapsed total, and no indication of which stage
broke. This is new parsing work, not new UI.

**`q` and `^C` are different exits.** `q` is graceful: Flutter receives the
key and shuts itself down (`⏏`). `^C` is an interrupt, and SIGINT is
forwarded to the child (`⏹`). The existing implementation already
distinguishes them and the UI should keep doing so.

**Boot is a dead end today, and should not be.** After a successful boot the
existing flow proceeds with a one-row device list it built itself, rather
than re-querying Flutter, because `flutter devices --machine` costs several
seconds of Dart VM startup while `adb` answers instantly.

---

## ⌨️ 5. Global Keyboard Shortcuts Cheatsheet

The frame switcher hotkeys (`1`-`7`) are removed along with the header. State
is decided by what Flutter is doing, so there is no `4` that makes a build
fail. Keeping them would also cost seven keys that must otherwise reach
Flutter untouched.

| Key Binding | Target Action | Scope |
| :--- | :--- | :--- |
| `↑` / `↓` | Navigate target list | States 2, 4 |
| `Enter` | Select device, or start the highlighted target | States 2, 4 |
| `Esc` | Cancel and exit (code 130) | States 2, 4 |
| `r` | Hot reload | States 8, 10, 11 |
| `R` | Hot restart | States 8, 10, 11 |
| `q` | Quit gracefully — Flutter shuts itself down (`⏏`) | States 6, 8-11 |
| `^C` | Stop — SIGINT forwarded to Flutter (`⏹`) | Any |
| `m` | Toggle mouse capture | Global |
| `:` | Enter command mode | States 8-11 |

### 5.1 Key forwarding

Every key not listed above is forwarded to Flutter verbatim. This is not a
detail: Flutter has its own interactive commands (`h` help, `d` detach,
`c` clear, `p` debug paint, `o` platform toggle, `w` widget tree, and more),
and intercepting them would silently remove functionality that works today.

Consequently the command prompt is **modal**. In NORMAL mode keystrokes go
to Flutter; `:` opens the input line and takes them back. A prompt that
captures keys at all times cannot coexist with key forwarding.

### 5.2 Mouse

Default **off**. Capturing the mouse takes text selection away from the
terminal, and copying a stack trace out of the log window is a large part of
what that window is for. `m` enables it when scroll wheel or clickable
controls are wanted; the footer shows which state it is in.

---

## 📝 6. Typography & Box Drawing Rules

1. **Font Settings**: `font-family: 'JetBrains Mono', monospace; font-size: 12px; line-height: 1.5;`.
2. **Text Selection**: belongs to the terminal, not the app. Mouse capture is
   off by default (see 5.2). The earlier `select-none` rule is dropped: it
   contradicts having a log window whose contents you need to copy.
3. **No Overlapping Text**: gutter widths are strictly bounded so labels never
   wrap or truncate mid-word.
4. **Border Nesting Math**: outer container corners are square
   (`rounded-none`) to uphold the retro terminal aesthetic.

### 6.1 Long log lines wrap

Real Flutter output runs 80-130 characters per line. At a 100-column canvas,
after borders, padding and the 18-column gutter, roughly 78 columns remain,
so a framework assertion loses about 50 characters off the end.

Those lines **wrap**, continuation rows indented to the message column with
the gutter left empty. Truncating is not acceptable for a stack trace, which
is exactly the output you most need to read in full.

Two consequences to design around:

* A single Dart exception can occupy 6-10 rows once wrapped, so the log
  window must be sized on the assumption that one entry is not one row.
* Consecutive lines from the same source share a timestamp and level. Those
  repeat the 18-column gutter for no information. Continuation rows omit it.

### 6.2 Responsive degradation

The `100x45` canvas is a target, not a hard floor. Below it the layout
shrinks progressively rather than refusing to draw, cheapest element first:

| Below | Dropped | Reclaimed |
| :--- | :--- | :--- |
| 40 rows | Flutter logo in ProjectCard | 4 rows |
| 33 rows | interactive prompt bar (keys stay in the footer) | 4 rows |
| 29 rows | ProjectCard and SelectedTargetCard collapse to one metadata row each | ~16 rows |

The log window is always the last thing to give up space, because it is the
only region whose contents are still changing.
