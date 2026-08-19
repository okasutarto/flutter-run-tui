# 🖥️ Flutter CLI Terminal UI (TUI) - Architecture & Design Specification (`design.md`)

Document Version: `1.8.1`  
Last Updated: `2026-08-19`  
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

The color architecture uses a dark noir base with high-contrast functional color coding from the **Cyberpunk Neon Powerline** palette to instantly signal state transitions:

| Visual Token | Hex / Constant | Primary Usage |
| :--- | :--- | :--- |
| **Canvas Background** | `#0c0e14` (`bg-[#0c0e14]`) | Outer application container & dark grid frame |
| **Box Background** | `#000000` (`bg-black/80`) | Inset cards, logs container, tables background |
| **Default Text** | `#e4e4e7` (`TEXT`) | Standard body copy, output logs, table keys |
| **Subdued Text** | `#71717a` (`MUTED`) | Table labels, key hints, timestamps |
| **Cyan Accent** | `#34edf3` (`BORDER`, `CYAN`) | Card borders, section headers (`◆`), selected items, active focus |
| **Lime / Emerald Success** | `#b8ff6a` (`EMERALD`) | Active device status (`✔`), build success, completed steps |
| **Yellow / Amber Warning** | `#ffe66d` (`AMBER`) | Git branch tags, no-device alert banner, pending steps |
| **Magenta / Rose Error** | `#f715ab` (`ROSE`) | Build/Hot Reload failures (`✖`), stack traces, pointers |
| **Purple Virtual** | `#cc4dff` (`PURPLE`) | Simulator/emulator badges, secondary runtime tags |
| **Badge Ink** | `#070e34` (`INK`) | Dark text ink drawn on top of filled accent badges |

---

## 🧩 3. Core Component Architecture

The interface is organized into modular TUI blocks:

```
┌────────────────────────────────────────────────────────────────────────────┐
│ ProjectCard: 2-Column Info (Left: Graphic Logo | Right: Metadata Table)     │
├────────────────────────────────────────────────────────────────────────────┤
│ SelectedTargetCard: DEVICE INFO — Target Details + [^D] (States 6-13)       │
├────────────────────────────────────────────────────────────────────────────┤
│ DYNAMIC STATE FRAME CONTENT                                                 │
│                                                                             │
│  Discovery                                                                  │
│  ├─ State 1:  FlutterDeviceManager (DETECTING)                              │
│  ├─ State 2:  removed — the picker is the only list, see 3.3 and 8.10       │
│  ├─ State 3:  FlutterDeviceManager (BOOTING)                                │
│  ├─ State 4:  FlutterDeviceManager (MULTIPLE_DEVICES - picker)              │
│  └─ State 5:  FlutterDeviceManager (SINGLE_DEVICE - superseded, see 7.6)    │
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

  **The box is fixed in cells, and the encoding has to follow the cell.** Eleven
  by five cells is the mark's size relative to the text beside it, which is the
  ratio worth holding; the terminal decides what a cell is worth in pixels. That
  makes the artwork's pixel size a function of the font size, and `Cmd -` in
  Ghostty changes it under a running process.

  Nothing was following it. `Picker::from_query_stdio` measures the cell once,
  before the alternate screen is entered, and exposes no setter; the encoded
  protocol was cached against the cell *box*, which is a constant, so it was built
  exactly once per process and never rebuilt. Text reflowed around a mark still
  drawn for the font the process started with.

  `Logo::follow_cell_size` re-measures every 200ms from `TIOCGWINSZ` and rebuilds
  the picker and the protocol when the answer changes, which under kitty is also
  what re-transmits the image. `TIOCGWINSZ` and not a second escape-sequence query:
  the query reads its reply off the input loop's stdin, which is the contention
  `FRUN_NO_QUERY` exists for, while the ioctl asks the kernel. Halfblocks are
  exempt — their 10x20 cell is a fiction that fixes the aspect ratio of a
  block-glyph render — and so is a terminal that reports no pixel size, since
  `ws_xpixel` is optional and inventing a cell there would replace a stale mark
  with no mark.

  The `Flutter Engine` and `Cross-Platform CLI` labels this section used to
  specify are gone. Both restated what the mark and the metadata column beside
  it already said, and `Flutter Engine` was inaccurate as well: the versions
  shown are the framework and the Dart SDK, not the engine.
* **Right Column (8 Cols)**: Key-value table showing:
  * `Project`: `cwclub`
  * `Version`: `1.4.0+12`
  * `Branch`: `main`
  * `Git Status`: `✔ clean`
  * **3-Column Technical Stats Row**: `Flutter` (`3.27.1`), `Dart` (`3.7.2`),
    `Runtime` (`(FVM)`, `(SDK)`, or the version manager's own name — whatever
    `probe::toolchain` resolved, see 8.11).
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
* **Header Bar**: Right-aligned path tag `~/cwclub`, nothing else. A `[COPY]`
  button was drawn there and removed: it was never wired to a clipboard, and a
  control that does nothing on click is worse than no control.

### 3.2 `SelectedTargetCard`
* **Titled `DEVICE INFO`.** It was `SELECTED DEVICE`, which put two naming schemes
  on two stacked cards — `PROJECT INFO` is directly above it. The word it gave up
  was carried by the card's existence anyway: there is no card without a chosen
  device, since `has_target` hides it in every picker state. `TARGET DEVICE` was the
  other candidate and is worse, because `Device Target` is the first row inside.

* **Target Details Table**, and nothing else: `Device Target`, `Platform ID`,
  `OS Version`, `Type` (Simulator / Hardware). Four rows, plus separators.

* **What the run is doing is a pill in the title bar**: `RUNNING`, then `STOPPED`,
  `DETACHED` or `DISCONNECTED` from `Ending` once it ends.

  ```text
  ╭─ ◆ DEVICE INFO ───────────────────────────────   RUNNING  ╮
  ╭─ ◆ DEVICE INFO ──────────────────────────   DISCONNECTED  ╮
  ```

  On this card because all four are statements about the *device*: the app is on it,
  or after `d` still on it with the tooling let go, or after `^S` gone from it, or
  after a `Lost` unreachable. In the title bar because that slot was empty here and is
  a status slot everywhere else — `~/cwclub`, `5 devices`, `N targets`,
  `[7 entries]` — so this closes the one card that was an exception rather than
  inventing a place.

  **Shown in every state that draws this card except the two the tracker owns.**
  `has_tracker` is the gate, the same one the log card's title uses for the build
  totals, and the symmetry is the point: while the tracker block is on screen it holds
  both the state word and the two numbers, and when it is not, the word is here and the
  numbers are there. Without the gate `BUILDING` would be on screen twice.

  `RUNNING` replaced `✔ BUILD FINISHED` on that arm of `status()`, which had no reader
  left — the tracker is not laid out once a build succeeds, so nothing could draw it.
  The pill's subject is the run rather than the build it came out of, and its three
  neighbours all answer that question, so a fourth reporting on the build would be
  answering a different one. It is also the word the device list already uses for this
  exact fact: ` running ` marks the row your app is on.

  **The glyph goes inside the fill**, so the pill is one object. Outside it read as a
  separate mark that happened to sit next to a badge, and on a border row — where `─`
  runs up to it from the left — a lone symbol between the rule and the pill has nothing
  to attach itself to. A play triangle in, a stop square out, from one icon family
  (`theme::GLYPH_PLAY`, `GLYPH_STOP`): two ends of one axis should not look like two
  unrelated facts.

  `PROJECT INFO` was the other candidate and is wrong twice over. Nothing about a
  project is disconnected, and its top-right is already spent on the cwd, so at any
  real path length it would force a drop rule whose choices are the news or a path
  you know by heart. It is also the region of the frame that has not changed all
  session, which is the region the eye has learned to skip.

  No `Status` label, because no other title slot has one and the position already
  says the value describes the whole card. A labelled `Status` field row under `Type`
  was the alternative and costs two rows — the row and its separator — which is
  exactly what 3.4 had just reclaimed by removing the tracker block. The title bar
  carries the same four values for nothing.

  A pill rather than a bare word, matching the chips on a device row, since this is a
  state the device is in.

* **No control row. `[^D] Switch` is on the footer** (3.7), in the five states where
  the key does something.

  It has been in three places. The top border beside the title, where it cost nothing
  but could not be clicked and sat in the slot every other card uses for a status. Then
  a row of its own at the bottom of the card, which fixed both of those and cost two
  rows: the keycap, plus a blank above it to stop it reading as a fifth field's value —
  a row spent saying that the row below it is not what it looks like. It was the only
  row in the card with nothing facing it on the left, and it showed.

  The footer was the answer both times and was skipped both times because `RUNNING` and
  the reload states did not carry the key on that row. They do now. This is the third
  in-card action row the project has removed, after the failure card's `[r] Retry Build
  [q] Quit` and the picker's `↑↓ move  ⏎ run  Esc cancel`, and all three went for the
  reason the second one's note gives best: the footer says it once and says it
  everywhere.

  So the card is a table of facts about the device and nothing else, ten rows, with the
  one state it is in as a pill in its title. The label shortened to `Switch` on the way
  out: the footer carries it in five states now instead of one, and the noun was never
  load-bearing — the argument was `switch` against `change target`, which survives.

* **`OS Version`, not `OS Version / Arch`.** The row is Flutter's
  `sdkNameAndVersion`, and no mobile platform puts an architecture in it:
  `Android 17 (API 37)`, or a runtime identifier for an iOS simulator. The arch
  lives in `targetPlatform` — `android-arm64` — which is `Platform ID`, the row
  directly above. Desktop is the one platform that carries both, inside Flutter's
  own prose (`macOS 26.6.1 25G76 darwin-arm64`), and it still does. So the second
  half of the label was either unfulfillable or a repeat of the line above it.

* **A device frun booted describes itself too.** It never passes through
  `flutter devices` — 3.3 forbids asking twice — so both `Platform ID` and
  `OS Version` used to be blank for exactly the device you had just waited three
  minutes for. Each is now filled from a source that costs nothing: an Android
  emulator is asked over adb the moment `sys.boot_completed` lands
  (`ro.product.cpu.abi`, `ro.build.version.release`, `ro.build.version.sdk`,
  spelled the way Flutter spells them), and an iOS simulator carries the runtime
  `simctl` filed it under from the moment it appears in the picker, booted or not.

* **The status banner is gone.** It read
  `✔ 1 device active: iPhone 17 Pro (emulator) (iOS)` in emerald, above a blank
  row. Every fact on it is in the table beneath it — the name is the `Device
  Target` pill, `(emulator)` is `Type`, the platform is the head of `Platform ID`
  — and what it added on top of that was a count that is one by construction,
  since this card only exists once a device has been chosen.

* **The command string is gone.** It read `❯ fvm flutter run -d <udid>`, also
  above a blank row. It was display only: `Session::spawn` assembles its own argv
  from the forwarded flags, so the string described a command rather than being
  one, and the device it named was the row directly above it. `App::command` and
  `flutter::command_line` went with it rather than being left as state nothing
  reads.

  Together that is four rows, in the state where the log window is hungriest. See
  6.2 for where they went.

  Implementation note: `flutter devices --machine` does supply
  `sdkNameAndVersion`, `targetPlatform` and an `emulator` flag, so every field
  above is available. The existing `_frun_device_list` currently discards all
  of them, keeping only id, name, platform and icon.

### 3.3 `FlutterDeviceManager` (Device Selector)

Five operation modes. Discovery is not one screen with three variants, it is
a short sequence with branches, and the branch taken is decided by how many
devices answered.

1. **`DETECTING`** (State 1): animated Braille spinner (`⠋ ⠙ ⠹ ...`) while
   discovery runs. Always entered, always first. Nothing below it is reachable
   without passing through here.

   **It is answered twice, and the first answer is the one you wait for.** `adb`,
   `simctl` and `emulator -list-avds` describe the same list in ~200ms that `fvm
   flutter devices --machine` describes in 6-9s, so the picker opens on theirs and
   Flutter's lands behind it as a refresh. Measured: `SELECT DEVICE` on screen inside
   0.4s where it used to be 8-9s. See 8.9.

2. **`NO_DEVICES`** (State 2): **removed, see 8.10.** It read `◆ NO DEVICE RUNNING`
   over `Nothing is attached. These can be started:` and a `Start a Device` heading,
   and it drew the same rows the picker draws, from the same code. It was also
   unreachable — `devices_answered` branched on `Device::attached`, which counts every
   iOS and Android row including the bootable ones — so one installed simulator made
   the picker the only answer. Nothing booted now opens `SELECT DEVICE` with every row
   offering a boot, which is 7.6's merged list saying what the amber heading said.

   The numbers of the other states are left as they are. They are labels shared with
   the code and quoted throughout this document, not indices, and renumbering eleven
   states to close a gap would invalidate every cross-reference to buy tidiness.

   **No per-row verb**, which was decided here and still holds for the picker. Rows
   carried `▶ Run`, first as a `Start`/`Run` split and then uniformly, and it ended up
   saying what the footer says once: `[⏎] Launch`, or `[⏎] Switch` in State 12. Seven
   columns per row repeated down the list for one shared answer, which is the same
   duplication that took the hint row out of the card. The one row that keeps a verb is
   the one where `Enter` does *not* build — ` ⏎ Keep ` on the running row in State 12
   (8.5).

   **Every target, not just mobile.** The existing implementation deliberately
   drops macOS, "Mac Designed for iPad" and Chrome, with the stated reason
   that frun exists to run on phones. That restriction is lifted:

   | Source | Targets | Cost |
   | :--- | :--- | ---: |
   | `adb devices` + `getprop` | running emulators and attached Android phones | 12ms |
   | `emulator -list-avds` | Android AVDs | 30ms |
   | `xcrun simctl list devices available -j` | every iOS/iPadOS simulator, booted or not | 119ms |
   | `flutter devices --machine` | macOS desktop, Chrome, physical iPhones, and any other platform Flutter reports | 6-8s |

   The first three are the fast answer and the fourth is the one that completes it.
   Only its own row is exclusive: everything above is in the picker before Flutter
   has finished starting its Dart VM.

   Note the asymmetry: emulators and simulators need booting, whereas desktop
   and web are always available. Those rows go straight to launch, skipping
   State 3.

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

   **The rule covers Ambiguous width too, and that is the harder half.** Wide is
   obvious; Ambiguous measures one cell in `unicode-width` and is drawn as two by
   any font that gives it emoji presentation, so the same string is correct in one
   terminal and a column out in the next. Three glyphs have been caught by it:
   `⚡` U+26A1 for hot reload, now `U+F0E7` (), and `⏹` U+23F9 and `⚠` U+26A0 on
   the collapsed tracker row, now `U+F04D` () and `U+F071` (). The bolt's
   symptom was an overflow past the card border; the other two opened their row, so
   theirs was an indent that pulled the row out of line with the borders above and
   below it (3.4).

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
   visible `last used` chip, so the reordering is explained rather than
   silent.

   **Two chips, and both appear when both conditions hold.** They answer
   different questions, so one cannot stand in for the other:

   | Chip | Condition | What it says |
   | :--- | :--- | :--- |
   | `active` (emerald) | already running, and the platform boots at all | Enter launches now, no boot wait |
   | `last used` (purple) | id matches `.frun-last-device` | this is the one you normally reach for, and why the row moved to the top |

   ```text
   ❯   Pixel 10 Pro XL   active   last used      emulator-5554  Android  virtual
        Pixel 8                                        Pixel_8  Android  virtual
        iPhone 17 Pro                           8A3F91C2-4D2E  iOS      virtual
   ```

   One mutually-exclusive slot was tried first, with `active` winning. It hid the
   answer to "is the device I always use the one that is up?", which is the single
   question the pair exists to answer: suppressing `last used` on a running row
   makes a booted favourite indistinguishable from a stranger that happens to be
   attached, so the reason the top row is on top disappears exactly when the top
   row is the good one.

   `active` is printed first because it is the chip that changes the consequence
   of pressing Enter. `last used` is a preference and costs nothing either way.

   `active` additionally requires `Platform::needs_boot()`. macOS and Chrome are
   always available, so "active" would describe a state they cannot be out of.

   Both chips are per-row properties rather than per-frame, which is why State 2
   and State 4 share one row renderer: a shut-down simulator keeps its `last used`
   chip and gets no `active` chip, in either frame, without the frame being
   consulted.

   **The row is budgeted, not truncated.** Two chips is the widest a row gets, and
   at 70 columns the pair used to push the right-hand group past the edge and take
   the row's verb with it. Same failure the footer has in 3.7, and the same fix:
   optional spans are charged against the leftover columns in priority order, and
   once one does not fit nothing after it is drawn either. Dropping the verb gave
   every row nine of those columns back, so the figures below are the floor rather
   than the ceiling now.

   | Priority | Span | Survives down to |
   | :--- | :--- | :--- |
   | never dropped | caret, platform glyph, device name | any width |
   | 1 | `active` chip | 58 and below |
   | 2 | `last used` chip | 64 |
   | 3 | device id | 79 |
   | 4 | platform label | 88 |
   | 5 | `virtual` tag | 97 |

   Measured on the widest row the mock produces — `Pixel 10 Pro XL`, both chips —
   so the figures move with the name's length. That is why one row can carry an id
   while the row under it does not, and the raggedness is accepted: the right-hand
   group is right-aligned rather than a fixed column grid, so the ids never lined
   up anyway, and a dropped span is readable where a clipped one is not. You cannot
   tell a span that was left out from one that was cut in half.

   The pair survives to 64 columns, four above the `60x14` floor below which 6.2
   refuses to draw at all. So the only widths that lose `last used` are 60 to 63,
   and at that size `active` is the one worth keeping.

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

  | Trigger | Row opened | Platform |
  | :--- | :--- | :--- |
  | *the pty spawns* | `Starting Flutter` | both |
  | `Launching lib/main.dart` | `Launching lib/main.dart` | both |
  | `flutter pub get` / `Resolving dependencies` | `Resolving dependencies` | when needed |
  | `Running pod install` | `Installing CocoaPods` | iOS |
  | `Running Xcode build` | `Building with Xcode` | iOS |
  | `Running Gradle task '<task>'` | `Gradle task <task>` | Android |
  | `Installing build/...` | `Installing app` | Android |
  | `Syncing files to device` | `Syncing files` | iOS, Android |
  | `Flutter run key commands` | `Application Running` | iOS, Android |
  | `Debug service listening on` / `To hot restart changes while running` | `Application Running` | Web |

  **The end of the build is announced per runner, not once.**
  `Flutter run key commands.` is `HotRunner.printHelp`; `ResidentWebRunner`
  overrides `printHelp` and never prints it. While that line was the only trigger,
  a Chrome run had nothing to end it: `Preparing build` kept spinning at
  `Stage 2/4` with the app already up in the browser, measured at 1m 7.9s and with
  nothing further to arrive. Web needs two triggers rather than one, because
  `Debug service listening on ws://...` comes from `attach()` and only exists where
  there is a service protocol, which a `--release` web build has none of, while the
  help line is printed in every mode. Whichever arrives first ends the build; the
  second is a no-op.

  Web also has no `Syncing files`: `ResidentWebRunner` updates its devFS behind
  `Waiting for connection from debug service on Chrome...`, which is a progress
  message and so carries no newline until it stops. That is one row fewer than the
  denominator expects, which is what an upper bound is for.

  **Every trigger opens a row. Nothing closes one.** A row is closed by the
  arrival of its successor, at the same instant it is charged its duration. That
  single rule is what guarantees a spinner is on screen for every second of the
  build: closing a row *is* opening the next, so there is no moment in between.

  Consequences worth stating, because they are the whole point:

  * **`Starting Flutter` is opened by frun, not by Flutter.** It covers `fvm`
    resolving the pinned SDK, the Dart VM booting flutter_tools, and
    flutter_tools starting up. Flutter's first line is what *ends* that span, so
    nothing in its output brackets it, and without a row of frun's own those
    seconds have no indicator at all.
  * **`Xcode build done`, `Built build/...`, `Compiling, linking and signing` and
    a bare duration line are swallowed.** They used to close a row. Each one
    stopped the spinner while the build carried on — on iOS the wait between
    `Xcode build done` and `Syncing files` is the app being installed and
    launched, and the tracker sat idle through it.
  * **A closed row's number never moves.** The old split — closed by its own
    trigger, charged later at the next open — let a row show
    `✔ Building with Xcode 11.1s` and silently become `14.5s` seconds afterwards.
    Measured on an iOS transcript: `0ms → 125ms`, `121ms → 241ms`,
    `124ms → 249ms`.
  * **The first platform phase adopts the generic row rather than following it.**
    `Preparing build` *becomes* `Building with Xcode` — one row that changes its
    name, not two rows with a boundary between them.

    This is the second answer to the same problem, and the first one is worth
    recording because it looked right. Flutter's timers start before its
    announcements reach us, and on iOS the lag is enormous: measured against a real
    build, it printed `Running Xcode build...` **16.3 seconds** after its own Xcode
    timer had started, then closed with `Xcode build done. 22.6s`. Timing the row
    from the announcement gave `Building with Xcode 5.6s` for work Flutter said took
    22.6s — wrong by four times, with the missing 16 seconds on the row above.

    The first fix believed Flutter's figure and moved the boundary between the two
    rows back to where the work began. It was accurate and unusable: a row already
    reading `✔ Preparing build 10s` became `3.5s` seconds later, so the two figures
    looked like they had swapped. Measured on a transcript: `405ms → 60ms`.

    The mistake was deeper than the display. **The two phases overlap.** Xcode was
    already building while frun still thought it was preparing, so two consecutive
    rows were asserting a boundary that cannot be observed at all. Correcting an
    unobservable boundary is not better than not claiming one. So the claim is
    dropped: one row, one span, one figure, and nothing to revise afterwards.

    A label changing while a row spins is a far smaller surprise than a settled
    number moving, and it is honest about what happened — *I am working, and now I
    can tell you what this is.*

  * **`Syncing files` takes Flutter's own figure.** Its opening line and the line
    that closes it arrive in a single read, so the measured span is zero and
    Flutter's `81ms` is the only evidence it took any time. Pinning one row's figure
    touches no neighbour, so nothing can appear to swap.

  Note Flutter formats elapsed time through `NumberFormat`, so values carry a
  group separator (`1,847ms`) and must not be matched with `[0-9]+`.

