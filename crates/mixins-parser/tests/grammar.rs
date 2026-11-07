use mixins_lexer::lex;
use mixins_parser::{parse, syntax::Syntax};
#[test]
fn template_types_trailing_returns_operators_and_noexcept() {
    let source = "template<class T> class C { public: T f(T x) noexcept(true) { return x; } }; template<class T> auto id(T x) -> T { return x; } std::vector<int> values() { return {}; } bool operator==(const X &a, const X &b) { return true; }";
    let index = parse(source, &lex(source));
    assert!(index.diagnostics.is_empty());
    for name in ["C::f", "id", "values", "operator=="] {
        index.select_function(name, None, source).unwrap();
    }
}
#[test]
fn declarations_and_deleted_defaulted_methods_are_indexed() {
    let source = "class C { public: C() = default; virtual int f(int x) const = 0; void g(); }; int free_function(double x);";
    let index = parse(source, &lex(source));
    assert_eq!(index.declarations.len(), 4);
    assert!(index.functions.is_empty());
    assert_eq!(index.declarations[0].qualified_name, "C::C");
}
#[test]
fn macro_arguments_are_structured_and_keep_nested_commas() {
    let source = "MACRO_CONFIG_INT(Name, console, nested(1, 2), {3, 4})";
    let l = lex(source);
    let syntax = Syntax::new(source, &l).unwrap();
    let calls = syntax.invocations("MACRO_CONFIG_INT").unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments.len(), 4);
    let span = calls[0].arguments[2];
    assert_eq!(&source[span.start..span.end], "nested(1, 2)");
}
#[test]
#[ignore = "requires supplied real DDNet"]
fn real_template_return_overload_is_now_indexed() {
    let root = std::env::var_os("DDMIX_TEST_DDNET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace/ddnet")
        });
    let source = std::fs::read_to_string(root.join("src/game/client/gameclient.cpp")).unwrap();
    let index = parse(&source, &lex(&source));
    assert!(
        index
            .select_function("CGameClient::CreateManagedTeeRenderInfo", None, &source)
            .unwrap_err()
            .contains("2 matches")
    );
    index
        .select_function(
            "CGameClient::CreateManagedTeeRenderInfo",
            Some("(const CClientData &Client)"),
            &source,
        )
        .unwrap();
}
