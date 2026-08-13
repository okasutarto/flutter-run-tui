# Palette. Sourced here so the functions below have it at
# definition time, and re-sourced at the top of frun() so the
# NO_COLOR / not-a-terminal checks reflect the actual invocation
# rather than whenever .zshrc happened to load this file.
FRUN_THEME="${0:A:h}/frun-theme.zsh"
[ -f "$FRUN_THEME" ] || FRUN_THEME="$HOME/.config/zsh/frun-theme.zsh"
[ -f "$FRUN_THEME" ] && source "$FRUN_THEME"


render_frun_header() {
  local project_name="$1"
  local project_version="$2"
  local branch="$3"
  local git_status="$4"
  local flutter_version="$5"
  local dart_version="$6"

  # Prefer the trimmed asset. flutter.png is a 400x300 canvas with
  # ~79px of transparent padding on each side, which chafa renders as
  # dead columns and throws off the left gutter. flutter-trim.png is
  # the same artwork cropped to its content (242x300).
  local logo="$HOME/.config/zsh/assets/flutter-trim.png"
  [ -f "$logo" ] || logo="$HOME/.config/zsh/assets/flutter.png"

  # Palette comes from frun-theme.zsh so the card, the sections
  # below it, and frun-runner cannot drift apart. Role names, not
  # colour names: CYAN used to mean 39 here and 117 in the runner.
  local CYAN="$FRUN_ACCENT"
  local LIGHT_CYAN="$FRUN_HEAD"
  local GREEN="$FRUN_OK"
  local YELLOW="$FRUN_WARN"
  local RESET="$FRUN_RESET"
  local BOLD="$FRUN_BOLD"

  # ==========================================================
  # Size the value column to the actual content
  # ==========================================================
  # The card auto-fits the longest value so the left and right
  # gutters end up visually identical. VCAP is a hard ceiling so a
  # very long branch name can't blow the card past a sane width.
  #
  # Geometry constants live here rather than next to the drawing
  # code because the value ceiling is derived from them, and the
  # ceiling has to be known before values are truncated.

  local GUTTER=2
  local LOGO_W=13
  local LOGO_GAP=3
  local LABEL_W=8

  local COLS=$(tput cols 2>/dev/null || echo 80)

  # The logo zone costs 16 columns before any content. On a window
  # too narrow to afford it, drop the logo rather than let the card
  # overflow: a wrapped card also breaks the overlay's cursor
  # arithmetic, which corrupts every row rather than just one.
  local SHOW_LOGO=1
  local PAD_L=$((GUTTER + LOGO_W + LOGO_GAP))

  # Fixed overhead around the value column: both borders, the left
  # pad, the label, the 2-col label gap, the right gutter.
  local CHROME=$((2 + PAD_L + LABEL_W + 2 + GUTTER))

  # 3 = "FVM", the shortest the value column is ever allowed to be.
  if (( COLS < CHROME + 3 )); then
    SHOW_LOGO=0
    PAD_L=$GUTTER
    CHROME=$((2 + PAD_L + LABEL_W + 2 + GUTTER))
  fi

  local VCAP=32

  # Second ceiling, from the terminal.
  local VFIT=$((COLS - CHROME))
  (( VFIT < VCAP )) && VCAP=$VFIT

  # Floor. Below roughly 19 columns nothing can be drawn sensibly
  # and the terminal will wrap whatever we emit.
  (( VCAP < 3 )) && VCAP=3

  local VMAX=3            # "FVM" is always present
  local _v _len

  # zsh's (l:n::c:) padding flag needs a parameter to act on, so
  # point it at an empty one. Used to draw the border runs without
  # forking seq once per card.
  local _e=""

  for _v in project_name project_version branch git_status \
            flutter_version dart_version; do
    _len=${#${(P)_v}}
    (( _len > VCAP )) && _len=$VCAP
    (( _len > VMAX )) && VMAX=$_len
  done

  # Truncate anything past the chosen width so the border stays put.
  for _v in project_name project_version branch git_status \
            flutter_version dart_version; do
    if (( ${#${(P)_v}} > VMAX )); then
      eval "$_v=\"\${$_v:0:\$((VMAX - 3))}...\""
    fi
  done

  # ==========================================================
  # Draw card
  # ==========================================================
  # Layout (columns, 1-based). GUTTER is the inner padding and is
  # identical on both sides, so the card reads symmetrically.
  #
  #   col 1        : left border  ╭ │ ╰
  #   col 2  - 3   : left gutter  (GUTTER = 2)
  #   col 4  - 16  : logo (chafa renders 13 cols x 8 rows)
  #   col 17 - 19  : gap (LOGO_GAP = 3)
  #   col 20 - 27  : label (8 cols; longest label is "Runtime" = 7)
  #   col 28 - 29  : gap (2 cols)
  #   col 30 ...   : value (VMAX cols, sized to the content)
  #   next 2 cols  : right gutter (GUTTER = 2)
  #   last col     : right border ╮ │ ╯
  #
  # PAD_L = left gutter + logo + logo gap = 2 + 13 + 3 = 18.
  # W (inner) = PAD_L + label + gap + value + right gutter.
  #
  # LOGO_W must match what chafa actually emits, otherwise the logo
  # and the label column drift apart.

  # GUTTER, LOGO_W, LOGO_GAP, LABEL_W and PAD_L are set further up,
  # alongside the value ceiling that depends on them.
  local W=$((PAD_L + LABEL_W + 2 + VMAX + GUTTER))
  local TOTAL=$((W + 2))

  # The title needs room too; widen the card if the content is tiny,
  # but only when the terminal can actually take it. Widening past
  # the window would reintroduce the wrapping VCAP just prevented.
  local TITLE="◆ PROJECT INFO"
  local TITLE_LEN=14
  if (( W < PAD_L + TITLE_LEN + GUTTER )) \
     && (( PAD_L + TITLE_LEN + GUTTER + 2 <= COLS )); then
    W=$((PAD_L + TITLE_LEN + GUTTER))
    TOTAL=$((W + 2))
  fi

  # Published so the separator rules in frun() and in frun-runner
  # match the card instead of the old hardcoded 49 columns.
  export FRUN_RULE_W="$TOTAL"

  # Helper: print a row with label + value.
  # Padding is computed from the PLAIN value length, then color
  # escapes are emitted separately so printf never counts them.
  # Args: $1=label, $2=value, $3=optional color, $4=optional bold flag
  _frun_row() {
    local label="$1" value="$2" color="${3:-}" bold="${4:-}"
    local pad_r
    pad_r=$((W - PAD_L - LABEL_W - 2 - ${#value}))
    [ "$pad_r" -lt 0 ] && pad_r=0

    printf "${CYAN}│${RESET}"
    printf "%${PAD_L}s" ""
    printf "%-${LABEL_W}s" "$label"
    printf "  "
    printf "%s%s%s" "${bold}${color}" "$value" "$RESET"
    printf "%${pad_r}s" ""
    printf "${CYAN}│${RESET}\n"
  }

  # Top border
  printf "${CYAN}╭"
  printf '%s' "${(l:$W::─:)_e}"
  printf "╮${RESET}\n"

  # Empty row
  printf "${CYAN}│${RESET}%${W}s${CYAN}│${RESET}\n" ""

  # Title row (centered)
  local title_pad_l=$(( (W - TITLE_LEN) / 2 ))
  local title_pad_r=$((W - TITLE_LEN - title_pad_l))
  printf "${CYAN}│${RESET}%${title_pad_l}s${LIGHT_CYAN}${BOLD}%s${RESET}%${title_pad_r}s${CYAN}│${RESET}\n" \
    "" "$TITLE" ""

  # Empty row
  printf "${CYAN}│${RESET}%${W}s${CYAN}│${RESET}\n" ""

  _frun_row "Project" "$project_name" "" "$BOLD"
  _frun_row "Version" "$project_version"
  _frun_row "Branch" "$branch"

  if [ "$git_status" = "Clean" ]; then
    _frun_row "Git" "$git_status" "$GREEN"
  else
    _frun_row "Git" "$git_status" "$YELLOW"
  fi

  # Empty row
  printf "${CYAN}│${RESET}%${W}s${CYAN}│${RESET}\n" ""

  _frun_row "Flutter" "$flutter_version"
  _frun_row "Dart" "$dart_version"
  _frun_row "Runtime" "FVM"

  # Empty row
  printf "${CYAN}│${RESET}%${W}s${CYAN}│${RESET}\n" ""

  # Bottom border
  printf "${CYAN}╰"
  printf '%s' "${(l:$W::─:)_e}"
  printf "╯${RESET}\n"

  # ==========================================================
  # Overlay Flutter PNG
  # ==========================================================

  # -t 1 because the kitty escape is a binary payload: piped or
  # redirected, it lands in the file as tens of kilobytes of base64
  # instead of a picture.
  if (( SHOW_LOGO )) \
     && [ -t 1 ] \
     && [ -f "$logo" ] \
     && command -v chafa >/dev/null 2>&1; then
    # Card is 14 rows tall:
    #   1  top border
    #   2  empty
    #   3  title
    #   4  empty
    #   5  Project      <- logo starts here
    #   6  Version
    #   7  Branch
    #   8  Git
    #   9  empty
    #   10 Flutter
    #   11 Dart
    #   12 Runtime      <- logo ends here (8 rows: 5..12)
    #   13 empty
    #   14 bottom border
    #
    # After the bottom border's newline the cursor sits on row 15,
    # so row N of the card is (15 - N) rows up.

    printf '\033[s'

    # Row 5 is 10 rows up. Move right past the border + left gutter
    # so the logo's left edge sits at the same inset as the right
    # gutter. The trimmed asset has no transparent margin, so the
    # artwork starts exactly at that column.
    #
    # 13x8 renders as 13 cols x 8 rows, matching the 8 text rows
    # (Project .. Runtime).
    printf '\033[10A'
    printf "\033[$((GUTTER + 1))C"

    chafa \
      --format kitty \
      --size ${LOGO_W}x8 \
      "$logo"

    printf '\033[u'

    # --------------------------------------------------------
    # Repair right border
    #
    # Kitty image placement can clobber cells on the right edge,
    # so redraw column $TOTAL for every card row.
    # --------------------------------------------------------

    printf '\033[s'

    # Row 1 (top border) = 14 rows up
    printf '\033[14A'
    printf "\033[${TOTAL}G"
    printf "${CYAN}╮${RESET}"

    # Rows 2..13 (12 body rows)
    local i
    for i in {1..12}; do
      printf "\033[1B\033[${TOTAL}G"
      printf "${CYAN}│${RESET}"
    done

    # Row 14 (bottom border)
    printf "\033[1B\033[${TOTAL}G"
    printf "${CYAN}╯${RESET}"

    printf '\033[u'
  fi
}


# ============================================================
# Flutter / Dart version lookup (fast path)
# ============================================================
# `fvm flutter --version --machine` costs 3-4s because it boots the
# Dart VM. The SDK already ships the same data as a plain JSON file
# at bin/cache/flutter.version.json, so read that instead.
#
# Prints two lines (flutter version, dart version) and exits non-zero
# when the manifest can't be located, so callers can fall back.
#
# This only reads what is already on disk. It deliberately does no
# update checking: `flutter run` owns that, and skipping it here keeps
# frun's behaviour identical to running `fvm flutter run` directly.

frun_sdk_versions() {
  python3 <<'PY'
import json
import os
import sys

from pathlib import Path


def read_json(path):
    try:
        with open(path) as handle:
            return json.load(handle)

    except Exception:
        return None


sdk = None

# Per-project symlink, when FVM created one.
link = Path(".fvm/flutter_sdk")

if link.exists():
    sdk = link.resolve()

# Otherwise resolve the pinned version against the FVM cache.
if sdk is None:
    pin = ""

    project = read_json(".fvmrc")

    if isinstance(project, dict):
        pin = (
            project.get("flutter")
            or project.get("flutterSdkVersion")
            or ""
        )

    if pin:
        cache = os.environ.get("FVM_CACHE_PATH", "")

        if not cache:
            settings = read_json(
                Path.home()
                / "Library/Application Support/fvm/.fvmrc"
            )

            if isinstance(settings, dict):
                cache = settings.get("cachePath") or ""

        if not cache:
            cache = str(Path.home() / "fvm")

        sdk = Path(cache).expanduser() / "versions" / pin

if sdk is None:
    sys.exit(1)

manifest = read_json(
    Path(sdk) / "bin/cache/flutter.version.json"
)

if not isinstance(manifest, dict):
    sys.exit(1)

flutter = (
    manifest.get("frameworkVersion")
    or manifest.get("flutterVersion")
    or ""
)

dart = manifest.get("dartSdkVersion") or ""

if not flutter and not dart:
    sys.exit(1)

print(flutter or "-")
print(dart or "-")
PY
}


# ============================================================
# Mobile device list
# ============================================================
# Turns `flutter devices --machine` JSON into tab-separated rows:
#
#   id \t display name \t platform \t icon
#
# Mobile only: macOS, Mac Designed for iPad and Chrome are dropped,
# since frun exists to run on phones. Android emulator names are
# resolved from the AVD via adb, because Flutter reports them as
# "sdk gphone64 arm64" which tells you nothing about which AVD it is.

_frun_device_list() {
  python3 - "$1" <<'PY'
import json
import re
import subprocess
import sys

try:
    with open(sys.argv[1]) as f:
        devices = json.load(f)
except Exception:
    sys.exit(0)


def clean_avd_name(name):
    name = name.replace("_", " ").replace("-", " ")
    name = re.sub(r"\s+", " ", name)
    return name.strip()


for d in devices:
    flutter_name = d.get("name", "Unknown")
    device_id = d.get("id", "")
    platform = d.get("targetPlatform", "")
    emulator = d.get("emulator", False)

    name_lower = flutter_name.lower()

    # iOS
    is_ios = (
        platform.startswith("ios")
        or "iphone" in name_lower
    )

    if is_ios:
        print(
            f"{device_id}\t"
            f"{flutter_name}\t"
            f"iOS\t"
            f""
        )
        continue

    # Android
    is_android = (
        platform.startswith("android")
        or "android" in name_lower
    )

    if not is_android:
        continue

    display_name = flutter_name

    # Resolve emulator AVD name.
    if emulator or device_id.startswith("emulator-"):
        try:
            result = subprocess.run(
                [
                    "adb",
                    "-s",
                    device_id,
                    "emu",
                    "avd",
                    "name",
                ],
                capture_output=True,
                text=True,
                timeout=2,
            )

            lines = [
                line.strip()
                for line in result.stdout.splitlines()
                if line.strip()
                and line.strip().upper() != "OK"
            ]

            if lines:
                display_name = clean_avd_name(lines[0])

        except Exception:
            pass

    print(
        f"{device_id}\t"
        f"{display_name}\t"
        f"Android\t"
        f""
    )
PY
}


# ============================================================
# Device scan
# ============================================================
# Sets DEVICE_LIST and DEVICE_COUNT. Deliberately not local: frun()
# keeps its state in globals and reads both afterwards.

_frun_scan_devices() {
  local title="$1"
  local tmp
  local rc

  tmp=$(mktemp)

  gum spin \
    --spinner dot \
    --title "$title" \
    -- \
    sh -c 'fvm flutter devices --machine > "$1" 2>/dev/null' \
    _ "$tmp"

  rc=$?

  if [ "$rc" -ne 0 ]; then
    rm -f "$tmp"

    frun_err "✘ Failed to detect Flutter devices"

    return 1
  fi

  DEVICE_LIST=$(_frun_device_list "$tmp")

  rm -f "$tmp"

  DEVICE_COUNT=$(
    printf "%s\n" "$DEVICE_LIST" |
    sed '/^$/d' |
    wc -l |
    xargs
  )

  return 0
}


# ============================================================
# Bootable targets
# ============================================================
# Things that are not running but could be, as tab-separated rows:
#
#   kind \t id \t display name \t platform \t icon
#
# kind is "avd" or "sim", and id is what the respective tool wants:
# an AVD name for the emulator, a UDID for simctl.

_frun_bootable_targets() {
  # Android. We only reach here with nothing attached, so every AVD
  # is a candidate. Underscores become spaces to match how the
  # running-device list already cleans AVD names.
  if command -v emulator >/dev/null 2>&1; then
    local avd
    emulator -list-avds 2>/dev/null | while IFS= read -r avd; do
      [ -n "$avd" ] || continue
      # Trailing field is the platform glyph, same one the running
      # device list uses. Leaving it empty is what made every picker
      # row start with a stray indent the first time around.
      printf 'avd\t%s\t%s\tAndroid\t\n' "$avd" "${avd//_/ }"
    done
  fi

  # iOS. Shutdown only: a Booted simulator that Flutter cannot see is
  # a different problem, and offering to boot it again would not fix
  # anything.
  if command -v xcrun >/dev/null 2>&1; then
    local sim_tmp
    sim_tmp=$(mktemp)

    xcrun simctl list devices available -j > "$sim_tmp" 2>/dev/null

    # The JSON travels via a file and argv rather than a pipe.
    # `python3 -` takes its program from stdin, and a heredoc on the
    # same command replaces that stdin, so the pipe never arrives and
    # sys.stdin reads empty. Same argv pattern as _frun_device_list.
    python3 - "$sim_tmp" <<'PY'
import json
import sys

try:
    with open(sys.argv[1]) as handle:
        data = json.load(handle)

except Exception:
    raise SystemExit

for runtime, devices in data.get("devices", {}).items():
    # Runtime keys look like
    # "com.apple.CoreSimulator.SimRuntime.iOS-26-5", so this also
    # excludes watchOS, tvOS and visionOS entries.
    if "iOS" not in runtime:
        continue

    for device in devices:
        if device.get("state") != "Shutdown":
            continue

        print(
            f"sim\t"
            f"{device['udid']}\t"
            f"{device['name']}\t"
            f"iOS\t"
            f"\uf179"
        )
PY

    rm -f "$sim_tmp"
  fi
}


# ============================================================
# AVD name -> adb serial
# ============================================================
# The picker offers AVD names, but Flutter identifies an emulator by
# its adb serial: you choose "Pixel_8" and Flutter calls it
# "emulator-5554". _frun_device_list already walks this mapping in the
# other direction via `adb emu avd name`, so this just inverts it.
#
# Doing it here avoids a second `flutter devices --machine`, which
# boots the Dart VM and costs about ten seconds. adb answers instantly.

_frun_avd_serial() {
  local want="$1"
  local serial name
  local -a serials

  serials=( ${(f)"$(adb devices 2>/dev/null | awk 'NR > 1 && $2 == "device" { print $1 }')"} )

  for serial in $serials; do
    name=$(
      adb -s "$serial" emu avd name 2>/dev/null |
      tr -d '\r' |
      grep -v '^OK$' |
      head -1
    )

    if [ "$name" = "$want" ]; then
      printf '%s' "$serial"
      return 0
    fi
  done

  return 1
}


# ============================================================
# Boot a target and wait for it
# ============================================================
# macOS has no timeout(1), so the Android wait is a bounded counter
# rather than a wrapped command.

_frun_boot_target() {
  local kind="$1"
  local id="$2"
  local name="$3"
  local rc

  # The id Flutter will use for the thing we just booted. Left empty
  # if it cannot be worked out, which tells the caller to fall back to
  # asking Flutter.
  FRUN_BOOTED_ID=""

  case "$kind" in
    avd)
      # Detached subshell so the emulator outlives this shell.
      ( emulator -avd "$id" >/dev/null 2>&1 & )

      # Polling `adb shell` is safe with nothing attached: it fails
      # fast instead of blocking, which `adb wait-for-device` would
      # do forever if the emulator never came up.
      #
      # sys.boot_completed, not just adb presence: adb answers well
      # before Android is ready to install an APK.
      gum spin \
        --spinner dot \
        --title "Booting $name..." \
        -- \
        sh -c '
          i=0

          while [ "$i" -lt 180 ]; do
            if [ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d "\r")" = "1" ]; then
              exit 0
            fi

            sleep 1
            i=$((i + 1))
          done

          exit 1
        '

      rc=$?

      if [ "$rc" -ne 0 ]; then
        return "$rc"
      fi

      FRUN_BOOTED_ID=$(_frun_avd_serial "$id")

      return 0
      ;;

    sim)
      # Bring the Simulator window up first, otherwise the device
      # boots headless and there is nothing to look at.
      open -a Simulator >/dev/null 2>&1

      # bootstatus -b boots the device if needed and blocks until it
      # has finished, so this needs no polling loop of its own.
      gum spin \
        --spinner dot \
        --title "Booting $name..." \
        -- \
        xcrun simctl bootstatus "$id" -b

      rc=$?

      # A simulator's Flutter device id *is* its simctl UDID, so
      # nothing needs looking up here.
      if [ "$rc" -eq 0 ]; then
        FRUN_BOOTED_ID="$id"
      fi

      return "$rc"
      ;;
  esac

  return 1
}


# ============================================================
# No-device fallback
# ============================================================
# Offers the bootable targets in the same picker style as the
# running-device list, boots the choice, and reports whether the
# caller should scan again.

_frun_boot_fallback() {
  local targets
  targets=$(_frun_bootable_targets)

  # Nothing to offer: let the caller fall through to its own error.
  if [ -z "${targets//[[:space:]]/}" ]; then
    return 1
  fi

  frun_head "◆ NO DEVICE RUNNING"

  echo

  frun_faint "  Nothing is attached. These can be started:"

  echo

  local picker
  picker=$(
    printf "%s\n" "$targets" |
    awk -F '\t' '
      NF >= 4 {
        # Field 1 is what fzf shows; the rest ride along for the
        # caller to pull back out. platform and icon come too, so the
        # booted device can be described without re-querying Flutter.
        printf "%s %-28s  %-8s\t%s\t%s\t%s\t%s\t%s\n",
          $5, $3, $4, $1, $2, $3, $4, $5
      }
    '
  )

  local selected
  selected=$(
    printf "%s\n" "$picker" |
    fzf \
      --height=~100% \
      --layout=reverse \
      --border=rounded \
      --border-label=" Start a Device " \
      --border-label-pos=2 \
      --padding="1,1,0,1" \
      --delimiter=$'\t' \
      --with-nth=1 \
      --no-info \
      --no-input \
      --pointer="❯" \
      --bind="ctrl-c:abort"
  )

  if [ -z "$selected" ]; then
    frun_errline "✘ Cancelled"

    return 1
  fi

  local kind id name platform icon
  kind=$(printf "%s" "$selected"     | awk -F '\t' '{print $2}')
  id=$(printf "%s" "$selected"       | awk -F '\t' '{print $3}')
  name=$(printf "%s" "$selected"     | awk -F '\t' '{print $4}')
  platform=$(printf "%s" "$selected" | awk -F '\t' '{print $5}')
  icon=$(printf "%s" "$selected"     | awk -F '\t' '{print $6}')

  if _frun_boot_target "$kind" "$id" "$name"; then
    frun_ok "✓ $name is ready"

    echo

    # Hand back a finished one-row device list. We know exactly what
    # was booted, so there is nothing to re-detect and the caller can
    # go straight to launching.
    if [ -n "$FRUN_BOOTED_ID" ]; then
      DEVICE_LIST=$(
        printf '%s\t%s\t%s\t%s' \
          "$FRUN_BOOTED_ID" "$name" "$platform" "$icon"
      )

      DEVICE_COUNT=1

      # This list was built, not discovered. The caller reads this to
      # stay quiet about counting devices: the user just picked one by
      # hand and watched it boot, so "1 device detected" would be the
      # third time in a row we name the same thing.
      FRUN_DEVICE_PRESELECTED=1
    fi

    return 0
  fi

  frun_err "✘ $name did not finish booting"

  return 1
}



frun() {
  clear

  # Per-run state. These are shell globals, so without clearing them
  # a previous invocation would leak its answers into this one.
  FRUN_DEVICE_PRESELECTED=""
  FRUN_BOOTED_ID=""

  # ============================================================
  # Validate Flutter project
  # ============================================================

  if [ ! -f "pubspec.yaml" ]; then
    frun_err "✘ pubspec.yaml not found"
    frun_faint "Run frun from a Flutter project directory."

    return 1
  fi


  # ============================================================
  # Project info
  # ============================================================

  PROJECT_NAME=$(awk '/^name:/ {print $2; exit}' pubspec.yaml)
  PROJECT_VERSION=$(awk '/^version:/ {print $2; exit}' pubspec.yaml)
  BRANCH=$(git branch --show-current 2>/dev/null)

  [ -z "$PROJECT_NAME" ] && PROJECT_NAME=$(basename "$PWD")
  [ -z "$PROJECT_VERSION" ] && PROJECT_VERSION="-"
  [ -z "$BRANCH" ] && BRANCH="-"


  # ============================================================
  # Git status
  # ============================================================

  GIT_COUNT=$(
    git status --porcelain 2>/dev/null |
    wc -l |
    xargs
  )

  if [ -z "$GIT_COUNT" ]; then
    GIT_STATUS="-"
  elif [ "$GIT_COUNT" -eq 0 ]; then
    GIT_STATUS="Clean"
  elif [ "$GIT_COUNT" -eq 1 ]; then
    GIT_STATUS="1 changed"
  else
    GIT_STATUS="$GIT_COUNT changed"
  fi


  # ============================================================
  # Flutter / Dart versions
  # ============================================================

  FLUTTER_VERSION=""
  DART_VERSION=""

  # Fast path: read the SDK's version manifest directly.
  SDK_VERSIONS=$(frun_sdk_versions 2>/dev/null)

  if [ -n "$SDK_VERSIONS" ]; then
    VERSION_LINES=("${(@f)SDK_VERSIONS}")

    FLUTTER_VERSION="${VERSION_LINES[1]}"
    DART_VERSION="${VERSION_LINES[2]}"
  fi

  # Slow path: ask the Flutter tool. Shown with a spinner because it
  # boots the Dart VM and takes a few seconds.
  if [ -z "$FLUTTER_VERSION" ] || [ -z "$DART_VERSION" ]; then
    VERSION_TMP=$(mktemp)

    gum spin \
      --spinner dot \
      --title "Reading Flutter version..." \
      -- \
      sh -c 'fvm flutter --version --machine > "$1" 2>/dev/null' \
      _ "$VERSION_TMP"

    MACHINE_VERSIONS=$(
      python3 - "$VERSION_TMP" <<'PY'
import json
import sys

try:
    with open(sys.argv[1]) as handle:
        data = json.load(handle)

except Exception:
    sys.exit(1)

print(data.get("frameworkVersion", "-"))
print(data.get("dartSdkVersion", "-"))
PY
    )

    rm -f "$VERSION_TMP"

    if [ -n "$MACHINE_VERSIONS" ]; then
      VERSION_LINES=("${(@f)MACHINE_VERSIONS}")

      FLUTTER_VERSION="${VERSION_LINES[1]}"
      DART_VERSION="${VERSION_LINES[2]}"
    fi
  fi

  [ -z "$FLUTTER_VERSION" ] && FLUTTER_VERSION="-"
  [ -z "$DART_VERSION" ] && DART_VERSION="-"


  # ============================================================
  # Dashboard header
  # ============================================================

  render_frun_header \
    "$PROJECT_NAME" \
    "$PROJECT_VERSION" \
    "$BRANCH" \
    "$GIT_STATUS" \
    "$FLUTTER_VERSION" \
    "$DART_VERSION"

  echo

  # ============================================================
  # Detect Flutter devices
  # ============================================================
  # Runs through a helper because it can happen twice: once now, and
  # again after booting an emulator when nothing was attached.

  _frun_scan_devices "Detecting Flutter devices..." || return 1


  # ============================================================
  # Nothing attached -> offer to start something
  # ============================================================
  # Previously this was a dead end: frun printed "No device(s)
  # detected" and returned 1, even with AVDs and simulators sitting
  # there ready to boot. Booting one by hand and re-running frun was
  # the only way forward.

  if [ "$DEVICE_COUNT" -eq 0 ]; then
    # The fallback fills DEVICE_LIST itself when it can identify what
    # it booted, which is the normal case. Asking Flutter again is
    # only for when that lookup came up empty.
    if _frun_boot_fallback && [ "$DEVICE_COUNT" -eq 0 ]; then
      _frun_scan_devices "Locating the new device..." || return 1
    fi
  fi

  if [ "$DEVICE_COUNT" -eq 0 ]; then
    frun_err "✘ No device(s) detected"

    return 1
  fi


  # ============================================================
  # Detection result
  # ============================================================
  # Skipped when the device came from the boot fallback, which has
  # already announced it as ready, by name.

  if [ -z "$FRUN_DEVICE_PRESELECTED" ]; then
    if [ "$DEVICE_COUNT" -eq 1 ]; then
      frun_ok "✓ 1 device detected"
    else
      frun_ok "✓ $DEVICE_COUNT devices detected"
    fi

    echo
  fi


  # ============================================================
  # Remember last device
  # ============================================================

  LAST_DEVICE_FILE="$HOME/.config/zsh/.frun-last-device"
  LAST_DEVICE_ID=""

  if [ -f "$LAST_DEVICE_FILE" ]; then
    LAST_DEVICE_ID=$(cat "$LAST_DEVICE_FILE")
  fi


  # ============================================================
  # One device -> auto select
  # ============================================================

  if [ "$DEVICE_COUNT" -eq 1 ]; then
    DEVICE_LINE=$(printf "%s\n" "$DEVICE_LIST")


  # ============================================================
  # Multiple devices -> picker
  # ============================================================

  else

    frun_head "◆ AVAILABLE TARGETS"

    echo

    SORTED_DEVICE_LIST=$(
      printf "%s\n" "$DEVICE_LIST" |
      awk -F '\t' -v last="$LAST_DEVICE_ID" '
        $1 == last {
          remembered = $0
          next
        }

        {
          others[++count] = $0
        }

        END {
          if (remembered != "")
            print remembered

          for (i = 1; i <= count; i++)
            print others[i]
        }
      '
    )

    PICKER_LIST=$(
      printf "%s\n" "$SORTED_DEVICE_LIST" |
      awk -F '\t' '
        {
          id       = $1
          name     = $2
          platform = $3
          icon     = $4

          # Single space after the icon: the slot used to be empty,
          # so two spaces were compensating for a glyph that never
          # arrived and every row started with a stray indent.
          printf "%s %-28s  %-8s\t%s\n",
            icon,
            name,
            platform,
            id
        }
      '
    )

    SELECTED=$(
      printf "%s\n" "$PICKER_LIST" |
      fzf \
        --height=~100% \
        --layout=reverse \
        --border=rounded \
        --border-label=" Select Flutter Devices " \
        --border-label-pos=2 \
        --padding="1,1,0,1" \
        --delimiter=$'\t' \
        --with-nth=1 \
        --no-info \
        --no-input \
        --pointer="❯" \
        --bind="ctrl-c:abort"
    )

    if [ -z "$SELECTED" ]; then
      frun_errline "✘ Cancelled"

      return 130
    fi

    SELECTED_ID=$(
      printf "%s" "$SELECTED" |
      awk -F '\t' '{print $2}'
    )

    DEVICE_LINE=$(
      printf "%s\n" "$DEVICE_LIST" |
      awk -F '\t' -v id="$SELECTED_ID" '
        $1 == id {
          print
          exit
        }
      '
    )

    echo
  fi


  # ============================================================
  # Extract selected target
  # ============================================================

  DEVICE_ID=$(
    printf "%s" "$DEVICE_LINE" |
    awk -F '\t' '{print $1}'
  )

  DEVICE_NAME=$(
    printf "%s" "$DEVICE_LINE" |
    awk -F '\t' '{print $2}'
  )

  DEVICE_PLATFORM=$(
    printf "%s" "$DEVICE_LINE" |
    awk -F '\t' '{print $3}'
  )

  DEVICE_ICON=$(
    printf "%s" "$DEVICE_LINE" |
    awk -F '\t' '{print $4}'
  )


  # ============================================================
  # Save last selected device
  # ============================================================

  printf "%s" "$DEVICE_ID" > "$LAST_DEVICE_FILE"


  # ============================================================
  # Selected target
  # ============================================================

  frun_head "◆ SELECTED TARGET"
  frun_okline "  ✓ Device       $DEVICE_ICON $DEVICE_NAME"
  frun_okline "  ✓ Platform     $DEVICE_PLATFORM"

  echo


  # ============================================================
  # Hot controls
  # ============================================================

  frun_bold "HOT CONTROLS"

  frun_plain "  r    ↻   Hot Reload"
  frun_plain "  R    ⚡  Hot Restart"
  # ⏏ U+23CF and ⏹ U+23F9 are both East Asian width N, so they
  # occupy one column everywhere and the 3-space gap holds. The ×
  # they replace was width Ambiguous, which renders as 2 columns
  # on a CJK-configured terminal and shifted these two rows.
  frun_plain "  q    ⏏   Quit"
  frun_plain "  ^C   ⏹   Stop"

  echo

  frun_rule

  echo

  frun_accent "🚀 Launching Flutter..."

  echo


  # ============================================================
  # Launch realtime runner
  # ============================================================

  "$HOME/.config/zsh/frun-runner" \
    "$DEVICE_ID" \
    "$@"
}