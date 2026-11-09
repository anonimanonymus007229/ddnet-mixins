//! Re-run against full real client checkouts; no private/client excerpts vendored.
use mixins_lexer::lex;
use mixins_parser::parse;
use std::{fs, path::PathBuf};
fn clients() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("DDMIX_TEST_DDNET") {
        return vec![PathBuf::from(path)];
    }
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace");
    let paths: Vec<_> = ["BestClient", "TClient", "RushieClient-ddnet"]
        .iter()
        .map(|s| workspace.join(s))
        .filter(|p| p.join("src").is_dir())
        .collect();
    assert!(
        !paths.is_empty(),
        "set DDMIX_TEST_DDNET to a real client checkout"
    );
    paths
}
#[test]
#[ignore = "requires real DDNet/client checkout(s)"]
fn cff_target_files_have_full_supported_declaration_coverage() {
    for root in clients() {
        for file in [
            "src/game/client/components/menus.h",
            "src/game/client/gameclient.cpp",
            "src/game/client/gameclient.h",
            "src/game/client/prediction/entities/character.h",
            "src/game/client/prediction/gameworld.h",
        ] {
            let source = fs::read_to_string(root.join(file)).unwrap();
            let index = parse(&source, &lex(&source));
            assert!(
                index.diagnostics.is_empty(),
                "{} {file}: {:?}",
                root.display(),
                index.diagnostics
            );
            assert!(
                index.unsupported.is_empty(),
                "{} {file}: {:?}",
                root.display(),
                index.unsupported
            );
            if file.ends_with("gameclient.h") {
                for name in [
                    "CGameClient::Graphics",
                    "CGameClient::Client",
                    "CGameClient::Console",
                ] {
                    index.select_function(name, Some("()"), &source).unwrap();
                }
                assert!(!index.enums.is_empty());
                assert!(!index.variables.is_empty());
            }
            if file.ends_with("gameclient.cpp") {
                index
                    .select_function("CGameClient::GetScoreComparator", None, &source)
                    .unwrap();
            }
            if file.ends_with("gameworld.h") {
                assert!(!index.anonymous_aggregates.is_empty());
            }
        }
        let source = fs::read_to_string(root.join("src/engine/client/client.cpp")).unwrap();
        let index = parse(&source, &lex(&source));
        let constructors: Vec<_> = index
            .functions
            .iter()
            .filter(|f| {
                f.qualified_name == "CClient::CClient"
                    && &source[f.parameters.start..f.parameters.end] == "()"
            })
            .collect();
        assert_eq!(constructors.len(), 1);
        assert!(
            !index
                .functions
                .iter()
                .any(|f| f.qualified_name == "m_FpsGraph")
        );
    }
}
#[test]
#[ignore = "requires real DDNet/client checkout(s)"]
fn every_indexed_client_function_has_paired_real_body_and_no_invalid_prefix() {
    for root in clients() {
        let files = mixins_core::source_files(&root.join("src")).unwrap();
        let mut count = 0;
        for file in &files {
            let source = fs::read_to_string(file).unwrap();
            let l = lex(&source);
            let index = parse(&source, &l);
            let st = mixins_parser::structure(&source, &l);
            for f in &index.functions {
                count += 1;
                let open = l
                    .tokens
                    .iter()
                    .position(|t| t.start == f.body.start)
                    .unwrap();
                assert_eq!(l.tokens[st.pairs[open].unwrap()].end, f.body.end);
                let header = source[f.declaration.start..f.body.start].trim_start();
                assert!(
                    !header.starts_with(','),
                    "{} invalid function {}",
                    file.display(),
                    f.qualified_name
                );
            }
        }
        eprintln!(
            "{}: {} files, {count} paired function bodies",
            root.display(),
            files.len()
        );
        assert!(count > 5000);
    }
}
