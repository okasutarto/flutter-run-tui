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
│ TerminalHeader: State Frame Switcher [1-7] & Title Command Banner           │
├────────────────────────────────────────────────────────────────────────────┤
│ ProjectCard: 2-Column Info (Left: Graphic Logo | Right: Metadata Table)    │
├────────────────────────────────────────────────────────────────────────────┤
│ SelectedTargetCard: Active Device Banner & Target Details (States 3-7)      │
├────────────────────────────────────────────────────────────────────────────┤
│ DYNAMIC STATE FRAME CONTENT                                                 │
│  ├─ State 1: FlutterDeviceManager (MULTIPLE_DEVICES)                       │
│  ├─ State 2: FlutterDeviceManager (NO_DEVICES - Launchable Simulators)      │
│  ├─ State 3: BuildPhaseTracker (Building in progress)                      │
│  ├─ State 4: BuildPhaseTracker (Build Failed + Stack Trace)                │
│  ├─ State 5: BuildPhaseTracker (Finished) + TerminalLogsView + CLI Prompt   │
│  ├─ State 6: HotReloadView (In Flight Progress)                             │
│  └─ State 7: HotReloadView (Reload Failed + Quick Actions)                 │
├────────────────────────────────────────────────────────────────────────────┤
│ Terminal Footer: Status Bar, Hotkey Cheatsheet, Grid Metrics              │
└────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 `TerminalHeader`
* **Title Bar**: Displays active command string (e.g., `fvm flutter run --target lib/main.dart -d ios-sim`).
* **Frame Navigation Bar**: Provides instant hotkey selectors (`[1] Pick Dev`, `[2] No Dev`, `[3] Build`, `[4] Build Fail`, `[5] Logs`, `[6] Reload`, `[7] Reload Fail`).

### 3.2 `ProjectCard` (2-Column Layout)
* **Left Column (4 Cols)**: Features a high-contrast Flutter Engine graphic logo badge (`my-2` vertical margin), version text (`Flutter Engine`), and `Cross-Platform CLI` subtitle without outer card borders.
* **Right Column (8 Cols)**: Key-value table showing:
  * `Project`: `cwclub`
  * `Version`: `1.4.0+12`
  * `Branch`: `main`
  * `Git Status`: `✔ clean`
  * **3-Column Technical Stats Row**: `Flutter` (`3.27.1`), `Dart` (`3.7.2`), `Runtime` (`(FVM)`).
* **Header Bar**: Includes path tag `~/cwclub` and a metadata `[COPY]` button.

### 3.3 `SelectedTargetCard`
* **Active Status Banner**: Green box header `✔ 1 device active: iPhone 16 Pro (emulator) (iOS)`.
* **Target Details Table**: Key-value grid specifying `Device Target`, `Platform ID`, `OS Version / Arch`, and `Type` (Simulator / Hardware).

### 3.4 `FlutterDeviceManager` (Device Selector)
Supports three distinct operation modes:
1. **`DETECTING`**: Displays animated Braille spinner (`⠋ ⠙ ⠹ ...`) during hardware scanning.
2. **`MULTIPLE_DEVICES`**: Displays interactive target list with arrow key selection (`↑↓`) and number hotkeys (`[1-N]`).
3. **`NO_DEVICES`**:
   * Header banner: `◆ NO DEVICE RUNNING` with subtitle `Nothing is attached. These can be started:`.
   * Title section: `Start a Device` with no category filter tabs, displaying a clean list of all launchable virtual simulators and emulators (`Pixel 10 Pro XL`, `Pixel 8`, `iPhone 17 Pro`, `iPhone 17 Pro Max`, `iPhone 17e`, `iPhone Air`, `iPhone 17`, `iPad Pro`, `macOS Desktop`, `Chrome Web`).
   * Action triggers: `▶ Start` button per row transitioning to `⠋ Booting...`.

### 3.5 `BuildPhaseTracker`
* **Progress Pipeline**: Real-time multi-stage list (`Resolving pubspec.yaml`, `Gradle build & Kotlin compilation`, `Compiling lib/main.dart`, `Syncing Flutter engine assets`).
* **Telemetry Bar**: Displays performance metrics (`CPU %`, `Memory MB`, `Build Time`, `Sync Time`).
* **Failure State**: Displays error summary, detailed stack trace, and action buttons (`[Retry Build]`, `[Fix with AI]`).

