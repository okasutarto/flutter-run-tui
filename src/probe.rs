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

        format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
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
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
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

    /// For `Device::to_handoff`. Not `Debug`: that is a formatting of a type, and
    /// nothing should be able to rename a variant and change a wire format.
    fn tag(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Android => "android",
            Platform::Desktop => "desktop",
            Platform::Web => "web",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "ios" => Some(Platform::Ios),
            "android" => Some(Platform::Android),
            "desktop" => Some(Platform::Desktop),
            "web" => Some(Platform::Web),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    /// Flutter's `targetPlatform`, shown as `Platform ID`.
    pub target_platform: String,
    /// Flutter's `sdk`, shown as `OS Version`.
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
    /// This row, flattened into one line, for the tab a `⇧⏎` spawns (8.4).
    ///
    /// **What this exists to avoid is a second discovery.** The first version handed
    /// over the id alone and let the new process resolve it against its own scan,
    /// which meant `fvm flutter devices --machine` — six seconds of Dart VM startup —
    /// before a build that needed none of it. The spawning tab already knows the row;
    /// the point of a handoff is that the answer travels with the question.
    ///
    /// Only the fields the new process cannot wait for: the id it runs, the name its
    /// card and its tab title show, the platform behind the glyph, whether it is
    /// virtual — `release_target` shuts virtual devices down, so guessing that one
    /// would kill a physical phone — and how to boot it when it is not up.
    /// `target_platform` and `sdk` are display-only and arrive with the background
    /// scan, so they are left out rather than duplicated.
    ///
    /// Tab-separated: device names, AVD names and simulator ids all contain spaces
    /// and colons, and none of them contains a tab.
    pub fn to_handoff(&self) -> String {
        let boot = match &self.boot {
            None => String::new(),
            Some(Boot::Avd(name)) => format!("avd:{name}"),
            Some(Boot::Sim(id)) => format!("sim:{id}"),
        };

        [
            self.id.as_str(),
            self.name.as_str(),
            self.platform.tag(),
            if self.virtual_device { "1" } else { "0" },
            boot.as_str(),
        ]
        .join("\t")
    }

    /// The other half of `to_handoff`.
    ///
    /// All five fields or nothing. A value with fewer is not a device this process
    /// can start without asking questions, and starting on a guess is worse than
    /// showing the picker: the platform decides the glyph and whether a boot is even
    /// possible, and `virtual_device` decides whether frun may shut the thing down.
    pub fn from_handoff(line: &str) -> Option<Device> {
        let fields: Vec<&str> = line.split('\t').collect();
        let [id, name, platform, virtual_device, boot] = fields[..] else {
            return None;
        };

        if id.is_empty() {
            return None;
        }

        let boot = match boot.split_once(':') {
            Some(("avd", name)) => Some(Boot::Avd(name.to_string())),
            Some(("sim", id)) => Some(Boot::Sim(id.to_string())),
            _ => None,
        };

        Some(Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: Platform::from_tag(platform)?,
            target_platform: String::new(),
            sdk: String::new(),
            virtual_device: virtual_device == "1",
            // The row it came from was the one under the cursor, not necessarily the
            // remembered one, and `choose` is about to write this id there anyway.
            last_used: false,
            boot,
        })
    }

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
                sdk: sdk_name(
                    d.get("sdk")
                        .or_else(|| d.get("sdkNameAndVersion"))
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                ),
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

