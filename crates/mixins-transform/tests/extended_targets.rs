use mixins_transform::{Action, Edit, Plan, Position, Target};
fn check(source: &str, target: Target) {
    let e = Edit {
        id: "head".into(),
        file: "src/test.cpp".into(),
        action: Action::Insert,
        target,
        position: Some(Position::BodyStart),
        content: Some("/* inserted */".into()),
        priority: 0,
    };
    let p = Plan::resolve(source, &[("test", &e)]).unwrap();
    assert_eq!(
        p.render(source).unwrap().replace("/* inserted */", ""),
        source
    );
}
#[test]
fn grammar_families_retain_source_preserving_edit_targets() {
    for (s, name) in [
        ("[[nodiscard]] int f() { return 1; }", "f"),
        ("extern \"C\" { void f() {} }", "f"),
        ("typedef struct Tag { void f() {} } Alias;", "Tag::f"),
        ("class Lock { void f() REQUIRES(lock) {} };", "Lock::f"),
        (
            "template<class T,int N=0> class C { explicit C(int x): Base<T,N>(x) {} };",
            "C::C",
        ),
        ("int (&C::f())[COUNT] { return data; }", "C::f"),
        (
            "namespace N { inline namespace V { void f() {} } }",
            "N::V::f",
        ),
        (
            "void f() { if(a) { work();\n#if B\n} else { more();\n#endif\n} }",
            "f",
        ),
    ] {
        check(
            s,
            Target::Function {
                name: name.into(),
                parameters: None,
            },
        );
    }
    check(
        "#if A\nint f(int x)\n#else\nlong g(int x)\n#endif\n{ return x; }",
        Target::ConditionalBody {
            names: vec!["f".into(), "g".into()],
        },
    );
    check(
        "template<class T> requires Concept<T> void f(T x) {}",
        Target::ConstrainedFunction {
            name: "f".into(),
            parameters: Some("(T x)".into()),
            constraint: "requires Concept<T>".into(),
        },
    );
}
