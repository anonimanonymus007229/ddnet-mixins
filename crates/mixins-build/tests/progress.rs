use mixins_core::progress::Event;
#[test]
#[ignore = "requires supplied real DDNet checkout"]
fn observed_plan_reports_real_mods_edits_components_and_coverage_without_changing_payload() {
    use std::path::PathBuf;
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ddnet = std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("workspace/ddnet"));
    let mut events = vec![];
    let observed =
        mixins_build::plan_observed(&ddnet, &repo.join("examples"), &mut |e| events.push(e))
            .unwrap();
    let plain = mixins_build::plan(&ddnet, &repo.join("examples")).unwrap();
    assert_eq!(
        serde_json::to_vec(&observed).unwrap(),
        serde_json::to_vec(&plain).unwrap()
    );
    let stages: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Stage { name, .. } = e {
                Some(*name)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(stages, ["discovery", "lowering", "resolve", "cmake"]);
    let expected = observed
        .files
        .values()
        .map(|p| p.patches.len())
        .sum::<usize>();
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Count{name:"patches",value}if *value==expected))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Info(s)if s.contains("register CDdmixExample")))
    );
    assert!(events.iter().any(|e|matches!(e,Event::Detail(s)if s.contains("controls-helper/snapinput-helper at src/game/client/components/controls.cpp:"))));
    let expected_warnings = observed
        .files
        .keys()
        .filter(|file| {
            let source = std::fs::read_to_string(ddnet.join(file)).unwrap();
            !mixins_parser::parse(&source, &mixins_lexer::lex(&source))
                .unsupported
                .is_empty()
        })
        .count();
    let actual_warnings = events
        .iter()
        .filter(|e| matches!(e,Event::Warning(s)if s.contains("unsupported syntax regions")))
        .count();
    assert_eq!(actual_warnings, expected_warnings);
}