/// `com.apple.CoreSimulator.SimRuntime.iOS-26-5` → `iOS-26-5`.
///
/// Not a Flutter bug being papered over: `IOSSimulator.sdkNameAndVersion` *is*
/// `simulatorCategory` (`ios/simulators.dart:599`), and that is the key
/// `simctl list --json` groups its devices under. So the field arrives as a
/// runtime's reverse-DNS identifier, which is not an OS version at all — and the
/// row it fills is labelled `OS Version`.
///
/// The prefix is 32 of the 43 columns it occupied, and all it said was "this is
/// an Apple simulator runtime", which the iOS glyph on the same card and the
/// `virtual` chip on the picker row already said twice.
///
/// Only the prefix goes. `iOS-26-5` is Apple's own spelling of the runtime, and
/// anything that is not a simulator runtime is Flutter's own prose — `Android 17
/// (API 37)`, `macOS 26.6.1 25G76 darwin-arm64` — which is already what the label
/// promises and is passed through untouched.
fn sdk_name(raw: &str) -> String {
    const SIM_RUNTIME: &str = "com.apple.CoreSimulator.SimRuntime.";

    raw.strip_prefix(SIM_RUNTIME).unwrap_or(raw).to_string()
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
///
/// There is no `Ready` variant. A target that needs no booting is `boot: None`,
/// which is the same thing an already-running device says, and they are treated
/// identically from the moment they are picked: launch now. Two ways to spell one
/// fact is one way too many.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Boot {
    /// An AVD name for `emulator -avd`.
    Avd(String),
    /// A simulator UDID for `simctl`.
    Sim(String),
}

/// Every target worth offering, in one list, in the order they should be shown.
///
/// One list and not two, per DESIGN.md 7.6. Splitting them was what trapped you
/// on whichever platform happened to be running: with one device attached there
/// was nothing to choose, so booting anything else was unreachable without
/// quitting. Booting *is* a choice, so it belongs in the picker.
///
/// Order is running first, then things that need starting, then the platforms
/// that are simply always there. That puts the single `Enter` case at the top and
/// the slowest options last.
///
/// The mobile-only restriction the shell version applied is lifted, per 3.3.
pub fn targets(reported: Vec<Device>, last_used: &str) -> Vec<Device> {
    let (attached, always_available): (Vec<Device>, Vec<Device>) =
        reported.into_iter().partition(|d| d.attached());

    let mut targets = attached;

    if let Some(out) = run("emulator", &["-list-avds"], QUICK) {
        for avd in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let name = pretty_avd(avd);

            // An AVD that is already running is in the attached list under this
            // same name — `android_name` resolves the serial back through
            // `adb emu avd name` precisely so the two agree — so listing it again
            // would offer to boot a device you are looking at.
            if targets.iter().any(|d| d.name == name) {
                continue;
            }

            targets.push(target(
                avd,
                &name,
                Platform::Android,
                "",
                Boot::Avd(avd.to_string()),
            ));
        }
    }

    // Shut-down only, so a booted simulator cannot appear twice.
    targets.extend(simulators());

    // macOS, Chrome and friends. No boot step and nothing to wait for, which is
    // why they sit last rather than competing with a device you can see.
    targets.extend(always_available);

    // Stamped here, on the merged list, and not only in `devices()`.
    //
    // A bootable row comes from `target()`, which had no way to know about the
    // remembered device and hardcoded `false`. So the chip vanished exactly when
    // it was most useful: the simulator you always reach for is off, and nothing
    // on screen says which one that was.
    //
    // Note the asymmetry this cannot fix. An iOS simulator carries the same UDID
    // whether running or shut down, so it matches either way. A running Android
    // emulator is `emulator-5554` while its bootable row is the AVD name, so
    // those never match and Android cannot be recovered this way once it is off.
    for device in &mut targets {
        device.last_used = !device.id.is_empty() && device.id == last_used;
    }

    // Sorted once, at the end. `devices()` sorts its own half, and the bootable
    // rows appended after it would otherwise undo that.
    targets.sort_by_key(|d| !d.last_used);

    targets
}

/// A bootable row. It has no `targetPlatform` because nothing has told us yet —
/// the thing is not running.
///
/// `sdk` is passed in rather than left blank, because for one of the two callers
/// it is already known: `simctl` groups its devices under the runtime, and that
/// runtime string is the whole of what Flutter would report for the same device
/// once booted. An AVD has no equivalent, so it passes `""`.
fn target(id: &str, name: &str, platform: Platform, sdk: &str, boot: Boot) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform,
        target_platform: String::new(),
        sdk: sdk.to_string(),
        virtual_device: true,
        last_used: false,
        boot: Some(boot),
    }
}

