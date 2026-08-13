//! Everything that reads a fact off the machine: the project, the SDK, the
//! devices, and booting one.
//!
//! DESIGN.md 7.3 items 1, 2 and 4. All of it is `Command` plus a little
//! parsing, so it lives in one module rather than three: the shared parts (a
//! spawn helper with a timeout, and the FVM SDK path) are the reason.
//!
//! No new dependency does any of this. `git2` for two `git` calls, a YAML crate
//! for two `pubspec.yaml` lines, or a regex engine for `split(':')` would each
//! be more code to hold than the code they replace.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

// ============================================================
// Running commands
// ============================================================

/// Run a command and return stdout, or `None` if it could not be run, exited
/// non-zero, or outlived `limit`.
///
/// The timeout exists for `adb`. A wedged adb server blocks its client
/// indefinitely, and the shell implementation guarded every `adb` call with
/// `timeout=2` for that reason. Discovery runs on a worker thread, so a hang
/// would not freeze the UI, but it would leave `DETECTING` on screen forever
/// with no way out, which is the same bug wearing a nicer coat.
pub fn run(program: &str, args: &[&str], limit: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + limit;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();

                if let Some(mut pipe) = child.stdout.take() {
                    let _ = pipe.read_to_string(&mut out);
                }

                return status.success().then_some(out);
            }

            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();

                    return None;
                }

                std::thread::sleep(Duration::from_millis(20));
            }

            Err(_) => return None,
        }
    }
}

/// Commands that are allowed to take their time: the Dart VM needs several
/// seconds just to start, and a first Gradle sync far longer.
const SLOW: Duration = Duration::from_secs(90);

/// Commands that answer immediately or are broken.
const QUICK: Duration = Duration::from_secs(3);

// ============================================================
// Wall clock
// ============================================================

/// Local-time offset in seconds, read once.
///
/// `std::time` has no notion of a timezone and there is no way to ask libc for
/// one without a dependency. `date` knows, costs one spawn at startup, and the
/// answer cannot go stale in a way that matters: a session that spans a DST
/// boundary would show one hour of log timestamps an hour out.
fn local_offset() -> i64 {
    let Some(raw) = run("date", &["+%z"], QUICK) else {
        return 0;
    };

    let raw = raw.trim();

    if raw.len() < 5 {
        return 0;
    }

    let sign = if raw.starts_with('-') { -1 } else { 1 };
    let hours: i64 = raw[1..3].parse().unwrap_or(0);
    let minutes: i64 = raw[3..5].parse().unwrap_or(0);

    sign * (hours * 3600 + minutes * 60)
}

/// `HH:MM:SS` in local time, for the log gutter.
pub struct Clock {
    offset: i64,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            offset: local_offset(),
        }
    }

    pub fn now(&self) -> String {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let day = (epoch + self.offset).rem_euclid(86_400);

        format!(
            "{:02}:{:02}:{:02}",
            day / 3600,
            (day % 3600) / 60,
            day % 60
        )
    }
}

// ============================================================
// 1. Project metadata
// ============================================================

pub struct Project {
    pub name: String,
    pub version: String,
    pub branch: String,
    /// Count of `git status --porcelain` rows. Zero is clean.
    pub dirty: usize,
    pub flutter: String,
    pub dart: String,
    /// Working directory with `$HOME` folded back to `~`.
    pub cwd: String,
}

/// Read what the ProjectCard shows. Never fails: an absent git repo or an
/// unresolvable SDK is a dash, not an error, because none of it stops a run.
pub fn project() -> Project {
    let (name, version) = pubspec();

    Project {
        name,
        version,
        branch: git(&["branch", "--show-current"]).unwrap_or_else(|| "-".into()),
        dirty: git(&["status", "--porcelain"])
            .map(|out| out.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
        flutter: "-".into(),
        dart: "-".into(),
        cwd: pretty_cwd(),
    }
}

/// `name:` and `version:` from `pubspec.yaml`.
///
/// A line scan, not a YAML parse. Both keys are top level, so the anchor is
/// column zero — which is also what makes this safe against the same keys
/// appearing nested under `dependencies:`, where they are indented.
fn pubspec() -> (String, String) {
    let text = std::fs::read_to_string("pubspec.yaml").unwrap_or_default();

    let field = |key: &str| -> Option<String> {
        text.lines()
            .find_map(|line| line.strip_prefix(key))?
            .split('#')
            .next()
            .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
            .filter(|v| !v.is_empty())
    };

    (
        field("name:").unwrap_or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "-".into())
        }),
        field("version:").unwrap_or_else(|| "-".into()),
    )
}

