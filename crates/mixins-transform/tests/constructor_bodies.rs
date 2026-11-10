use mixins_transform::{Action, Edit, Plan, Position, Target};
#[test]
fn explicit_body_edit_preserves_include_generated_initializer() {
    let s = "class C { C():\n#define X(a) value(a),\n#include \"list.h\"\n#undef X\nlast(1) { run(); } };";
    let e = Edit {
        id: "ctor".into(),
        file: "src/test.h".into(),
        action: Action::Insert,
        target: Target::ConstructorBody {
            name: "C::C".into(),
            parameters: Some("()".into()),
        },
        position: Some(Position::BodyStart),
        content: Some("/* hook */".into()),
        priority: 0,
    };
    let p = Plan::resolve(s, &[("test", &e)]).unwrap();
    let output = p.render(s).unwrap();
    assert_eq!(output.replace("/* hook */", ""), s);
    assert!(output.contains("last(1) {/* hook */"));
}
