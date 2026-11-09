//! Whole supplied client trees are the primary compatibility corpus.
use mixins_lexer::lex;
use mixins_parser::parse;
use std::{fs, path::PathBuf};
#[test]
#[ignore = "requires real DDNet/client checkout(s)"]
fn recovered_families_have_nodes_and_function_selectors() {
    let roots = if let Some(p) = std::env::var_os("DDMIX_TEST_DDNET") {
        vec![PathBuf::from(p)]
    } else {
        let w = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace");
        ["BestClient", "TClient", "RushieClient-ddnet"]
            .iter()
            .map(|n| w.join(n))
            .filter(|p| p.join("src").exists())
            .collect()
    };
    assert!(!roots.is_empty());
    for root in roots {
        let s = fs::read_to_string(root.join("src/base/log.cpp")).unwrap();
        let i = parse(&s, &lex(&s));
        i.select_function("log_log_impl", None, &s).unwrap();
        assert!(!i.attributes.is_empty());
        let s = fs::read_to_string(root.join("src/base/aio.cpp")).unwrap();
        let i = parse(&s, &lex(&s));
        i.select_function("aio_lock", None, &s).unwrap();
        assert!(!i.annotations.is_empty());
        let s = fs::read_to_string(root.join("src/base/types.h")).unwrap();
        let i = parse(&s, &lex(&s));
        assert!(i.type_aliases.iter().any(|a| a.qualified_name == "NETADDR"));
        let s = fs::read_to_string(root.join("src/engine/client/client.cpp")).unwrap();
        let i = parse(&s, &lex(&s));
        i.select_function("CClient::DemoRecorders", None, &s)
            .unwrap();
        assert!(!i.conditional_functions.is_empty());
        let s = fs::read_to_string(root.join("src/test/str_test.cpp")).unwrap();
        let i = parse(&s, &lex(&s));
        assert!(!i.macro_bodies.is_empty());
        assert!(
            i.macro_bodies
                .iter()
                .all(|m| &s[m.body.start..m.body.start + 1] == "{")
        );
    }
}
