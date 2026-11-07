use mixins_lexer::lex;
use mixins_parser::{parse, structure};

#[test]
fn branch_local_and_enclosing_pairs_are_preserved() {
    let source = "void f() {\n#if A\nif (a) {}\n#else\nwhile (b) {}\n#endif\n}";
    let l = lex(source);
    let s = structure(source, &l);
    assert!(s.diagnostics.is_empty());
    for (i, pair) in s.pairs.iter().enumerate() {
        if let Some(j) = pair {
            assert_eq!(s.pairs[*j], Some(i));
        }
    }
    assert_eq!(s.pairs.iter().flatten().count(), 12);
    let index = parse(source, &l);
    assert_eq!(
        index.select_function("f", None, source).unwrap().body.end,
        source.len()
    );
}
#[test]
fn divergent_branches_cannot_be_selected() {
    for source in [
        "void f() {\n#if A\n}\n#endif\n}",
        "#if A\nvoid f() {\n#else\nvoid f() {\n#endif\n}",
    ] {
        let index = parse(source, &lex(source));
        assert!(!index.diagnostics.is_empty());
        assert!(index.select_function("f", None, source).is_err());
    }
}
#[test]
fn no_guessed_functions_from_macros_lambdas_or_initializers() {
    let source = "MACRO(x) {}\nauto lambda = []() {};\nThing item{2};\nvoid f() { auto nested = []() {}; if (true) {} }";
    let index = parse(source, &lex(source));
    assert_eq!(index.functions.len(), 1);
    assert_eq!(index.functions[0].qualified_name, "f");
    assert_eq!(index.unsupported.len(), 3);
}
#[test]
fn template_overload_is_indexed_and_requires_disambiguation() {
    let source = "void f() {}\ntemplate<class T> void f(T x) {}";
    let index = parse(source, &lex(source));
    assert_eq!(index.functions.len(), 2);
    assert!(
        index
            .select_function("f", None, source)
            .unwrap_err()
            .contains("2 matches")
    );
}
#[test]
fn namespaces_classes_constructors_destructors_and_parameters() {
    let source = "namespace N { class C { public: C() {} ~C() {} int f(int x) const { return x; } int f() { return 0; } }; }";
    let index = parse(source, &lex(source));
    assert_eq!(index.classes[0].qualified_name, "N::C");
    assert!(
        index
            .select_function("N::C::f", None, source)
            .unwrap_err()
            .contains("2 matches")
    );
    assert!(
        index
            .select_function("N::C::f", Some("(int x)"), source)
            .is_ok()
    );
    assert!(index.select_function("N::C::C", None, source).is_ok());
    assert!(index.select_function("N::C::~C", None, source).is_ok());
}
