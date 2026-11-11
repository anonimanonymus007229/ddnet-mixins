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
fn every_indexed_client_function_has_paired_real_body_and_no_invalid_prefix() {
    for root in clients() {
        let files = mixins_core::source_files(&root.join("src")).unwrap();
        let mut count = 0;
        for file in &files {
            let source = fs::read_to_string(file).unwrap();
            let l = lex(&source);
            let index = parse(&source, &l);
            let st = mixins_parser::structure(&source, &l);
            assert!(
                index.diagnostics.is_empty(),
                "{}: {:?}",
                file.display(),
                index.diagnostics
            );
            assert!(
                index.unsupported.is_empty(),
                "{}: {:?}",
                file.display(),
                index.unsupported
            );
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