fn git(args: &[&str]) -> Option<String> {
    let out = run("git", args, QUICK)?;
    let out = out.trim_end_matches('\n').to_string();

    (!out.trim().is_empty()).then_some(out)
}

fn pretty_cwd() -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let home = home();

    match cwd.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => cwd.display().to_string(),
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

// ============================================================
// SDK versions
// ============================================================

/// Framework and Dart versions, from the SDK's own manifest.
///
/// `fvm flutter --version --machine` reports the same two values and costs 3-4
/// seconds because it boots the Dart VM. `bin/cache/flutter.version.json` is a
/// plain file that is already on disk. The shell implementation worked this out
/// and this is the same conclusion.
pub fn sdk_versions() -> Option<(String, String)> {
    let manifest = fvm_sdk()?.join("bin/cache/flutter.version.json");
    let json: Value = serde_json::from_str(&std::fs::read_to_string(manifest).ok()?).ok()?;

    let flutter = json
        .get("frameworkVersion")
        .or_else(|| json.get("flutterVersion"))
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();

    let dart = json
        .get("dartSdkVersion")
        .and_then(Value::as_str)
        // The manifest carries the full build string; the version is its head.
        .and_then(|v| v.split_whitespace().next())
        .unwrap_or("-")
        .to_string();

    Some((flutter, dart))
}

/// Slow path, for a project FVM has not materialised an SDK for yet.
pub fn sdk_versions_slow() -> Option<(String, String)> {
    let raw = run("fvm", &["flutter", "--version", "--machine"], SLOW)?;

    // Flutter prepends its own notices to this, so take the JSON object rather
    // than assuming the output starts with one.
    let start = raw.find('{')?;
    let json: Value = serde_json::from_str(&raw[start..]).ok()?;

    Some((
        json.get("frameworkVersion")
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string(),
        json.get("dartSdkVersion")
            .and_then(Value::as_str)
            .and_then(|v| v.split_whitespace().next())
            .unwrap_or("-")
            .to_string(),
    ))
}

/// Resolve the SDK this project pins.
///
/// Two routes, in the order FVM itself creates them: the per-project symlink
/// when it exists, otherwise the pin in `.fvmrc` resolved against the cache.
fn fvm_sdk() -> Option<PathBuf> {
    let link = Path::new(".fvm/flutter_sdk");

    if link.exists() {
        return std::fs::canonicalize(link).ok();
    }

    let project: Value = serde_json::from_str(&std::fs::read_to_string(".fvmrc").ok()?).ok()?;

    let pin = project
        .get("flutter")
        .or_else(|| project.get("flutterSdkVersion"))
        .and_then(Value::as_str)?;

    Some(fvm_cache().join("versions").join(pin))
}

fn fvm_cache() -> PathBuf {
    if let Some(path) = std::env::var_os("FVM_CACHE_PATH") {
        return PathBuf::from(path);
    }

    let settings = home().join("Library/Application Support/fvm/.fvmrc");

    if let Ok(text) = std::fs::read_to_string(settings) {
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            if let Some(path) = json.get("cachePath").and_then(Value::as_str) {
                return PathBuf::from(path);
            }
        }
    }

    home().join("fvm")
}

// ============================================================
// 2. Device discovery
// ============================================================

/// Which family a target belongs to. Decides the glyph, the badge, and whether
/// the thing has to be booted before it can be run on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Platform {
    Ios,
    Android,
    Desktop,
    Web,
}

impl Platform {
    /// From Flutter's `targetPlatform`.
    fn from_target(target: &str) -> Self {
        if target.starts_with("ios") {
            Platform::Ios
        } else if target.starts_with("android") {
            Platform::Android
        } else if target.starts_with("web") {
            Platform::Web
        } else {
            // darwin, linux-x64, windows-x64.
            Platform::Desktop
        }
    }

    /// Desktop and web are always available, so they never pass through
    /// `BOOTING`, and they are not what "nothing is attached" is about.
    pub fn needs_boot(self) -> bool {
        matches!(self, Platform::Ios | Platform::Android)
    }
}

#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    /// Flutter's `targetPlatform`, shown as `Platform ID`.
    pub target_platform: String,
    /// Flutter's `sdk`, shown as `OS Version / Arch`.
    pub sdk: String,
    pub virtual_device: bool,
    pub last_used: bool,

    /// How to make this device runnable, for a row in `NO_DEVICES`. `None` means
    /// it is already attached and Flutter can address it now.
    ///
    /// A bootable target being the same type as a running device is what lets
    /// states 2 and 4 share one list widget and one row renderer. They differ by
    /// a `▶ Start` badge, not by shape.
    pub boot: Option<Boot>,
}

