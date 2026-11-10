#[test]
#[ignore = "requires supplied DDNet/client checkouts"]
fn generated_constructor_and_platform_entry_body_edits_preserve_original_source() {
    use mixins_transform::{Action, Edit, Plan, Position, Target};
    use std::{fs, path::PathBuf};
    let roots = if let Some(p) = std::env::var_os("DDMIX_TEST_DDNET") {
        vec![PathBuf::from(p)]
    } else {
        let w = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace");
        ["BestClient", "TClient", "RushieClient-ddnet"]
            .iter()
            .map(|n| w.join(n))
            .filter(|p| p.exists())
            .collect()
    };
    assert!(!roots.is_empty());
    for root in roots {
        for file in ["src/game/editor/editor.h", "src/base/thread.cpp"] {
            let s = fs::read_to_string(root.join(file)).unwrap();
            let i = mixins_parser::parse(&s, &mixins_lexer::lex(&s));
            let target = if file.ends_with("editor.h") {
                Target::ConstructorBody {
                    name: "CEditor::CEditor".into(),
                    parameters: Some("()".into()),
                }
            } else {
                let node = i
                    .conditional_functions
                    .first()
                    .expect("thread platform alternative missing");
                Target::ConditionalBody {
                    names: node
                        .alternatives
                        .iter()
                        .map(|f| f.qualified_name.clone())
                        .collect(),
                }
            };
            let e = Edit {
                id: "body".into(),
                file: file.into(),
                action: Action::Insert,
                target,
                position: Some(Position::BodyStart),
                content: Some("/* region hook */".into()),
                priority: 0,
            };
            let rendered = Plan::resolve(&s, &[("test", &e)])
                .unwrap()
                .render(&s)
                .unwrap();
            assert_eq!(rendered.replace("/* region hook */", ""), s);
        }
    }
}
