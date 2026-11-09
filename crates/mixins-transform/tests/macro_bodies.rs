use mixins_transform::{Action, Edit, Plan, Position, Target};
#[test]
fn explicit_macro_target_resolves_arguments_and_preserves_other_bytes() {
    let source = "TEST(Suite, One) { work(); } TEST(Suite, Two) { other(); }";
    let e = Edit {
        id: "test".into(),
        file: "src/test.cpp".into(),
        action: Action::Insert,
        target: Target::MacroBody {
            name: "TEST".into(),
            arguments: Some(vec!["Suite".into(), "Two".into()]),
        },
        position: Some(Position::BodyStart),
        content: Some("/* inserted */".into()),
        priority: 0,
    };
    let p = Plan::resolve(source, &[("mod", &e)]).unwrap();
    let rendered = p.render(source).unwrap();
    assert_eq!(rendered.replace("/* inserted */", ""), source);
    assert!(rendered.contains("Two) {/* inserted */"));
    let mut e = e;
    e.target = Target::MacroBody {
        name: "TEST".into(),
        arguments: None,
    };
    assert!(Plan::resolve(source, &[("mod", &e)]).is_err());
}
