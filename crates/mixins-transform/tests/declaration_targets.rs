//! Real regressions: newly indexed declaration patterns must also be editable.
#[test]
#[ignore = "requires real DDNet/client checkout(s)"]
fn head_and_tail_at_recovered_method_boundaries_preserve_all_other_bytes() {
    use mixins_transform::{Action, Edit, Plan, Position, Target};
    use std::{fs, path::PathBuf};
    let roots = if let Some(p) = std::env::var_os("DDMIX_TEST_DDNET") {
        vec![PathBuf::from(p)]
    } else {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace");
        ["BestClient", "TClient", "RushieClient-ddnet"]
            .iter()
            .map(|n| workspace.join(n))
            .filter(|p| p.join("src").is_dir())
            .collect()
    };
    assert!(!roots.is_empty());
    for root in roots {
        for (file, name) in [
            ("src/game/client/gameclient.h", "CGameClient::Graphics"),
            (
                "src/game/client/gameclient.cpp",
                "CGameClient::GetScoreComparator",
            ),
        ] {
            let source = fs::read_to_string(root.join(file)).unwrap();
            let head = Edit {
                id: "head".into(),
                file: file.into(),
                action: Action::Insert,
                target: Target::Function {
                    name: name.into(),
                    parameters: None,
                },
                position: Some(Position::BodyStart),
                content: Some("\n/* recovered method head */\n".into()),
                priority: 0,
            };
            let mut tail = head.clone();
            tail.id = "tail".into();
            tail.position = Some(Position::BodyEnd);
            tail.content = Some("\n/* recovered method tail */\n".into());
            let plan = Plan::resolve(&source, &[("test", &head), ("test", &tail)]).unwrap();
            let rendered = plan.render(&source).unwrap();
            assert_eq!(
                rendered
                    .replace(head.content.as_ref().unwrap(), "")
                    .replace(tail.content.as_ref().unwrap(), ""),
                source
            );
        }
    }
}
