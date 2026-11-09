use mixins_lexer::lex;
use mixins_parser::parse;
#[test]
fn branch_signatures_are_explicit_and_ordinary_selection_refuses_them() {
    let s = "#if A\nstatic int f(int x)\n#else\nstatic long g(int x)\n#endif\n{ return x; }";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty());
    assert_eq!(i.conditional_functions.len(), 1);
    let alternatives = &i.conditional_functions[0].alternatives;
    assert_eq!(alternatives.len(), 2);
    assert_eq!(alternatives[0].qualified_name, "f");
    assert_eq!(alternatives[1].qualified_name, "g");
    assert_eq!(alternatives[0].body, alternatives[1].body);
    assert!(
        i.select_function("f", None, s)
            .unwrap_err()
            .contains("branch-aware")
    );
}
#[test]
fn enum_includes_and_lambdas_preserve_opaque_source_not_invent_expansion() {
    let s = "enum E {\n#include \"items.h\"\nLAST,\n}; auto work=[](int x) { return x; };";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty());
    assert_eq!(i.enums[0].opaque_regions.len(), 1);
    assert_eq!(i.enums[0].enumerators.len(), 1);
    assert_eq!(i.lambdas.len(), 1);
    assert!(i.functions.is_empty());
}
#[test]
fn inline_anonymous_namespaces_use_stable_distinct_scope_names() {
    let s = "namespace N { inline namespace V { namespace { void f() {} } } }";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty());
    assert_eq!(i.namespaces.len(), 3);
    assert!(i.namespaces[1].inline);
    assert!(i.namespaces[2].anonymous);
    assert!(
        i.functions[0]
            .qualified_name
            .starts_with("N::V::(anonymous@")
    );
}
