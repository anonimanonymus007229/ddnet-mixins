#[test]
#[ignore = "requires supplied real DDNet checkout"]
fn real_snapinput_hook_is_inlined_and_not_compiled_separately() {
    use std::{fs, path::PathBuf};
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ddnet = std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("workspace/ddnet"));
    let relative = "src/game/client/components/controls.cpp";
    let before = fs::read_to_string(ddnet.join(relative)).unwrap();
    let snippet =
        fs::read_to_string(repo.join("examples/controls-hook/hooks/ccontrols_snapinput.cpp"))
            .unwrap();
    let plan = mixins_build::plan(&ddnet, &repo.join("examples")).unwrap();
    let patches = &plan.files[relative].patches;
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].mod_id, "controls-hook");
    assert_eq!(patches[0].content, snippet);
    let offset = patches[0].span.start;
    assert_eq!(
        plan.rendered[relative],
        format!("{}{}{}", &before[..offset], snippet, &before[offset..])
    );
    assert!(!plan.generated_cmake.contains("ccontrols_snapinput.cpp"));
    let diff = plan.diff(&ddnet).unwrap();
    assert!(diff.contains("+++ b/src/game/client/components/controls.cpp"));
    assert!(diff.contains("+\t(void)this;"));
    assert_eq!(fs::read_to_string(ddnet.join(relative)).unwrap(), before);
}
