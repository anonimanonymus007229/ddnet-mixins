use std::{fs, path::PathBuf, process::Command};
#[test]
fn successful_build_logs_every_stage_and_keeps_stdout_empty() {
    let root = std::env::temp_dir().join(format!("ddmix-build-log-{}", std::process::id()));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fs::create_dir_all(root.join("ddnet/src/engine/client")).unwrap();
    let _cleanup = Cleanup(root.clone());
    fs::create_dir(root.join("ddnet/cmake")).unwrap();
    fs::create_dir_all(root.join("mods/demo")).unwrap();
    fs::write(root.join("ddnet/CMakeLists.txt"),"cmake_minimum_required(VERSION 3.16)\nproject(build_log LANGUAGES CXX)\nadd_library(game-client STATIC src/test.cpp)\nconfigure_file(cmake/checksummed_extra.txt checksummed_extra.txt)\nmessage(WARNING \"EXPECTED_BUILD_WARNING\")\n").unwrap();
    fs::write(root.join("ddnet/src/test.cpp"), "void f() {}\n").unwrap();
    fs::write(
        root.join("ddnet/src/engine/client/checksum.h"),
        "unsigned m_aFiles[1024];",
    )
    .unwrap();
    fs::write(root.join("ddnet/cmake/checksummed_extra.txt"), "extra\n").unwrap();
    fs::write(root.join("mods/demo/mixin.toml"),"[mod]\nid='demo'\nname='Demo'\nversion='1'\n[[injections]]\nid='head'\nfile='src/test.cpp'\ntarget='f'\nkind='HEAD'\ncontent='/* build log */'\n").unwrap();
    let run = |quiet: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ddmix"));
        if quiet {
            cmd.arg("-q");
        }
        cmd.args(["build", "--color=never", "--workspace"])
            .arg(&root)
            .arg("--ddnet")
            .arg(root.join("ddnet"))
            .arg("--mods")
            .arg(root.join("mods"))
            .arg("--build-dir")
            .arg(root.join("build"))
            .output()
            .unwrap()
    };
    let output = run(false);
    let log = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{log}");
    assert!(output.stdout.is_empty());
    assert!(log.contains("[1/9] prepare"));
    assert!(log.contains("[9/9] compile"));
    assert!(log.contains("game-client compiled successfully"));
    let summary = log
        .lines()
        .find(|l| l.contains("finished"))
        .expect("summary missing");
    assert!(!summary.contains("; 0 warnings"), "{log}");
    assert!(log.contains("EXPECTED_BUILD_WARNING"));
    assert!(log.contains("elapsed"));
    assert!(
        fs::read_to_string(root.join("build/ddmix-configure.log"))
            .unwrap()
            .contains("EXPECTED_BUILD_WARNING")
    );
    assert!(root.join("build/ddmix-build.log").is_file());
    let quiet = run(true);
    let log = String::from_utf8_lossy(&quiet.stderr);
    assert!(quiet.status.success(), "{log}");
    assert!(quiet.stdout.is_empty());
    assert!(log.contains("EXPECTED_BUILD_WARNING"));
    assert!(!log.contains("finished"));
    assert!(!log.contains("8888888b."));
}