impl Device {
    /// Whether this counts as attached, which is what decides the discovery
    /// branch in DESIGN.md 4.
    ///
    /// macOS and Chrome are always present in `flutter devices` output, so
    /// counting them would make `NO_DEVICES` unreachable on any Mac and its
    /// "Nothing is attached" copy false. They are targets, not attachments.
    pub fn attached(&self) -> bool {
        self.platform.needs_boot()
    }
}

/// `fvm flutter devices --machine`, with the fields the shell version threw
/// away.
///
/// Returns `Err` only when the command itself failed, which DESIGN.md 4 treats
/// as fatal. An empty list is a successful answer of "nothing", not an error.
pub fn devices(last_used: &str) -> Result<Vec<Device>, String> {
    let raw = run("fvm", &["flutter", "devices", "--machine"], SLOW)
        .ok_or_else(|| "Failed to detect Flutter devices".to_string())?;

    let start = raw
        .find('[')
        .ok_or_else(|| "Flutter reported no device list".to_string())?;

    let parsed: Vec<Value> = serde_json::from_str(&raw[start..])
        .map_err(|e| format!("Could not read the device list: {e}"))?;

    let mut devices: Vec<Device> = parsed
        .iter()
        .map(|d| {
            let id = str_field(d, "id");
            let target_platform = str_field(d, "targetPlatform");
            let emulator = d.get("emulator").and_then(Value::as_bool).unwrap_or(false);

            Device {
                name: android_name(&id, &str_field(d, "name"), emulator),
                platform: Platform::from_target(&target_platform),
                // `sdkNameAndVersion` is the field's name inside flutter_tools;
                // the machine output calls it `sdk`. Accept either.
                sdk: d
                    .get("sdk")
                    .or_else(|| d.get("sdkNameAndVersion"))
                    .and_then(Value::as_str)
                    .unwrap_or("-")
                    .to_string(),
                virtual_device: emulator,
                last_used: !id.is_empty() && id == last_used,
                boot: None,
                target_platform,
                id,
            }
        })
        .collect();

    // The remembered device goes to the top, and says why it moved. Stable
    // otherwise, so Flutter's own order survives.
    devices.sort_by_key(|d| !d.last_used);

    Ok(devices)
}

fn str_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Turn `emulator-5554` into the AVD's own name.
///
/// Flutter reports the running emulator as `sdk gphone64 arm64`, which is the
/// same string for every AVD on the machine and so tells you nothing about
/// which one is attached. `adb` knows, and answers instantly.
fn android_name(id: &str, flutter_name: &str, emulator: bool) -> String {
    if !(emulator || id.starts_with("emulator-")) {
        return flutter_name.to_string();
    }

    match avd_name(id) {
        Some(name) => name,
        None => flutter_name.to_string(),
    }
}

fn avd_name(serial: &str) -> Option<String> {
    let out = run("adb", &["-s", serial, "emu", "avd", "name"], QUICK)?;

    out.lines()
        .map(|l| l.trim_end_matches('\r').trim())
        .find(|l| !l.is_empty() && *l != "OK")
        .map(pretty_avd)
}

/// `Pixel_10_Pro_XL` reads as a filename; `Pixel 10 Pro XL` reads as a phone.
fn pretty_avd(name: &str) -> String {
    name.replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================
// Bootable targets
// ============================================================

/// What starting a target actually requires.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Boot {
    /// An AVD name for `emulator -avd`.
    Avd(String),
    /// A simulator UDID for `simctl`.
    Sim(String),
    /// Already available: a Flutter device id that needs no boot at all.
    Ready(String),
}

/// Everything launchable while nothing is attached: AVDs, shut-down simulators,
/// and whatever Flutter already offers that needs no booting.
///
/// The mobile-only restriction the shell version applied is lifted here, per
/// DESIGN.md 3.3.
pub fn bootable(available: &[Device]) -> Vec<Device> {
    let mut targets = Vec::new();

    if let Some(out) = run("emulator", &["-list-avds"], QUICK) {
        for avd in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
            targets.push(target(
                avd,
                &pretty_avd(avd),
                Platform::Android,
                Boot::Avd(avd.to_string()),
            ));
        }
    }

    targets.extend(simulators());

    // Desktop and web, straight from what Flutter reported. No boot step: these
    // go directly to launch, which is the asymmetry DESIGN.md 3.3 calls out.
    for device in available.iter().filter(|d| !d.attached()) {
        let mut row = device.clone();
        row.boot = Some(Boot::Ready(device.id.clone()));
        targets.push(row);
    }

    targets
}

