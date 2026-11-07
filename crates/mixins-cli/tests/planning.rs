use std::{fs, path::PathBuf, process::Command};
#[test]
fn plan_and_check_are_stable_read_only_and_reject_path_escape() {
    let root = std::env::temp_dir().join(format!("ddmix-cli-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(root.join("ddnet/src")).unwrap();
    fs::create_dir_all(root.join("mods/test")).unwrap();
    fs::write(root.join("ddnet/CMakeLists.txt"), "# read-only fixture").unwrap();
    let source = "void f() {\r\n}\r\n";
    fs::write(root.join("ddnet/src/test.cpp"), source).unwrap();
    let manifest = "[mod]\nid='test'\nname='Test'\nversion='1'\n[[injections]]\nid='head'\nfile='src/test.cpp'\ntarget='f'\nkind='HEAD'\ncontent='example'\n";
    fs::write(root.join("mods/test/mixin.toml"), manifest).unwrap();
    let run = |cmd| {
        Command::new(env!("CARGO_BIN_EXE_ddmix"))
            .arg(cmd)
            .arg("--ddnet")
            .arg(root.join("ddnet"))
            .arg("--mods")
            .arg(root.join("mods"))
            .output()
            .unwrap()
    };
    let plan = run("plan");
    assert!(plan.status.success(), "{:?}", plan);
    let check = run("check");
    assert!(check.status.success());
    assert_eq!(plan.stdout, check.stdout);
    assert_eq!(
        fs::read_to_string(root.join("ddnet/src/test.cpp")).unwrap(),
        source
    );
    assert!(!root.join(".ddmix").exists());
    fs::write(
        root.join("mods/test/mixin.toml"),
        manifest.replace("src/test.cpp", "src/../../outside.cpp"),
    )
    .unwrap();
    let failed = run("plan");
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("normalized"));
}