* **Each row's timer runs while its phase runs.** It starts at zero the moment
  the row opens, ticks live, and freezes at its measured figure when the
  successor closes it. One formatter for both halves, so `1.8s` running becomes
  `1.9s` frozen rather than switching units mid-life.

  There was a three-second delay before a clock appeared, inherited from
  `frun-runner`, on the grounds that a stage which finishes quickly should never
  show a number. That made the fast rows read as though they were not being timed
  at all. The delay is gone: a row that has just opened reads `0ms` and starts
  moving.

* **Timing Bar**: `Build Time` and `Sync Time` only. CPU and memory gauges are
  removed. Whose CPU is ambiguous, and reporting system-wide load would be
  decorative rather than informative; making it honest would mean resolving
  the Dart VM, Gradle daemon, Xcode and simulator process tree for a number
  that is rarely acted on.

* **Progress Bar**: filled bar with `Stage 4/5`. A blank row separates it from
  the stage list.

  The bar itself is capped at 44 columns; the row it sits on is not, so the
  stage count still right-aligns to the border. A 118-column bar conveys nothing
  a short one does not and turns a glance into a scan.

  **The denominator comes from the platform**, which is the correction to what
  this section used to say. It read "no denominator", on the grounds that the
  total is not knowable in advance because the stage set is platform-dependent and
  Flutter skips stages it does not need. The first half of that is backwards: the
  platform is chosen *before* the build starts, and the table above is indexed by
  it, so the phase set follows from the target:

  | Platform | Phases counted | Total |
  | :--- | :--- | :--- |
  | Android | starting, Gradle, install, syncing, running | 5 |
  | iOS, macOS | starting, Xcode, syncing, running | 4 |
  | Web | starting, preparing, syncing, running | 4 |

  There is no `launching` row in the first two: the platform phase adopts it. Web
  keeps it, because nothing on web announces itself to do the adopting.

  Android is the one with six: it installs the APK as a step of its own, which iOS
  does not. And **CocoaPods is not counted**, for the same reason
  `Resolving dependencies` is not — both are skipped on most runs, `pod install`
  whenever `Podfile.lock` is current, so counting them would leave the bar
  permanently a row short on every ordinary build. When either does run,
  `expected_stages` raises the total to match, because it is floored at the number
  of rows that actually exist.

  The skipping is real and is handled by direction rather than by avoidance. The
  figure is an upper bound, never a floor — `pod install` is skipped when
  `Podfile.lock` is current, the APK install when attaching to an app already
  there — so the bar can stall one stage short and then complete. The failure this
  section warned about, a bar reaching 100% and then continuing to work, is
  precisely what an upper bound cannot do; it was the old denominator, the count of
  stages announced so far, that produced it. See 7.6.

  **The numerator is the stage you are on, not the count that has closed.** It was
  the closed count, which read `Stage 0/6` for the whole of the first stage — a
  build four seconds in reporting that nothing had happened. The row currently
  spinning is the stage you are on, so it counts, and the bar is filled from the
  same two numbers the label shows so the two cannot disagree.

  **On completion the denominator collapses to what actually ran**, so the row
  reads `6/6`, or `5/5` on an iOS build that skipped CocoaPods. The estimate has
  done its job by then, and an upper bound that stays put would end the build on
  `5/6` — a build reporting that it stopped short of itself.

* **There are no marker rows.** An earlier revision named `Flutter started` and
  `Application Running` markers — announcements rather than work — and gave
  them a special rule: report the distance to the next stage, since timing the
  announcement itself yields `0ms`.

  The special case is gone because the general rule absorbed it. Every row is now
  measured from its own announcement to the next one, so a row that represents an
  instant automatically reports the span that follows it, with no class of row
  needing different treatment. `Starting Flutter` gets the startup gap because it
  opens before Flutter speaks; `Application Running` is closed by the end of
  the build, which is the only row without a successor. See 7.7.

* **The card closes when the build does.** Everything above describes a card that
  exists for the length of a build. When the build ends, the whole of it collapses
  to one borderless row:

  ```text
  ⏹ STOPPED                                      Build time 20.2s   Sync 68ms
  ```

  Not a size concession. At any height, in `STOPPED` and — collapsed by height
  alone — in `BUILD_FAILED`, this is the tracker. `BUILD_FAILED` keeps the full
  card wherever the rows allow it, because there the stage list is what the error
  is read against.

  Every row it replaces had stopped moving: five labels, five frozen durations,
  and a progress bar reading `Stage 5/5` in emerald sitting directly beneath a
  title already saying `BUILD FINISHED` in emerald. Nine rows asserting one fact
  that the row above states in three words, over a log window pinned to its floor.
  6.2 has the arithmetic and the two attempts it took to get the mechanism right.

  The words and the colour are the card's own title, and the two figures are the
  card's own title bar, unchanged in wording and near enough in position. The
  collapse should read as the card closing, not as a different component arriving,
  and both halves come from one function each (`status`, `timings`) so the card and
  the row cannot drift apart — they did once, when the card said `BUILD FINISHED`
  and the row beneath it said `build finished`.

* **And in `RUNNING` the row goes too.** `✔ BUILD FINISHED  Build time 20.2s
  Sync 68ms` was the collapse's own example and is the one case it could not
  justify. A build that succeeded and is now running has nothing to report that the
  frame is not already reporting: the log stream underneath is streaming, which is
  what `BUILD FINISHED` means, and the three reload frames inherit the same row for
  the same non-reason. So `RUNNING` and the three reload states have no tracker
  block at all, and the row plus the gap above it go to the log window.

  **The two figures move into the log, as its first entry.** `session_ready` writes
  `build finished in 20.2s · sync 68ms` at `INF` the moment both totals are final,
  before Flutter's first line arrives, so the stream opens with the build result
  as its head. Better than the row it replaces on three counts: it is timestamped,
  it scrolls away like every other one-off fact, and a rebuild after `r` appends a
  second entry rather than overwriting the first, so two builds in one session are
  both on record. frun already speaks in the stream in its own voice — `lost
  connection to the device`, `new tab — <name>`, `device refresh failed` — so this
  needs no new vocabulary and no new level. The `BLD` and `OK` levels stay
  removed.

  **And `STOPPED` does not keep it either.** The row survived one more round on the
  grounds that it was the only place distinguishing `STOPPED` from `DETACHED` from
  `DISCONNECTED` — the distinction 8.8 exists to make, since after Flutter's own `d`
  the app is still live on the device and after `^S` it is gone. That was a reason to
  keep the *words*, not a reason to keep a block. They are a pill in the target
  card's title bar now (3.2), which is where a fact about the device belongs and
  which costs nothing.

  What that buys, beyond the row and the blank above it, is worth more than either:
  **the log window no longer changes height when a run ends.** The banner used to
  arrive and push every line of the stream down two rows, at the moment the user is
  most likely to be reading one. `chrome(STOPPED)` now equals `chrome(RUNNING)`.

  The pill is on screen throughout, reading `RUNNING` in emerald, so the transition is
  a word and a colour changing in place rather than a banner arriving. That is weaker
  as an alarm and it is not the only signal: the footer swaps to `[r] Build again` at
  the same moment, and a `Lost` puts `ERR lost connection to the device` in the log.
  What it buys is a card that says what the run is doing at every point in the session
  rather than only at the end of one.

  `State::has_tracker` is the predicate, and it is `BUILDING | BUILD_FAILED`. It is
  deliberately narrower than `has_build`, which asks whether there is a run behind
  the frame — what `^D` and `^S` need to know, and true throughout a run.

* **`DISCONNECTED` is rose, not amber.** It was amber on the argument that rose is
  for something broken and muted is for a stop that was asked for, and a device
  that vanished is neither. That weighed the wrong axis. Amber in this palette is
  *in progress* — the `BUILDING` spinner, pending stage rows, the reload note, the
  ` in use ` chip, the `^S` hint — so a terminal state wearing it competes with
  five live ones, and a run that died is the last thing that should read as still
  working.

* **The two ending glyphs are Nerd Font.** `⏹` U+23F9 and `⚠` U+26A0 are East
  Asian Ambiguous, one code point from the `⚡` U+26A1 that section 2 already has a
  scar about. This row is where that is visible as an *indent* rather than as an
  overflow: the glyph is the row's first cell, so a font drawing it with emoji
  presentation pushes the whole row half a cell right of the card borders above
  and below it. `theme::GLYPH_STOP` (U+F04D) and `theme::GLYPH_WARN` (U+F071)
  measure and render one cell.

  **The per-stage breakdown is gone, and that is the cost.** The tracker is the
  only place it ever existed: the `BLD` and `OK` log levels were removed on the
  grounds that the tracker owned those facts, so `Building with Xcode 14.5s` is
  unreadable for the rest of the session. Accepted rather than mitigated. That
  figure is watched while its row is spinning, which is exactly when the full card
  is on screen, and the total it rolls up into is kept. The `5 stages` count the
  earlier one-line summary carried is dropped with it; it answered nothing the
  build being over does not.

* **Failure State**: error summary, exit code, elapsed total, and which stage
  broke.

  **Code frame.** The error carries `lib/main.dart:42:18`, and Dart already
  emits the offending source line plus a caret, so that much is free
  passthrough. Showing the lines *around* it (41 and 43) means reading the
  file from disk at the reported line number. Worth it: one line of context
  either side is usually the difference between recognising the mistake and
  opening the editor.

  **Single action: `[r] Retry Build`, on the footer only.** This is not a keypress
  forwarded to Flutter. It kills the child in the pty, waits for it to be reaped,
  and spawns a fresh `fvm flutter run -d <id>` with all stage state reset. Hot
  restart is not a build retry and cannot substitute for one.

  The card used to close with its own `[r] Retry Build  [q] Quit` row. That row is
  gone: the footer advertises both keys in this state, with the same clickable
  region behind `[r]`, so the card was repeating the cheatsheet in the one place
  where every row belongs to the compiler's output. It cost three — the row, the
  blank above it, and a third reserved by truncating the message to leave room —
  and what the truncation cut was the oldest build output, which on a Gradle
  failure is usually where the cause is. `FAIL_MIN` stays at 14, so those rows go
  to the message rather than back to the layout.

  Two notes. The Gradle daemon survives the kill, so a retry is usually much
  faster than the first build. And the failed build's log is kept rather than
  cleared, because comparing the two runs is the point.

  `r` is free in this state: there is no live Flutter session to hot reload,
  so the key carries no conflicting meaning.

  `[Fix with AI]` / `[f] Fix Code with AI Assistant` is removed. There is no
  such capability in this project, and a button that cannot act is worse than
  an absent one.

### 3.5 `TerminalLogsView`
* **Strict Gutter System**, one width per frame:
  ```
  1 14:32:01 INF Log content message line...
  2 14:32:02  ⚡  Reloaded 125 of 1824 libraries in 148ms.
  3 14:32:04 ERR The following assertion was thrown building
                 CheckoutScreen(dirty, dependencies: [_Inherited…
  ```
  `1␣14:32:01␣INF␣` is 15 columns and `4000␣14:32:01␣INF␣` is 18. Continuation
  rows leave the gutter empty and indent to the message column (see 6.1).

  **The width is computed from the entry count, not fixed.** It was a constant 18,
  sized for a four-digit number, which meant the first nine entries of every
  session were right-aligned into a column that was three quarters empty and the
  stream began three columns adrift of the card holding it. What has to be constant
  is that *one* number describes the gutter — the drawn width and the continuation
  indent are the same expression — and that survives being computed. The cost is a
  one-column shift as the log crosses 10, 100 and 1000 entries; three reflows in a
  session against a hole on screen at the start of every one.

  The level badge is padded to three cells for the same reason. `INF`, `WRN` and
  `ERR` are three; the reload bolt is one, so reload rows drew a 16-column gutter
  while everything around them drew 18, and a wrapped reload line indented to a
  column its own first row never reached.

* **Clean Header**: section title (`◆ APP LOGS STREAM`) and a count
  (`[N entries]`). No search bar and no filter toggles.

  Dropping the filters also resolves a data problem: `FLUTTER` is detectable
  from the `I/flutter (12345):` prefix, but `SYSTEM` and `NET` have no source
  in Flutter's output and would have to be guessed.

* **On screen during the build, not only after it.** This view used to appear
  only once the interactive session was live, on the grounds that nothing had
  printed yet. That was false: Flutter prints throughout, and the eight seconds
  between `Launching lib/main.dart` and the first Gradle line are full of Impeller
  notices, daemon messages and warnings. Those rows were being spent instead on a
  placeholder reading `Waiting for the application to start...`, which said nothing
  the spinner above it was not already saying. See 7.6.

* **Scrollable.** `log_scroll` counts rows back from the live tail, driven by the
  arrow keys, `j`/`k` and the wheel; zero is the bottom, so new output keeps
  arriving without being followed. The title says `· scrolled` when the window is
  away from the tail. `e` gives the log window the whole frame (7.7).

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
not be advertised: `↑↓ Enter ⇧Enter Esc` during discovery, `r R ^D q ^C` while running.