/// How to start `device` again, once it is no longer running.
///
/// `Device::boot` is cleared when a device is adopted as the target — a running
/// device has nothing left to start — so a target that has since stopped carries no
/// way back. This reconstructs one:
///
/// * A simulator keeps its UDID whether booted or not, so `Boot::Sim` needs nothing
///   looked up.
/// * An emulator runs as `emulator-5554` and its AVD name is not recoverable from
///   the serial once it is dead — `adb emu avd name` needs a device to answer. So the
///   AVD is found by name instead, against `emulator -list-avds`, which is the same
///   join `targets()` uses to de-duplicate the two.
///
/// `None` for a physical device, macOS or Chrome: there is nothing frun can start.
pub fn boot_target(device: &Device) -> Option<Boot> {
    if let Some(boot) = &device.boot {
        return Some(boot.clone());
    }

    if !device.virtual_device {
        return None;
    }

    match device.platform {
        Platform::Ios => Some(Boot::Sim(device.id.clone())),

        Platform::Android => {
            let out = run("emulator", &["-list-avds"], QUICK)?;

            out.lines()
                .map(str::trim)
                .find(|avd| pretty_avd(avd) == device.name)
                .map(|avd| Boot::Avd(avd.to_string()))
        }

        Platform::Desktop | Platform::Web => None,
    }
}

