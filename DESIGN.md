# 🖥️ Flutter CLI Terminal UI (TUI) - Architecture & Design Specification (`design.md`)

Document Version: `1.6.0`  
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

   **Superseded, see 7.6.** One attached device turned out not to mean no
   choice: it means no other *running* device, and booting is a choice that this
   mode makes unreachable. The picker now shows running devices and bootable
   targets in one list, always, with the last-used device preselected.

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

### 3.6 `InteractivePrompt` — removed

This specified a `➜ ~/cwclub ❯` text input with a submit button and popup
autocomplete. It is gone, and the rows are the log window's.

It had nothing to command. Flutter's interactive session reads **single
keypresses**, not lines, so a typed string could not be forwarded to it: sending
`quit` would have delivered `q` and quit on the first character. The only safe
version was frun parsing a private vocabulary of its own — and every command
worth having (`r`, `R`, `q`) is already a single key that works without opening
a prompt first, and is already listed in the footer.

So the box cost three rows plus its separating gap to duplicate four keycaps and
offer autocomplete for a command set of three. That is four rows off the one
region on screen that is permanently short of them: at the 106x45 target the log
window went from 12 rows to 19, and the ProjectCard logo stopped being conceded
to pay for it (see 6.2).

The modal-input design in 5.1 goes with it. Key forwarding is no longer in
tension with anything, so there is no NORMAL mode and no `:` to leave it.

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

This is now the only place keys are listed. The prompt bar that used to carry a
second copy is gone (3.6), which also removes the reason this section once had to
explain why it was not a duplicate.

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
| `1`-`9` | Select the nth target directly | States 2, 4 |
| `Enter` | Select device, or start the highlighted target | States 2, 4 |
| `Esc` | Cancel and exit (code 130) | States 1, 2, 4 |
| `r` | Hot reload | States 8, 10, 11 |
| `r` | Retry build — kill, reap, respawn | State 7 |
| `R` | Hot restart | States 8, 10, 11 |
| `q` | Quit gracefully — Flutter shuts itself down (`⏏`) | States 6, 8-11 |
| `^C` | Stop — SIGINT forwarded to Flutter (`⏹`) | Any |
| `m` | Toggle mouse capture | Global |

`:` is not bound. It used to open the command prompt, which is gone (3.6), so the
key now forwards to Flutter like any other.

The digit keys are scoped to the two states that show a list. Everywhere else a
digit is Flutter's and has to arrive unchanged.

### 5.1 Key forwarding

Every key not listed above is forwarded to Flutter verbatim. This is not a
detail: Flutter has its own interactive commands (`h` help, `d` detach,
`c` clear, `p` debug paint, `o` platform toggle, `w` widget tree, and more),
and intercepting them would silently remove functionality that works today.

There is no mode. The earlier design needed one because a permanently focused
text input cannot coexist with key forwarding; removing the input removed the
conflict rather than managing it.

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
                                   body = metadata 6 + separators 3
                                   the logo shares these rows, it does not add any
  SELECTED TARGET        14 rows   2 border + 1 title gap + 11 body
                                   body = banner + blank + 4 fields + blank
                                          + command + 3 separators
  BUILD PHASE            10 rows   2 border + 1 title gap + bar + blank + 5 stages
  footer                  1 row
  gaps between blocks     4 rows   blocks - 1, not a constant
  ────────────────────────────────
  TOTAL                  41 rows
