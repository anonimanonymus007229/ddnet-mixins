//! Primary integration corpus: supplied DDNet checkout, never a generated imitation.
use mixins_lexer::{Kind, lex};
use std::{fs, path::PathBuf};

#[test]
#[ignore = "requires real DDNet; set DDMIX_TEST_DDNET and run --ignored"]
fn supplied_ddnet_corpus() {
    let root = std::env::var_os("DDMIX_TEST_DDNET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace/ddnet"));
    assert!(
        root.join("CMakeLists.txt").is_file(),
        "supply a real DDNet root via DDMIX_TEST_DDNET"
    );
    let files = mixins_core::source_files(&root.join("src")).unwrap();
    assert!(files.len() > 100, "not a representative DDNet corpus");
    let mut failures = Vec::new();
    let mut conditional = 0;
    for file in &files {
        let source = fs::read_to_string(file).unwrap();
        let index = lex(&source);
        let mut end = 0;
        for (i, token) in index.tokens.iter().enumerate() {
            assert_eq!(token.start, end, "gap in {}", file.display());
            assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
            end = token.end;
            if let Some(other) = index.pairs[i] {
                assert_eq!(index.pairs[other], Some(i));
                assert_eq!(token.kind, Kind::Punctuation);
            }
        }
        assert_eq!(end, source.len());
        conditional += usize::from(index.conditional);
        if !index.diagnostics.is_empty() {
            failures.push((file.clone(), index.diagnostics));
        }
    }
    eprintln!(
        "Scanned {} real DDNet files; {} conditional files conservatively deferred",
        files.len(),
        conditional
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