**Centred, at a fixed two columns between hints.** The group was flush right, and the
reason given was that the right edge keeps it beside the diagnostics rather than leaving
a gulf between them. That optimised for a companion which is usually absent: `proto n/12`
is prototype-only, `mouse on` is off by default, and `[given up]` is empty whenever the
layout is not conceding — which at the design size it no longer is. So the row was
anchored to an empty slot, and under two full-width cards it read as a group shoved into
a corner. Centred, it reads as a caption for the frame above it.

Centred within the hints region rather than the whole row, so the diagnostics keep the
slot they are given and nothing can overlap them. When there are none, that region is
the whole row.

Fixed spacing is the part of the older decision that stands. Space-between came first —
the leftover split into equal gaps so the row spanned the full width — and it stretched
the keys apart as the window grew, until reading the row meant crossing ninety columns
of blank to find the next key.

The discovery and switch rows read, in full:

```text
  [↑↓] Move     [⏎] Launch     [⇧⏎] Launch in new tab     [Esc] Cancel
  [↑↓] Move     [⏎] Switch     [⇧⏎] Launch in new tab     [Esc] Back
```

`[⇧⏎] Launch in new tab` is the same words in both, deliberately. `⏎` changes
meaning between the two rows and has to say which it means (8.5), but `⇧⏎` does not:
it launches, in a new tab, whether or not this tab has a run behind it (8.4).

**It fits at the floor, which is the question a fourth hint raises.** Labelled, the
keycaps and words measure 53 columns plus 3 single-column gaps on the picker row and
51 plus 3 on the switch row, against the 60 of `MIN_W`. Below that the row does what
it already does for the eight-hint running state: it drops every *word* and keeps
every *key*, since `[^C]` alone is still a key you can press and a truncated `[^C`
is not. So the new hint cannot cost another hint its place — the failure mode it
brings closer is a wordless row, not a missing key.

**The running row is eight hints now, and it is the one that pays for that.** `[^D]
Switch` joined it when the target card's control row was removed (3.2), and labelled the
row measures 99 columns. It fits at the design width and drops to bare keycaps below
about 100, where it used to keep its words down to about 86. That is the cost, taken
knowingly: a 90-column terminal shows eight pressable keys without their words, rather
than seven with words and an eighth that is only discoverable by reading the source.

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

**What the layout gave up is reported only when the layout decided something.**
The expanded log view (`e`, 7.7) bypasses the ladder rather than solving it, so
there are no concessions to name there — and naming them anyway printed
`[separators, dense devices, build collapsed, cards collapsed]` about cards that
are not on screen, spending 56 columns of this row to do it. That is what
collapsed the spacing between the keys when the log window was expanded: the keys
were spaced across what was left after a 56-column lie.

**The keys sit on the right edge, two columns apart, and degrade in three tiers.**

They used to be spaced across the whole row, the slack divided into equal gaps
with the remainder handed out one column at a time so the row spanned the full
width. That kept every key in the same place at any size, and it also pulled them
apart as the window grew, until reading the row meant crossing ninety columns of
blank to find the next key. Fixed spacing keeps the group readable *as a group*,
and the right edge keeps it beside the diagnostics rather than leaving a gulf
between the two.

The tiers, in the order they are given up, because this row must never truncate:

| Tier | Given up | Running row needs |
| :--- | :--- | :--- |
| 1 | nothing — two columns between keys | 86 columns |
| 2 | one column between keys | 80 |
| 3 | the words; keycaps only | 26 |

`[^C]` on its own is still a key you can press. A `[^C` that was cut off is not,
and nothing on screen would have said it had been — which is what the row did
below 80 columns before the tiers existed, and it dropped the stop keys first
because they sit last.

The gaps count as content in every fit test, including the one that decides
whether the diagnostics group is kept. Leaving them out is how the row overflowed
at 106 columns: seven keys measured 84 against 87 available, and the missing six
columns were the gaps.

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
  resolve the toolchain           FRUN_FLUTTER · .fvmrc + fvm · flutter on PATH   (8.11)
       │                          once per process; every Flutter call goes through it
       ▼
  read SDK versions
    ├─ fast   fvm    .fvm/flutter_sdk or .fvmrc → bin/cache/flutter.version.json
    ├─ fast   plain  which flutter → <sdk>/bin/cache/flutter.version.json
    └─ slow   flutter --version --machine   ⠋ 3-4s, boots the Dart VM
       │
       ▼
  ╔═══════════════════════╗
  ║ render ProjectCard    ║   needs the SDK versions, so it cannot paint
  ╚═══════════╤═══════════╝   before the step above resolves
              ▼
      ┌───────────────────┐
      │ 1  DETECTING      │  adb + simctl + -list-avds, ~200ms   (8.9)
      └─────────┬─────────┘  flutter devices --machine behind it,
                │            landing as a refresh 6-8s later
                ├── exits non-zero ──► ✖ FATAL  Failed to detect Flutter devices
                │
                ├── nothing at all ──► ✖ FATAL  No device(s) detected
                │
                ▼  one list, always (8.10)
      ┌──────────────────────────────────────────────┐
      │ 4  MULTIPLE_DEVICES  (picker)                │
      │    running, in use and last used on top,     │
      │    then every bootable target                │
      └───┬──────────────────────────────────────┬───┘
          │ Esc                                  │ Enter
          ▼                                      │
   ✖ CANCELLED (130)                             │
     ┌─────────── needs booting ─────────────────┤
     │                                           │ attached, nothing to start
     ▼                                           │
┌─────────────────┐                              │
│ 3  BOOTING      │  avd: poll sys.boot_completed, 180s cap
│ ⠋ + elapsed     │  sim: open -a Simulator, simctl bootstatus -b
└────┬───────┬────┘                              │
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
      │ 6  BUILDING              │  pty spawn: <toolchain> run -d <id>
      │                          │
      │  stages are platform-    │  iOS      pod install → Xcode build
      │  dependent, not a fixed  │  Android  Gradle task → install APK
      │  list                    │  both     syncing files
      └────────┬─────────────────┘
               │
               ├── failure ──► ┌──────────────────────┐
               │               │ 7  BUILD_FAILED      │
               │               │ summary + stack trace│
               │               │ r retries, on footer │
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

  ^D from any of 6-11                    ^S from 6, 8-11
       │                                      │
       ▼                                      ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 12 SWITCH  (8.5)             │   │ 13 STOPPED  (8.8)            │
│ ⏎ other  → kill, reap, 6     │   │ the child is gone, frun is not│
│ ⏎ same   → back, nothing done│   │ r  → build again → 6          │
│ Esc      → back, nothing done│   │ ^D → 12                       │
└──────────────────────────────┘   │ q  → leave                    │
                                   └──────────────────────────────┘
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

**State 13 is the first frame with no child behind it.** Everything above assumes a
run is coming or here, which is why stopping used to mean leaving: there was nowhere
to stand. See 8.8.

**State 12 is not a step in the flow, which is why it hangs off the side of it.**
Every other frame is reached from exactly one place; this one is reached from six,
and it returns to whichever of them it came from. It is a state rather than a flag
on state 4 for a reason worth keeping: the two frames draw the same list and mean
different things by it, and a frame that no `--dump` slug can name is a frame
nothing checks (8.5).

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
| `↑` / `↓` | Navigate target list | States 2, 4, 12 |
| `1`-`9` | Select the nth target directly | States 2, 4, 12 |
| `Enter` | Select device, or start the highlighted target | States 2, 4, 12 |
| `⇧Enter` | Launch the highlighted target in a new terminal tab (8.4) | States 2, 4, 12, where the terminal reports the Kitty keyboard protocol |
| `Esc` | Cancel and exit (code 130) | States 1, 2, 4 |
| `Esc` | Back to the run, nothing killed | State 12 |
| `r` | Hot reload | States 8, 10, 11 |
| `r` | Retry build — kill, reap, respawn | State 7 |
| `R` | Hot restart | States 8, 10, 11 |
| `↑` / `↓`, `j` / `k` | Scroll the log window | States 6, 8-11 |
| `e` | Give the log window the whole frame (expand/collapse) | States 6, 8-11 |
| `q` | Quit gracefully — Flutter shuts itself down (`⏏`) | States 6, 8-11 |
| `^C` | Force stop — SIGINT forwarded to Flutter (`⏹`), ends the process | Any |
| `^D` | Switch device — reopen the target list over the live run (8.5) | States 6-11, 13 |
| `^S` | Stop the run and stay in frun (8.8) | States 6, 8-11 |
| `d` / `D` | Flutter's detach, forwarded — and frun stays, in state 13 (8.8) | States 8-11 |
| `r` | Build again, after a stop or a detach | State 13 |
| `m` | Toggle mouse capture | Global |

`:` is not bound. It used to open the command prompt, which is gone (3.6), so the
key now forwards to Flutter like any other.

The digit keys are scoped to the two states that show a list, and the arrows are
scoped by what is on screen: a log window if there is one, the device list
otherwise. There is never both. Everywhere else a digit is Flutter's and has to
arrive unchanged.

`j`, `k` and `e` are the three letters frun takes from Flutter beyond the table
above, all three for the log window. Flutter binds none of them, and reaching a
stack trace eight rows tall inside a twelve-row window is worth them.

`^D` and `^S` are not a fourth and fifth letter, and that is the point of them.
Flutter's interactive commands are all bare single bytes, so a modifier costs nothing
here — where the mnemonics `d` and `s` would have cost detach and the screenshot key
(8.3, 8.8). `^S` is XOFF under a cooked terminal; raw mode clears `IXON`, so it
arrives as a key event like any other.

**Three ways out of a run, three words.** `^S` ends the run and stays in frun. `q`
ends the run and leaves. `^C` ends the process whatever the run is doing, which is
also the escape hatch behind `^S`: if Flutter is wedged and never answers the request
to shut down, `^C` still works, and that is why `^S` needs no timeout of its own.

`⇧Enter` is the same bargain and one step further: it takes no letter at all, and
`Enter` in those three states is frun's already. What it costs instead is a
capability the terminal has to grant. In the legacy encoding `Enter`, `⇧Enter` and
`^Enter` are all CR, so crossterm reports `KeyCode::Enter` with no modifier and an
arm matching on `SHIFT` can never fire. It becomes distinguishable only once the
Kitty keyboard protocol is pushed, which is 8.4's to do and 8.4's to explain.

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
after borders, padding and a full 18-column gutter, roughly 78 columns remain,
so a framework assertion loses about 50 characters off the end.

Those lines **wrap**, continuation rows indented to the message column with
the gutter left empty. Truncating is not acceptable for a stack trace, which
is exactly the output you most need to read in full.

Two consequences to design around:

* A single Dart exception can occupy 6-10 rows once wrapped, so the log
  window must be sized on the assumption that one entry is not one row.
* Consecutive lines from the same source share a timestamp and level. Those
  repeat the gutter for no information. Continuation rows omit it and indent to
  the same width the first row drew (3.5).

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
  DEVICE INFO            10 rows   2 border + 1 title gap + 7 body
                                   body = 4 fields + 3 separators
                                   the run status is a title-bar pill, not a row
                                   [^D] is on the footer, not a row either
  BUILD PHASE            10 rows   2 border + 1 title gap + bar + blank + 5 stages
                                   the stage count is live, so this is the tallest
                                   case: an Android build with all five phases
                                   announced. iOS ends at 9.
  footer                  1 row
  gaps between blocks     3 rows   blocks - 1, not a constant
  ────────────────────────────────
  TOTAL                  36 rows
```

That is the tallest the frame ever gets, and it is a frame with a build in it.
`RUNNING` is a different shape rather than a shorter version of it: the tracker
block is absent there entirely (3.4), so the total is **25** — two cards, a footer
and two gaps.

**`STOPPED` is 25 as well, and that is deliberate.** Every fact the tracker's summary
row carried has a home that costs nothing — the two totals in the log card's title,
the ending word as a pill in the target card's title — so the two frames are the same
shape and the log window does not resize when a run ends. It used to grow a banner and
push every line of the stream down two rows, at the moment the user is most likely to
be reading one.

At the 106x45 target that leaves the log window **20 rows**, against the **8** a
five-line Dart exception occupies when wrapped at 84 columns of message space. Nothing
is conceded at the design size any more; the ladder below starts mattering at 37 rows
and under.

This total has come down four times, and every time the rows went to the log window
rather than back to the layout. It was 45, leaving nothing at all, until the prompt bar
and its gap were removed (3.6). It was 41 until the target card gave up its status
banner and its command string, with the blank each of them needed (3.2). It was 27 for
a run until the settled tracker's row and gap went too (3.4). And it is 25 now that
`[^D] Switch` and the blank above it left the target card for the footer, which is the
third in-card action row to go that way.

**Every one of those four was the same mistake and the same fix.** A component held a
row for something a border row or a shared row was already carrying, or could carry for
nothing: a command string that described an argv, a banner whose every fact was in the
table beneath it, a summary whose numbers now sit in the log card's title, a keycap that
belongs on the cheatsheet. Nothing was reorganised for tidiness — each time the question
was which component *owns* the fact, and the row followed the answer.

**The tracker's height follows the number of rows it actually has.** `build_h`
takes the count rather than assuming one, so the card is four rows tall while four
phases have been announced and grows as the fifth opens.

It was a fixed six, chosen so the card would never change height mid-build. That
reserved space for phases nobody had announced yet: during `Starting Flutter` the
card held one row of content and five blank ones, which reads as a rendering fault
rather than as room. Growing costs the log window a row each time a phase opens,
which is a visible reflow, and that is the better trade — the reflow is the build
making progress, whereas the blank rows were describing nothing.

Capped at eight (`MAX_STAGES`), so an unforeseen flood of phases cannot push the
log window off the screen. Android's six plus CocoaPods and `pub get` is the most
that has ever been observed.

Charging exactly what is drawn is not optional here: this is trap one in 7.5, and
a row past the charged height is clipped with no error at all.

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
  LOG_MIN  = 12 rows     enough for one wrapped exception plus context
  FAIL_MIN = 14 rows     the compiler-error card, whole
  else        3 rows     the device lists, which scroll
```

Two floors and a default, decided in one place — `Budget::floor(state)` — because
the flexible middle is a different region in different states and `--rows` reports
the number the solver used. A list needs no floor of its own: it scrolls, and in
every state that shows one the cards above it are hidden anyway.

The floor is defended during `BUILDING` as well, now that the log stream is on
screen from the start of the build (3.5). In practice that costs the separators
during a build and keeps the tracker's stage list, which is the right way round:
the stage list is the thing changing.

Elements are given up in this order until the floor is met, cheapest first:

| Order | Given up | Reclaimed |
| :--- | :--- | :--- |
| 1 | Row separators in both cards | 6 rows |
| 2 | Device rows go dense, one line each | ~7 rows in States 2 and 4 |
| 3 | BuildPhaseTracker collapses to one summary line — only reachable while a build is still running, see below | 9 rows |
| 4 | ProjectCard and SelectedTargetCard collapse to one metadata row each | ~20 rows |

Rung 3 is unreachable in `RUNNING` and the reload states for a second reason now:
there is no tracker there to collapse (3.4). It applies to `BUILDING`, where the
stage list is what is moving, and to `BUILD_FAILED`.

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

**Step 3 is not a step at all once the build has settled.** `Budget::solve`
switches `full_build` off before the first concession is considered, so the
tracker is one row at every height in `RUNNING`, `STOPPED` and the three reload
frames. See 3.4 for what that row carries.

This arrived in two attempts and the first one is worth recording, because it was
half right. The observation is the same either way: after the build ends every row
the tracker holds is frozen, and it has no business holding nine of them while the
log window — the only region still changing — is at its floor. The first fix
believed that and still made the size decide it, by having `concede` take the
state and promote the tracker to the front of the queue. So a 45-row window got
its rows back and a 62-row window kept all nine, on the reasoning that it could
afford them.

Affordability was the wrong question. A claim weak enough to lose to the
separators at 46 rows is not a claim that becomes legitimate at 62; the rows were
just as static there, and the log window just as hungry. Rung 3 was doing two
different jobs — reclaiming rows under pressure, and expressing a fact about
finished builds — and only the first belongs in a ladder.

Separating them paid twice over:

* **The ladder is one fixed order again**, with no `state` parameter and no
  branch. The table above now stands as written in every frame.
* **`STOPPED` stops being an exception nobody noticed.** `build_done` is false
  there, because that predicate answers "is there a live child to send a key to",
  so a stopped run was ranked as though its build were still in progress. At 45
  rows it paid the separators *and* the roomy device rows to keep ten rows of
  finished timings, in the one state whose entire purpose is reading the log. The
  tracker now asks `State::build_settled`, which is `build_done` plus `STOPPED`,
  and the two frames behave identically.

`BUILD_FAILED` is deliberately not settled and keeps the full card: the failure
note names which stage broke, so there the stage list is context for the error
rather than history.

The rung itself stays in the ladder for the frames where the rows are
load-bearing. Deleting it outright was the tempting version of this and it does
not fit: at `60x14` a build with both cards already collapsed still needs 19 rows
if the tracker is expanded, and a layout charged more rows than it has is clipped
in silence (7.5). So during `BUILDING` the tracker is the last thing given up, and
on a terminal that small it is given up.

What this buys, from `--rows running 106` — the numbers on the left are what the
promoted-rung version produced:

```
  rows      before   after
    45          18      18     unchanged, it was already collapsed there
    50          14      23
    56          20      29
    62          26      35
```