```

At the 106x45 target that leaves the log window **4 rows**. A five-line Dart
exception, wrapped at 84 columns of message space, occupies **8 rows**. So a
single error still would not fit on screen at the design's own target size, and
the cards have to yield.

It was 45 rows, leaving nothing at all, until the prompt bar and its gap were
removed (3.6).

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
| 1 | Row separators in both cards | 6 rows |
| 2 | Device rows go dense, one line each | ~7 rows in States 2 and 4 |
| 3 | BuildPhaseTracker collapses to one summary line once the build finished | 9 rows |
| 4 | ProjectCard and SelectedTargetCard collapse to one metadata row each | ~19 rows |

**The logo is no longer a rung, and never should have been.** It was step 1, on
the stated grounds that it freed four rows. It freed none: the artwork occupies
the same row range as the metadata beside it, so the card's height is
`max(metadata, logo)` and metadata is never the smaller of the two. The ladder's
cheapest step therefore spent the mark and bought nothing, then took the
separators on the next pass anyway. Every rung must now reclaim rows in the state
it applies to, and there is a test that says so.

The logo goes when the whole card collapses, at step 4, and on a window too
narrow to afford 14 columns beside the metadata. Those are the two cases where it
actually costs something.

Step 3 is the important one, and it is state-dependent rather than
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

### 7.1 Status

All eleven state frames render at any terminal size, and every value on them is
read from the machine.

**What that claim rests on.** Three different levels of evidence, kept apart
because they are not worth the same. *Live* means it ran against a real device on
a real project and the screen was read. *Tested* means unit tests cover the logic
but no device has exercised it. *Unrun* means the code compiles and has never
executed.

| Area | Evidence | Notes |
| :--- | :--- | :--- |
| Layout, all 11 states, 6 sizes | live + tested | `--dump`/`--all`; two tests measure every row in cells |
| Degradation ladder | tested | `--rows`; a test requires each rung to reclaim rows |
| Click regions | tested | `--hits` probes every rectangle it registered |
| 1. Project metadata | live | `--probe` on a real project; every field cross-checked against `pubspec.yaml`, `git`, the FVM manifest |
| 2. Device discovery | live | 5 devices reported, `emulator-5554` resolved to its AVD name, branch into `SINGLE_DEVICE` and `MULTIPLE_DEVICES` both seen |
| 3. pty + build parser | live | Gradle build to `build finished`, stage durations, 908 log lines classified |
| 3. Hot reload / restart | live | `Reloaded 0 libraries in 90ms`, `Restarted application in 4,072ms` |
| 3. `BUILD_FAILED` | live | `BUILD ERROR` card, exit code 1, retry action registered |
| 3. `RELOAD_FAILED` (10) | tested | needs a Dart compile error introduced mid-session |
| 3. `RELOAD_DROPPED` (11) | tested | needs Flutter to silently swallow a keypress, which is not reliably forceable |
| 4. Boot, and `NO_DEVICES` (2) | live | AVD booted through `sys.boot_completed`, name mapped back to `emulator-5554`, picker skipped, straight into the build |
| 5. Shell cutover | live | `frun` through the shim: flag passthrough, fatal path, exit codes 0 / 1 / 130 |
| Mouse capture | unrun | `m` toggles it; only the geometry is covered, by `--hits` |

Two of these are worth naming as gaps rather than leaving in a table. States 10
and 11 are the subtlest logic in the parser — the ack/timeout machine — and
neither has run against real Flutter output. Both are ported line for line from
`frun-runner`, where they have been in daily use, which is the argument for
believing them; it is not the same as having seen them.

What the live runs covered, for the record: a picker with 5 devices and a
`NO_DEVICES` screen with 16 targets, a booted AVD and an attached-emulator run, a
Gradle build reaching `build finished` in 10.6s, 908 log lines in one session, a
hot reload, a hot restart, a build failure, `Esc` at the picker, and `q` from the
log stream. Ten of the eleven state frames have been on screen with real data
behind them; `RELOAD_DROPPED` is the one that has not.

`src/` layout:

```text
  theme.rs     palette + Nerd Font glyph vocabulary
  data.rs      App state, the 11-variant State enum, mock data per state
  budget.rs    responsive ladder; owns every component height
  widgets.rs   pill, badge, keycap, card, spread, field, separator, elide, wrap
  probe.rs     pubspec, git, FVM manifest, device discovery, boot
  flutter.rs   pty session + the Flutter output parser
  dump.rs      TestBackend -> Buffer -> ANSI, plus hit probing and row reports
  ui/          mod (assembly), project, target, devices, build, logs, chrome, logo
```

Verification tooling, which exists because the failures here are silent — a
clipped row draws no error, and a device query that answers wrongly still answers:

```text
  --dump <state> [WxH]   render one frame to stdout
  --all [WxH]            every state in flow order
  --hits <state> [WxH]   probe every clickable region
  --rows <state> [W]     the degradation ladder across heights
  --states               list the slugs
  --demo                 walk the flow on a timer
  --probe                report what the machine actually answered