/// A bootable row. It has no `targetPlatform` or `sdk` because nothing has told
/// us yet — the thing is not running.
fn target(id: &str, name: &str, platform: Platform, boot: Boot) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform,
        target_platform: String::new(),
        sdk: String::new(),
        virtual_device: true,
        last_used: false,
        boot: Some(boot),
    }
}

/// Shut-down iOS simulators.
///
/// Shutdown only: a booted simulator Flutter cannot see is a different problem,
/// and booting it again would not fix it.
fn simulators() -> Vec<Device> {
    let Some(raw) = run(
        "xcrun",
        &["simctl", "list", "devices", "available", "-j"],
        QUICK,
    ) else {
        return Vec::new();
    };

    let Ok(json) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };

    let Some(runtimes) = json.get("devices").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut targets = Vec::new();

    for (runtime, devices) in runtimes {
        // Keys look like `com.apple.CoreSimulator.SimRuntime.iOS-26-5`, so this
        // also excludes watchOS, tvOS and visionOS.
        if !runtime.contains("iOS") {
            continue;
        }

        for device in devices.as_array().into_iter().flatten() {
            if device.get("state").and_then(Value::as_str) != Some("Shutdown") {
                continue;
            }

            let (Some(udid), Some(name)) = (
                device.get("udid").and_then(Value::as_str),
                device.get("name").and_then(Value::as_str),
            ) else {
                continue;
            };

            targets.push(target(
                udid,
                name,
                Platform::Ios,
                Boot::Sim(udid.to_string()),
            ));
        }
    }

    targets
}

// ============================================================
// 4. Boot
// ============================================================

/// How long an Android boot is given before it is called hung.
///
/// The same 180s the shell version used. Android genuinely takes minutes on a
/// cold AVD, which is why the UI shows an elapsed clock rather than a bare
/// spinner: three minutes of animation cannot be told apart from a wedge.
const BOOT_LIMIT: Duration = Duration::from_secs(180);

/// Boot a target and return the id Flutter will address it by.
///
/// Blocking, and meant to be called on a worker thread.
pub fn boot(target: &Boot) -> Result<String, String> {
    match target {
        Boot::Ready(id) => Ok(id.clone()),
        Boot::Sim(udid) => boot_sim(udid),
        Boot::Avd(name) => boot_avd(name),
    }
}

fn boot_sim(udid: &str) -> Result<String, String> {
    // Bring the window up first, or the device boots headless and there is
    // nothing to look at.
    let _ = run("open", &["-a", "Simulator"], QUICK);

    // `bootstatus -b` boots if needed and blocks until the device is ready, so
    // there is no polling loop to write.
    run("xcrun", &["simctl", "bootstatus", udid, "-b"], BOOT_LIMIT)
        .map(|_| udid.to_string())
        .ok_or_else(|| "did not finish booting".to_string())
}