Full chrome for a settled run is 27 rows rather than 36, and there is no height at
which the tracker is expanded over a live log again.

One further consequence, small but the kind that rots: `Budget::describe` takes
the state and omits `build collapsed` when the collapse was state-driven. The
footer prints that string, so without it every run would have carried a permanent
`[build collapsed]` tag reporting a concession the layout never made — the same
defect 6.3 records for the expanded log view. `--rows` says it once in its header
instead of ten times in a column about the ladder.

Nothing below `60x14` is drawable. At that point the app says so rather than
rendering a broken grid.

**Width.** Cards stop widening at 142 columns so a very wide window does not
stretch a label to one edge and its value to the other. The log window is
exempt and takes every column available, because more columns means fewer
wrapped rows per entry.

---

## 🚧 7. Implementation status

### 7.1 Status

All thirteen state frames render at any terminal size, and every value on them is
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
| 3. `RELOAD_DROPPED` (11) | tested, not forceable | attempted live by sending `r` during a hot restart; Flutter queued the key and serviced it instead of dropping it. Three tests cover frun's half: a request with no ack resolves, an acked one is never dropped, and a late ack reopens it |
| 4. Boot, and `NO_DEVICES` (2) | live | AVD booted through `sys.boot_completed` on its own serial, picker skipped, straight into the build. Re-verified with a phone attached, which is the case that broke it (7.5) |
| 5. Shell cutover | live | `frun` through the shim: flag passthrough, fatal path, exit codes 0 / 1 / 130 |
| 7.6 Progress denominator | live + tested | `Stage 1/5` on a real build, now `x/6`; a test walks a build and forbids the fraction decreasing |
| One row always spinning | tested | iOS and Android transcripts replayed; `open == 1` asserted after every line, and `0` only once the build ends |
| Closed rows never re-timed | tested | a row's duration is compared before and after two later lines arrive |
| Live per-stage timer | tested | the `building` mock holds a row opened six seconds ago, so one frame proves it ticks |
| 7.6 Log stream during build | live | on screen from the first frame of a real Gradle build |
| 7.6 Merged picker | live | 16 rows on a real machine, running first, `Pixel_10_Pro_XL` de-duplicated against the running `emulator-5554` |

| 7.6 Log scrolling | tested | rendered at a size where content overflows; asserts rows change and the offset was not clamped to zero |
| 8.4 New tab, the spawn and the title | live | the AppleScript run against Ghostty: a tab opened, and reported back `FRUN_DEVICE`, its working directory, and `fvm`/`adb` resolving on the handed `PATH`. `OSC 2` returned `iPhone 17 Pro · cwclub` as the tab's own name |
| 8.4 New tab, `⇧⏎` and `FRUN_DEVICE` | **unrun** | the key needs a press under the pushed flags; `handed_over()` needs a real project and a device. Three tests cover what the argv and environment must contain |
| 8.5 Switch device, the frame | tested | `--dump switch`; a test asserts the title, the `running` badge, `Esc → Back`, and that the target card and tracker are gone |
| 8.5 Switch device, the respawn | **unrun** | kill, reap, respawn onto another device, and shutting the outgoing emulator down. No harness reaches the pty; needs a project and two devices |
| 7.7 Expand log (`e`) | tested | fills the frame at the right width |
| 7.7 Marker gap durations | tested | the gap fills on the next stage, and the final row stays blank |
| Mouse capture | unrun | `m` toggles it; only the geometry is covered, by `--hits` |

One of these is worth naming as a gap rather than leaving in a table. State 10
(`RELOAD_FAILED`) has not run against real Flutter output; it needs a Dart compile
error introduced while the app is live. It is ported line for line from
`frun-runner`, where it has been in daily use, which is the argument for believing
it — not the same as having seen it.

**State 11 is a different case, and worth being precise about.** Its trigger is not
frun's to produce: Flutter has to silently discard a keypress. An attempt to force
that by sending `r` during a hot restart failed — Flutter queued the key and
serviced it when the restart finished, rather than dropping it. So the trigger
remains unobserved here, and the `frun-runner` comment describing it (7.5) is the
only evidence it happens at all.

What *is* covered is frun's half, which is the half that can go wrong: a request
that is never acknowledged resolves instead of spinning forever, an acknowledged
one carries no deadline and so cannot be falsely dropped, and a late acknowledgement
reopens a stage already declared dropped. That last one is why a wrong timeout is
harmless: `runSourceGenerators()` runs before Flutter's progress line appears, so a
slow accept and a drop look identical for a moment.

What the live runs covered, for the record: a merged picker with 16 rows, a
`NO_DEVICES` screen, a booted AVD and an attached-emulator run, a Gradle build
reaching `build finished` in 10.6s, 908 log lines in one session, a hot reload, a
hot restart, a build failure, a number hotkey selecting a row, `Esc` at the picker,
and `q` from the log stream. Nine of the thirteen state frames have been on screen
with real data behind them. `RELOAD_DROPPED` has not, `SINGLE_DEVICE` no longer can
be, and `SWITCH` (8.5) has not been driven on a real machine yet — it renders and its
logic is tested, but the kill-reap-respawn behind it has never run against a device.

Two of the 7.6 fixes are covered by render tests rather than live runs, and that is
a deliberate choice rather than a shortfall. Log scrolling does nothing unless the
content overflows the window, which depends on how chatty the app happens to be at
the moment a key is pressed. Tests can guarantee the condition; a live run can only
hope for it. Three attempts to catch scrolling live all landed on a window that was
not full, which is exactly the false pass the tests are written to exclude.

The same reasoning applies to the one-row-spinning invariant, for a sharper
reason: the failure is a *gap between* two output lines, so no single frame can
show it and no single line can be tested for it. Replaying whole transcripts is
the only way to see it at all, which is why that check feeds an iOS and an Android
transcript line by line rather than asserting on a rendered frame.

`src/` layout:

```text
  theme.rs     palette + Nerd Font glyph vocabulary
  data.rs      App state, the 11-variant State enum, mock data per state
  budget.rs    responsive ladder; owns every component height
  widgets.rs   pill, badge, keycap, card, spread, field, separator, elide, wrap
  probe.rs     pubspec, git, toolchain + SDK manifest, device discovery, boot
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

53 tests. The ones that matter most: no row in any state at any size exceeds its
width budget (measured in cells, not bytes), every state fills exactly the height
it was given, the drawn log gutter matches the continuation indent at every entry count and level, and every
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
the answer. The frame it applies to is now ~250ms long rather than 6-8s (8.9), so
this is a rule about a window that is hard to press a key inside of.

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
| `portable-pty` | spawning `flutter run` behind a pty |

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

**Half of that is superseded by 7.6, and the other half by 8.10.** What went first was
the branch it fed — skipping the picker when exactly one device is attached. Running it
for real showed why: with only the iPhone simulator up there is no way to ask for
Android, because booting is a choice and that branch removes it. The merged single-list
picker in 7.6 replaces it.

The counting rule itself was kept here on the grounds that it is "what keeps
`NO_DEVICES` reachable and its subtitle true", and that turned out to be false the
moment the lists merged. `Device::attached()` is `platform.needs_boot()`, so the
bootable rows 7.6 added to the list count as attached, and the count it guards can no
longer be zero on a machine with one simulator installed. 8.10 removes the state rather
than the rule: `attached()` survives as the sort key it had already become.

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
this project only rendering. Its appeal is real: zero risk of regressing the ack
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
  `start_stage` is keyed on an enum and idempotent, which is what every
  `if not pods_started` guard was doing by hand. It ended up being the only stage
  operation there is: closing a row happens inside it, as the next row opens, so
  there is nothing for a `finish_stage` to do (see 7.6).
* **`FRUN_LOG_DELAY` is not ported.** The five-second hold before releasing app
  logs existed because the shell printed the build summary and the log stream into
  one scrolling region, so a chatty app scrolled the summary away. The tracker and
  the log window are separate regions here, so there is nothing to protect them
  from. Startup logs still buffer, for free: `has_logs()` is false until the
  session is ready, so they simply are not on screen yet.

Also: stages time themselves when Flutter reports no duration. `Syncing files` and
`Application Running` never carry one, and measuring is both honest and
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
frun() { "$HOME/.config/zsh/flutter-run-tui/target/release/frun-tui" "$@" }
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

The first fix for that was a wider constant, and it bought a second defect with the
same shape: the test measured one level, `INF`, so the one-cell reload bolt went on
drawing a 16-column gutter under an 18-column indent, and eight entries reserved
four columns for one digit. A width that describes drawn output has to be *derived*
from the output — the count and the padded badge — and the test has to walk every
input that can change it. Both are now the same expression, and 3.5 records what
that costs.

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

**Findings about Flutter and the platform, not about this code.** Harvested from
the comments in the shell implementation this replaced (`frun.zsh`, `frun-runner`,
`frun-theme.zsh`; the snapshot lived in `reference/` and is now only in git
history, last present at `08e1f53`). None of them are derivable from Flutter's
documentation and each was paid for once already, so they are recorded here rather
than left in an archive nobody reads.

* **Flutter formats elapsed time through `NumberFormat`.** Durations carry a
  group separator, so `Restarted application in 1,234ms` clips to `234ms` if
  matched with `[0-9]+`. There is a test pinning this (3.4).
* **Flutter re-emits its progress lines with partial elapsed values.** The
  `Running Gradle task '<task>'` line is animated through CR and each redraw
  carries a longer figure, so treating the first one seen as completion reports a
  far too short build. `Built build/...` is the unambiguous finished signal.

  This is survivable for the tracker, which uses those figures to place a phase
  *boundary* rather than to decide a phase has ended (3.4). A partial value is
  still an honest elapsed-so-far, so `now - partial` lands on the same start
  either way; what it must never do is close a row.
* **Flutter's timers start before its announcements arrive.** On a cold iOS build
  it began the Xcode build 16.3 seconds before printing `Running Xcode build...`.
  Any attempt to time a phase from the line that announces it will understate that
  phase and overstate whatever came before (3.4).
* **Flutter discards terminal input while it is busy, and says so only through
  `printTrace()`**, which never reaches stdout. A keypress is a request, not a
  fact. This is the entire reason `HOT_RELOAD_DROPPED` (state 11) exists: without
  a deadline the spinner it starts has no terminating condition and runs forever.
* **macOS has no `timeout(1)`.** The Android boot wait is a bounded counter rather
  than a wrapped command, and `probe::run` carries its own deadline for the same
  reason.
* **`flutter.png` has ~79px of transparent padding per side**, which renders as
  dead columns. `flutter-trim.png` is the same artwork cropped to its content
  (3.1).

**`adb` without `-s` asks whichever device happens to be attached.** With a phone
on wireless adb and a freshly spawned emulator not yet registered, `adb shell
getprop sys.boot_completed` was answered *by the phone* — instantly, with `1`. The
boot was declared finished after about a second, the serial lookup then found
nothing, and the run died with `booted, but adb never reported a serial for it`
while the emulator carried on booting in plain sight.

The fix was to establish the serial first and address everything after it with
`-s`. The emulator is identified as the `emulator-*` serial that was **not** in
`adb devices` before we spawned one, which also retired the old name lookup: a
serial that appeared is the emulator we started, and no name has to match for that
to be true.

Worth noting how this hid: every boot test passed until a phone was attached. A
command that reads correctly with one device and silently addresses the wrong one
with two is invisible to any test run on a clean machine.

**Anything that can be done in two steps will eventually be done in one of them.**
A stage row was closed by one trigger and charged by another, and both defects that
came out of it — a tracker sitting idle mid-build, and a finished row whose number
kept moving — are the same shape: two operations that must coincide, expressed as
two things that can happen apart. The fix each time was not to synchronise them but
to make it impossible to do one alone. `finish_stage` was deleted rather than
corrected.

The same shape is worth watching for elsewhere in this codebase: a row drawn
without being charged in `budget.rs` (trap one), and a gutter drawn at one width
while continuation rows indent to a constant (trap three) are both two expressions
of one fact.

### 7.6 Defects found by running it, and their fixes

All five were found by using it against a real project rather than by reading it.
All five are now fixed. Each entry keeps its diagnosis, because the diagnosis is
the part worth having later, and records what was actually done.

**Progress bar runs backwards.** `build.rs` computes `done / total` with
`total = app.stages.len()`, and `flutter.rs` appends stages as Flutter announces
them. At `Flutter started` that is 1 of 1, so 100%; when Gradle appears it
becomes 1 of 2, so 50%.

3.4 forbids a denominator and the implementation contradicted it.

**Done, and 3.4 is what changed.** The first instinct was to drop the denominator
entirely — indeterminate while building, full on completion. That was wrong for a
simpler reason than it looked: the total *is* knowable. The platform is chosen
before the build starts, and 3.4's own trigger table is per-platform, so the stage
set follows from the target. `Platform::stage_count` states it: five for Android,
four for iOS and macOS, four for web — Android being the one that installs the APK
as a step of its own. CocoaPods and `pub get` are not counted, since both are
skipped on most runs, and neither is the generic `Preparing build` row, which is
adopted by the first platform phase rather than surviving beside it.

It is an upper bound, never a floor, and that asymmetry is what makes it safe.
Flutter skips stages it does not need — no `pod install` when `Podfile.lock` is
current, no install when attaching to an app already present — so the bar can
stall a stage short and then complete, which reads correctly. The failure 3.4
warned about, a bar reaching 100% and then carrying on, cannot happen from an
upper bound. `expected_stages` also floors the total at the number of stages
already seen, so a platform that announces more than predicted still cannot
overflow it.

The bar now shows `Stage 4/5`, and the count of stages that actually ran on
completion. A test walks a full build and asserts the fraction never decreases.

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
elapsed clock on the stage currently running. Its reason still holds — a spinner
alone cannot distinguish slow from stuck.

It arrived with `frun-runner`'s three-second delay before the clock appeared, and
that delay is now gone too. A row that has just opened reads `0ms` and starts
moving. The delay existed so a fast stage never showed a number, but the effect was
that the fast rows looked untimed, and it put a change of units in the middle of a
row's life.

**Done, all three.** `State::has_logs()` now includes `Building`, so the middle
region carries the log stream from the moment the build starts, and
`waiting_for_build` is deleted. Every stage row carries a clock, ticking from zero
while it is open.

Two consequences worth noting. The `LOG_MIN` floor in 6.2 is now defended during
`BUILDING` as well, so the build tracker keeps its detail and the separators are
conceded instead. And the third item here — the unmarked startup gap — turned out
not to be fixable by the log stream alone: output during the gap explains *what*
is happening, but the tracker still showed no row for it. That needed a row frun
opens itself, which is `Starting Flutter` above.

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

**Done.** `probe::targets` builds the one list: attached devices, then AVDs, then
shut-down simulators, then the always-available platforms. Verified against a real
machine at 16 rows.

Three things fell out of it, each a deletion:

* `Boot::Ready` is gone. A target that needs no booting is `boot: None`, which is
  what an already-running device already said, and both mean the same thing at the
  moment they are picked: launch now. Two spellings of one fact is one too many.
* The `▶ Start` / `▶ Run` badge was decided per row rather than per frame, because
  with one merged list a per-frame flag would have labelled a shut-down simulator
  `Run` purely because a phone happened to be plugged in. The badge is gone
  entirely now (3.3): one answer shared by every row is the footer's job.
* `enter()` lost a branch. Both frames now hold the same list, so the only
  question is whether the chosen row needs starting first.

De-duplication is the one part that needed care: a running emulator appears in
Flutter's list under its AVD name, because `android_name` resolves the serial back
through `adb emu avd name`. `targets` skips an AVD whose name is already attached,
so it never offers to boot a device you are looking at. Booted simulators cannot
collide, since `simctl` is asked only for shut-down ones.

**`SINGLE_DEVICE` (state 5) is now unreachable in the live flow.** The variant is
kept rather than removed, and this is a deliberate trade: states 6 through 11 are
referenced by number throughout this document, and renumbering them to close a gap
would be a far larger and more error-prone change than leaving one frame that only
`--dump single` reaches. It stays a rendering target, not a state.

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

**Done.** `log_scroll` counts rows back from the live tail, so zero is the bottom
and new output arrives without having to be followed. Arrows and `j`/`k` route to
whichever list is on screen — there is never both — and the wheel moves three rows
a notch. The title says `· scrolled` when the window is not at the tail, because
scrolling back and then seeing nothing new is otherwise indistinguishable from the
app having gone quiet.

The clamp lives in `logs.rs`, not next to the keypress, and that placement is the
point: the ceiling is a count of *visual* rows, which is only known after wrapping
at the width the window is actually drawn at. One Dart exception is eight rows, so
clamping against an entry count would stop in the wrong place.

Verified by two render tests rather than live. Scrolling only does anything when
the content overflows the window, and whether it does at any given moment depends
on how chatty the app happens to be — an unreliable thing to hang a check on. The
tests render at a size where it definitely overflows, assert the visible rows
change, and assert the offset was not silently clamped to zero, which would make a
scroll that does nothing look like a pass.

### 7.7 Open questions, now answered

**More log on screen.** Font size is not available to a terminal application; one
font at one size covers the whole grid, and only Ghostty can change it. The
app-side equivalent is a key that hides the three static cards and gives the log
window the screen: at 62 rows that is 17 rows becoming 58.

**Done: `e` (Expand).** It bypasses the `Budget` rather than being taught to it, because
there is no ladder to solve when one block occupies the frame — the whole point of
the ladder is choosing what to give up, and zoom/expand has already answered that. Two
details:

* Width is taken unclamped, ignoring the 142-column cap. That cap exists so a card
  cannot stretch a label to one edge and its value to the other, and there are no
  cards here; more columns means fewer wrapped rows per entry, which is the entire
  objective.
* The footer swaps its own label to `e Collapse` when expanded, so the key that got you
  here is visibly the key that gets you back. It is the only thing on screen at
  that point, and a mode with no advertised exit is a trap.

`e` is taken from Flutter's forwarding, which 5.1 otherwise forbids. Flutter binds
neither `e` nor `j`/`k`, and this is the second of the two places where frun claims
a letter for the log window.

**Marker stages show `0ms`, and blanking them is the wrong fix.** `Flutter
started` and `Application Running` are markers; nothing takes zero
milliseconds. But emptying the column would remove the only place the eight-second
startup gap could ever appear, since that gap happens immediately after `Flutter
started`.

So a marker row carries the time from itself until the next stage begins. Both
timestamps are already known, so the figure is measured rather than invented, and
the most meaningless number on the card becomes the most informative one.

**That was right about the fix and wrong about its scope, and the scope is what
changed.** Two rows were treated as special cases while the rest kept Flutter's
own figures. Applying the rule to *every* row instead — each one measured from its
announcement to the next — removed the special case, removed the mixing of two
time sources, and turned out to be the same change that fixes the idle-tracker
defect above: a row that is charged at the next announcement can also be *closed*
there, and then nothing ever stops spinning early.

What it looks like now, with the two rows this question was about at either end:

```text
  ✓ Starting Flutter               3.6s     fvm, Dart VM, flutter_tools
  ✓ Launching lib/main.dart        1.1s     pub, Gradle daemon warmup
  ✓ Gradle task assembleDebug      2.8s
  ✓ Installing app                 1.2s
  ✓ Syncing files                   81ms
  ✓ Application Running             0.9s     closed by the end of the build