```

`--probe` is the counterpart to `--dump`: that one checks the layout without a
device, this one checks discovery without a terminal.

34 tests. The ones that matter most: no row in any state at any size exceeds its
width budget (measured in cells, not bytes), every state fills exactly the height
it was given, the log gutter is the same width at every entry count, and every
rung of the degradation ladder reclaims rows where it applies.

Crates: `ratatui`, `textwrap`, `unicode-width`, `ratatui-image`, `image`,
`serde_json`, `portable-pty`. `ratatui-image` runs with default features off,
because the default set dynamically links libchafa.

Two crates the plan named and the code does without. `vte`, because emulating a
terminal is more code than replaying backspaces, and there is no cursor
addressing to honour — the UI redraws itself from state, so all that is wanted
from the byte stream is its text. And no regex engine: `strip_prefix`, `split`
and `find` cover every pattern `frun-runner` used one for, including the
group-separated `1,847ms`.

### 7.2 Known limits

Not defects, but the places where the implementation stops short on purpose.

**`Esc` during `DETECTING` exits, it does not cancel.** `flutter devices
--machine` runs to completion on its worker thread either way; there is no way to
interrupt a spawned Dart VM that is cheaper than letting it finish and dropping
the answer.

**A `SIGTERM` from outside leaks the child.** `q` and `^C` both reap Flutter, and
so does a normal return from the event loop. Being killed outright skips all
three, and `fvm` survives the pty hangup that follows. A signal handler would fix
it; nothing in normal use reaches that path.

**A booted emulator outlives frun, but not the terminal.** `emulator -avd` runs
under `nohup`, so quitting frun leaves it attached and the next run finds it
already there — verified. Destroying the terminal itself immediately afterwards
still takes it down, and ignoring SIGHUP is not enough to prevent that. Booting
again is the only cost, and no interactive use reaches this: you quit frun and
keep your window.

**Log history is capped at 4000 entries**, dropping the oldest 1000 when it
fills. A day-long run would otherwise grow without bound. `flutter logs` is the
archive.

**One attached device used to win outright.** That was 3.3 mode 5 as specified,
and it is now overturned: it traps you on whichever platform happens to be
running. See 7.6.

### 7.3 Crates

What is in the tree, and what was declined.

| Crate | For |
| :--- | :--- |
| `ratatui` + `ratatui-image` + `image` | rendering, and the logo as a real image |
| `textwrap` + `unicode-width` | log wrapping and column arithmetic |
| `serde_json` | `flutter devices --machine`, `simctl list -j`, `flutter.version.json`, `.fvmrc` |
| `portable-pty` | spawning `fvm flutter run` behind a pty |

**Declined after the plan named them.** Recorded so they are not reconsidered
from scratch:

* `vte` — the plan wanted it for terminal emulation over the pty stream. But the
  UI redraws itself from state, so nothing here honours cursor addressing; all
  that is wanted from the stream is its text. Implementing `Perform` is more code
  than replaying `\b` and dropping escape sequences, which is ~40 lines ported
  from an implementation that has been doing it in production for months.
* `regex` — every pattern turned out to be `starts_with`, `contains`, `split` or a
  digit scan. The one genuinely fiddly case, Flutter's group-separated `1,847ms`,
  is a hand-rolled scan with a test pinning it.
* `anyhow` — the fallible surface is small and each error has exactly one
  consumer, which either shows it in a card or exits. `Result<_, String>` and
  `Option` cover it.
* `serde`'s derive — three JSON shapes read a handful of fields each, so
  `Value::get` is less code than the structs would be.

**`ansi-to-tui` is not on that list, on reflection.** It converts ANSI-coloured
bytes into styled `Span`s, which would be needed only if Flutter's own colours
were preserved verbatim. Nothing here wants that: every log line is recoloured
into INF/WRN/ERR from our palette, and the BUILD_FAILED code frame is rebuilt
with our own styling.

The evidence is in the implementation being replaced. `frun-runner` strips every
escape Flutter emits, via `ANSI_RE` and `LITERAL_ANSI_RE`, and recolours from
scratch. That has been the behaviour all along and it has never been a
complaint. `flutter::clean` does the same stripping directly, which is the whole
of what was wanted from a terminal emulator here.

If it is ever wanted, it fits: 8.0.1 depends on `ratatui-core ^0.1`, which is
what ratatui 0.30.2 is built on. Adding it now would be a dependency for a
decision not yet taken.

**Deliberately not used.** Recorded so they are not reconsidered from scratch:

* `serde_yaml` — deprecated, its version literally reads `0.9.34+deprecated`.
  Only two fields are needed from `pubspec.yaml`, `name` and `version`, which is
  a six-line parse rather than a whole YAML dependency of uncertain future.
* `git2` / `gix` — for `branch --show-current` and `status --porcelain` this is
  enormous. `git2` builds libgit2; `gix` adds dozens of crates. Shell out.
* `tokio` — two threads and one channel is the whole concurrency requirement.
  Async buys nothing here and costs a runtime.
* `strip-ansi-escapes` — pulls `vte`, which is itself declined above. Deleting
  escape sequences is only half the job anyway: the other half is replaying
  backspace and treating CR as a line break, which is the actual disease behind
  `frun-runner`'s six braille regexes, and no crate does that half for us.
  `flutter::clean` does both in about forty lines.
* `unicode-segmentation` — grapheme clustering is already handled by ratatui at
  render time.

### 7.4 How it was built, in order

The five steps below were the plan and are now the record. Each one is described
as it was designed; where the implementation departed from it, the departure is
called out.

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

**Departure: what "how many answered" counts.** The flow in section 4 branches on
the number of devices, and section 3.3 lists `flutter devices --machine` as one of
the sources feeding the `NO_DEVICES` screen. Those two cannot both be read
literally: macOS, Mac Designed for iPad and Chrome are in every answer on a Mac,
so a count of everything listed is never zero and `NO_DEVICES` — the screen whose
subtitle reads *nothing is attached* — becomes unreachable.

Resolved by splitting the two jobs. The **branch** counts attached devices only,
where attached means a platform that has to be booted or plugged in, so zero means
what the copy says. The **picker** lists everything Flutter reported, because that
is the screen whose whole purpose is choosing. `Device::attached()` is the one
predicate, and a test pins it.

**3. pty and the Flutter parser.** The bulk of the work, and where all the
regression risk lives. `portable-pty` to spawn, then the state machine from
`frun-runner`: platform-dependent stage detection, the hot reload ack/timeout
machine, `printBox` passthrough, startup log buffering.

This step said `vte`, to emulate the terminal rather than strip escapes with
regexes. It shipped without it, for the reason in 7.3.

`BUILD_FAILED` has to be built from nothing. `frun-runner` has no
build-failure detection at all: if Gradle dies the output falls through to raw
passthrough and the spinner simply stops.

**On reusing `frun-runner` rather than porting it.** Considered and declined.

Measured, `frun-runner` is 1,749 lines that are 1,012 lines of code, and those
divide roughly into 166 lines of rules worth keeping, 187 lines of
infrastructure that crates replace, and the rest Python boilerplate.

So the valuable part is 166 lines of *rules*, not 1,748 lines of code. Those get
ported line for line, not redesigned: which trigger string marks which stage,
that Flutter formats elapsed time with a group separator so `[0-9]+` clips
`1,847ms`, that `Built build/` is the unambiguous Gradle-finished signal while
the Gradle line itself re-emits partial durations. None of that is derivable from
Flutter's documentation.

The infrastructure is where crates win outright. `portable-pty` replaces
`pty.fork()` and the termios juggling.

It wins less completely than expected, though: this paragraph used to claim `vte`
replaced the 18 lines of ANSI cleanup and was strictly more correct. It is not,
because emulation is not what the cleanup is for. The six braille regexes exist
because Flutter animates in place with `\b` and CR, and the fix for that is
replaying those two bytes — 40 lines in `flutter::clean`, against implementing
`vte::Perform` for a screen model this UI never reads. See 7.3.

The third option was keeping `frun-runner` alive behind a JSON event stream, with
frun-tui only rendering. Its appeal is real: zero risk of regressing the ack
machine, the subtlest logic here. It was declined because parsing in that file is
entangled with rendering — `process_line` does not return events, it calls
`complete_spinner()` and `line()` directly. Reusing it as a process means first
extracting 141 lines of rendering, then designing an event protocol, then
maintaining an IPC boundary where every new field is a change in two languages,
and Python stays in the chain. Against that: the ack machine is 26 lines. Porting
26 lines faithfully is a smaller risk than designing a protocol.

**Departures in the port.** Three, all in the direction of less code:

* **`BUILD_FAILED` is one condition, not a catalogue.** The trigger is that the
  child exited without ever opening an interactive session. Gradle dying, an Xcode
  signing failure, `pub get`, a missing entrypoint and whatever breaks next all
  arrive through it, where a per-error pattern list would have to be extended for
  each. The failure card then reads the code frame off disk when the output names
  a `file.dart:line:col`, and shows the tail of the build output when it does not
  — a Gradle dependency failure names no source position, and calling that a
  compiler error would send you looking in the wrong place.
* **The twelve stage booleans are gone.** `frun-runner` tracks `pods_started`,
  `pods_completed`, `gradle_started` and nine more, plus a scratch string per
  stage for durations Flutter re-emits. Here the stage list *is* the state:
  `start_stage` and `finish_stage` are keyed on an enum and idempotent, which is
  what every `if not pods_started` guard was doing by hand.
* **`FRUN_LOG_DELAY` is not ported.** The five-second hold before releasing app
  logs existed because the shell printed the build summary and the log stream into
  one scrolling region, so a chatty app scrolled the summary away. The tracker and
  the log window are separate regions here, so there is nothing to protect them
  from. Startup logs still buffer, for free: `has_logs()` is false until the
  session is ready, so they simply are not on screen yet.

Also: stages time themselves when Flutter reports no duration. `Syncing files` and
`Interactive session ready` never carry one, and measuring is both honest and
smaller than special-casing them.

**4. Boot.** `emulator -avd` with `sys.boot_completed` polling capped at 180
seconds, and `xcrun simctl bootstatus -b`. Depends on 2.

**5. Cut over.** Last, not first: all three shell files stay working until the
Rust version reaches parity, so `frun` keeps running throughout.

| File | Fate |
| :--- | :--- |
| `frun.zsh` | 1,201 lines down to a shim that resolves the binary and forwards `"$@"`. |
| `frun-runner` | unreferenced. All 1,748 lines are step 3. |
| `frun-theme.zsh` | unreferenced. `theme.rs` owns the palette. |
| `.frun-last-device` | kept, read and written by the Rust version. |

**The palette question is settled: `theme.rs` owns it outright, and
`frun-theme.zsh` is not parsed.** The theme file carries 256-colour indices,
which is what a shell `printf` can address; `theme.rs` carries the RGB values
section 2 of this document specifies. Parsing the former would mean converting
256-colour indices back into truthful RGB, which is lossy in the wrong direction:
the palette here is the source and the shell file was always the approximation.
It also had exactly two consumers, `frun.zsh` sourcing it and `frun-runner`
parsing its `FRUN_C_*` lines, and both are gone.

`frun-runner` and `frun-theme.zsh` are left on disk rather than deleted. Nothing
reads them, so they cost nothing but a directory listing, and deleting a working
1,748-line implementation is a decision worth taking deliberately rather than as a
side effect of a cutover. All three files were copied to
`.backup/*.20260814-041959` first.

**Exit codes are part of the interface.** `0` on a normal exit, `130` when a pick
is cancelled with `Esc` — the shell's convention, and what the implementation
being replaced returned — `1` on a fatal, and Flutter's own code when a build
fails. Without this a script wrapping `frun` cannot tell a failed build from a
successful run.

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

### 7.5 Traps this codebase has already sprung

All of these cost real debugging time. They will recur.

**Every drawn row must be charged in `budget.rs`.** The Layout splits fixed
blocks by those heights, so a row added to a card without a row added to the
budget is clipped in silence — no error, no warning, content simply gone. It
has happened three times: the title gap, the logo labels, and the blank under
the progress bar.

**A failed build leaves the old binary in place.** `cargo build` fails,
`./target/release/frun-tui` still runs, and the output looks stale rather than
broken. Use `cargo run --release --` so a compile error surfaces instead of
last-good output.

**A fixed-width gutter has to be measured, not assumed.** The log gutter was
drawn with a two-column entry number and indented by a constant 18. Both were
right up to 99 entries. The first real run reached 908 and every wrapped line sat
a column off its own first line. Any constant that describes something drawn
elsewhere needs a test that measures what is drawn.

**Dropping the master pty handle kills the child.** `pair.master` has to live as
long as the session. Letting it fall out of scope at the end of `Session::spawn`
hangs up Flutter's terminal the instant it starts.

**`Logo::detect()` can cost you the keyboard.** It queries the terminal and reads
the reply from stdin. Against a terminal that never answers — a bare pty, some
task runners — it returns normally but leaves stdin unreadable, so the UI renders
every frame and ignores every key, including `q`. Measured: 63 frames drawn, zero
key events. `FRUN_NO_QUERY=1` skips the query and falls back to halfblocks.

**`main` returning `Err` prints the `Debug` form.** A carefully formatted `✖ FATAL`
line was followed by `Error: Custom { kind: NotFound, .. }`. Print the message and
`std::process::exit`.

### 7.6 Pending: defects, and the fixes decided for them

Found by running it against a real project. All open.

**Progress bar runs backwards.** `build.rs` computes `done / total` with
`total = app.stages.len()`, and `flutter.rs` appends stages as Flutter announces
them. At `Flutter started` that is 1 of 1, so 100%; when Gradle appears it
becomes 1 of 2, so 50%.

3.4 already forbids a denominator and the implementation contradicts it. The
total genuinely cannot be known ahead: the stage set is platform-dependent, and
Flutter skips stages when it can. Fix: indeterminate while building, full on
completion, and state the count only once it is known.

**A placeholder occupies the space output should be in.** During `BUILDING` the
middle area renders `Waiting for the application to start...`, which says nothing
the tracker above is not already saying with a spinner.

**Nothing marks the gap after `Flutter started`.** Roughly eight seconds pass
before `Running Gradle task` with no indication of progress: the `fvm` shim, the
Dart VM boot, flutter_tools startup, dependency resolution, Gradle daemon warmup.
None of it announces itself as a stage.

Those two share one fix. Flutter *is* printing during that gap — Impeller
messages, Gradle daemon notices, warnings — so the log stream should be on screen
during `BUILDING` instead of the placeholder. The gap fills with real output and
the placeholder disappears.

Plus one thing `frun-runner` already had and that was not carried over: an
elapsed clock on the stage currently running, appearing only after about three
seconds (`ELAPSED_AFTER`). Its comment gives the reason, which still holds — a
spinner alone cannot distinguish slow from stuck.

**Auto-select traps you on the wrong platform.** With only the iPhone simulator
up, `main.rs` finds exactly one attached device and launches on it. There is no
way to say "boot Android instead" short of quitting and booting it by hand.

The premise was that one attached device means no choice. It does not: it means no
other *running* device. Booting is a choice, and it is unreachable.

Decided fix: **one merged list.** Running devices and bootable targets in a single
picker, running ones first, always shown. The device from `.frun-last-device` is
preselected, so the common case stays a single `Enter`. Costs one keystroke per
run and buys predictability, which beats a heuristic that is right most of the
time and maddening the rest. This supersedes 3.3 mode 5.

**App logs cannot be scrolled.** `logs.rs` always takes the last `height` rows,
with no offset. `Up`/`Down` call `select_next`/`select_prev`, which move the
*device* selection, and `app.scroll` belongs to the device list. So in `RUNNING`
the arrow keys do nothing.

Wrapping makes this worse than it sounds: one Dart exception occupies eight rows
against a log window of twelve to nineteen, so anything older than about two
entries is unreachable.

Fix: a `log_scroll` distinct from the device `scroll`, driven by arrows, `j`/`k`
and the wheel, sticking to the bottom while already there so new output stays
visible.

### 7.7 Pending: open questions

**More log on screen.** Font size is not available to a terminal application; one
font at one size covers the whole grid, and only Ghostty can change it. The
app-side equivalent is a key that hides the three static cards and gives the log
window the screen: at 62 rows that is 17 rows becoming 58. Not started, not
decided.

**Marker stages show `0ms`, and blanking them is the wrong fix.** `Flutter
started` and `Interactive session ready` are markers; nothing takes zero
milliseconds. But emptying the column would remove the only place the eight-second
startup gap could ever appear, since that gap happens immediately after `Flutter
started`.

So a marker row carries the time from itself until the next stage begins. Both
timestamps are already known, so the figure is measured rather than invented, and
the most meaningless number on the card becomes the most informative one:

```text
  ✓ Flutter started                8.0s     fvm, Dart VM, flutter_tools, pub, daemon
  ✓ Gradle task assembleDebug    2,834ms
  ✓ Installing app               1,207ms
  ✓ Syncing files                   81ms
  ✓ Interactive session ready              last row, nothing follows it
```

Rows that already carry a duration keep it. `2,834ms` is Flutter's own figure and
matches what raw `fvm flutter run` prints, which is worth preserving; measuring
those ourselves would make the card disagree with Flutter for no gain. Gap-to-next
applies only to marker rows that have no duration of their own.

A pleasant side effect: the column comes close to summing to `Build time` without
adding an `other` row or renaming the label.
