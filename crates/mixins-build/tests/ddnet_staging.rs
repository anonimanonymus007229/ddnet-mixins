//! Real-DDNet staging is isolated in an ephemeral external workspace.
#[test]
#[ignore = "requires supplied real DDNet checkout"]
fn real_apply_twice_is_identical_and_upstream_unchanged() {
    use std::{fs, path::PathBuf};
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ddnet = std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("workspace/ddnet"));
    let workspace = std::env::temp_dir().join(format!("ddmix-real-stage-{}", std::process::id()));
    fs::create_dir(&workspace).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(workspace.clone());
    let paths = [
        "CMakeLists.txt",
        "src/game/client/gameclient.cpp",
        "src/engine/shared/config_variables.h",
    ];
    let originals: Vec<_> = paths
        .iter()
        .map(|p| fs::read(ddnet.join(p)).unwrap())
        .collect();
    let staged = mixins_build::apply(&workspace, &ddnet, &repo.join("examples")).unwrap();
    assert!(!staged.join(".git").exists());
    let state = fs::read(workspace.join(".ddmix/state.json")).unwrap();
    let transformed = fs::read_to_string(staged.join("src/game/client/gameclient.cpp")).unwrap();
    assert_eq!(transformed.matches("// Example HEAD injection.").count(), 1);
    let cmake = fs::read_to_string(staged.join("CMakeLists.txt")).unwrap();
    assert_eq!(
        cmake
            .matches("# DDMIX-BEGIN: generated integration")
            .count(),
        1
    );
    assert!(
        !staged
            .join("src/game/client/components/ddmix_probe.cpp")
            .exists()
    );
    let generated = fs::read_to_string(workspace.join(".ddmix/generated.cmake")).unwrap();
    assert!(generated.contains("target_sources(game-client PRIVATE"));
    assert!(generated.contains("ddmix_probe.cpp"));
    assert!(generated.contains("CHECKSUM_SRC"));
    mixins_build::apply(&workspace, &ddnet, &repo.join("examples")).unwrap();
    assert_eq!(
        fs::read(workspace.join(".ddmix/state.json")).unwrap(),
        state
    );
    for (path, original) in paths.iter().zip(originals) {
        assert_eq!(fs::read(ddnet.join(path)).unwrap(), original);
    }
}