```

`Starting Flutter` is opened by frun when the pty spawns, which is what gives the
startup gap a row of its own rather than hiding it inside a marker's figure. The
last row is closed by build completion, the one row with no successor to close it.

The cost, stated plainly: `2.8s` is our measurement, not Flutter's `2,834ms`. The
column now sums to the build total, which the mixed version could not, but it no
longer agrees with what raw `fvm flutter run` prints for individual stages.

**Done, and it is mostly deletion.** `start_stage` closes the previous row and
charges it, together, and is the only place either happens. `StageKey::is_marker`,
`finish_stage`, `stage_duration`, `ELAPSED_AFTER` and the `waiting` branch in
`build.rs` are all gone, along with every `if gradle_started && !gradle_completed`
safety net — those existed because a close could be missed, and a close that cannot
happen on its own cannot be missed.

Both timestamps were already being recorded, so this added no measurement — only
the arithmetic between two numbers that were sitting there unused.
---

## 🧭 8. Target-card controls and concurrent runs

**Points 3 and 4 are built (8.4, 8.5), and 8.8 with them. Point 3 is built in a
different shape from the one it was asked in — a second process rather than a tab bar
— and 8.4 says why. Points 1-2 are not built.** This section was written before any
of it existed so the costs would be argued once here rather than discovered one at a
time in the diff, and 8.4, 8.5 and 8.8 are now descriptions of code rather than
proposals. Everything else below is still a plan. Each item carries what has to
change and what it is paid for with, because in this layout every new row is taken
from the log window (6.2) and every new letter is taken from Flutter (5.1).

### 8.1 What was asked for

Four points, as stated:

1. **Controls exclusive to the `SelectedTargetCard`.** Starting an additional
   device and changing the current target are offered only inside the active
   target card. No launcher controls in the header, the device list, or the
   footer.
2. **No dialog, modal or overlay.** Pressing the control opens a settings panel
   *inline*, inside the card's own borders, drawn with the same box characters as
   everything else.
3. **Terminal tabs for additional devices.** A tab bar above the log window. `➕
   Run another device` opens a new tab with its own independent runtime — its own
   build stages, its own log buffer, its own hot reload, its own prompt. Tabs are
   switchable and closable.
4. **Reuse the session when changing target.** `🔄 Change target (this session)`
   retargets the *current* tab without opening a new one, keeping the interface
   and the history that is already there.

The wording those points arrived in — modal, backdrop, "when the button is
clicked" — is DOM wording. There is no overlay primitive in this codebase to
remove: ratatui draws into a cell buffer, and an overlay is something you would
have to build with `Clear` and hand-computed geometry. So point 2 is not a
cleanup, it is a constraint on what gets built. It is also the only sane shape
here, and it is recorded as a rule rather than as a change.

### 8.2 Verdict

| Point | Possible | Cost | Blocked on |
| :--- | :--- | :--- | :--- |
| 1 — controls only in the target card | Yes | Small | A key that is not Flutter's |
| 2 — inline, no overlay | Yes, already the only option | Small-medium | A variable-height card in the `Budget` |
| 3 — tabs, in-app | Yes | **Large — architectural** | `Msg` identity, splitting `App`, quit semantics |
| 3 — tabs, as a second process | **Done** | Small | — |
| 4 — retarget the current session | **Done, not as described** | Small | — |

Point 4 was the cheapest and the most mis-described, and it shipped first: 8.5 is
what it turned into. Point 3 is the only one that is not an addition to the
existing shape but a change of it — which is why the row is split, and why the
second row is the one that will be built. 8.4 has the argument.

One thing point 4 did **not** need, and it is worth naming here because 8.3 and
8.4 assume otherwise: the inline panel. The list frun already has is the device
list, so the switch reuses it — a twelfth state that draws the same rows, and no
rows taken from the log window. So the panel is now optional where it used to be a
prerequisite: 8.3 buys presentation, not capability.

### 8.3 Controls in the card, and the panel inside it

`ui/target.rs` is read-only today: `render` takes `&App` and every field is read
off `app.target`. Two things follow from putting controls in it.

* It needs `&mut App`, to push into `app.hits` the way `chrome.rs` and
  `devices.rs` already do. Hit regions are rebuilt every frame on purpose (7.5) —
  a stale list leaves invisible buttons at coordinates a card no longer occupies —
  so nothing about that changes, only which module contributes.
* `Action` grows the two verbs, and `apply()` grows the two arms. One path for
  keys and clicks, as now.

**Clicking cannot be the only way to reach them.** Mouse capture is off by
default and deliberately so (5.2); a control that only answers the mouse is dead
in every session where `m` was never pressed. So each verb needs a key, and the
key budget is the hard part: `q m r R e j k ^D` and the digits are frun's, and
every other letter is Flutter's and has to arrive unchanged. `j`, `k` and `e` were
already justified one at a time and that argument does not extend a fourth and
fifth time.

The way out is modifiers. Flutter's interactive commands are all bare single
bytes, so `Ctrl-T` (new tab) and `Ctrl-D` (switch device) take nothing from it, and
crossterm reports the modifier separately, so no ambiguity has to be resolved.
**The new verbs are the first frun keys that are not plain letters, and that is why
they are affordable.** `^D` is taken already, by 8.5.

**`^T` is released, and one verb is left for this card.** 8.4 puts the new-tab verb
in the picker, on `⇧Enter`, where a device is already being chosen — so nothing in
the target card has to offer it. That halves what the panel would carry: one
control, which 8.5 already ships as an inset title costing no rows. What is left of
8.3 is presentation for a control that exists, which is the conclusion 8.2 reached
about the panel from the other direction.

The mnemonic that had to be turned down is worth recording, because it is the
trap 5.1 exists to catch. `[s] Switch Device` reads better than `[^D]` and `s` is
Flutter's screenshot key — it writes `flutter.png` into the project root. Nothing
would have failed loudly; screenshots would simply have stopped working, which is
exactly the silent removal 5.1 forbids. `S` is taken too, by the accessibility
tree dump. There is no good bare letter left for this, and that is the whole
argument for the modifier.

The panel itself, inline, inside the card's bottom border:

```text
╭─ SELECTED DEVICE ─────────────────────────────────────────────────╮
│ Device Target                                       iPhone 17 Pro │
│ Platform ID                               ios (2C4A8B1E-...-9F3D) │
│ OS Version                                               iOS 26.0 │
│ Type                                                    Simulator │
│ ┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈ │
│  🔄 ^D Change target (rebuilds, keeps this tab)                   │
╰───────────────────────────────────────────────────────────────────╯
```

One row, not two: the `➕ ^T Run another device` line this mock used to carry is gone
with `^T` itself. And once the control is chosen, the target list opens in the same
place, under the same border, with the same rows `devices.rs` already knows how to
draw — where `⇧Enter` is then waiting, which is 8.4's whole economy.

**What it is paid for with.** `Budget::target_h()` is a constant today: four
fields, three optional separators, two borders, plus the title gap.
`Budget::solve(area, state, stages)` has no idea a panel exists. Both have to take
the panel into account, and the rows come from the one flexible region in the
frame — the log window, which is `Constraint::Min(3)`. That is the honest trade
and it should be stated in the UI's terms: opening the panel shortens the log.

> **Where this landed.** The panel was rejected and the control went into the top
> border, costing nothing. It has since come back inside the card as a single
> right-aligned row (3.2) — one row rather than the two this mock wanted, no
> dotted rule, and always open rather than toggled. `target_h()` charges it, which
> is the paragraph above coming true at a tenth of the price.

**The collapsed case has to be answered, not discovered.** Below the `full_cards`
rung of the ladder (6.2) this card is *one row* — `collapsed()`, a name, an id and
a version. A panel cannot open inside one row. Three options, and the middle one
is the proposal: refuse the panel at that size and say so; raise `MIN_H` so the
rung cannot be reached with the panel open; or let the panel take over the middle
region instead of the card, which is a device picker with extra steps and
contradicts point 1. Whichever is chosen, a row drawn past its charged height is
clipped in silence (7.5), so this cannot be left to chance.

### 8.4 Terminal tabs

**Built: a second `frun` process in a real terminal tab, dispatched from the picker
with `⇧Enter`.** Not a tab bar inside one process. The in-app shape is possible and
is costed at the end of this section, because a decision this size only means
something next to the price it declines.

Three parts of it are verified by hand and three are not; the list is at the end of
the section rather than here, because what it cost is the more useful heading.

Four rules, and each one removes work rather than adding it:

1. **The verb lives in the picker, not in the target card.** States 2, 4 and 12 are
   already the place a device is chosen, already draw one list through one row
   renderer, and already have `Enter` bound to *launch this*. `⇧Enter` is *launch
   this, elsewhere*.
2. **What crosses to the new process is the row, flattened, plus the flags this one
   was given.** Enough to start without asking anything: id, name, platform, virtual,
   and how to boot it. Nothing shared, and nothing the new tab has to wait for.
3. **No row is refused.** The one genuinely conflicting row already says so in
   words the list has carried since 8.5.
4. **One capability is asked of the terminal.** If it is not granted the feature is
   absent and unadvertised, not broken.
5. **Each tab names itself** `<device> · <project>`, by `OSC 2`, from the one place
   every target is set. Two tabs called `frun` would answer nothing the strip is
   asked.

```text
tab A                                       tab B — new process, knows nothing of A
─────                                       ──────────────────────────────────────
picker, no run yet (states 2, 4)
  ⏎    launch here, list closes
  ⇧⏎   dispatch, LIST STAYS OPEN  ────────►  FRUN_DEVICE=<the row> + A's extra flags
       (fire several, then ⏎ for the           no DETECTING — the answer came with
        one you want in this tab)              the question
                                                 ├─ attached   → Building, at once
switch picker, run alive (state 12, ^D)          └─ needs boot → Booting, at once
  ⏎    retarget this tab (8.5)                                    → Building
  ⇧⏎   dispatch, then back to the live
       run, exactly as Esc does:               discovery runs behind all of that:
       nothing is killed                       it fills `Platform ID` / `OS Version`
                                               and gives `^D` a list
