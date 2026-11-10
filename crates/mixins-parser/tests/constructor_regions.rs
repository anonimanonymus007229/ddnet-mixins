use mixins_lexer::lex;
use mixins_parser::parse;
#[test]
fn opaque_constructor_initializers_are_indexed_not_guessed() {
    for source in [
        "class C { C():\n#define X(a) field(a),\n#include \"fields.h\"\n#undef X\nlast(1) {} };",
        "C::C()\n#ifdef ACTIVE\n:field(this)\n#endif\n{}",
        "class C { C():first(1)\n#ifndef NDEBUG\n,second(2)\n#endif\n{} };",
    ] {
        let i = parse(source, &lex(source));
        assert!(i.diagnostics.is_empty());
        assert!(i.unsupported.is_empty(), "{:?}", i.unsupported);
        assert_eq!(i.constructor_regions.len(), 1);
        assert_eq!(i.constructor_regions[0].qualified_name, "C::C");
        assert!(!i.constructor_regions[0].opaque_regions.is_empty());
        assert!(i.select_function("C::C", None, source).is_err());
    }
}
