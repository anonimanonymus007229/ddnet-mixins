use mixins_lexer::lex;
use mixins_parser::parse;
#[test]
fn elaborated_return_types_are_functions_not_class_definitions() {
    let source = "class C { public: class IGraphics *Graphics() const { return nullptr; } struct S &State() { return state; } class IClient *Client() const; };";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty(), "{:?}", index.unsupported);
    assert_eq!(index.classes.len(), 1);
    assert_eq!(index.functions.len(), 2);
    assert_eq!(index.declarations.len(), 1);
    index
        .select_function("C::Graphics", Some("()"), source)
        .unwrap();
    index.select_function("C::State", None, source).unwrap();
}
#[test]
fn function_type_template_return_is_not_mistaken_for_parameter_list() {
    let source = "std::function<bool(int, int)> C::Compare(bool enabled) { return {}; }";
    let index = parse(source, &lex(source));
    let f = index.select_function("C::Compare", None, source).unwrap();
    assert_eq!(
        &source[f.parameters.start..f.parameters.end],
        "(bool enabled)"
    );
    assert!(index.unsupported.is_empty());
}
#[test]
fn enums_keep_names_scoping_values_and_exact_braces() {
    let source =
        "enum class E : unsigned { A = 1 << 0, B = call(1, 2), }; class C { enum { X, Y=3 }; };";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty());
    assert_eq!(index.enums.len(), 2);
    let e = &index.enums[0];
    assert_eq!(e.qualified_name.as_deref(), Some("E"));
    assert!(e.scoped);
    assert_eq!(e.enumerators[0].name, "A");
    assert_eq!(e.enumerators[1].name, "B");
    assert_eq!(&source[e.body.start..e.body.start + 1], "{");
    assert!(index.enums[1].qualified_name.is_none());
    assert!(index.functions.is_empty());
}
#[test]
fn anonymous_aggregate_with_array_tail_is_not_a_function_or_fake_class() {
    let source = "class C { struct { int a; struct Inner { int x; }; } values[COUNT]; union { int x; float y; }; };";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty());
    assert_eq!(index.anonymous_aggregates.len(), 2);
    assert_eq!(index.classes.len(), 1);
    assert!(index.functions.is_empty());
}
#[test]
fn braced_members_arrays_nested_initializers_do_not_invent_functions() {
    let source = "class C { std::chrono::nanoseconds duration{0}; std::vector<int> values = {{1}, {2}}; int array[COUNT] = {0}; Map layer = Map{Kind::BACKGROUND}; void f() {} };";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty(), "{:?}", index.unsupported);
    assert_eq!(index.variables.len(), 4);
    assert_eq!(index.functions.len(), 1);
    index.select_function("C::f", None, source).unwrap();
}
#[test]
fn linkage_namespace_and_braced_constructor_keep_actual_functions() {
    let source = "extern \"C\" void exported() {} namespace N { static Color color() { return Color(1,2); } } C::C() : first{1,2}, second(3) { work(); }";
    let index = parse(source, &lex(source));
    assert!(index.unsupported.is_empty(), "{:?}", index.unsupported);
    assert_eq!(index.functions.len(), 3);
    for name in ["exported", "N::color", "C::C"] {
        index.select_function(name, None, source).unwrap();
    }
    assert!(!index.functions.iter().any(|f| f.qualified_name == "second"));
}
#[test]
fn final_empty_classes_are_not_variables_and_multiple_ctor_braces_are_safe() {
    let source = "class Empty final {}; struct Also final {}; C::C() : first{1}, second{{2},{3}} { work(); }";
    let index = parse(source, &lex(source));
    assert_eq!(index.classes.len(), 2);
    assert!(index.variables.is_empty());
    assert!(index.unsupported.is_empty());
    index.select_function("C::C", None, source).unwrap();
}
#[test]
fn unknown_and_ambiguous_declarations_remain_diagnostics() {
    for source in [
        "auto lambda = []<class T>(T x) {};",
        "int (*factory())(int) UNKNOWN_SUFFIX {}",
        "enum E { A = };",
        "class C : MACRO(x) {};",
    ] {
        let index = parse(source, &lex(source));
        assert!(!index.unsupported.is_empty(), "silently accepted: {source}");
        assert!(index.functions.is_empty(), "invented function: {source}");
    }
}