```

**Keeping the list open is the part worth defending.** Enter closes it because the
choice was made; `⇧Enter` did not make a choice about *this* tab, so closing would
be answering a question nobody asked. It also turns the list into a dispatcher: two
devices out of one scan, two keystrokes, then `Enter` for the one you want here. In
state 12 there is a run behind the list, so the return is to that run and not to the
list — the same door `Esc` uses (8.5), and for the same reason.

#### The key, and the one thing it costs

`⇧Enter` reads as what it does. `Enter` launches here, `⇧Enter` launches there: one
verb, one modifier, one difference. That is what a modifier on `Enter` should mean,
and it is why this is not `^T` — `^C`, `^D` and a hypothetical `^T` are *unrelated*
verbs that happen to be modified, whereas this one is the same verb aimed somewhere
else. `^T` is released back to nothing (8.3).

The digits keep meaning *launch here*. There is no shifted digit that is not simply
another character, so `Enter` is the only key this second meaning can hang from.

**In the legacy encoding the modifier does not survive the wire.** `Enter`, `⇧Enter`
and `^Enter` are all CR, so crossterm reports `KeyCode::Enter` with an empty
modifier set and an arm matching on `SHIFT` never fires. Nothing about `⇧Enter` is
special here; every modified `Enter` has the same problem. It becomes
distinguishable once the Kitty keyboard protocol is pushed:

* `PushKeyboardEnhancementFlags(DISAMBIGUATE_ESCAPE_CODES)`, **after
  `EnterAlternateScreen`**, popped *before* `LeaveAlternateScreen`.
* It touches only what frun *reads*. The bytes frun forwards to Flutter are ones
  frun writes itself (5.1), so the pty side is unchanged, and `KeyEventKind::Press`
  is already filtered in `key_press`, so nothing new arrives on key release.
* Verify by hand, three commands: `printf '\e[>1u'`, then `cat -v` and press the
  keys, then `printf '\e[<u'`. Plain `Enter` is `^M`; under the protocol `⇧Enter`
  is `^[[13;2u`.

**That ordering is not a detail, and it was the first bug this shipped with.** The
protocol keeps **one flag stack per screen**, so a push made on the main screen is
not in effect on the alternate screen the app then switches to. Pushed before
`EnterAlternateScreen`, the flags applied to a screen frun was about to leave, and
`⇧Enter` produced *no event at all* in frun — not a plain `Enter`, nothing — while
the identical push in a program that never leaves the main screen reported
`Enter + SHIFT` correctly. That asymmetry is what made it findable: the terminal was
answering `\e[?1u` and delivering `\e[13;2u` on the main screen the whole time.

Measured, in this order, which is also the order worth reaching for next time
something like this happens: the terminal reports the flags active and sends
`\e[13;2u`; a throwaway binary using this crate's own crossterm reports
`Enter + SHIFT, kind Press`; frun's `key_press` sees nothing. Two of the three agree,
so the disagreement is frun's, and the only thing frun does differently is the
alternate screen.

**Whether to advertise it needs a query, and the query has one honest home.**
`supports_keyboard_enhancement()` writes an escape sequence and reads the reply off
stdin — the same contention `FRUN_NO_QUERY` exists for (see `Logo::detect`). Raw mode
is not the constraint; crossterm enables it for the query itself when it is not on.
An uncontended stdin is, so the call belongs beside the logo query in `run()`, in
the slot already commented as needing exactly that, and its answer is one `bool` the
footer hint follows. A key advertised and dead is the failure 3.1 recorded about
`[COPY]`.

**The flags are pushed whether the query answered or not, and only the hint follows
the answer.** They are two different things: pushing is a sequence a terminal either
understands or ignores, while the query is a question that can go unanswered. So
`FRUN_NO_QUERY` leaves `⇧Enter` working and merely unadvertised, which is the safe way
round — a key that works unannounced costs nothing.

**What the query costs when it fails is 2 seconds.** crossterm sends
`CSI ? u` followed by `CSI c` and waits for the first; a terminal that answers only
the second is indistinguishable from one still thinking, so the wait is a timeout, and
it is 2000ms. Measured in the terminal this was written in: the reply is `\e[?1;2c`
alone, no flags report, so this is not hypothetical. It is additive to the logo query's
own wait in the same situation, and it is the same valve that turns both off. Ghostty
answers immediately.

**Two alternatives turned down.** A bare-letter fallback (`t`) for terminals without
the protocol: a branch that never executes here, since Ghostty implements it, and a
letter taken from Flutter for a session that does not exist. And `^Enter`, which is
equally possible and slightly safer — `shift+enter` is a popular target for terminal
keybinds (`shift+enter=text:\n`, so a CLI can take multiline input), and such a
binding swallows the key before frun sees it, silently. There is no such config on
this machine today; the risk is accepted, and the symptom is worth writing down so
it is recognised if it ever appears.

#### The handoff

**`FRUN_DEVICE`, an environment variable, not a flag.** `FRUN_FLAGS` is a closed
list and everything else beginning with `--` is Flutter's by rule, so `--device`
would both take a name Flutter already uses (`--device-id`) and open a hole in that
rule. An env var takes nothing from Flutter's namespace, has precedent
(`FRUN_NO_QUERY`), and rides both Ghostty's `environment variables` surface field
and `tmux new-window`'s environment without any quoting games.

**The row travels, not just the id, and that correction came from using it.** The
first version handed over the id alone and let the new process resolve it against its
own scan. It worked, and it was wrong in the way that matters: the new tab sat on
`DETECTING` through its own `fvm flutter devices --machine` — six seconds of Dart VM
startup — before starting a build that needed none of it. The point of a handoff is
that the answer travels with the question. `flutter run -d <id>` resolves its own
device anyway.

So `Device::to_handoff` flattens the row into one tab-separated line, and
`handed_over` runs in `run()` **before the first frame**: attached devices go straight
to `Building`, bootable ones straight to `Booting`. Measured, both: no `DETECTING`
frame in either, `SELECTED DEVICE` and `Starting Flutter` in the first, `Booting Fake
Pixel` in the second.

Five fields, and the two that are not decoration are the reason it is all-or-nothing:
`platform` decides whether a boot is even possible, and `virtual_device` is frun's
licence to shut a device down (`release_target`), so guessing it could kill a physical
phone. A value with fewer fields is refused, logged, and the picker comes up as usual.
Tab-separated because device names, AVD names and simulator ids all contain spaces and
colons and none contains a tab — verified to survive the trip through AppleScript and
Ghostty's `environment variables` intact.

`target_platform` and `sdk` are deliberately left behind. They are `Platform ID` and
`OS Version`, they exist only once Flutter has answered, and `fill_target` writes them
in when the background scan lands — the same only-ever-fill-a-blank rule
`booted_device` follows.

**Discovery still runs in the new tab, just not in front of the build.** `^D` needs a
list to open, and `devices_refreshed` was already built to swap rows under a live run.
Until it lands, the handed row *is* the list, of one, which is also what lets
`booted_device` recover the name and platform of an AVD whose serial it has just
learned: with an empty list it falls through to a bare shell that hardcodes Android,
and a handed-over iOS simulator would have come back wearing the wrong glyph.

**`extra` has to ride along.** Tab A was launched with whatever `--flavor` and
`--dart-define` the user typed, and a second tab building a different flavour is
worse than no second tab: the two-device comparison the feature exists for would be
comparing two different apps.

#### The spawn

`$TMUX` set means `tmux new-window -c $PWD`; otherwise `osascript` against
Ghostty's AppleScript dictionary, which exposes `new surface configuration` with
`initial working directory`, `command` and `environment variables`, and `new tab in
<window>` ([Ghostty AppleScript](https://ghostty.org/docs/features/applescript)).
Setting `wait after command` keeps the tab open after frun exits, so its transcript
can still be read.

The command is `current_exe()`, not `frun`. `frun` is a zsh function sourced from
`.zshrc` and `~/.cargo/bin` is deliberately off `PATH` (see `frun.zsh`), so the name
does not exist for a shell started by anything but that shell. Asking the running
process where it lives also survives the crate being moved or renamed.

**`PATH` has to be handed over, and finding that out cost a measurement.** A surface
Ghostty creates with a `command` runs that command *instead of* a shell, so no
`.zshrc` runs and the environment is the app's own. Measured on this machine, `PATH`
in that tab is `/usr/bin:/bin:/usr/sbin:/sbin:/Applications/Ghostty.app/Contents/MacOS`,
where `fvm`, `flutter`, `adb` and `emulator` are all absent — so the new tab would
have died on its first command with `fvm` not found, and it would have read as frun's
bug rather than as a missing variable. This process's `PATH` is the one that works,
because a shell built it, so it travels with the device. `FVM_CACHE_PATH` travels too
when it is set, since `probe::fvm_cache` honours it and the two tabs must resolve the
same SDK. Nothing else is copied: this is a handoff, not a session transfer.

The environment is a list the script walks (`repeat with i from 3 to count of argv`),
so a variable can be added without touching the AppleScript. Values travel as `argv`
rather than being interpolated into the script, so a path or an id can contain
anything; the one part that does need quoting is `command`, which Ghostty hands to a
shell as a single string.

Blocking, and never detached, which is the one place this shape pays for keeping `Msg`
untouched: a thread could not report a failure back without a new variant. `tmux`
answers in milliseconds and so does `osascript`, except on the first call of a session
where macOS may raise its automation-consent dialog first — the frame is frozen while
that dialog is up, and the user is looking at the dialog.

A failure — no Ghostty, no tmux, or consent refused — is one line in the log and
nothing else, quoting the shortest decisive line of `stderr`. Tab A is mid-run; a tab
that did not open must not take a session with it.

#### The tab's name

Two tabs are only useful if the tab strip says which is which, and `frun  frun` says
nothing. So the tab carries `<device> · <project>` — `iPhone 17 Pro · cwclub`.

**Device and project, not device alone.** A tab strip has to answer three questions —
is this frun, which device, which project — and the device answers one of them. The
project name is the field the ProjectCard already reads (`app.project`), so the strip
and the card cannot disagree.

**`OSC 2`, written by frun itself.** `\x1b]2;<title>\x07`, which crossterm exposes as
`SetTitle`. No AppleScript, no automation permission, and nothing about it depends on
this being a second tab: the first tab gets its name the same way, which is what
actually makes two of them distinguishable. This part is worth building whether or
not `⇧Enter` ever is.

**One call site, because every path already funnels through it.** `launch()` calls
`app.choose(device)` and then `spawn_session`, and it is reached by the first pick, by
a device that had to be booted (through the `Booted` handler), and by a switch — which
does `stop_session`, `release_target`, and then falls into the same `match
device.boot`. One line there covers all three.

The one path that deliberately does not write a title is the one that changes nothing:
`Enter` on the row that is already running during a switch returns through
`app.goto(state)` before reaching `launch` (8.5), and the target it would name is the
target already named.

**The stale window during a boot is accepted.** The title is written when the build
starts, not when the pick is committed, so switching to a shut-down AVD leaves the tab
reading the outgoing device — or `frun`, on a first run — for as long as the boot takes,
up to the 180-second cap. The alternative is a second call site before `Booting`, which
names the destination immediately and then lies if the boot times out. One place that is
always true beats two places that are usually true, and `Booting <name>` with an elapsed
clock is already on screen saying what is being waited for.

**Nothing needs restoring on exit.** The title outlives frun only until the next shell
prompt, which writes its own — the same mechanism that put `frun` there to begin with,
since that string is the shell reporting the running command, not frun naming itself.

**Two things that will look like bugs.** Inside tmux, `OSC 2` sets the *pane* title;
the window name needs `\x1bk<name>\x1b\\` and loses to `automatic-rename` when that is
on, so the tmux path either does that or says plainly that naming is Ghostty-only. And
if Ghostty's `title` option is ever set in config, titles are pinned to it and escape
sequences are ignored — the first suspect if the name stops changing.

#### What is not blocked, and why

The list already distinguishes the two facts that matter here. `active` means the
device is up, which is true of any simulator left booted and is exactly the case a
second tab is *best* at: no boot, no wait. `running` means *your app is running on
it*, is true of one row, and that row is the only one carrying a verb — ` ⏎ Keep `
(8.5). So
the one combination that genuinely conflicts — same project, same device, second
install replacing the first and leaving tab A reloading into a dead VM — is already
named on screen, in a different word, on precisely one row.

That is enough. A refusal would need its own rule, its own message and its own
reason for the row renderer to know this feature exists; the chip carries it for
free, and `devices.rs` stays untouched. The cost of ignoring the chip is one lost
session, recoverable by quitting and running again, with nothing destroyed.

**What frun cannot see, it should not claim.** Another frun in another tab already
building on that device is invisible: there is no cross-process registry, and adding
one (a pid file per device id) is real machinery for one warning. So no wording
anywhere should suggest the check is global. It covers this session only.

#### What it costs

| File | Change |
| :--- | :--- |
| `data.rs` | `Action::NewTab`, key `⇧⏎`, label `Launch in new tab`, and `App::shift_enter` for whether the terminal reports the protocol — the same shape as `live` and `mouse_on` |
| `main.rs` | `KeyCode::Enter` with `SHIFT` in the three picker states, flags pushed after the alternate screen and popped before it, the support query beside `Logo::detect`, `handed_over()` before the first frame, `start()` split out of `enter()` so a handoff and a pick share one branch, `fill_target()` on a later scan, `new_tab`/`handoff_env`/`tab_command`, and `name_tab()` in `launch()` |
| `probe.rs` | `Device::to_handoff`/`from_handoff` and `Platform::tag`/`from_tag` — a wire format of its own, so renaming an enum variant cannot silently change it |
| `ui/chrome.rs` | one hint, `[⇧⏎] Launch in new tab`, in the hint lists for states 2, 4 and 12. It is a key, so it is never dropped; it moves the labels-off threshold and nothing else (3.7) |
| `ui/devices.rs`, `ui/target.rs`, `Msg`, `App`'s two lifetimes, `Budget`, `dump.rs` | untouched |

`dump.rs` was expected to need a line and did not: `shift_enter` defaults to `true` in
`App::empty()` and `run()` overwrites it with the terminal's answer, so the harness
draws the hint without knowing the flag exists.

Zero rows. The footer already exists and the hint is a fourth key on a row that
degrades by dropping words rather than keys, so the log window gives up nothing —
which is the whole reason this shape was chosen over a tab bar, whose first cost is a
permanent row and a higher `MIN_H`.

#### What is verified, and what is not

Three of each. The split matters because the parts a harness can reach and the parts
it cannot are not the same parts.

| Verified | How |
| :--- | :--- |
| The frame, at every width | `--dump picker\|switch`. At `MIN_W` the row measures 59 of 60 columns with its words intact, which is what the arithmetic in 3.7 predicted |
| What the new tab is told | Three tests on `tab_command`/`handoff_env`: the device travels in `FRUN_DEVICE`, `extra` travels with it, `PATH` travels, and a `'` in a flag cannot break out of the quoting |
| The spawn | The AppleScript run against the real Ghostty: a tab opened, and a command in it reported back `FRUN_DEVICE`, the working directory, and `fvm` and `adb` resolving on the handed `PATH` |
| `⇧Enter`, end to end | Driven by Ghostty's own `send key … modifiers "shift"` into a running frun: the arm fired in `NoDevices` and in `MultipleDevices`, `osascript` exited 0, and the tab it opened returned its id. This is also what caught the screen-stack bug above |
| `name_tab()`, in real use | The live session's tab reads `iPhone 17 Pro · cwclub`, read back through AppleScript from the tab itself |

| `handed_over()`, both paths | frun run under `script` in a real project with `FRUN_DEVICE` set by hand: an attached handoff drew `SELECTED DEVICE`, `BUILDING` and `Starting Flutter`, a bootable one drew `Booting Fake Pixel`, and **`DETECTING` appeared in neither** — which is the whole point of the change. The capture also shows `\e[?1049h` then `\e[>1u`, so the screen-stack ordering is in the record too |

| Unverified | Why, and what would do it |
| :--- | :--- |
| `fill_target()` | Needs the background scan to answer with a row matching a handed-over device, so it needs a real device. Six lines, and it only ever fills a blank |
| `name_tab()` on the boot and switch paths | The first pick is observed, and so is a handoff (`Ghost Device · cwclub`, read out of the capture). That `launch()` is also reached through `Msg::Booted` and through a switch is read from the code |

**How `⇧Enter` was driven, since it is worth reusing.** Ghostty's AppleScript
dictionary can synthesise input (`send key "enter" to <terminal> modifiers "shift"`),
so a real keypress into a real frun in a real terminal is scriptable — no hands. That
turns "needs a human" into "needs eight seconds", and it is how the screen-stack bug
went from *the user says it does not work* to a measured cause. The one thing it
cannot drive is a device, which is why the row above it is still unrun.

#### The in-app tab bar, and what it would cost

Kept because the decision above is a decision, and the price it declined belongs
next to it. It is a refactor rather than a feature. What stands in the way, in the
order it will be hit:

* **`Msg` has no identity.** `Line`, `Partial`, `Eof`, `Booted`, `Versions` all
  arrive on one `mpsc` shared by every worker, and the receiving end assumes there
  is one of everything. With two tabs, tab B's output lands in tab A's log. The
  fix is to tag the message with the tab it came from; `std::sync::mpsc` has no
  `select`, so a channel per tab is not the alternative it looks like.
* **`App` mixes two lifetimes.** Project, branch, SDK versions, the logo,
  `mouse_on` and `hits` belong to the process. `state`, `stages`, `logs`,
  `target`, `pending`, `log_scroll`, `failure`, `exit_code` and `fatal` belong to
  one run. Tabs mean splitting those: a shell that owns the chrome, and
  `tabs: Vec<Tab>` plus an active index. This touches `data.rs`, `main.rs`, every
  `ui/*` module, and `dump.rs` — the whole `--dump/--all/--hits/--rows` harness
  builds an `App::new(state)`, and that harness is the verification story (7.5),
  so it cannot be allowed to rot.
* **`Session` is already fine.** `portable-pty` holds no global state and
  `Session::spawn` takes its device as an argument, so N children is not a code
  problem. It is a machine problem: each tab is another Dart VM plus another
  Gradle or Xcode. Two tabs is reasonable, four is not, and nothing should pretend
  otherwise.
* **Keys route to the active tab only.** Straightforward — `forward()` already
  goes through one place. Background tabs keep filling their own buffers without
  help, because their pump threads never stopped.
* **Three semantics have to be decided, not inferred.** `q` and `^C` end the
  process today; with tabs they either close a tab or end everything, and the
  footer has to say which. `run()` returns one exit code — whose? And it replays
  one transcript into the shell's scrollback on the way out — whose, or all of
  them, separated how?
* **A tab bar is a permanent row**, and `MIN_H` (14) goes up by it.

```text
  TABS  [ 1 iPhone 17 Pro ✕ ]  [ 2 Pixel 9 · building ✕ ]  [ + ]
```

**And what it buys, since that is the half a cost list hides.** One project card for
both runs, one window to watch two devices reload in, and one `q` that means
something definite. Those are real, and they are what is being given up.

The exchange rate is what settles it. Every item above is paid in the two currencies
this design has least of — rows, and structural certainty about who owns `App` —
while the spawned shape is paid in one footer span and one environment variable, and
the multiplexing is done by a terminal that is already good at it. One thing is
already prepaid should this ever be revisited: `alive` in `Session` is the smallest
half of the `Msg`-identity problem, and it is done.

### 8.5 Switching device (built)

**Done.** `[^D] Switch`, advertised on the footer and reachable from every state that
has a run behind it. It was in the target card's border when this was written, then a
row inside the card, and 3.2 has the reasoning for both moves.

**The description was wrong in a way that mattered.** `flutter run` is bound to its
device at spawn — `Session::spawn` builds `flutter run -d <id>` — and Flutter has
no interactive command to move a live session to another device. There is nothing
to "change" on a running session.

So "reuse the session" can only mean reuse the **tab**: its position, its scroll,
and its log history. The Flutter process is killed, reaped and respawned. The label
carries that: `Switch`, not `Change target (this session)`, because a label that reads
like a live switch leaves the user watching a forty-second Gradle build and concluding
the tool has hung. The noun was dropped when the footer took the key in five states
rather than one; the verb is what the argument was about.

The respawn was already load-bearing. `Action::RetryBuild` kills, reaps, resets
stage state and respawns, and it deliberately *keeps* the previous log rather than
clearing it, because comparing the two runs is the point (3.4). A switch is that
same path with the device chosen first, so the kill lives in one place —
`stop_session()` — that both verbs call. A respawn racing an unreaped child is how
two Gradle daemons end up fighting over a lock, which is also why this cannot be a
forwarded keypress.

#### The flow

No inline panel. `^D` reopens the list frun already has, over the run that is still
alive, as a state of its own — `Switching`, slug `switch`:

```text
   ╭─ ◆ DEVICE INFO ─────────────────────────────────────────────╮
   │ Device Target                             Pixel 10 Pro XL  │   8  Running
   ╰──────────────────────────────────────────────────────────────╯
   ╭─ ◆ APP LOGS STREAM ─────────────────────────────────────────╮
                              │ ^D
                              ▼
   ╭─ ◆ PROJECT INFO ────────────────────────────────  ~/cwclub ╮
   ╰──────────────────────────────────────────────────────────────╯
   ╭─ ◆ SWITCH DEVICE ───────────────────────────────  5 devices ╮
   │ ❯  Pixel 10 Pro XL   running   last used         ⏎ Keep    │  12  Switching
   │    Pixel 8                          Pixel_8  Android        │   child alive
   │    iPhone 17 Pro             8A3F91C2-4D2E  iOS  virtual    │
   ╰──────────────────────────────────────────────────────────────╯
     [↑↓] Move        [⏎] Switch        [Esc] Back
          │                                  │
   ⏎ other│                 ⏎ Keep / Esc     │
          ▼                                  ▼
   kill + reap, shut the old device    back to the run,
   down if frun booted it, respawn     nothing killed
   → 6 Building                        → whatever 8-11 it now is
```

**The titles are `DEVICE INFO`, `SELECT DEVICE` and `SWITCH DEVICE`.** One noun
for one thing: `target` and `device` were the same object under two names, and the
list that picks one now shares its vocabulary with the card that shows it and with
the key that changes it. The card's title was `SELECTED DEVICE` and lost the
adjective to `PROJECT INFO` above it (3.2), which costs this paragraph nothing — the
noun is what the paragraph is about. The struct is still `SelectedTargetCard` in the
code, which is a rename with no reader and can wait.

`App::run_state()` — `resume.unwrap_or(state)` — is what lets the screen hide the run
without the code losing track of it. One caller and it is load-bearing:
`child_exited`. If the app dies while the list is open, reading `state` there sees a
frame with no build in it and ends the process, throwing away the failure it was
called to report.

**The adopted device replaces the row it was picked from** (`App::choose`), and on
Android that is what makes the badges work at all. An AVD is offered under its AVD
name and runs as a serial — `Pixel_10_Pro_XL` becomes `emulator-5554` — so a list
left untouched showed the emulator frun was running as a row offering to *boot* it,
with no ` running ` badge, because the badge matches on `target.id`, and no
` last used ` either, because that compares ids too. iOS hid the bug for a while: a
simulator keeps its UDID whether booted or not. The row is matched on name as well
as id, which is the same join `probe::targets` already uses to de-duplicate a running
emulator against its own AVD row.

**The list says which row it is leaving**, and with the cards gone it is the only
thing that can. ` running ` on the row whose id matches the target, in this state
only. A separate word from the existing ` active ` chip, which means *the device is
up* and is true of every simulator left booted; this one means *your app is on it*
and is true of exactly one row. `active` is suppressed there — it would be the same
fact in a second word — and it is the one row that carries a verb, ` ⏎ Keep `, because
`Enter` there is the only `Enter` in the list that does not build. Every other row
shares one answer and the footer gives it once (3.3).

**The cards go while the list is up.** `has_target()` and `has_build()` are both
false in `Switching`, exactly as in the first picker, so the frame is the project
card and the list. Keeping them was the first attempt — the run has not stopped, so
both were still describing something true — and it was wrong twice over: the target
card named the device being left directly above the row that says the same thing
with ` running `, and at 70x24 the three cards took the height and the list drew a
border around nothing. Hiding them removed the need for a floor of its own; the
list scrolls, and `Budget::floor(state)` (6.2) is where all of that is decided.

**Nothing is killed until a device is picked.** That is what makes `Esc` free: the
child keeps running, keeps streaming into the log buffer, and going back costs
nothing. `Esc` on the first pick means cancel-and-exit-130, so this frame's footer
reads `[⏎] Switch  [Esc] Back` where the picker's reads `[⏎] Launch  [Esc] Cancel`.
Same three keys, four different words, because one key meaning two things has to say
which one it means.

**The cached list opens the frame; a recheck corrects it.** `enter()` and the boot
path used to empty `app.devices` after launching, and no longer do, so `^D` draws
rows immediately instead of putting a spinner in front of a list frun already has.
The rows are then rechecked behind them and swapped when the answer lands, with the
title reading `⠋ 5 rechecking` until it does.

**The recheck is not discovery, and the difference is measured.** On this machine
`fvm flutter devices --machine` is 6113ms — Dart VM startup, mostly — against 12ms
for `adb devices` and 119ms for `simctl list -j`. Six seconds is long enough for the
list to settle *after* the user has picked from it, which is the same bug as a stale
row wearing a different hat. So `probe::alive()` asks the two fast tools which ids
are up, rows that were up and no longer answer are dropped, and `probe::targets()`
rebuilds the bootable rows around what survived: a shut-down emulator comes back as
a row offering to boot it. About 190ms end to end.

Physical devices are kept whether or not they answered. `adb` covers Android, but a
physical iPhone is visible only to Flutter's own scan, and dropping a row this cannot
see would be worse than keeping one that has gone. The consequence is honest and
bounded: a device that was never in the cache — a simulator booted by hand while frun
was running — will not appear until the next full run, because the only tool that
would have found it is the six-second one.

Cache alone was the first attempt and it shipped a real failure. Every chip on a row
is a fact about a device *at the moment it was scanned*: ` active ` means no boot is
needed, ` last used ` means it is the one in `.frun-last-device`. A device that
stopped since — including the one frun shut down on its own way out of a previous
switch — still read as ready, so picking it went straight to `flutter run -d <id>`
and failed the build. The stale row was not a cosmetic problem; it was the one row
the user was most likely to pick.

Two rules make the re-scan safe over a live run. **Only the first answer decides the
flow**: `Msg::Devices` sets state and can be fatal when `state == Detecting`, and
otherwise only replaces the rows. Without that, an answer landing after `Esc` would
drag the user out of their session into the picker, and a failed scan would end a run
that was working. And **the selection follows the device, not the row number**, since
discovery reorders by what is running.

**Every list is rechecked while it is on screen, every four seconds.** Not only the
switch list: the first picker had no refresh at all, and it makes the same claim. The
case that forced it was reported as "two devices are running and only one says
`active`", and the discovery was correct — `--probe` shows both as ready when both are
up. What was wrong was the age of the answer. Measured while chasing it: two booted
simulators shut themselves down inside a minute with nobody asking, and a simulator
booted *after* detection sat in the list offering to boot something already running.
The chips are claims about right now, so they are re-earned rather than remembered.

That made the recheck a two-way street. A row that is up and marked bootable has its
`boot` cleared — which is the whole of the ` active ` chip — and a row marked ready
that no longer answers is dropped, so `targets()` puts it back under the spelling the
boot tools use. One asymmetry stays, and it is Android's: a bootable AVD row is keyed
by AVD name while `alive()` reports serials, so an emulator started outside frun
cannot be promoted this way and waits for the next full scan.

**Rechecking on a timer made `targets()` idempotent, the hard way.** That function is
written as "Flutter's list, plus what can be booted", and it trusted its input to be
Flutter's list: the AVD loop skipped names already present, but `simulators()` was
appended without a duplicate check. A recheck hands it a list it produced itself, so
every pass appended the same shut-down simulators again. On a machine with eleven of
them the picker read `214 devices` inside a couple of minutes — a header that made the
bug obvious and would have been invisible without the count. The fix is a duplicate
check by id, the same rule the AVD loop already followed, and one test asserts no id
appears twice after a list is merged into itself. **A function whose output can become
its input has to be idempotent, and a precondition in a comment is not idempotence.**

Five decisions inside the flow, each of which could have gone the other way:

* **The outgoing child dies at the pick, not at the respawn.** A boot can take
  three minutes, and an old app still running on a device that has already been
  replaced is a second run nobody asked for.
* **The outgoing device is shut down with it, if it is virtual.** `simctl shutdown`
  for a simulator, `adb -s <serial> emu kill` for an emulator, on a worker thread
  with nothing reported back. `adb shell reboot -p` was the wrong call to reach for:
  it powers the guest down and leaves the emulator process answering `adb devices`
  with a device that cannot be used.

  The first rule here was narrower — shut down only what frun booted, tracked in an
  `App::booted_target` flag — and it silently did nothing in the commonest case.
  `boot_avd` starts the emulator under `nohup` *so that it outlives frun*, so on
  every run after the first the emulator is already attached, nothing was booted,
  and switching away left it running. The flag is gone. One rule that always holds
  beats a rule that holds only in the session that started the device, and the cost
  is stated plainly: switching away closes a simulator or emulator you may have had
  open for something else. Physical devices are untouched, and so are macOS and
  Chrome, where the nearest equivalent would be closing the user's browser.
* **Picking the device that is already running is a return, not a rebuild.** Its row
  is the highlighted one when the list opens, so a reflexive `Enter` lands on it,
  and `Switch` does not promise a rebuild.
* **`^D` works during `Building` too**, not just once the app is up — bailing out
  of a slow build onto another device is the case that wants it most.
* **No marker line in the log.** The log is not cleared across a switch, so the
  two runs do run together; the build tracker resetting to `Starting Flutter` is
  the boundary, and a second one was not worth a row.

#### Retry has to be able to bring the device back

Not part of the four points, and found by using them: shut a simulator down while it
is building and Flutter fails with `No supported devices found with name or id
matching '<udid>'`. `[r] Retry Build` then respawned `flutter run -d <udid>` against
a device that was still off and failed identically, as often as it was pressed. The
retry could not fix the only thing that was wrong.

So a retry now brings a virtual target up first. `probe::alive()` decides whether
anything needs starting — `simctl bootstatus -b` is idempotent but `boot_avd` is not,
and booting a running emulator leaves two of them — and the existing `BOOTING` frame
and `Msg::Booted` path carry it from there, which means the rebuild afterwards is the
same code as a first launch. A physical device, macOS or Chrome takes the plain
respawn, because there is nothing frun could start.

Two things had to be corrected to make that safe. `probe::boot_target()` reconstructs
the `Boot` a running target no longer carries: a simulator by its UDID, an emulator by
matching its name against `emulator -list-avds`, since `adb emu avd name` needs a live
device to answer and the serial is useless once it is dead. And `booted_device()` now
resolves which row a boot belongs to by id — the running row, then the target, then
the cursor — where before it always took the cursor. That was right for a first pick
and wrong for a retry: after a recheck reordered the list, the retry would have kept
the correct id and inherited another device's name and platform.

A failed boot is also no longer fatal once a run exists. Before, a reboot that did not
come up ended the process, throwing away the failure card and the log the user was
reading — which is the reason they were on that screen.

#### Two defects this uncovered

**`Msg` has no session identity, and a killed child keeps talking.** This is the
same gap 8.4 lists as tab work, and it turned out to be a live bug in `RetryBuild`
long before tabs. `Line`, `Partial` and `Eof` from every child arrive on one
channel; after `kill()` the pump thread is still draining the pty and its `Eof`
lands *after* the replacement has been spawned. `child_exited` then reads a
`Building` state, gets `None` from the new child's `try_wait`, and marks a healthy
run `BuildFailed` with the old child's death. Stale `Line`s are worse in a quieter
way: they drive stage detection, so `Running Gradle task...` from a session that no
longer exists opens a stage in the run that replaced it.

Tagging every message with its session is the 8.4-sized fix. The cheap one is to
silence the source: `Session` holds an `Arc<AtomicBool>`, `kill()` clears it
*before* killing, and the pump checks it before every send and before `Eof`.
Messages already queued when the flag drops are output the old child really
produced before it died, and they are chronologically where they belong.

**A live child can move the state while the picker is open.** `flutter::feed`
calls `goto(Running)`, the reload paths call `goto(ReloadInFlight|ReloadFailed)`,
and `tick_pending()` can reach `ReloadDropped` with no output at all. Any of those
arriving mid-choice would close the list under the cursor.

Every transition goes through `App::goto`, so the guard goes there and nowhere
else: while `app.resume` is set, `goto` banks the state instead of drawing it. The
list stays up, and `Esc` restores what Flutter actually reached rather than what it
was doing when `^D` was pressed — a reload that failed while the list was up comes
back as `ReloadFailed`, not as a `Running` that is no longer true. `resume` is
cleared at the pick, or the `Booting` and `Building` transitions that follow would
be banked too and never reach the screen.

#### What it cost, and what verifies it

Zero rows. The control is an inset title in a border that already had one
(`title_top(...).right_aligned()`, the same call `render_picker` uses for its
count), so `Budget::target_h()` is untouched and the log window gives up nothing.
At `MIN_W` the two titles need 41 of the 58 columns inside the border. Below the
`full_cards` rung the card has no border to carry it and the hint disappears; the
key still works, which is the same bargain `e` and `j`/`k` already have.

It is also not clickable. `ui/target.rs` stays `&App`, so no hit region is pushed
(7.5) and the mouse cannot reach the control at all — acceptable because capture is
off by default (5.2) and the key is the primary path, and 8.3 has to take that
signature to `&mut App` anyway.

Verified by the harness, which is the reason the switch is a state and not a flag:
`--dump switch` draws the whole frame at any size. `--dump building|build-failed|
reload-failed|running` all carry the hint and `--dump single` does not (no run yet);
`--dump running 60x45` proves it fits at the minimum width. `mock_devices` returns
the picker list for every state where `has_build()` is true, because live runs always
have one, and `mock_goto` points the switch mock's target at a row in its own list —
otherwise the frame would show a switch away from a device that is not there.

Three tests carry the parts a frame cannot show.
`the_switch_list_says_it_is_replacing_a_run` asserts the title, the badge, the footer
and the surviving hint together, since any one of them reverting leaves a frame that
reads like a first launch. `the_switch_list_is_not_starved_by_the_cards_above_it`
walks six heights and requires three target rows at each.
`a_live_transition_does_not_close_the_picker` covers the banking in `goto`.

The pty path — kill, reap, respawn onto another device, and shutting the outgoing
emulator down — has no coverage and no dump can reach it: it needs a real project and
two real devices. **It is unverified, and it is the part of 8.5 most worth exercising
by hand first.**

### 8.6 What none of this includes

The request mentions custom targets and wireless ADB as things the panel would
offer. Neither exists. `probe::Boot` has exactly two variants, `Avd(String)` and
`Sim(String)`, and discovery is `flutter devices --machine` plus `simctl` plus
`adb devices`. Entering a device id or an `adb connect host:port` by hand is a new
feature in `probe.rs` with its own failure modes, and it is not a consequence of
any of the four points above. It is listed here only so it is not mistaken for
one.

Typed input is also the one thing 3.6 removed on purpose. A field for an
`adb connect` address is a much smaller claim than the command prompt was — it is
frun's own input, not a line forwarded to Flutter, which is what made the prompt
unworkable — but it is still an input in an application that currently has none,
and it should be argued on its own.

### 8.7 Decisions still needed

Answered by building 8.5, and recorded here because each one closes a question the
list below used to carry:

* **The key is `^D`**, and `^T` is still reserved for tabs. `s` was rejected: it is
  Flutter's screenshot (8.3).
* **The control is an inset title in the target card's border**, which is why it
  costs no rows and why the collapsed rung needed no rule of its own.
* **The list is reused, not rebuilt as a panel** — but as a state of its own
  (`Switching`, slug `switch`), so the frame is something `--dump` can name.
* **The cached list opens the frame and a fresh scan replaces it**, because chips
  from a stale snapshot offered devices that were no longer there.
* **The outgoing device is shut down**, when frun is the one that booted it.

Answered by building 8.4, whose spawn and title are verified by hand and whose key
and handoff are not (8.4):

* **A second process in a real terminal tab**, not tabs in-app. That closes the
  question everything in 8.4 used to hang on, and with it the one about `q`, the
  exit code and the transcript: each tab is a process, so each already owns all
  three.
* **The key is `⇧Enter`, in the picker.** Not `^T` in the target card, which is
  released. `^Enter` was the safer twin and was declined on how the key reads.
* **The handoff is `FRUN_DEVICE` plus `extra`**, resolved against the new process's
  own scan, so the boot happens in the tab that waits for it.
* **No row is refused.** The `running` chip and its `Keep` verb already mark the one
  conflicting row, and the check is per-session, never claimed as global.
* **The tab is named `<device> · <project>` by `OSC 2`**, written in `launch()` and
  therefore on every path that sets a target, switches included. It is stale for the
  length of a boot, deliberately, and it is worth building on its own.

Still open:

1. **Where the inline panel's rows come from** — the log window is the only
   answer available — **and what the panel does at the collapsed rung.** 8.5 no
   longer depends on either: the panel is presentation now, not the way in.

Order from here: exercise 8.4's three unverified paths by hand, since two of them
(`handed_over`, `name_tab`) only ever run against a real project. Then 8.3, if the
panel is still wanted for its own sake — by then it carries one control rather than
two, and what it really unlocks is the click path that `&mut App` in `ui/target.rs`
gives.

### 8.8 Stopping without leaving (built)

`[^S] Stop` ends the run and keeps frun on screen. `^C` is unchanged and still ends
the process — that was the constraint this was built under, and it turned out to be
what makes the rest simple.

**Because `^C` stays, `^S` needs no signal.** A graceful stop is the mechanism `q`
already uses: forward `q` to Flutter, let it shut itself down, wait for the pty to
close. The only difference between the two is where frun lands afterwards, which is
one flag — `App::stopping` — read in `child_exited`. During a build there is no
interactive session to read a key, so `^S` there sends SIGINT instead; Gradle and
Xcode answer to nothing else.

And no escalation ladder behind it. If Flutter is wedged and ignores the request,
`^C` still ends the process. A timeout that killed the child would be a second answer
to a question the user already has a key for.

**The frame is state 13, `Stopped`, and it is all reuse.** `has_target()`,
`has_build()` and `has_logs()` are true, so the target card, the tracker and the log
window are the ones already written; `build_done()` is false, so hot reload is not
advertised for a session that is gone. The tracker's title becomes `STOPPED` in muted
rather than rose — the run ending was asked for, and colouring it like a failure would
make a deliberate stop read as something breaking.

```text
╭─ ◆ DEVICE INFO ────────────────────────────────   STOPPED  ╮
╰──────────────────────────────────────────────────────────────╯
╭─ ◆ APP LOGS STREAM ──── Build time 3.4s   Sync 240ms  [8 entries] ╮
      [r] Build again   [^D] Switch   [↑↓] Scroll   [e] Expand   [q] Quit
```

**Which word appears is the whole of this section**, and it is the title-bar pill of
3.2 that carries it: `STOPPED` and `DETACHED` in muted, `DISCONNECTED` in rose, in
place of the `RUNNING` in emerald that was there a moment earlier. There is no tracker
block here and no summary row — `chrome(STOPPED)` equals `chrome(RUNNING)`, so nothing
on screen moves when the run ends. 3.4 has the argument for that trade.

The device is left booted and the log is left intact, which is what makes `r` from
here worth having: it is `Action::RetryBuild`, so it already brings the device back up
if it has since stopped. Exit code after `^S` then `q` is Flutter's own, normally 0 —
a stop is a graceful shutdown, the same as `q`, taken in two steps rather than one.

**`[r] Build again`, and the word is the whole difference.** Same `Action` as
`[r] Retry Build` in state 7, so the key and the click stay one path, but nothing
failed here and calling it a retry would say something did.

Two things had to be fixed before that key worked at all, and both were the same
mistake: code that asked *what is on screen* where it should have asked *what is
running*.

* `r` resolved to `Action::Reload` outside `BuildFailed`, and a hot reload with no
  child declines itself — silently, because `build_done()` is false. The footer
  advertised the rebuild, the mouse click on that same hint performed it, and the
  key did nothing. `apply()` exists so a key and a click cannot drift apart; the
  drift was in the routing above it.
* Picking the same device in the switch list returned without doing anything, which
  is right over a live run — the row is highlighted when the list opens, so a
  reflexive `Enter` lands on it, and it should not cost a rebuild. Opened from
  `Stopped` there is no session to keep, so the same row now means "start this one
  again". `ctx.session.is_some()` is the test, not the state.

The switch list also stops claiming the app is on that device: ` running ` and
` ⏎ Keep ` are drawn only when the banked state is a live run. From `Stopped` the
device is still booted but the app on it is gone, and the row's ` active ` and
` last used ` chips are all that remain true of it.

**Flutter's own `d` lands here too, and used to close frun.** `d`/`D` is detach —
it ends `flutter run` and leaves the app running on the device — and 5.1 forwards it
like any other key of Flutter's. The pty then closes, and a closing pty is all
`child_exited` had to go on, so a detach read as Flutter quitting and took frun with
it while the app carried on running two feet away.

The fix is to record it rather than intercept it. `d` still reaches Flutter and still
detaches; frun notes on the way past *why* the pty is about to close, and `App::ending`
carries which of the two happened — `Stopped` for `^S`, `Detached` for `d` — so the
frame can say `DETACHED` where the app is still up and `STOPPED` where it is not. The
flag is only set once the interactive session is live, which is the only time Flutter
can read the key; setting it during a build would outlive the keypress and turn the
next crash into a false detach.

Detach is worth having as an ending, and worth nothing as a shortcut back. Measured on
an iOS simulator, `d` returns in 0.2s and the app survives — verified by its process
and its listening socket — but `flutter attach` never reconnects to it: not by
discovery (150s), not with `--debug-url` from the URL `flutter run` printed (that URL
belongs to DDS, which dies with the tool), not with `--host-vmservice-port` and
`--disable-service-auth-codes`, and not when pointed straight at the port the app is
actually listening on. Four ways, all `Connection refused` or a wait that never ends.
So coming back costs a full `flutter run` — 20s on that project — and `[r] Build
again` is the same key either way. Hot restart, for the comparison, answered in 0.5s.
That is the argument for not ending a run you only wanted to restart.

**One thing this exposed in the footer.** The hint row had no truncation rule at all.
Only the diagnostics group knew how to drop itself, and its fit test measured the
hints without the single column each gap needs, so at 106 columns the seven running
hints measured 84 against 87 and were clipped at the buffer edge in silence (7.5) —
losing `[^C] Force stop`, the one key that matters when Flutter is wedged. Two fixes,
both small: the minimum gaps count as content in every fit test, and when the row
still does not fit, the labels are dropped and the keycaps are kept. `[^C]` alone is
still a key you can press; a `[^C` that was cut off is not, and nothing on screen
would have said so.

`[h] Help` also left the row. It is Flutter's key and the only hint on the cheatsheet
that can find itself: pressing `h` makes Flutter print its own list.

### 8.9 Detection you do not wait for (built)

**`DETECTING` was six to eight seconds of Dart VM startup, and almost none of it was
being spent on the answer.** Measured on this machine, in the project frun is used on:

| Command | Time | What only it can answer |
| :--- | ---: | :--- |
| `adb devices` | 12ms | running emulators, attached Android phones |
| `emulator -list-avds` | 30ms | every AVD |
| `xcrun simctl list devices available -j` | 119ms | every simulator, and which are booted |
| `fvm flutter devices --machine` | 6113-8412ms | macOS, Mac Designed for iPad, Chrome, physical iPhones |

Thirteen of the sixteen rows in the picker on that machine come from the first three,
and they come from them *anyway*: `probe::targets` has always walked `-list-avds` and
`simctl` for the ordering, and the 4-second recheck (8.5) has always used `adb` and
`simctl` alone precisely because six seconds is too slow to keep a list honest. The
three rows the wait actually bought are macOS, Mac Designed for iPad and Chrome —
which rank last in the list by construction, being nobody's first choice.

So the wait is turned around. `probe::quick_targets` asks the cheap tools, sends
`Msg::Quick`, and the picker opens on that; `probe::devices` keeps running on the same
thread and lands as `Msg::Devices`, which `devices_refreshed` was already built to
swap under a live list. **Measured, under a pty, in `cwclub`,** by killing the process
at a series of fixed times and reading the capture: `Detecting Flutter devices` alone
at 0.25s, `SELECT DEVICE` with 13 rows and a `⠋ 13 rechecking` title at 0.4s, `16
devices` and the macOS/Chrome rows at ~9s. The frame that used to take 8-9s takes
under 0.4s.

**Two rules make an incomplete list safe to open the picker on**, and they are the
same two that already make the recheck safe over a live run.

*It cannot be fatal.* `devices_answered` ends the process on an empty list, which is
right for the full scan and wrong here: the cheap tools seeing nothing is exactly the
case where a physical iPhone may be the only device on the machine. An empty
`Msg::Quick` is dropped, `DETECTING` stays up, and `Msg::Devices` decides — including
its `No device(s) detected`.

*It cannot decide anything twice.* `Msg::Quick` acts only while `state ==
Detecting`, so a tab that was handed a device (8.4) and is already building is
untouched, and a full answer that somehow arrived first keeps the flow.

**The fast list cannot open a different frame, and finding out why removed code.**
A `NO_DEVICES` → `MULTIPLE_DEVICES` correction was written into `devices_refreshed`
first, on the assumption that a physical iPhone — invisible to the cheap tools, and
1420ms away through `xcrun xcdevice list --timeout 1` — would open the wrong frame with
`Nothing is attached. These can be started:` on it. It could never fire.

`devices_answered` branches on `Device::attached`, which is `platform.needs_boot()`:
every iOS and Android row, **bootable ones included**. So the branch is decided by the
AVD and simulator rows, and those are the same rows in both lists, from the same two
tools. A non-empty fast list always holds at least one of them — `quick_targets` cannot
produce a desktop or web row at all — so the frame is already the picker, and an empty
fast list is dropped without deciding anything. The correction was deleted rather than
kept as insurance: unreachable code that looks like a safeguard is worse than neither.

**A consequence worth recording, and it is not this change's.** Because `attached`
counts bootable rows, `NO_DEVICES` (State 2) was unreachable on any machine with a
single AVD or simulator installed — which is every machine frun is used on. That is
7.6's rule ("the picker is always shown") arriving at its logical end, and it predates
8.9: the fast list neither caused it nor made it worse, it only made the dead branch
visible while the fast path was being reasoned about. 8.10 removes the state.

Physical Android is covered rather than left to the full scan, and not for symmetry:
`adb` sees it in 12ms, and a phone on the desk missing from the first list is the one
omission that would read as a bug instead of a count that is still settling.
`android_device` describes it the way Flutter would — `ro.product.model` for the name,
`android_facts` for `Platform ID` and `OS Version` — so the fast row is not a
placeholder that the slow scan has to repair. `probe::adopted` is now that same
function, which is how a device that only ever arrives through a recheck stopped
carrying a dash on the target card: there is no second full scan behind it to fill
those fields in.

**What the fast path is not allowed to be is a different list.** `quick_targets`
returns through `probe::targets`, the same merge, so the rows are in the same slots the
full answer would have put them in — verified by `--probe`, which now prints both
answers with their timings and their frame verdicts, and the 13 shared rows appear in
the same order in each. Anything else and the list would rearrange itself under the
cursor at the eight-second mark, which is the wandering-row bug of 7.6 arriving by
another route.

| Verified | How |
| :--- | :--- |
| Fast list matches the slow list, row for row and in order | `--probe` in `cwclub`: `quick 13 rows in 202ms, frame picker` against `picker 16 rows`, the extra three being macOS, Mac Designed for iPad and Chrome |
| The picker really opens before the scan lands | pty capture killed at 0.25s / 0.4s / 1.0s, release build: `DETECTING` alone, then `SELECT DEVICE` + `rechecking`, with no macOS row |
| The full answer completes the list in place | same capture at 11s: title moves from `⠋ 13 rechecking` to `16 devices` |
| Nothing else regressed | 74 tests, `cargo clippy --all-targets` clean |

| Unverified | Why, and what would do it |
| :--- | :--- |
| A physical Android in the fast list | No phone was attached during these runs. `android_device` is the same `getprop` path `android_facts` uses after every Android boot, which is exercised |
| The empty-fast-list path | Needs a machine with no AVDs, no simulators and no adb device. Reachable by reading it: `Msg::Quick` returns on `targets.is_empty()`, leaving `DETECTING` up until `Msg::Devices` answers, which is the behaviour that existed before 8.9 |

### 8.10 State 2 removed (built)

**`NO_DEVICES` could not be reached, and the frame it would have shown was the picker
with two rows spent on a heading.** Both halves have to be said, because either alone
argues for a different fix.

*Unreachable.* `devices_answered` branched on `targets.iter().any(Device::attached)`,
and `attached` is `platform.needs_boot()` — true of every iOS and Android row,
**including the bootable ones 7.6 merged into the list**. On this machine that is 2 AVDs
and 11 simulators, so the predicate is true with nothing running at all, and State 4 was
the only arm the branch ever took. The `NO_DEVICES` arm survived because the reasoning
that introduced the predicate (7.4, "Departure: what how many answered counts") was
written before the lists merged and was never re-read afterwards.

*Redundant.* `render_bootable` called the same `list()` the picker calls, with the same
rows in the same order. What it added was `NO DEVICE RUNNING` in amber, the subtitle
`Nothing is attached. These can be started:`, and a `Start a Device` heading — two rows,
in the frame with the most targets to show, saying what the rows say for themselves:
every row has a `boot` and none has an ` active ` chip.

So the fix is not to repair the predicate. Making `attached` mean
`boot.is_none() && needs_boot()` would have brought the screen back for the common case
of nothing being booted, and bought a second list frame to keep true — which is the
class of bug 8.9 had just finished tracing.

What was removed: the `State::NoDevices` variant and its `ALL` entry, its `no-devices`
slug, `ui::devices::render_bootable`, its arms in `ui::mod::render`, `chrome`'s footer
hints, `mock_devices`, `State::has_target`, and the branch in `devices_answered`, which
is now one `goto`. `State::ALL` is 12 rather than 13, so `--states`, `--all` and every
harness that walks it follow without being told.

**The numbers of the other states are left alone.** They are labels this document and
the code share — 8.5 is "State 12", 3.4 talks about "State 6" — and closing the gap
would rewrite every cross-reference in 3,000 lines to buy tidiness. State 2 is now a
number with a note on it, in the state table in 1 and in 3.3.

| Verified | How |
| :--- | :--- |
| The state is gone from every surface | `--states` lists 12 slugs with no `no-devices`; `--dump no-devices` answers `unknown state "no-devices"` and prints the 12 that exist |
| The picker still renders | `--dump picker 90x30` |
| Nothing referenced it | `grep -rn NoDevices\|render_bootable src/` is empty apart from the comments recording the removal, and the crate compiles |

| Unverified | Why |
| :--- | :--- |
| The one machine it was reachable on | Flutter with no AVDs, no simulators and no phone — a Linux CI box, or a Mac without Xcode. There the list is macOS and Chrome, `attached` is false, and State 2 would have drawn. It now opens the picker with those two rows, which is the same list under a title that does not claim anything about attachment |

**Still dead, and left alone deliberately: `SingleDevice` (State 5).** Nothing sets it —
`devices_answered` is the only path into the picker states and it never names it — so it
is reachable only through `--dump single` and the demo walk. It was already marked
*superseded, see 7.6* in the state table, for the same reason State 2 is gone: 7.6
auto-launched a lone device and using it disproved the idea. Removing it is the same
edit as this one and belongs in its own change, with its own decision.

### 8.11 The toolchain is resolved, not assumed (built)

**`fvm` was hard-coded in three argv strings, and that made the tool unusable for
anyone who does not use FVM.** Not degraded — unusable. `Session::spawn`,
`probe::devices` and `probe::sdk_versions_slow` each built their own
`CommandBuilder::new("fvm")` / `run("fvm", &["flutter", …])`, so on a machine
where Flutter came from a tarball, Homebrew, asdf, mise or puro the first spawn
died with `fvm: command not found`, the device scan returned `Err`, and DESIGN.md
4 turns that into `✖ FATAL Failed to detect Flutter devices`. The header said
`Runtime (FVM)` because a literal in `ui::project` said so.

**What made this a small change is what FVM actually is.** `fvm flutter run`
resolves the version a project pins and execs that SDK's own `flutter` with the
arguments it was handed. So FVM is not a different way of running Flutter, it is
two words in front of one — and the whole of the difference fits in a struct:

```rust
pub struct Toolchain {
    program: String,     // fvm | flutter | whatever FRUN_FLUTTER names
    lead: Vec<String>,   // ["flutter"] under FVM, empty otherwise
    label: String,       // the Runtime column
    fvm: bool,           // where the version manifest is looked for
}
```

**Four rules, in order, and the order is the design.**

| Order | Rule | Runtime |
| ---: | :--- | :--- |
| 1 | `FRUN_FLUTTER` is set, parsed as an executable path or shell words | whatever it names |
| 2 | `.fvmrc` or `.fvm/` present **and** `fvm` on `PATH` | `FVM` |
| 3 | `flutter` on `PATH` | `SDK` |
| 4 | `fvm` on `PATH` and `flutter` not | `FVM` |
| — | neither | `SDK`, so the failure names `flutter` |

**Both halves of rule 2 are the rule.** A `.fvmrc` is a checked-in file, so it
travels to machines that have no FVM on them, where honouring it would fail every
spawn on a file that describes someone else's setup. And `fvm` installed against a
project that pins nothing is an indirection that resolves to the global SDK
anyway. Either half alone is a wrong answer that looks like a right one.

**One decision, asked by everyone, in a `OnceLock`.** The alternative — each call
site probing for itself — is a build that goes through `fvm` while the header
reports the version of a different SDK on `PATH`, and both halves being
individually defensible is exactly what would make that bug survive being looked
at. `argv()` returns the program and its prefix as one vector for the same reason:
`lead` without `program` is `flutter run` handed to `fvm`, and `program` without
`lead` is `fvm run`, which is not a command FVM has.

**The version manifest follows the same decision, and got faster for non-FVM
users as a side effect.** 7.3 item 1 reads `bin/cache/flutter.version.json`
rather than paying 3-4 seconds for `--version --machine`; that path is now found
two components up from the `flutter` on `PATH` when FVM is not in play.
`canonicalize` is what makes that work rather than nearly work — Homebrew leaves a
symlink in `/opt/homebrew/bin`, and two up from *that* is `/opt/homebrew`, which
has no `bin/cache`. A wrapper (`mise`, `puro`) always uses
`sdk_versions_slow`, because a separate `flutter` on `PATH` may select a different
SDK and its manifest would make the header lie about what the wrapper will run.

**`which` is a `PATH` walk, not a spawn.** `std::env::split_paths` plus a mode
test is two `stat`s against a process and a pipe, `which` is a builtin in some
shells and a separate binary in others, and — the actual reason —
`split_paths` is the same resolution `Command` and `CommandBuilder` will perform
later. The answer has to be the binary that will really run.

**Nothing in the process table needed changing, and that is not luck.**
`probe::busy` matches `flutter` + a bare `run` + `-d`, never `fvm`, because FVM
execs the SDK's own binary — so a `pgrep -fl flutter` line reads
`~/fvm/versions/3.35.0/bin/flutter run -d …` under FVM and
`~/development/flutter/bin/flutter run -d …` without it. The ` in use ` chip was
already toolchain-blind.

**What is on screen is what was resolved.** The `Runtime` column, the `DETECTING`
screen's command line, and the first line of `--probe` all read the same
`Toolchain`. The mock App holds `Toolchain::fvm()` unconditionally, so `--dump`
frames stay comparable across machines — a layout harness whose output depends on
which laptop ran it is not a harness.

**`FRUN_FLUTTER` travels with a `⇧Enter` handoff**, next to `PATH` and
`FVM_CACHE_PATH` and for the same reason 8.4 gives: the second tab re-resolves
everything itself, so a variable that changes the answer has to arrive with the
device or the two tabs build with different SDKs.

| Verified | How |
| :--- | :--- |
| Rule 2, on a real FVM project | `--probe` in a project pinning `3.29.3`: `runtime FVM  fvm flutter run -d <id>`, versions `3.29.3 / 3.7.2` from `flutter.version.json` — the pinned SDK, not the newer one on `PATH` |
| Rule 3, same machine | `--probe` in a project with no `.fvmrc`: `runtime SDK  flutter run -d <id>`, versions `3.44.9 / 3.12.2`, matching the manifest under `~/development/flutter` |
| Rule 1 outranks rule 2 | `FRUN_FLUTTER=/…/flutter/bin/flutter` in the FVM project reports `SDK` and that explicit SDK's versions |
| A wrapper with its own arguments | `FRUN_FLUTTER='mise exec flutter --'` → `runtime mise  mise exec flutter -- run -d <id>` |
| Rule 2 declines without `fvm` | `PATH` stripped of `/opt/homebrew/bin` in the FVM project falls back to `SDK` instead of failing |
| Neither present | `PATH=/usr/bin:/bin` reports `SDK` and `versions unresolved`, no panic and no hang |
| Mock frames unchanged | `--dump detecting` from a non-FVM project still prints `fvm flutter devices --machine`; the collapsed header still ends `Dart 3.7.2  FVM`; `--rows running` still reports 25 rows of full chrome |

| Unverified | Why |
| :--- | :--- |
| The pty spawn without FVM | Needs a build on a machine whose only Flutter is a plain one. `Session::spawn` and `probe::devices` take their argv from the same `Toolchain::argv` that `--probe` prints and that `probe::devices` was driven through live, so what is untested is the pty, not the argv |
| asdf and puro specifically | Neither is installed here. asdf's shim directory is `<sdk>/bin/flutter` after `canonicalize` and so is covered by rule 3; puro needs `FRUN_FLUTTER` and was exercised only through the `mise` shape |
