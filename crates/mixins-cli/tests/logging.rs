use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ddmix-logs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for path in [
            "ddnet/src/engine/client",
            "ddnet/cmake",
            "mods/test",
            "mods/off",
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        fs::write(root.join("ddnet/CMakeLists.txt"),"cmake_minimum_required(VERSION 3.16)\nproject(logtest LANGUAGES NONE)\nadd_library(game-client INTERFACE)\nconfigure_file(cmake/checksummed_extra.txt checksummed_extra.txt)\n").unwrap();
        fs::write(
            root.join("ddnet/src/engine/client/checksum.h"),
            "unsigned m_aFiles[1024];",
        )
        .unwrap();
        fs::write(root.join("ddnet/cmake/checksummed_extra.txt"), "extra\n").unwrap();
        fs::write(root.join("ddnet/src/test.cpp"), "void f() {}\n").unwrap();
        fs::write(root.join("mods/test/mixin.toml"),"[mod]\nid='test'\nname='Test'\nversion='1'\n[[injections]]\nid='head'\nfile='src/test.cpp'\ntarget='f'\nkind='HEAD'\ncontent='/* log probe */'\n").unwrap();
        fs::write(
            root.join("mods/off/mixin.toml"),
            "[mod]\nid='off'\nname='Disabled'\nversion='1'\nenabled=false\n",
        )
        .unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ddmix"))
            .args(args)
            .args(["--ddnet"])
            .arg(self.0.join("ddnet"))
            .arg("--mods")
            .arg(self.0.join("mods"))
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn normal_quiet_verbose_preserve_machine_stdout_and_report_skips() {
    let f = Fixture::new();
    let normal = f.run(&["plan", "--color=never"]);
    assert!(normal.status.success());
    let stderr = String::from_utf8_lossy(&normal.stderr);
    assert!(stderr.contains("8888888b."));
    assert!(stderr.contains("DDNet Mixins v"));
    assert!(stderr.contains("[4/4] cmake"));
    assert!(stderr.contains("1 injections"));
    assert!(stderr.contains("elapsed"));
    assert!(stderr.contains("mod off: disabled"));
    assert!(!stderr.contains('\x1b'));
    let json: serde_json::Value = serde_json::from_slice(&normal.stdout).unwrap();
    assert!(json["files"].is_object());
    let quiet = f.run(&["--quiet", "plan"]);
    assert!(quiet.status.success());
    assert_eq!(normal.stdout, quiet.stdout);
    let stderr = String::from_utf8_lossy(&quiet.stderr);
    assert!(!stderr.contains("8888888b."));
    assert!(!stderr.contains("finished"));
    assert!(stderr.contains("skipped"));
    let verbose = f.run(&["plan", "-v", "--color=never"]);
    assert!(verbose.status.success());
    assert_eq!(normal.stdout, verbose.stdout);
    let stderr = String::from_utf8_lossy(&verbose.stderr);
    assert!(stderr.contains("test/head at src/test.cpp:1:"));
    assert!(stderr.contains("Dependency order"));
    assert!(stderr.contains("timing"));
}
#[test]
fn explicit_color_and_no_color_auto_and_quiet_errors() {
    let f = Fixture::new();
    let color = f.run(&["plan", "--color=always"]);
    assert!(color.stderr.contains(&0x1b));
    let auto = Command::new(env!("CARGO_BIN_EXE_ddmix"))
        .env("NO_COLOR", "1")
        .args(["plan", "--ddnet"])
        .arg(f.0.join("ddnet"))
        .arg("--mods")
        .arg(f.0.join("mods"))
        .output()
        .unwrap();
    assert!(!auto.stderr.contains(&0x1b));
    let manifest = f.0.join("mods/test/mixin.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, text.replace("target='f'", "target='absent'")).unwrap();
    let failed = f.run(&["check", "-q", "--color=never"]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(stderr.contains("error"));
    assert!(stderr.contains("resolve"));
    assert!(stderr.contains("mod test"));
    assert!(stderr.contains("absent"));
    assert!(stderr.contains("failed"));
}
#[test]
fn inspect_diagnostics_and_diff_keep_stdout_usable() {
    let f = Fixture::new();
    let output = f.run(&["plan", "--diff", "--color=never"]);
    assert!(output.status.success());
    let diff = String::from_utf8_lossy(&output.stdout);
    assert!(diff.starts_with("--- a/"));
    assert!(!diff.contains("DDNet Mixins"));
    fs::write(f.0.join("ddnet/src/test.cpp"), "void f() {\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ddmix"))
        .args(["inspect", "--quiet", "--color=never", "--ddnet"])
        .arg(f.0.join("ddnet"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["diagnostic_count"].as_u64().unwrap() > 0);
    let log = String::from_utf8_lossy(&output.stderr);
    assert!(log.contains("test.cpp:1:"));
    assert!(log.contains("warning"));
    assert!(log.contains("failed"));
    assert!(!log.contains("8888888b."));
    let conflicting = f.run(&["plan", "--quiet", "--verbose"]);
    assert!(!conflicting.status.success());
    assert!(String::from_utf8_lossy(&conflicting.stderr).contains("cannot be used"));
}
#[test]
fn configure_failure_keeps_complete_transcript_and_reports_tail_in_quiet() {
    let f = Fixture::new();
    let cmake = f.0.join("ddnet/CMakeLists.txt");
    let text = fs::read_to_string(&cmake).unwrap();
    fs::write(
        &cmake,
        format!("{text}\nmessage(FATAL_ERROR \"LOGGING_EXPECTED_CONFIGURE_FAILURE\")\n"),
    )
    .unwrap();
    let build = f.0.join("build");
    let out = Command::new(env!("CARGO_BIN_EXE_ddmix"))
        .args(["-q", "build", "--workspace"])
        .arg(&f.0)
        .arg("--ddnet")
        .arg(f.0.join("ddnet"))
        .arg("--mods")
        .arg(f.0.join("mods"))
        .arg("--build-dir")
        .arg(&build)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("LOGGING_EXPECTED_CONFIGURE_FAILURE"), "{err}");
    assert!(err.contains("configure"));
    assert!(err.contains("ddmix-configure.log"));
    assert!(
        fs::read_to_string(build.join("ddmix-configure.log"))
            .unwrap()
            .contains("LOGGING_EXPECTED_CONFIGURE_FAILURE")
    );
}
