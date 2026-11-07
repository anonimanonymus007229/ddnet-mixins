#[test]
#[ignore = "requires supplied real DDNet checkout"]
fn real_snapinput_helper_has_explicit_bindings_and_no_separate_compilation() {
    use std::{fs, path::PathBuf};
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ddnet = std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("workspace/ddnet"));
    let original =
        fs::read_to_string(ddnet.join("src/game/client/components/controls.cpp")).unwrap();
    let plan = mixins_build::plan(&ddnet, &repo.join("examples")).unwrap();
    let output = &plan.rendered["src/game/client/components/controls.cpp"];
    let pos = output.find("static void ddmix_helper_").unwrap();
    let method = output.find("int CControls::SnapInput(").unwrap();
    assert!(pos < method);
    assert!(output.contains("CControls * controls, int * data, int & player_flags"));
    assert!(
        output.contains("((this), (pData), (m_aInputData[g_Config.m_ClDummy].m_PlayerFlags));")
    );
    assert_eq!(output.matches("static void ddmix_helper_").count(), 1);
    assert!(!plan.generated_cmake.contains("controls-helper/hooks"));
    assert_eq!(
        fs::read_to_string(ddnet.join("src/game/client/components/controls.cpp")).unwrap(),
        original
    );
}