/// Ids that are up right now, asked of the two tools that answer immediately.
///
/// Measured on this machine: `adb devices` 12ms, `simctl list -j` 119ms,
/// `fvm flutter devices --machine` **6113ms**. That gap is the entire reason this
/// exists. Rechecking the switch list does not need to rediscover devices — the
/// list is already there — it needs to know which of its rows are still real, and
/// that question is 145ms rather than six seconds (8.5).
///
/// Every adb serial, not only emulators: a phone unplugged mid-session is the same
/// stale row as an emulator that was shut down.
pub fn alive() -> std::collections::HashSet<String> {
    let mut up = std::collections::HashSet::new();

    if let Some(out) = run("adb", &["devices"], QUICK) {
        up.extend(
            out.lines()
                .skip(1)
                .filter_map(|line| line.split_whitespace().next())
                .filter(|serial| !serial.is_empty())
                .map(str::to_string),
        );
    }

    if let Some(raw) = run(
        "xcrun",
        &["simctl", "list", "devices", "available", "-j"],
        QUICK,
    ) {
        if let Ok(json) = serde_json::from_str::<Value>(&raw) {
            let runtimes = json.get("devices").and_then(Value::as_object);

            for devices in runtimes.into_iter().flatten().map(|(_, v)| v) {
                for device in devices.as_array().into_iter().flatten() {
                    if device.get("state").and_then(Value::as_str) != Some("Booted") {
                        continue;
                    }

                    if let Some(udid) = device.get("udid").and_then(Value::as_str) {
                        up.insert(udid.to_string());
                    }
                }
            }
        }
    }

    up
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

            // The runtime is the key this device is filed under, so its version
            // is known before it boots and without asking anything: the same
            // string, through the same `sdk_name`, that Flutter reports once the
            // simulator is up. A shut-down simulator therefore describes itself
            // exactly as a running one does.
            targets.push(target(
                udid,
                name,
                Platform::Ios,
                &sdk_name(runtime),
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

/// What a finished boot knows about the device it started.
///
/// More than the id, because the id is not enough to describe the device on the
/// SelectedTargetCard and this is the only moment the facts are cheap. Discovery
/// has already run and will not run again — 3.3 is explicit that the device is
/// not to be looked up a second time — so anything not gathered here is a dash
/// on the card for the rest of the session.
///
/// Both fields may be empty. That is not a failure: a booted simulator has
/// nothing to add to what `simctl` said before it started, so it returns the id
/// alone and the picked row supplies the rest.
pub struct Booted {
    /// The id Flutter will address the device by, which for Android is the
    /// serial and not the AVD name it was started from.
    pub id: String,
    pub target_platform: String,
    pub sdk: String,
}

impl Booted {
    /// A boot that learned nothing beyond the id.
    ///
    /// Also the answer for a device that was already up, which is how a retry says
    /// "nothing to start here" through the same channel as a real boot.
    pub fn bare(id: String) -> Self {
        Self {
            id,
            target_platform: String::new(),
            sdk: String::new(),
        }
    }
}

/// Boot a target and return what Flutter will need to address it.
///
/// Blocking, and meant to be called on a worker thread.
pub fn boot(target: &Boot) -> Result<Booted, String> {
    match target {
        Boot::Sim(udid) => boot_sim(udid),
        Boot::Avd(name) => boot_avd(name),
    }
}

fn boot_sim(udid: &str) -> Result<Booted, String> {
    // Bring the window up first, or the device boots headless and there is
    // nothing to look at.
    let _ = run("open", &["-a", "Simulator"], QUICK);

    // `bootstatus -b` boots if needed and blocks until the device is ready, so
    // there is no polling loop to write.
    run("xcrun", &["simctl", "bootstatus", udid, "-b"], BOOT_LIMIT)
        .map(|_| Booted::bare(udid.to_string()))
        .ok_or_else(|| "did not finish booting".to_string())
}

fn boot_avd(name: &str) -> Result<Booted, String> {
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
    //
    // Snapshot first: the emulator is identified by the serial that appears, so we
    // have to know which ones were already there. Taking it after the spawn would
    // race the emulator into the list.
    let before = emulator_serials();

    Command::new("nohup")
        .args(["emulator", "-avd", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start the emulator: {e}"))?;

    let deadline = Instant::now() + BOOT_LIMIT;

    // The serial comes first, and everything after it is addressed with `-s`.
    //
    // This is not a refinement, it is the fix for a real failure. `adb shell` with
    // no `-s` picks a device only when exactly one is attached: with a phone on
    // wireless adb and the emulator not yet registered, `getprop
    // sys.boot_completed` was answered *by the phone*, instantly and with `1`. The
    // boot was declared finished after about a second, the serial lookup then found
    // nothing, and the run died with `booted, but adb never reported a serial`
    // while the emulator carried on booting in plain sight.
    //
    // Identifying by *which serial appeared* also retires the old name lookup, which
    // asked every attached device `emu avd name` and compared the answer. A serial
    // that was not there before we spawned an emulator is the emulator we spawned;
    // no name has to match for that to be true.
    let serial = loop {
        if Instant::now() >= deadline {
            return Err("never appeared in adb".to_string());
        }

        if let Some(serial) = emulator_serials().difference(&before).next() {
            break serial.clone();
        }

        std::thread::sleep(Duration::from_secs(1));
    };

    // `sys.boot_completed`, not adb presence: adb answers well before Android will
    // accept an APK.
    while Instant::now() < deadline {
        let ready = run(
            "adb",
            &["-s", &serial, "shell", "getprop", "sys.boot_completed"],
            QUICK,
        )
        .map(|out| out.trim().trim_end_matches('\r') == "1")
        .unwrap_or(false);

        if ready {
            let (target_platform, sdk) = android_facts(&serial);

            return Ok(Booted {
                id: serial,
                target_platform,
                sdk,
            });
        }

        std::thread::sleep(Duration::from_secs(1));
    }

    Err("did not finish booting".to_string())
}

/// Shut a virtual device down again.
///
/// Called when a run moves to another device (8.5) and only for the device frun
/// booted itself. That restriction is the whole safety argument: a simulator that
/// was already up when frun started belongs to whatever the user was doing with
/// it, and shutting it down would close a window they were using. A physical
/// device is never touched — there is nothing here that could.
///
/// Blocking, so it belongs on a worker thread. Nothing is reported back: a device
/// that refuses to stop costs memory and nothing else, and the run that matters is
/// already starting on another one.
pub fn shutdown(id: &str, platform: Platform) {
    match platform {
        // `simctl shutdown` leaves the Simulator app open with no device booted,
        // which is what quitting a simulator from its own menu does.
        Platform::Ios => {
            let _ = run("xcrun", &["simctl", "shutdown", id], QUICK);
        }

        // `emu kill` is the emulator's own console command and stops the process.
        // `adb -s <serial> shell reboot -p` would power the guest Android down and
        // leave the emulator process running, which is a device that still answers
        // `adb devices` and can no longer be used.
        Platform::Android => {
            let _ = run("adb", &["-s", id, "emu", "kill"], QUICK);
        }

        // macOS and Chrome are the host. There is no device to stop, and the
        // nearest equivalent would be closing the user's browser.
        Platform::Desktop | Platform::Web => {}
    }
}

/// `targetPlatform` and `sdk` for a booted emulator, asked of the emulator.
///
/// This is the fix for the dash. A device that frun booted itself never passed
/// through `flutter devices`, so both fields were empty strings and the card
/// showed `emulator-5554` with no platform and `-` for its version — for a device
/// that was, by then, running and answering questions.
///
/// Asked of Android rather than of Flutter on purpose. `flutter devices` would
/// answer both, and costs several seconds because it boots the Dart VM; these are
/// three `getprop` calls over an adb connection that boot detection just finished
/// using, and they return immediately.
///
/// Both values are built in Flutter's own spelling, so a device that arrives this
/// way is indistinguishable on the card from one that was already attached. That
/// is the point: two spellings of `android-arm64` would read as two different
/// facts.
///
/// Either half may come back empty, and an empty half must not overwrite what is
/// already known — see `booted_device` in main.rs.
fn android_facts(serial: &str) -> (String, String) {
    let target_platform = getprop(serial, "ro.product.cpu.abi")
        .map(|abi| android_target_platform(&abi))
        .unwrap_or_default();

    // `AndroidDevice.sdkNameAndVersion` is `'Android $release (API $sdk)'`, from
    // exactly these two properties, so this reproduces the string rather than
    // inventing a format for it.
    let sdk = match (
        getprop(serial, "ro.build.version.release"),
        getprop(serial, "ro.build.version.sdk"),
    ) {
        (Some(release), Some(api)) => format!("Android {release} (API {api})"),
        (Some(release), None) => format!("Android {release}"),
        _ => String::new(),
    };

    (target_platform, sdk)
}

fn getprop(serial: &str, property: &str) -> Option<String> {
    let out = run("adb", &["-s", serial, "shell", "getprop", property], QUICK)?;

    let value = out.trim().to_string();

    (!value.is_empty()).then_some(value)
}

/// An Android ABI as Flutter names the same thing.
///
/// The four are the whole of Flutter's Android target list
/// (`build_info.dart`), and the mapping is the one `AndroidDevice.targetPlatform`
/// uses. An ABI outside them is passed through as it came: a wrong `android-*`
/// triple would be read as fact, where an unfamiliar ABI reads as what it is.
fn android_target_platform(abi: &str) -> String {
    match abi {
        "arm64-v8a" => "android-arm64",
        "armeabi-v7a" => "android-arm",
        "x86_64" => "android-x64",
        "x86" => "android-x86",
        other => other,
    }
    .to_string()
}

/// Serials of every attached emulator, whatever state adb reports them in.
///
/// `offline` counts: an emulator appears that way for a few seconds after launch,
/// and its identity is settled long before Android is ready. Waiting for `device`
/// here would just move the race.
fn emulator_serials() -> std::collections::HashSet<String> {
    let Some(out) = run("adb", &["devices"], QUICK) else {
        return std::collections::HashSet::new();
    };

    out.lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .filter(|serial| serial.starts_with("emulator-"))
        .map(str::to_string)
        .collect()
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

    /// 8.4: the row has to survive the trip to another process, or the tab it starts
    /// has to run a discovery to learn what the tab that spawned it already knew.
    ///
    /// The two fields that decide behaviour rather than presentation are the ones
    /// worth asserting: `platform`, without which a boot cannot be attempted, and
    /// `virtual_device`, which is frun's licence to shut the device down.
    #[test]
    fn a_handed_over_device_round_trips() {
        let avd = Device {
            id: "Pixel_8".into(),
            name: "Pixel 8".into(),
            platform: Platform::Android,
            target_platform: "android-arm64".into(),
            sdk: "Android 17 (API 37)".into(),
            virtual_device: true,
            last_used: true,
            boot: Some(Boot::Avd("Pixel_8".into())),
        };

        let back = Device::from_handoff(&avd.to_handoff()).expect("should parse");

        assert_eq!(back.id, "Pixel_8");
        assert_eq!(back.name, "Pixel 8");
        assert_eq!(back.platform, Platform::Android);
        assert!(back.virtual_device);
        assert_eq!(back.boot, Some(Boot::Avd("Pixel_8".into())));

        // Display-only, and deliberately absent: they arrive with the background scan
        // rather than being duplicated into an environment variable.
        assert_eq!(back.target_platform, "");
        assert_eq!(back.sdk, "");

        // A running device has no boot, and that is the difference that decides
        // whether the new tab builds now or waits three minutes first.
        let attached = Device {
            boot: None,
            platform: Platform::Ios,
            virtual_device: false,
            ..avd
        };

        let back = Device::from_handoff(&attached.to_handoff()).expect("should parse");

        assert_eq!(back.boot, None);
        assert_eq!(back.platform, Platform::Ios);
        assert!(!back.virtual_device);
    }

    /// Anything short of the full form must be refused, because every field it is
    /// missing would otherwise be a guess about a device frun is about to run or
    /// shut down.
    #[test]
    fn a_partial_handoff_is_refused() {
        assert!(Device::from_handoff("emulator-5554").is_none());
        assert!(Device::from_handoff("emulator-5554\tPixel 8").is_none());
        assert!(Device::from_handoff("").is_none());
        assert!(Device::from_handoff("\tPixel 8\tandroid\t1\t").is_none());
        assert!(Device::from_handoff("id\tPixel 8\tmartian\t1\t").is_none());
    }

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

    /// A simulator runtime identifier is not an OS version.
    ///
    /// What the card showed: `com.apple.CoreSimulator.SimRuntime.iOS-26-5` under
    /// `OS Version`, straight from Flutter, which passes the `simctl` JSON key
    /// through as `sdkNameAndVersion`.
    #[test]
    fn a_simulator_runtime_loses_its_bundle_prefix() {
        assert_eq!(
            sdk_name("com.apple.CoreSimulator.SimRuntime.iOS-26-5"),
            "iOS-26-5"
        );

        // Every other platform already answers the label, so nothing is touched.
        assert_eq!(sdk_name("Android 17 (API 37)"), "Android 17 (API 37)");
        assert_eq!(
            sdk_name("macOS 26.6.1 25G76 darwin-arm64"),
            "macOS 26.6.1 25G76 darwin-arm64"
        );
        assert_eq!(sdk_name("iOS 18.2 22C150"), "iOS 18.2 22C150");
        assert_eq!(sdk_name("-"), "-");
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

    /// A booted emulator has to describe itself the way Flutter would.
    ///
    /// The card puts `Platform ID` one row above the version, so `arm64-v8a` and
    /// `android-arm64` would sit where the other had been the run before and read
    /// as a different device.
    #[test]
    fn an_abi_is_translated_into_flutters_target_triple() {
        assert_eq!(android_target_platform("arm64-v8a"), "android-arm64");
        assert_eq!(android_target_platform("armeabi-v7a"), "android-arm");
        assert_eq!(android_target_platform("x86_64"), "android-x64");
        assert_eq!(android_target_platform("x86"), "android-x86");

        // Not mapped to a plausible-looking triple it might not be.
        assert_eq!(android_target_platform("riscv64"), "riscv64");
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
        assert!(macos
            .get("sdk")
            .and_then(Value::as_str)
            .unwrap()
            .contains("arm64"));
    }
}