### 3.6 `TerminalLogsView`
* **16-Character Strict Gutter System**:
  ```
  01 14:32:01 [INF] Log content message line...
  02 14:32:02 [ OK] ⚡ Reloaded 125 of 1824 libraries in 148ms.
  ```
* **Clean Header**: Displays section title (`◆ APP LOGS STREAM`) and log count indicator (`[N entries]`) without cluttered search bars or filter toggles.

### 3.7 `InteractivePrompt`
* **CLI Input Box**: `➜ ~/cwclub ❯` interactive text input with `[Enter]` submit button and popup autocomplete command suggestions.
* **Hotkey Legend Bar**: Displayed underneath the input prompt in fine `text-[9px]` monospace print (`[r] Hot reload`, `[R] Hot restart`, `[h] Help`, `[c] Clear logs`, `[q] Quit`, and `Flutter 3.29.3 CLI`).

### 3.8 `HotReloadView`
* **`HOT_RELOAD_IN_FLIGHT`**: Live progress indicator (`⚡ Syncing updated Dart libraries to iPhone 16 Pro...`).
* **`HOT_RELOAD_FAILED`**: Displays Dart compilation error details, affected file line numbers, and instant recovery buttons (`[Retry Hot Reload]`, `[Hot Restart]`, `[Undo Changes]`).

---

## 🔄 4. State Frames & User Interaction Flows

```
                   ┌──────────────────────────┐
                   │   State 1: Device Picker │
                   └────────────┬─────────────┘
                                │ Select Device / Press [1-3]
                                ▼
                   ┌──────────────────────────┐
                   │     State 3: Building    │
                   └────────────┬─────────────┘
                                │
                 ┌──────────────┴──────────────┐
                 │ Build Success               │ Build Fail
                 ▼                             ▼
   ┌──────────────────────────┐  ┌──────────────────────────┐
   │   State 5: Running Logs  │  │   State 4: Build Failed  │
   │ (Build Phase Card + Logs)│  └──────────────────────────┘
   └─────────────┬────────────┘
                 │ Press 'r' / Hot Reload
                 ▼
   ┌──────────────────────────┐
   │    State 6: Hot Reload   │
   └─────────────┬────────────┘
                 │
                 ├─── Reload Success ───► Back to State 5
                 │
                 └─── Reload Fail ──────► State 7: Hot Reload Failed
```

---

## ⌨️ 5. Global Keyboard Shortcuts Cheatsheet

| Key Binding | Target Action | Scope |
| :--- | :--- | :--- |
| `1` | Switch to **State 1 (Device Picker)** | Global |
| `2` | Switch to **State 2 (No Devices)** | Global |
| `3` | Switch to **State 3 (Building)** | Global |
| `4` | Switch to **State 4 (Build Failed)** | Global |
| `5` | Switch to **State 5 (Running Logs)** | Global |
| `6` | Switch to **State 6 (Hot Reload In Flight)** | Global |
| `7` | Switch to **State 7 (Hot Reload Failed)** | Global |
| `r` / `reload` | Perform **Hot Reload** | State 5 / 7 |
| `R` / `restart` | Perform **Hot Restart** | State 5 / 7 |
| `↑` / `↓` | Navigate target list items | State 1 & 2 |
| `Enter` | Launch / Select highlighted device | State 1 & 2 |

---

## 📝 6. Typography & Box Drawing Rules

1. **Font Settings**: `font-family: 'JetBrains Mono', monospace; font-size: 12px; line-height: 1.5;`.
2. **Text Selection**: `select-none` enforced globally for app-like desktop feel.
3. **No Overlapping Text**: Gutter widths are strictly bounded (`w-[120px]`, `w-16`) to guarantee that labels never wrap or truncate mid-word.
4. **Border Nesting Math**: Outer container border radii set to `0px` or `rounded-none` to uphold pure retro terminal aesthetics.
