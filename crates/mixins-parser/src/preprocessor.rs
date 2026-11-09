//! Recognize exact conditional linkage wrappers without choosing an active branch.
//! Only isolated `extern "C" {` / `}` arms with identical textual guards qualify.
use super::{Kind, Lexed};
use std::collections::BTreeMap;
fn guard(text: &str) -> Option<String> {
    let text = text.trim();
    let rest = text
        .strip_prefix("#ifdef")
        .map(|s| format!("ifdef:{}", s.trim()))
        .or_else(|| {
            text.strip_prefix("#if ")
                .map(|s| format!("if:{}", s.split_whitespace().collect::<String>()))
        })?;
    if rest.contains("\\\n") {
        None
    } else {
        Some(rest)
    }
}
pub(super) fn linkage_pairs(source: &str, lexed: &Lexed) -> BTreeMap<usize, usize> {
    let sig: Vec<_> = lexed
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !matches!(t.kind, Kind::Whitespace | Kind::Comment))
        .map(|(i, _)| i)
        .collect();
    let word = |p: usize| {
        let t = &lexed.tokens[sig[p]];
        &source[t.start..t.end]
    };
    let directive = |p: usize| lexed.tokens[sig[p]].kind == Kind::Directive;
    let mut pending: Vec<(String, usize)> = vec![];
    let mut pairs = BTreeMap::new();
    let mut p = 0;
    while p < sig.len() {
        if directive(p) {
            if let Some(condition) = guard(word(p)) {
                let mut declaration = p + 1;
                while declaration < sig.len()
                    && directive(declaration)
                    && word(declaration).trim_start().starts_with("#include")
                {
                    declaration += 1;
                }
                if declaration + 3 < sig.len()
                    && word(declaration) == "extern"
                    && word(declaration + 1) == "\"C\""
                    && word(declaration + 2) == "{"
                    && directive(declaration + 3)
                    && word(declaration + 3).trim() == "#endif"
                {
                    pending.push((condition, sig[declaration + 2]));
                    p = declaration + 4;
                    continue;
                }
                if p + 2 < sig.len()
                    && word(p + 1) == "}"
                    && directive(p + 2)
                    && word(p + 2).trim() == "#endif"
                {
                    if let Some((open_guard, open)) = pending.last() {
                        if *open_guard == condition {
                            pairs.insert(*open, sig[p + 1]);
                            pairs.insert(sig[p + 1], *open);
                            pending.pop();
                        }
                    }
                    p += 3;
                    continue;
                }
            }
        }
        p += 1;
    }
    pairs
}
