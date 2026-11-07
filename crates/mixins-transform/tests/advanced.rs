use mixins_transform::{Action, Edit, Plan, Target};
fn replace(target: Target, content: &str) -> Edit {
    Edit {
        id: "test".into(),
        file: "src/test.cpp".into(),
        action: Action::Replace,
        target,
        position: None,
        content: Some(content.into()),
        priority: 0,
    }
}
#[test]
fn standalone_calls_nested_arguments_and_return_replacement() {
    let source = "int f() { send(1, nested(2, 3)); return 4; }";
    let call = Target::Call {
        function: "f".into(),
        parameters: None,
        callee: "send".into(),
        argument: Some(1),
    };
    let edit = replace(call, "nested(8, 9)");
    let rendered = Plan::resolve(source, &[("a", &edit)])
        .unwrap()
        .render(source)
        .unwrap();
    assert_eq!(rendered, "int f() { send(1, nested(8, 9)); return 4; }");
    let ret = replace(
        Target::ReturnExpression {
            function: "f".into(),
            parameters: None,
        },
        "42",
    );
    assert!(
        Plan::resolve(source, &[("a", &ret)])
            .unwrap()
            .render(source)
            .unwrap()
            .ends_with("return 42; }")
    );
}
#[test]
fn nested_calls_control_flow_and_templates_are_not_guessed() {
    for source in [
        "void f() { if (a) send(1); }",
        "void f() { auto x = send(1); }",
        "void f() { if (a) { send(1); } }",
        "void f() { send(pair<int, int>()); }",
    ] {
        let edit = replace(
            Target::Call {
                function: "f".into(),
                parameters: None,
                callee: "send".into(),
                argument: None,
            },
            "",
        );
        assert!(Plan::resolve(source, &[("a", &edit)]).is_err(), "{source}");
    }
    let edit = replace(
        Target::ReturnExpression {
            function: "f".into(),
            parameters: None,
        },
        "0",
    );
    for source in [
        "int f() { if (a) return 1; return 2; }",
        "int f() { if (a) { return 1; } }",
        "int f() { if (a) return 1; }",
    ] {
        assert!(Plan::resolve(source, &[("a", &edit)]).is_err());
    }
}
#[test]
fn boundary_inserts_can_surround_a_replaced_statement() {
    use mixins_transform::Position;
    let source = "void f() { send(1); }";
    let target = Target::Call {
        function: "f".into(),
        parameters: None,
        callee: "send".into(),
        argument: None,
    };
    let mut before = replace(target.clone(), "before(); ");
    before.action = Action::Insert;
    before.position = Some(Position::Before);
    let middle = replace(target.clone(), "send(2);");
    let mut after = replace(target, " after();");
    after.action = Action::Insert;
    after.position = Some(Position::After);
    let plan = Plan::resolve(source, &[("b", &middle), ("a", &before), ("c", &after)]).unwrap();
    assert_eq!(
        plan.render(source).unwrap(),
        "void f() { before(); send(2); after(); }"
    );
}
#[test]
fn wrap_is_explicit_original_block_and_rejects_early_returns() {
    let edit = replace(
        Target::Body {
            name: "f".into(),
            parameters: None,
        },
        "before(); {{original}} after();",
    );
    assert_eq!(
        Plan::resolve("void f() { work(); }", &[("a", &edit)])
            .unwrap()
            .render("void f() { work(); }")
            .unwrap(),
        "void f() {before(); { work(); } after();}"
    );
    assert!(Plan::resolve("void f() { return; }", &[("a", &edit)]).is_err());
}
#[test]
#[ignore = "requires real DDNet checkout"]
fn real_call_argument_return_and_overwrite() {
    let root = std::env::var_os("DDMIX_TEST_DDNET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace/ddnet")
        });
    let source = std::fs::read_to_string(root.join("src/game/client/gameclient.cpp")).unwrap();
    let edit = replace(
        Target::ReturnExpression {
            function: "CGameClient::DDNetVersion".into(),
            parameters: Some("()".into()),
        },
        "DDNET_VERSION_NUMBER + 0",
    );
    assert!(
        Plan::resolve(&source, &[("test", &edit)])
            .unwrap()
            .render(&source)
            .unwrap()
            .contains("return DDNET_VERSION_NUMBER + 0;")
    );
    let edit = replace(
        Target::Call {
            function: "CGameClient::SendSwitchTeam".into(),
            parameters: None,
            callee: "Client()->SendPackMsgActive".into(),
            argument: Some(1),
        },
        "MSGFLAG_VITAL",
    );
    // This query deliberately covers a real member-chain standalone statement.
    Plan::resolve(&source, &[("test", &edit)]).unwrap();
    let edit = replace(
        Target::Body {
            name: "CGameClient::OnEnterGame".into(),
            parameters: None,
        },
        "\n// overwritten body\n",
    );
    Plan::resolve(&source, &[("test", &edit)]).unwrap();
}
