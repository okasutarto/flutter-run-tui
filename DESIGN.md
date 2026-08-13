# 🖥️ Flutter CLI Terminal UI (TUI) - Architecture & Design Specification (`design.md`)

Document Version: `1.3.0`  
Last Updated: `2026-08-13`  
Design System: **Monospace Character Grid TUI (Terminal User Interface)**  
Target Font: `JetBrains Mono`, `Fira Code`, or `ui-monospace` (12px base)  
Canvas Dimensions: `106x45` character matrix target, widening to `142` columns
on a larger window and degrading progressively below the target (see 6.2).
Derived from the web mockup's 768px default and 1024px maximum at JetBrains
Mono 12px, where one cell measures ~7.2px. Columns are the unit of record
from here on; a terminal has no pixels.

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
* **Left Column**: the Flutter mark and nothing else, centred on both axes.

  A real image, not block art: the PNG is embedded with `include_bytes!` and
  rendered through whichever graphics protocol the terminal reports, falling
  back to halfblocks. Embedded rather than read from disk because frun runs from
  whichever project directory you are in, so a relative path is useless and an
  absolute one is fragile. `flutter-trim.png` and not `flutter.png`: the latter
  carries ~79px of transparent padding per side, which renders as dead columns.

  Five rows in an 11-column box, inside a column three columns wider. `Resize::Fit`
  scales to whichever dimension binds first, so at five rows the height binds and
  extra width goes unused.

  The `Flutter Engine` and `Cross-Platform CLI` labels this section used to
  specify are gone. Both restated what the mark and the metadata column beside
  it already said, and `Flutter Engine` was inaccurate as well: the versions
  shown are the framework and the Dart SDK, not the engine.
* **Right Column (8 Cols)**: Key-value table showing:
  * `Project`: `cwclub`
  * `Version`: `1.4.0+12`
  * `Branch`: `main`
  * `Git Status`: `✔ clean`
  * **3-Column Technical Stats Row**: `Flutter` (`3.27.1`), `Dart` (`3.7.2`), `Runtime` (`(FVM)`).
* **Row Separators**: a hairline rule between metadata rows, in the border
  colour. Costs 3 rows; see 6.2 for what that displaces.
* **Widths follow the terminal.** Values right-align to the card border and the
  separators span the block. An absolute content cap was tried and removed: at
  142 columns the border reached the window while the values stopped at column
  44, and the separators stopped with them, hanging in open space.
* **Technical stats row** is three columns spaced across the width: Flutter
  flush left, Dart centred, Runtime flush right. Not a single spread line, which
  pushed `Runtime (FVM)` to the border and cut it off from the two values it
  belongs with, and not one left-aligned group, which left two thirds of the row
  empty.
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
   * Title section: `Start a Device`, no category filter tabs.
   * Action trigger: `▶ Start` per row, transitioning to State 3.

   **Every target, not just mobile.** The existing implementation deliberately
   drops macOS, "Mac Designed for iPad" and Chrome, with the stated reason
   that frun exists to run on phones. That restriction is lifted:

   | Source | Targets |
   | :--- | :--- |
   | `emulator -list-avds` | Android AVDs |
   | `xcrun simctl list devices available -j` | shut-down iOS/iPadOS simulators |
   | `flutter devices --machine` | macOS desktop, Chrome, and any other platform Flutter reports |

   Note the asymmetry: emulators and simulators need booting, whereas desktop
   and web are always available and need no `▶ Start` at all. Those rows go
   straight to launch, skipping State 3.

   **Row height: roomy.** One row per target plus a separator, and no blank
   between them: the separator already divides one target from the next, so a
   blank on top of it spends a row per device saying the same thing twice. The
   last row gets no separator, since a rule directly above the card border
   reads as a stray line rather than a division.

   At that density around 12 targets are visible at once, so the list scrolls
   and carries a scrollbar. Dense drops the separators as well and is the
   fourth concession in the ladder (see 6.2).

   **Platform glyphs are Nerd Font, not emoji.** `U+F179` () for Apple and
   `U+F17B` () for Android. Emoji (🍎, 🤖) are East Asian Width Wide, so they
   occupy two cells and break the column grid, and terminal emoji rendering is
   inconsistent. `frun.zsh` already ships `U+F179` today.

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

