use mixins_transform::{Action, Edit, Plan, Position, Target};
fn insert(id: &str, content: &str) -> Edit {
    Edit {
        id: id.into(),
        file: "src/test.cpp".into(),
        action: Action::Insert,
        target: Target::Function {
            name: "f".into(),
            parameters: None,
        },
        position: Some(Position::BodyStart),
        content: Some(content.into()),
        priority: 0,
    }
}
#[test]
fn deterministic_same_offset_inserts_and_conflicts() {
    let source = "void f() {}";
    let a = insert("a", "A");
    let b = insert("b", "B");
    let plan = Plan::resolve(source, &[("first", &a), ("second", &b)]).unwrap();
    assert_eq!(plan.render(source).unwrap(), "void f() {AB}");
    let mut replace = a.clone();
    replace.action = Action::Replace;
    replace.position = None;
    assert!(
        Plan::resolve(source, &[("a", &a), ("b", &replace)])
            .unwrap_err()
            .contains("overlapping")
    );
}
#[test]
fn comment_replace_remove_and_missing_targets() {
    let source = "/* anchor */\r\nvoid f() {}";
    let mut edit = insert("a", "new");
    edit.target = Target::Comment {
        text: "anchor".into(),
    };
    edit.action = Action::Replace;
    edit.position = None;
    assert_eq!(
        Plan::resolve(source, &[("a", &edit)])
            .unwrap()
            .render(source)
            .unwrap(),
        "new\r\nvoid f() {}"
    );
    edit.action = Action::Remove;
    edit.content = None;
    assert_eq!(
        Plan::resolve(source, &[("a", &edit)])
            .unwrap()
            .render(source)
            .unwrap(),
        "\r\nvoid f() {}"
    );
    edit.target = Target::Comment {
        text: "missing".into(),
    };
    assert!(Plan::resolve(source, &[("a", &edit)]).is_err());
}
#[test]
#[ignore = "requires supplied real DDNet"]
fn real_ddnet_head_tail_and_config_comment() {
    let root = std::env::var_os("DDMIX_TEST_DDNET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace/ddnet")
        });
    let source = std::fs::read_to_string(root.join("src/game/client/gameclient.cpp")).unwrap();
    let mut head = insert("head", "\n// injected HEAD\n");
    head.target = Target::Function {
        name: "CGameClient::OnInit".into(),
        parameters: Some("()".into()),
    };
    let mut tail = head.clone();
    tail.id = "tail".into();
    tail.position = Some(Position::BodyEnd);
    tail.content = Some("\n// injected TAIL\n".into());
    let plan = Plan::resolve(&source, &[("a", &head), ("b", &tail)]).unwrap();
    let transformed = plan.render(&source).unwrap();
    assert_eq!(
        transformed
            .replace("\n// injected HEAD\n", "")
            .replace("\n// injected TAIL\n", ""),
        source
    );
    assert_eq!(transformed.matches("// injected HEAD").count(), 1);
    let source =
        std::fs::read_to_string(root.join("src/engine/shared/config_variables.h")).unwrap();
    let mut config = insert(
        "config",
        "\nMACRO_CONFIG_INT(ClDdmixTest, cl_ddmix_test, 0, 0, 1, CFGFLAG_CLIENT, \"Test\")\n",
    );
    config.target = Target::Comment {
        text: "Add config variables for mods below this comment".into(),
    };
    config.position = Some(Position::After);
    let p = Plan::resolve(&source, &[("test", &config)]).unwrap();
    let output = p.render(&source).unwrap();
    assert_eq!(output.replace(config.content.as_ref().unwrap(), ""), source);
}
