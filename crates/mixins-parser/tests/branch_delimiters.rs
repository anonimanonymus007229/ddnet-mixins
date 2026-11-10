use mixins_lexer::lex;
use mixins_parser::{parse, structure};
#[test]
fn optional_else_arms_keep_enclosing_body_pair_and_expose_inner_alternatives() {
    let s = "void f() { if(a) { work();\n#ifdef B\n} else { more();\n#endif\n} }";
    let l = lex(s);
    let st = structure(s, &l);
    assert!(st.diagnostics.is_empty(), "{:?}", st.diagnostics);
    assert!(!st.alternatives.is_empty());
    let i = parse(s, &l);
    let f = i.select_function("f", None, s).unwrap();
    assert_eq!(&s[f.body.start..f.body.end], &s[9..]);
}
#[test]
fn malformed_branch_still_fails_and_no_partner_is_guessed() {
    let s = "void f() {\n#if A\n}\n#endif\n}";
    let i = parse(s, &lex(s));
    assert!(!i.diagnostics.is_empty());
    assert!(i.select_function("f", None, s).is_err());
}
#[test]
fn objective_c_is_explicit_foreign_syntax_not_cpp_functions() {
    let s = "@interface Obj:NSObject { int x; } @end\n@implementation Obj\n-(void)run { [self work]; }\n@end\nvoid cpp() {}";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty());
    assert_eq!(i.foreign_regions.len(), 2);
    assert_eq!(i.foreign_regions[1].bodies.len(), 1);
    assert_eq!(i.functions.len(), 1);
    i.select_function("cpp", None, s).unwrap();
}