* **Progress Bar**: filled bar with a step counter, but **no denominator**. A
  blank row separates it from the stage list.

  The bar itself is capped at 44 columns; the row it sits on is not, so the
  stage count still right-aligns to the border. A 118-column bar conveys nothing
  a short one does not and turns a glance into a scan.
  `Step 4`, not `Step 4/4`, and no percentage.

  The total is not knowable in advance. Stage count depends on platform, and
  Flutter skips stages: `pod install` is skipped when `Podfile.lock` is
  current, APK install is skipped when attaching to an already-installed app.
  Any denominator shown before the build finishes is a guess, and a progress
  bar that reaches 100% and then keeps working is worse than no bar.

  Once the build completes the count is known and can be stated
  (`4 stages · 3.4s`).

* **Failure State**: error summary, exit code, elapsed total, and which stage
  broke.

  **Code frame.** The error carries `lib/main.dart:42:18`, and Dart already
  emits the offending source line plus a caret, so that much is free
  passthrough. Showing the lines *around* it (41 and 43) means reading the
  file from disk at the reported line number. Worth it: one line of context
  either side is usually the difference between recognising the mistake and
  opening the editor.

  **Single action: `[r] Retry Build`.** This is not a keypress forwarded to
  Flutter. It kills the child in the pty, waits for it to be reaped, and
  spawns a fresh `fvm flutter run -d <id>` with all stage state reset. Hot
  restart is not a build retry and cannot substitute for one.

  Two notes. The Gradle daemon survives the kill, so a retry is usually much
  faster than the first build. And the failed build's log is kept rather than
  cleared, because comparing the two runs is the point.

  `r` is free in this state: there is no live Flutter session to hot reload,
  so the key carries no conflicting meaning.

  `[Fix with AI]` / `[f] Fix Code with AI Assistant` is removed. There is no
  such capability in this project, and a button that cannot act is worse than
  an absent one.

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

* **Application output only.** Build progress does not belong here. The design
  frames showed `[BLD] Running Xcode build...` and `[OK] Xcode build complete
  11.1s` in the log stream while `BuildPhaseTracker` was displaying the same
  two facts as steps, one screen, twice. The tracker owns build progress; this
  view owns what the application printed.

  Consequently the only levels are the ones the app itself produces:

  | Badge | Source |
  | :--- | :--- |
  | `INF` | `I/flutter (nnnn):` and plain application prints |
  | `WRN` | `W/` android log level, Flutter warnings, `printBox` notices |
  | `ERR` | `E/`, Dart exceptions, stack traces |

  `SYS`, `BLD` and `OK` are removed. Everything they carried is a build stage
  and already has a home.

  One deliberate exception: hot reload results (`⚡ Reloaded 125 of 1824
  libraries in 148ms`) stay in the stream. They happen while the app is
  running, they interleave with app output, and their position relative to
  surrounding log lines is the information.

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
The right-hand group is optional and priority-ordered, dropped whole until it
fits: mouse state, then what the layout gave up, then the prototype position.
Keys are never dropped. `spread` pads to fit and silently truncates past that,
so without this the tail of the cheatsheet simply disappeared on the running
state — and a cheatsheet that truncates is worse than a short one, because you
cannot tell whether the key you want is absent or just cut off.

Mouse state appears **only when capture is on**. Printing `mouse off` while off
is the default spends a permanent slot on a non-event; capture is worth naming,
because it takes text selection away from the terminal and the only other
symptom is that copying a stack trace quietly stops working.

`Flutter <version> CLI` was here and is gone. The version is already on the
ProjectCard, it changed no decision taken from the footer, and it cost 18
columns on the one row that must never truncate.

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

The layout adapts to the actual terminal size. `106x45` is a target, not a
floor and not a cap.