fn boot_avd(name: &str) -> Result<String, String> {
    // `nohup`, so the emulator ignores the hangup when frun exits.
    //
    // The emulator has to outlive frun: booting costs half a minute at best, and
    // the next run should find it already there. That part works — quit with `q`
    // and `adb devices` still reports it.
    //
    // Honest about the limit: `nohup` does not save it from the terminal itself
    // being destroyed. Measured under a pty that closes the instant frun exits,
    // the emulator dies anyway, so something beyond SIGHUP is involved there.
    // Ignoring SIGHUP is still the right thing to ask for and is what the shell
    // implementation reached for with its detached subshell; surviving a
    // terminal that is being torn down is a further problem, and not one that
    // arises when a human quits frun and keeps their window.
    //
    // stdio is null, so no `nohup.out` is written.
    Command::new("nohup")
        .args(["emulator", "-avd", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start the emulator: {e}"))?;

    let deadline = Instant::now() + BOOT_LIMIT;

    // `sys.boot_completed`, not adb presence: adb answers well before Android
    // will accept an APK. Polling `adb shell` is safe with nothing attached,
    // where it fails fast rather than blocking the way `wait-for-device` would.
    while Instant::now() < deadline {
        let ready = run("adb", &["shell", "getprop", "sys.boot_completed"], QUICK)
            .map(|out| out.trim().trim_end_matches('\r') == "1")
            .unwrap_or(false);

        if ready {
            return avd_serial(name).ok_or_else(|| {
                "booted, but adb never reported a serial for it".to_string()
            });
        }

        std::thread::sleep(Duration::from_secs(1));
    }

    Err("did not finish booting".to_string())
}

/// AVD name -> adb serial.
///
/// The picker offers `Pixel_8`; Flutter wants `emulator-5554`. `adb emu avd
/// name` already walks this mapping in the other direction, so this inverts it
/// rather than paying for a second `flutter devices --machine`, which would
/// cost another ten seconds of Dart VM startup.
fn avd_serial(want: &str) -> Option<String> {
    let out = run("adb", &["devices"], QUICK)?;
    let wanted = pretty_avd(want);

    for line in out.lines().skip(1) {
        let mut cols = line.split_whitespace();

        let (Some(serial), Some("device")) = (cols.next(), cols.next()) else {
            continue;
        };

        if avd_name(serial).as_deref() == Some(wanted.as_str()) {
            return Some(serial.to_string());
        }
    }

    None
}

// ============================================================
// Last used device
// ============================================================

/// Where the previous choice is remembered.
///
/// Same path the shell implementation used, so an existing install keeps its
/// memory across the cutover.
fn last_device_file() -> PathBuf {
    home().join(".config/zsh/.frun-last-device")
}

pub fn last_device() -> String {
    std::fs::read_to_string(last_device_file())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn remember_device(id: &str) {
    let _ = std::fs::write(last_device_file(), id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_comes_from_the_target_triple() {
        assert_eq!(Platform::from_target("ios"), Platform::Ios);
        assert_eq!(Platform::from_target("ios-simulator"), Platform::Ios);
        assert_eq!(Platform::from_target("android-arm64"), Platform::Android);
        assert_eq!(Platform::from_target("web-javascript"), Platform::Web);
        assert_eq!(Platform::from_target("darwin"), Platform::Desktop);

        // Unknown triples must land somewhere rather than being dropped: the
        // whole point of lifting the mobile-only filter is that a target frun
        // has never heard of still shows up.
        assert_eq!(Platform::from_target("fuchsia-arm64"), Platform::Desktop);
    }

    #[test]
    fn desktop_and_web_are_not_attachments() {
        let device = |platform| Device {
            id: "x".into(),
            name: "x".into(),
            platform,
            target_platform: String::new(),
            sdk: String::new(),
            virtual_device: false,
            last_used: false,
            boot: None,
        };

        // The distinction that keeps NO_DEVICES reachable: macOS and Chrome are
        // in every `flutter devices` answer, so counting them as attached would
        // make "nothing is attached" a state that never happens.
        assert!(device(Platform::Ios).attached());
        assert!(device(Platform::Android).attached());
        assert!(!device(Platform::Desktop).attached());
        assert!(!device(Platform::Web).attached());
    }

    #[test]
    fn avd_names_read_as_device_names() {
        assert_eq!(pretty_avd("Pixel_10_Pro_XL"), "Pixel 10 Pro XL");
        assert_eq!(pretty_avd("Pixel_8"), "Pixel 8");
        // Collapsed, not doubled up.
        assert_eq!(pretty_avd("Nexus__5X-API_30"), "Nexus 5X API 30");
    }

    #[test]
    fn the_clock_wraps_the_day_rather_than_going_negative() {
        // A negative UTC offset just before midnight UTC must not produce
        // `-01:...`, which `%` alone would.
        let clock = Clock { offset: -8 * 3600 };
        let stamp = clock.now();

        assert_eq!(stamp.len(), 8, "{stamp}");
        assert!(!stamp.starts_with('-'), "{stamp}");
    }

    /// The device list is the one parse where a real payload is worth pinning:
    /// every field here was discarded by the shell version and 3.2 needs them.
    #[test]
    fn the_machine_device_list_keeps_the_fields_the_target_card_needs() {
        let raw = r#"[
          {"name":"sdk gphone16k arm64","id":"emulator-5554","targetPlatform":"android-arm64",
           "emulator":true,"sdk":"Android 17 (API 37)"},
          {"name":"macOS","id":"macos","targetPlatform":"darwin","emulator":false,
           "sdk":"macOS 26.6.1 25G76 darwin-arm64"},
          {"name":"Chrome","id":"chrome","targetPlatform":"web-javascript","emulator":false,
           "sdk":"Google Chrome 151.0"}
        ]"#;

        let parsed: Vec<Value> = serde_json::from_str(raw).expect("fixture parses");
        assert_eq!(parsed.len(), 3);

        let macos = &parsed[1];
        assert_eq!(str_field(macos, "id"), "macos");
        assert_eq!(
            Platform::from_target(&str_field(macos, "targetPlatform")),
            Platform::Desktop
        );
        assert!(macos.get("sdk").and_then(Value::as_str).unwrap().contains("arm64"));
    }
}
