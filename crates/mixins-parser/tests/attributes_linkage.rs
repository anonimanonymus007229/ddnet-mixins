use mixins_lexer::lex;
use mixins_parser::parse;
#[test]
fn balanced_attributes_preserve_function_spans_and_selectors() {
    let source = "[[nodiscard]] int f() { return 1; } [[gnu::format(printf,1,2)]] void log(const char *s,...) {} __declspec(noinline) void win() {} void g() __attribute__((noinline)) {} template<class T> [[nodiscard]] T id(T x) { return x; }";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty(), "{:?}", index.unsupported);
    assert_eq!(index.functions.len(), 5);
    assert_eq!(index.attributes.len(), 5);
    for name in ["f", "log", "win", "g", "id"] {
        index.select_function(name, None, source).unwrap();
    }
    assert_eq!(index.functions[0].declaration.start, 0);
    assert_eq!(
        &source[index.attributes[0].span.start..index.attributes[0].span.end],
        "[[nodiscard]]"
    );
}
#[test]
fn extern_blocks_are_linkage_not_namespaces() {
    let source = "namespace N { extern \"C\" { int f() { return 1; } void g(); } } extern \"C\" { void h() {} }";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty());
    assert_eq!(index.linkages.len(), 2);
    assert_eq!(index.declarations[0].qualified_name, "N::g");
    index.select_function("N::f", None, source).unwrap();
    index.select_function("h", None, source).unwrap();
}
#[test]
fn exact_conditional_linkage_is_paired_without_selecting_a_branch() {
    let s = "#ifdef __cplusplus\nextern \"C\" {\n#endif\nint f() { return 1; }\n#ifdef __cplusplus\n}\n#endif\n";
    let i = parse(s, &lex(s));
    assert!(i.diagnostics.is_empty(), "{:?}", i.diagnostics);
    assert_eq!(i.linkages.len(), 1);
    i.select_function("f", None, s).unwrap();
    let s = "#ifdef A\nextern \"C\" {\n#endif\nvoid f() {}\n#ifdef B\n}\n#endif";
    assert!(!parse(s, &lex(s)).diagnostics.is_empty());
}
#[test]
fn typedef_aggregate_aliases_have_body_and_original_declaration() {
    let source = "typedef struct Tag { int value; int get() { return value; } } Alias, *Ptr; typedef struct { int x; } Anonymous; typedef enum { A,B } Kind;";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty(), "{:?}", index.unsupported);
    assert_eq!(index.type_aliases.len(), 4);
    assert_eq!(index.classes[0].qualified_name, "Tag");
    index.select_function("Tag::get", None, source).unwrap();
    assert_eq!(index.enums.len(), 1);
    assert_eq!(index.anonymous_aggregates.len(), 1);
    assert_eq!(
        &source[index.type_aliases[0].declaration.start..index.type_aliases[0].declaration.end],
        "typedef struct Tag { int value; int get() { return value; } } Alias, *Ptr;"
    );
}
#[test]
fn attributes_do_not_let_lambdas_and_macro_bodies_become_functions() {
    for source in [
        "[[attr]] MACRO(x) {}",
        "auto lambda=[]() [[attr]] {};",
        "[[broken] int f() {}",
    ] {
        let index = parse(source, &lex(source));
        assert!(index.functions.is_empty());
        assert!(
            !index.unsupported.is_empty()
                || !index.diagnostics.is_empty()
                || !index.lambdas.is_empty()
        );
    }
}