**The problem this has to solve.** Row separators (3.1), roomy device rows
(3.3) and the blank row under each card title are all worth having, but none
of them are free. Full chrome, enumerated from the rows the cards actually
draw rather than estimated:

```
  PROJECT INFO           12 rows   2 border + 1 title gap + 9 body
                                   body = max(metadata 6 + separators 3, logo 9)
  SELECTED TARGET        14 rows   2 border + 1 title gap + 11 body
                                   body = banner + blank + 4 fields + blank
                                          + command + 3 separators
  BUILD PHASE            10 rows   2 border + 1 title gap + bar + blank + 5 stages
  prompt bar              3 rows
  footer                  1 row
  gaps between blocks     5 rows   blocks - 1, not a constant
  ────────────────────────────────
  TOTAL                  45 rows
```

At the 106x45 target that leaves the log window **1 row**. A five-line Dart
exception, wrapped at 84 columns of message space, occupies **8 rows**. So a
single error would not fit on screen at the design's own target size, and full
detail now needs a 57-row window.

Two notes on the arithmetic, both learned by getting it wrong:

* The gap count is `blocks - 1`, because `Layout::spacing(1)` inserts a row
  *between* blocks. Hardcoding it undercounted the running view by one row.
* The title gap has to be charged here as well as applied in the card. Adding
  it to the card alone left the heights unchanged, so the padding ate into the
  inner area and the bottom of every card was clipped in silence — the target
  card lost its `Type` field and its command string, and the logo lost its
  label.

**Therefore the priority is inverted.** The log window is not what is left
over; it is a floor that the cards must yield to.

```
  LOG_MIN = 12 rows      enough for one wrapped exception plus context
```

Elements are given up in this order until the floor is met, cheapest first:

| Order | Given up | Reclaimed |
| :--- | :--- | :--- |
| 1 | Flutter logo in ProjectCard | 4 rows |
| 2 | Row separators in both cards | 6 rows |
| 3 | Interactive prompt bar (keys remain in the footer) | 4 rows |
| 4 | Device rows go dense, one line each | ~7 rows in States 2 and 4 |
| 5 | BuildPhaseTracker collapses to one summary line once the build finished | 9 rows |
| 6 | ProjectCard and SelectedTargetCard collapse to one metadata row each | ~19 rows |

Step 5 is the important one, and it is state-dependent rather than
size-dependent: after the build succeeds, every row the tracker occupies is
static. It has no reason to hold nine rows while the only region still
changing is starved.

Nothing below `60x14` is drawable. At that point the app says so rather than
rendering a broken grid.

**Width.** Cards stop widening at 142 columns so a very wide window does not
stretch a label to one edge and its value to the other. The log window is
exempt and takes every column available, because more columns means fewer
wrapped rows per entry.

---

## 🚧 7. Implementation status

### 7.1 Done

All eleven state frames render, at any terminal size, verified rather than
eyeballed. `src/` layout:

```text
  theme.rs     palette + Nerd Font glyph vocabulary
  data.rs      App state, the 11-variant State enum, mock data per state
  budget.rs    responsive ladder; owns every component height
  widgets.rs   pill, badge, keycap, card, spread, field, separator, elide, wrap
  dump.rs      TestBackend -> Buffer -> ANSI, plus hit probing and row reports
  ui/          mod (assembly), project, target, devices, build, logs, chrome, logo
```

Verification tooling, which exists because layout bugs here are silent:

```text
  --dump <state> [WxH]   render one frame to stdout
  --all [WxH]            every state in flow order
  --hits <state> [WxH]   probe every clickable region
  --rows <state> [W]     the degradation ladder across heights
  --states               list the slugs
  --demo                 walk the flow on a timer
```

18 tests. The two that matter most: no row in any state at any size exceeds
its width budget (measured in cells, not bytes), and every state fills exactly
the height it was given.

Crates: `ratatui`, `textwrap`, `unicode-width`, `ratatui-image`, `image`.
`ratatui-image` runs with default features off, because the default set
dynamically links libchafa.

### 7.2 Not done

**Every value on screen is static.** Nothing is read, nothing is spawned. No
pty, no `flutter` invocation, no git, no device query. `Tab` moves between
states because in a prototype nothing else can.

