//! Branch-state delimiter analysis. Every arm is explored; equivalent stacks
//! merge. Multi-partner tokens remain explicit alternatives, never guessed pairs.
use super::{Kind, Lexed, Structure, diag, preprocessor, text};
use std::collections::{BTreeMap, BTreeSet};
struct Frame {
    incoming: Vec<Vec<usize>>,
    exits: Vec<Vec<usize>>,
    has_else: bool,
}
fn merge(states: &mut Vec<Vec<usize>>) {
    states.sort();
    states.dedup();
}
pub(super) fn build(source: &str, lexed: &Lexed) -> Structure {
    let mut out = Structure {
        pairs: vec![None; lexed.tokens.len()],
        conditional: vec![false; lexed.tokens.len()],
        diagnostics: vec![],
        alternatives: BTreeMap::new(),
    };
    let links = preprocessor::linkage_pairs(source, lexed);
    for (&a, &b) in &links {
        out.pairs[a] = Some(b);
    }
    let mut states = vec![vec![]];
    let mut frames: Vec<Frame> = vec![];
    let mut relations: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut reported = BTreeSet::new();
    for (i, t) in lexed.tokens.iter().enumerate() {
        out.conditional[i] = !frames.is_empty();
        let w = text(source, t);
        if t.kind == Kind::Directive {
            let d = w[1..]
                .trim_start()
                .split(|c: char| !c.is_ascii_alphabetic())
                .next()
                .unwrap_or("");
            match d {
                "if" | "ifdef" | "ifndef" => frames.push(Frame {
                    incoming: states.clone(),
                    exits: vec![],
                    has_else: false,
                }),
                "else" | "elif" => {
                    if let Some(f) = frames.last_mut() {
                        f.exits.append(&mut states);
                        states = f.incoming.clone();
                        if d == "else" {
                            f.has_else = true;
                        }
                    }
                }
                "endif" => {
                    if let Some(mut f) = frames.pop() {
                        f.exits.append(&mut states);
                        if !f.has_else {
                            f.exits.extend(f.incoming);
                        }
                        states = f.exits;
                        merge(&mut states);
                        if states.len() > 256 {
                            out.diagnostics.push(diag(t,"STRUCT003","conditional delimiter state limit exceeded; split the source query or use a configured preprocessing mode"));
                            return out;
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        if t.kind != Kind::Punctuation || links.contains_key(&i) {
            continue;
        }
        if matches!(w, "(" | "[" | "{") {
            for state in &mut states {
                state.push(i);
            }
        } else if matches!(w, ")" | "]" | "}") {
            for state in &mut states {
                if let Some(&open) = state.last() {
                    let a = text(source, &lexed.tokens[open]);
                    if matches!((a, w), ("(", ")") | ("[", "]") | ("{", "}")) {
                        state.pop();
                        relations.entry(open).or_default().insert(i);
                        relations.entry(i).or_default().insert(open);
                    } else if reported.insert(i) {
                        out.diagnostics.push(diag(
                            t,
                            "STRUCT002",
                            "mismatched delimiter in a preprocessor alternative",
                        ));
                    }
                } else if reported.insert(i) {
                    out.diagnostics.push(diag(
                        t,
                        "STRUCT002",
                        "unmatched closing delimiter in a preprocessor alternative",
                    ));
                }
            }
            merge(&mut states);
        }
    }
    let mut unclosed = BTreeSet::new();
    for s in states {
        unclosed.extend(s);
    }
    for &i in &unclosed {
        out.diagnostics.push(diag(
            &lexed.tokens[i],
            "STRUCT002",
            "delimiter remains open in a preprocessor alternative",
        ));
    }
    for (&a, partners) in &relations {
        if partners.len() == 1 && !unclosed.contains(&a) {
            let b = *partners.first().unwrap();
            if relations.get(&b).is_some_and(|p| p.len() == 1) && !unclosed.contains(&b) {
                out.pairs[a] = Some(b);
                continue;
            }
        }
        out.alternatives
            .insert(a, partners.iter().copied().collect());
    }
    out
}
