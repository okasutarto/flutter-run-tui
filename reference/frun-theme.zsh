# ============================================================
# frun palette - single source of truth
# ============================================================
# Read by two consumers:
#
#   frun.zsh      sources this file directly
#   frun-runner   parses the FRUN_C_* lines (see load_theme there)
#
# Because of the second consumer, every colour must stay on its
# own line in the exact form:
#
#   FRUN_C_<ROLE>=<256-colour code>
#
# No quotes, no arithmetic, no trailing comments on those lines.
# Anything else in this file is ignored by the parser.
#
# Roles, not names. The old code had CYAN at both 39 and 117 and
# two different reds (196 in frun.zsh, 203 in frun-runner) for
# the same meaning, so there was no way to answer "what colour
# is an error?" without grepping both files.

# Card border, and the launch line. The structural accent.
FRUN_C_ACCENT=39

# Section headings: PROJECT INFO, AVAILABLE TARGETS, APP LOGS.
FRUN_C_HEAD=117

# Success, completed stages, clean git.
FRUN_C_OK=82

# In-progress stages, dirty git, dropped keypresses.
FRUN_C_WARN=220

# Platform build stages: CocoaPods, Gradle, Xcode, install.
FRUN_C_STAGE=212

# Failures and aborts. Previously split across 196 and 203 for
# no stated reason; unified on 203 because 196 at full
# saturation is harsh next to the rest of the palette. Change
# this one line to move every error message in both files.
FRUN_C_ERR=203

# ============================================================
# Derived escapes (zsh consumers only)
# ============================================================
# $'...' so these hold real ESC bytes: they are printed via %s,
# which does not interpret \033.

FRUN_ACCENT=$'\033[38;5;'"${FRUN_C_ACCENT}"$'m'
FRUN_HEAD=$'\033[38;5;'"${FRUN_C_HEAD}"$'m'
FRUN_OK=$'\033[38;5;'"${FRUN_C_OK}"$'m'
FRUN_WARN=$'\033[38;5;'"${FRUN_C_WARN}"$'m'
FRUN_STAGE=$'\033[38;5;'"${FRUN_C_STAGE}"$'m'
FRUN_ERR=$'\033[38;5;'"${FRUN_C_ERR}"$'m'

FRUN_RESET=$'\033[0m'
FRUN_BOLD=$'\033[1m'
FRUN_FAINT=$'\033[2m'

# Honour NO_COLOR, and drop colour when stdout is not a terminal
# so piped output stays clean.
if [[ -n "${NO_COLOR:-}" ]] || [[ ! -t 1 ]]; then
  FRUN_ACCENT="" FRUN_HEAD="" FRUN_OK="" FRUN_WARN=""
  FRUN_STAGE="" FRUN_ERR=""
  FRUN_RESET="" FRUN_BOLD="" FRUN_FAINT=""
fi


# ============================================================
# Output helpers
# ============================================================
# These replace `gum style` for static text. gum is a separate
# process per call, and frun's happy path made about 15 of them
# just to colour fixed strings: measured at 264ms versus 3ms for
# the equivalent printf.
#
# `gum spin` is still worth its process, since it needs to stay
# alive and animate while another command runs. Only the static
# styling moved here.
#
# Verified byte-identical in effect to the calls they replace:
# gum style emits ESC[<attrs>m<text>ESC[0m and nothing else, no
# implicit margin or padding.

frun_err()     { printf '%s%s%s%s\n' "$FRUN_ERR"    "$FRUN_BOLD" "$1" "$FRUN_RESET" }
frun_errline() { printf '%s%s%s\n'   "$FRUN_ERR"                 "$1" "$FRUN_RESET" }
frun_ok()      { printf '%s%s%s%s\n' "$FRUN_OK"     "$FRUN_BOLD" "$1" "$FRUN_RESET" }
frun_okline()  { printf '%s%s%s\n'   "$FRUN_OK"                  "$1" "$FRUN_RESET" }
frun_head()    { printf '%s%s%s%s\n' "$FRUN_HEAD"   "$FRUN_BOLD" "$1" "$FRUN_RESET" }
frun_accent()  { printf '%s%s%s%s\n' "$FRUN_ACCENT" "$FRUN_BOLD" "$1" "$FRUN_RESET" }
frun_faint()   { printf '%s%s%s\n'   "$FRUN_FAINT"                "$1" "$FRUN_RESET" }
frun_bold()    { printf '%s%s%s\n'   "$FRUN_BOLD"                 "$1" "$FRUN_RESET" }
frun_plain()   { printf '%s\n' "$1" }

# Separator sized to the header card. render_frun_header exports
# FRUN_RULE_W; the fallback matters only if a caller draws a rule
# before the card, which nothing currently does.
frun_rule() {
  local w="${FRUN_RULE_W:-49}"
  local _p=""
  printf '%s%s%s\n' "$FRUN_FAINT" "${(l:$w::─:)_p}" "$FRUN_RESET"
}
