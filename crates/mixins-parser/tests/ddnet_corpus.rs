//! Real checkout is the primary parser corpus. No excerpts are vendored.
use mixins_lexer::lex;
use mixins_parser::parse;
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace/ddnet"))
}
#[test]
#[ignore = "requires supplied DDNet checkout"]
fn real_method_bodies_classes_and_overloads() {
    let root = root();
    let source = fs::read_to_string(root.join("src/game/client/gameclient.cpp")).unwrap();
    let index = parse(&source, &lex(&source));
    assert!(index.diagnostics.is_empty(), "{:?}", index.diagnostics);
    for name in [
        "CGameClient::OnInit",
        "CGameClient::OnRender",
        "CGameClient::OnConsoleInit",
    ] {
        let f = index.select_function(name, Some("()"), &source).unwrap();
        assert_eq!(&source[f.body.start..f.body.start + 1], "{");
        assert_eq!(&source[f.body.end - 1..f.body.end], "}");
        assert!(source[f.declaration.start..f.body.start].contains(name));
        let l = lex(&source);
        let st = mixins_parser::structure(&source, &l);
        let open = l
            .tokens
            .iter()
            .position(|t| t.start == f.body.start)
            .unwrap();
        assert_eq!(l.tokens[st.pairs[open].unwrap()].end, f.body.end);
    }
    let source = fs::read_to_string(root.join("src/game/client/gameclient.h")).unwrap();
    let index = parse(&source, &lex(&source));
    assert_eq!(
        index
            .classes
            .iter()
            .filter(|c| c.qualified_name == "CGameClient")
            .count(),
        1
    );
    let source = fs::read_to_string(root.join("src/game/collision.cpp")).unwrap();
    let index = parse(&source, &lex(&source));
    assert!(
        index
            .select_function("CCollision::GetIndex", None, &source)
            .unwrap_err()
            .contains("2 matches")
    );
    index
        .select_function("CCollision::GetIndex", Some("(int Nx, int Ny)"), &source)
        .unwrap();
}
#[test]
#[ignore = "requires supplied DDNet checkout"]
fn scan_real_tree_and_validate_all_indexed_spans() {
    let root = root();
    let files = mixins_core::source_files(&root.join("src")).unwrap();
    let (mut functions, mut classes, mut unsupported, mut diagnostics) = (0, 0, 0, 0);
    for file in &files {
        let source = fs::read_to_string(file).unwrap();
        let index = parse(&source, &lex(&source));
        functions += index.functions.len();
        classes += index.classes.len();
        unsupported += index.unsupported.len();
        diagnostics += index.diagnostics.len();
        for f in &index.functions {
            assert_eq!(&source[f.body.start..f.body.start + 1], "{");
            assert_eq!(&source[f.body.end - 1..f.body.end], "}");
            assert!(f.declaration.start < f.body.start && f.declaration.end == f.body.end);
        }
        // Known DDNet-authored files must not regress on branch pairing.
        if !file
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/external/")
        {
            assert!(
                index.diagnostics.is_empty(),
                "{}: {:?}",
                file.display(),
                index.diagnostics
            );
        }
    }
    eprintln!(
        "{} files; {functions} function definitions; {classes} classes; {unsupported} unsupported regions; {diagnostics} structural diagnostics",
        files.len()
    );
    assert!(functions > 5000 && classes > 500);
}
