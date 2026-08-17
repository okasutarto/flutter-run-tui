use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after the epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "frun-toolchain-{name}-{}-{nonce}",
        std::process::id()
    ));

    fs::create_dir_all(&root).expect("fixture directory should be created");
    root
}

fn executable(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("executable should have a parent"))
        .expect("executable directory should be created");
    fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("fixture executable should be written");

    let mut permissions = fs::metadata(path)
        .expect("fixture executable should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("fixture executable should be executable");
}

fn probe(root: &Path, flutter: &str, path: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_frun-tui"))
        .arg("--probe")
        .current_dir(root)
        .env("FRUN_FLUTTER", flutter)
        .env("PATH", path)
        .output()
        .expect("frun --probe should start");

    assert!(
        output.status.success(),
        "probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("probe output should be UTF-8")
}

fn auto_probe(root: &Path, path: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_frun-tui"))
        .arg("--probe")
        .current_dir(root)
        .env_remove("FRUN_FLUTTER")
        .env("PATH", path)
        .output()
        .expect("frun --probe should start");

    assert!(
        output.status.success(),
        "probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("probe output should be UTF-8")
}

fn fake_flutter(path: &Path, framework: &str) {
    executable(
        path,
        &format!(
            r#"case "$1" in
  --version) printf '%s\n' '{{"frameworkVersion":"{framework}","dartSdkVersion":"3.9.9"}}' ;;
  devices) printf '%s\n' '[]' ;;
esac"#
        ),
    );
}

fn fake_fvm(path: &Path, framework: &str) {
    executable(
        path,
        &format!(
            r#"case " $* " in
  *" --version "*) printf '%s\n' '{{"frameworkVersion":"{framework}","dartSdkVersion":"3.9.9"}}' ;;
  *" devices "*) printf '%s\n' '[]' ;;
esac"#
        ),
    );
}

#[test]
fn an_explicit_flutter_path_may_contain_spaces() {
    let root = fixture("path-with-spaces");
    let flutter = root.join("Flutter SDK/bin/flutter");

    executable(
        &flutter,
        r#"case "$1" in
  --version) printf '%s\n' '{"frameworkVersion":"9.9.9","dartSdkVersion":"3.9.9"}' ;;
  devices) printf '%s\n' '[]' ;;
esac"#,
    );

    let output = probe(
        &root,
        flutter.to_str().expect("fixture path should be UTF-8"),
        "/usr/bin:/bin",
    );

    assert!(output.contains("runtime   SDK"), "{output}");
    assert!(
        output.contains("sdk       flutter 9.9.9  dart 3.9.9"),
        "{output}"
    );
}

#[test]
fn a_wrapper_reports_the_sdk_version_it_actually_runs() {
    let root = fixture("wrapper-version");
    let wrapper = root.join("bin/mise");
    let global_sdk = root.join("global-sdk");
    let global_flutter = global_sdk.join("bin/flutter");
    let manifest = global_sdk.join("bin/cache/flutter.version.json");

    executable(
        &wrapper,
        r#"[ "$1" = exec ] && [ "$2" = flutter ] && [ "$3" = -- ] || exit 64
shift 3
case "$1" in
  --version) printf '%s\n' '{"frameworkVersion":"9.9.9","dartSdkVersion":"3.9.9"}' ;;
  devices) printf '%s\n' '[]' ;;
esac"#,
    );
    executable(&global_flutter, "exit 0");
    fs::create_dir_all(manifest.parent().expect("manifest should have a parent"))
        .expect("manifest directory should be created");
    fs::write(
        &manifest,
        r#"{"frameworkVersion":"1.0.0","dartSdkVersion":"2.0.0"}"#,
    )
    .expect("global SDK manifest should be written");

    let path = format!(
        "{}:{}/bin:/usr/bin:/bin",
        wrapper
            .parent()
            .expect("wrapper should have a parent")
            .display(),
        global_sdk.display()
    );
    let command = format!("{} exec flutter --", wrapper.display());
    let output = probe(&root, &command, &path);

    assert!(output.contains("runtime   mise"), "{output}");
    assert!(
        output.contains("sdk       flutter 9.9.9  dart 3.9.9"),
        "{output}"
    );
    assert!(!output.contains("flutter 1.0.0"), "{output}");
}

#[test]
fn a_pinned_project_prefers_fvm_when_both_commands_exist() {
    let root = fixture("pinned-prefers-fvm");
    let bin = root.join("bin");

    fs::write(root.join(".fvmrc"), r#"{"flutter":"9.9.9"}"#).expect("FVM pin should be written");
    fake_fvm(&bin.join("fvm"), "9.9.9");
    fake_flutter(&bin.join("flutter"), "1.0.0");

    let path = format!("{}:/usr/bin:/bin", bin.display());
    let output = auto_probe(&root, &path);

    assert!(
        output.contains("runtime   FVM  fvm flutter run"),
        "{output}"
    );
    assert!(
        output.contains("sdk       flutter 9.9.9  dart 3.9.9"),
        "{output}"
    );
}

#[test]
fn an_unpinned_project_prefers_plain_flutter() {
    let root = fixture("unpinned-prefers-sdk");
    let bin = root.join("bin");

    fake_fvm(&bin.join("fvm"), "9.9.9");
    fake_flutter(&bin.join("flutter"), "1.0.0");

    let path = format!("{}:/usr/bin:/bin", bin.display());
    let output = auto_probe(&root, &path);

    assert!(output.contains("runtime   SDK  flutter run"), "{output}");
    assert!(
        output.contains("sdk       flutter 1.0.0  dart 3.9.9"),
        "{output}"
    );
}

#[test]
fn a_pinned_project_falls_back_when_fvm_is_unavailable() {
    let root = fixture("pinned-fallback");
    let bin = root.join("bin");

    fs::write(root.join(".fvmrc"), r#"{"flutter":"9.9.9"}"#).expect("FVM pin should be written");
    fake_flutter(&bin.join("flutter"), "1.0.0");

    let path = format!("{}:/usr/bin:/bin", bin.display());
    let output = auto_probe(&root, &path);

    assert!(output.contains("runtime   SDK  flutter run"), "{output}");
    assert!(
        output.contains("sdk       flutter 1.0.0  dart 3.9.9"),
        "{output}"
    );
}

#[test]
fn fvm_is_used_when_no_plain_flutter_exists() {
    let root = fixture("fvm-only");
    let bin = root.join("bin");

    fake_fvm(&bin.join("fvm"), "9.9.9");

    let path = format!("{}:/usr/bin:/bin", bin.display());
    let output = auto_probe(&root, &path);

    assert!(
        output.contains("runtime   FVM  fvm flutter run"),
        "{output}"
    );
    assert!(
        output.contains("sdk       flutter 9.9.9  dart 3.9.9"),
        "{output}"
    );
}
