use mixins_lexer::lex;
use mixins_parser::parse;
#[test]
fn constrained_overloads_keep_expressions_and_require_a_precise_selector() {
    let s = "template<class T> requires Concept<T> int f(T x) { return 1; } template<class T> requires (!Concept<T>) int f(T x) { return 2; }";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
    assert!(i.select_function("f", None, s).is_err());
    let f = i
        .select_function_constrained("f", Some("(T x)"), Some("requires Concept<T>"), s)
        .unwrap();
    assert!(s[f.body.start..f.body.end].contains("return 1"));
}
#[test]
fn requires_expression_body_is_not_a_function_body() {
    let s =
        "template<class T> int f(T x) requires requires(T value) { value.run(); } { return 1; }";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
    let f = i.select_function("f", None, s).unwrap();
    assert_eq!(&s[f.body.start..f.body.end], "{ return 1; }");
    assert_eq!(f.constraints.len(), 1);
    assert!(s[f.constraints[0].start..f.constraints[0].end].contains("value.run()"));
}