### 7.3 Remaining work, in order

**1. Project metadata.** `pubspec.yaml` for name and version, `git branch
--show-current` and `git status --porcelain` for branch and dirty count, and
`bin/cache/flutter.version.json` inside the FVM SDK for the Flutter and Dart
versions.

Read the manifest, not `fvm flutter --version --machine`: the latter boots the
Dart VM and costs 3-4 seconds, and `frun.zsh` already worked this out. Resolve
the SDK through `.fvm/flutter_sdk` if the symlink exists, otherwise the pin in
`.fvmrc` against `FVM_CACHE_PATH`.

Cheapest step and the first one that is verifiable in a real project. Scope is
one card.

**2. Device discovery.** `flutter devices --machine` through `serde_json`, plus
`adb -s <serial> emu avd name` to turn `emulator-5554` into a name that means
something, `emulator -list-avds` and `xcrun simctl list devices available -j`
for bootable targets.

Keep `sdkNameAndVersion`, `targetPlatform` and the `emulator` flag, all of
which `_frun_device_list` currently discards, because 3.2 needs them.

Brings four states to life at once: Detecting, NoDevices, MultipleDevices,
SingleDevice.

**3. pty and the Flutter parser.** The bulk of the work, and where all the
regression risk lives. `portable-pty` to spawn, `vte` to emulate the terminal
rather than stripping escapes with regexes, then the state machine from
`frun-runner`: platform-dependent stage detection, the hot reload ack/timeout
machine, `printBox` passthrough, startup log buffering.

`BUILD_FAILED` has to be built from nothing. `frun-runner` has no
build-failure detection at all: if Gradle dies the output falls through to raw
passthrough and the spinner simply stops.

**4. Boot.** `emulator -avd` with `sys.boot_completed` polling capped at 180
seconds, and `xcrun simctl bootstatus -b`. Depends on 2.

**5. Cut over.** Last, not first: all three shell files stay working until the
Rust version reaches parity, so `frun` keeps running throughout.

| File | Fate |
| :--- | :--- |
| `frun-runner` | deleted. All 1,748 lines are step 3. |
| `frun.zsh` | 1,201 lines down to a one-line shim. |
| `frun-theme.zsh` | deleted. `theme.rs` owns the palette. |
| `.frun-last-device` | kept, read and written by the Rust version. |

`frun.zsh` is not a caller that survives: every part of it is work that moves
into Rust. Reading pubspec, git and the SDK manifest is step 1; device discovery
and the picker is step 2; booting is step 4; `render_frun_header` is already
built. What is left is only the shim, and only because `frun` is invoked as a
command that `.zshrc` line 179 sources:

```zsh
frun() { "$HOME/.config/zsh/frun-tui/target/release/frun-tui" "$@" }
```

Even that is optional. `frun` changes no shell state — no `cd`, no exports — so
a binary on `PATH` would do and `frun.zsh` could go entirely. The shim is the
lighter choice only because `~/.cargo/bin` is deliberately not on `PATH`.

`frun-theme.zsh` exists solely because it had two consumers: `frun.zsh` sourced
it and `frun-runner` parsed its `FRUN_C_*` lines. With both gone nothing reads
it, and `theme.rs` already carries a newer palette.

`.frun-last-device` is the one file that must survive, currently holding
`emulator-5554`. It is data, not code, and reading the same path means the
remembered device is not lost in the migration — no reselecting a target on the
first run after the switch.

### 7.4 Two traps this codebase has already sprung

Both cost real debugging time. They will recur.

**Every drawn row must be charged in `budget.rs`.** The Layout splits fixed
blocks by those heights, so a row added to a card without a row added to the
budget is clipped in silence — no error, no warning, content simply gone. It
has happened three times: the title gap, the logo labels, and the blank under
the progress bar.

**A failed build leaves the old binary in place.** `cargo build` fails,
`./target/release/frun-tui` still runs, and the output looks stale rather than
broken. Use `cargo run --release --` so a compile error surfaces instead of
last-good output.
