use mixins_lexer::lex;
use mixins_parser::parse;
#[test]
fn template_defaults_specialization_and_explicit_base_constructors() {
    let s = "template<class T, int N=0> class Buffer { public: explicit Buffer(int n): Base<T, N>(n) {} }; template<> struct std::hash<Key> { int operator()(Key k) { return 0; } };";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
    assert_eq!(i.classes.len(), 2);
    i.select_function("Buffer::Buffer", None, s).unwrap();
    assert!(
        i.classes
            .iter()
            .any(|c| c.qualified_name == "std::hash<Key>")
    );
    i.select_function("std::hash<Key>::operator()", None, s)
        .unwrap();
}
#[test]
fn annotations_are_explicit_nodes_and_unknown_macros_stay_unsupported() {
    let s = "class CAPABILITY(\"mutex\") Lock { int a[2] GUARDED_BY(lock) = {0}; public: void f() REQUIRES(lock) {} void g() override RELEASE(lock) {} };";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
    assert_eq!(i.annotations.len(), 4);
    i.select_function("Lock::f", None, s).unwrap();
    i.select_function("Lock::g", None, s).unwrap();
    let s = "void f() UNKNOWN_MACRO(x) {}";
    assert!(!parse(s, &lex(s)).unsupported.is_empty());
}
#[test]
fn constraints_and_alignment_have_explicit_spans() {
    let s = "class alignas(int) Aligned {}; template<class T> requires(!Trait<T>::value) int f(T x) { return 0; } int g() requires(true) { return 1; }";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
    assert_eq!(i.constraints.len(), 2);
    assert_eq!(i.attributes.len(), 1);
    i.select_function("f", None, s).unwrap();
    i.select_function("g", None, s).unwrap();
}
#[test]
fn recursive_return_declarators_keep_entity_parameters() {
    let s = "Item (&C::Items())[COUNT] { return items; } int (*Factory(int x))(double) { return nullptr; } C::operator int() const { return 1; } class C { operator float() const { return 1; } };";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
    for name in [
        "C::Items",
        "Factory",
        "C::operator int",
        "C::operator float",
    ] {
        i.select_function(name, None, s).unwrap();
    }
    let f = i.select_function("Factory", None, s).unwrap();
    assert_eq!(&s[f.parameters.start..f.parameters.end], "(int x)");
}
#[test]
fn macro_bodies_are_not_callable_function_definitions() {
    let s = "TEST(Math, Add) { ASSERT_EQ(1, 1); } TEST_F(Fixture, Run) { work(); }";
    let i = parse(s, &lex(s));
    assert!(i.unsupported.is_empty());
    assert!(i.functions.is_empty());
    assert_eq!(i.macro_bodies.len(), 2);
    assert_eq!(
        &s[i.macro_bodies[0].arguments[1].start..i.macro_bodies[0].arguments[1].end],
        "Add"
    );
    assert!(i.select_function("TEST", None, s).is_err());
}
