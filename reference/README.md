# Reference: the shell implementation being replaced

Snapshot taken before any of it was deleted, because `~/.config/zsh` is not a
git repository: without this the only copies would be timestamped files in
`.backup/`, themselves unversioned.

Kept for the porting work, not for running. See DESIGN.md 7.4.

| File | Lines | What to take from it |
| :--- | :--- | :--- |
| `frun-runner` | 1,749 | The ~166 lines of rules: which Flutter output marks which stage, the hot reload ack/timeout machine, startup log buffering. |
| `frun.zsh` | 1,201 | SDK version fast path, device list parsing, bootable target enumeration, boot-and-wait, AVD-to-serial lookup. |
| `frun-theme.zsh` | 102 | Superseded by `theme.rs`. Kept only so the old palette can be compared. |

The comments matter more than the code. Several encode findings that are not
derivable from Flutter's documentation and were paid for once already:

* Flutter formats elapsed time through `NumberFormat`, so durations carry a
  group separator (`1,234ms`) and `[0-9]+` clips them.
* `Built build/...` is the unambiguous Gradle-finished signal; the Gradle line
  itself re-emits partial elapsed values, so treating the first as completion
  reports a far too short build.
* Flutter discards terminal input while busy and reports it only via
  `printTrace()`, which never reaches stdout. A keypress is a request, not a
  fact, which is the entire reason State 11 exists.
* `flutter.png` carries ~79px of transparent padding per side, which renders as
  dead columns; `flutter-trim.png` is the same artwork cropped.
* macOS has no `timeout(1)`, so the Android boot wait is a bounded counter
  rather than a wrapped command.
