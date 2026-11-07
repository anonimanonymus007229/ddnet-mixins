#[test]
#[ignore = "requires real DDNet checkout"]
fn real_components_diff_config_duplicates_and_determinism() {
    use std::{fs, path::PathBuf};
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ddnet = std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("workspace/ddnet"));
    let plan = mixins_build::plan(&ddnet, &repo.join("examples")).unwrap();
    let h = &plan.rendered["src/game/client/gameclient.h"];
    assert!(h.contains("CDdmixExample m_DdmixExample;"));
    let cpp = &plan.rendered["src/game/client/gameclient.cpp"];
    assert_eq!(cpp.matches("&m_DdmixExample").count(), 2);
    let diff = plan.diff(&ddnet).unwrap();
    assert!(diff.contains("+++ b/src/game/client/gameclient.h"));
    assert!(diff.contains("+\tCDdmixExample m_DdmixExample;"));
    let tmp = std::env::temp_dir().join(format!("ddmix-config-test-{}", std::process::id()));
    fs::create_dir_all(tmp.join("bad")).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(tmp.clone());
    let original = fs::read_to_string(repo.join("examples/basic-hooks/mixin.toml")).unwrap();
    fs::write(
        tmp.join("bad/mixin.toml"),
        original
            .replace("ClDdmixExample", "ClTouchControls")
            .replace("cl_ddmix_example", "cl_touch_controls"),
    )
    .unwrap();
    let error = match mixins_build::plan(&ddnet, &tmp) {
        Err(e) => e,
        Ok(_) => panic!("duplicate config accepted"),
    };
    assert!(error.contains("duplicate config"));
}
