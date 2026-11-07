//! Token-based anchors, invocation arguments and conservative statement queries.
//! Invocations are lexical (including macro syntax); no macro expansion implied.
use crate::{Span, structure};
use mixins_lexer::{Kind, Lexed};
use serde::Serialize;

pub struct Syntax<'a> {
    pub source: &'a str,
    pub lexed: &'a Lexed,
    pub tokens: Vec<usize>,
    pub pairs: Vec<Option<usize>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Invocation {
    pub name: String,
    pub span: Span,
    pub arguments: Vec<Span>,
}
impl<'a> Syntax<'a> {
    pub fn new(source: &'a str, lexed: &'a Lexed) -> Result<Self, String> {
        let st = structure(source, lexed);
        if !lexed.diagnostics.is_empty() || !st.diagnostics.is_empty() {
            return Err("invalid or branch-divergent source structure".into());
        }
        let tokens: Vec<_> = lexed
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.kind, Kind::Whitespace | Kind::Comment))
            .map(|(i, _)| i)
            .collect();
        let mut pos = vec![None; lexed.tokens.len()];
        for (p, &i) in tokens.iter().enumerate() {
            pos[i] = Some(p);
        }
        let pairs = tokens
            .iter()
            .map(|&i| st.pairs[i].and_then(|j| pos[j]))
            .collect();
        Ok(Self {
            source,
            lexed,
            tokens,
            pairs,
        })
    }
    pub fn word(&self, p: usize) -> &'a str {
        let t = &self.lexed.tokens[self.tokens[p]];
        &self.source[t.start..t.end]
    }
    pub fn span(&self, a: usize, b: usize) -> Span {
        Span {
            start: self.lexed.tokens[self.tokens[a]].start,
            end: self.lexed.tokens[self.tokens[b]].end,
        }
    }
    pub fn sequence(&self, words: &[String], within: Span, block: bool) -> Result<Span, String> {
        if words.is_empty() {
            return Err("empty token selector".into());
        }
        let mut found = vec![];
        for p in 0..self.tokens.len().saturating_sub(words.len() - 1) {
            if !words.iter().enumerate().all(|(n, s)| self.word(p + n) == s) {
                continue;
            }
            let last = p + words.len() - 1;
            let span = self.span(p, last);
            if span.start < within.start || span.end > within.end {
                continue;
            }
            let selected = if block {
                if !matches!(self.word(last), "{" | "(" | "[") {
                    return Err("block token selector must end in an opening delimiter".into());
                }
                self.span(last, self.pairs[last].ok_or("unpaired selected block")?)
            } else {
                span
            };
            found.push(selected);
        }
        match found.as_slice() {
            [s] => Ok(*s),
            _ => Err(format!("token selector {words:?}: {} matches", found.len())),
        }
    }
    /// Split only top-level commas, jumping balanced ()/[]/{} regions.
    /// Angle-bearing argument lists are rejected until template/expression
    /// disambiguation is implemented. Never silently split a template comma.
    pub fn arguments(&self, open: usize) -> Result<Vec<Span>, String> {
        if self.word(open) != "(" {
            return Err("arguments require '('".into());
        }
        let close = self.pairs[open].ok_or("unpaired invocation")?;
        if close == open + 1 {
            return Ok(vec![]);
        }
        let mut start = open + 1;
        let mut p = start;
        let mut args = vec![];
        while p < close {
            match self.word(p) {
                "(" | "[" | "{" => {
                    p = self.pairs[p].ok_or("unpaired argument delimiter")? + 1;
                    continue;
                }
                "<" | ">" | ">>" | "<<" => return Err(
                    "angle-bearing argument expression requires template-aware parsing; refused"
                        .into(),
                ),
                "," => {
                    if p == start {
                        return Err("empty invocation argument".into());
                    }
                    args.push(self.span(start, p - 1));
                    start = p + 1;
                }
                _ => {}
            }
            p += 1;
        }
        if start == close {
            return Err("trailing empty invocation argument".into());
        }
        args.push(self.span(start, close - 1));
        Ok(args)
    }
    pub fn invocations(&self, name: &str) -> Result<Vec<Invocation>, String> {
        let mut found = vec![];
        for p in 0..self.tokens.len().saturating_sub(1) {
            if self.word(p) == name && self.word(p + 1) == "(" {
                let end = self.pairs[p + 1].ok_or("unpaired invocation")?;
                found.push(Invocation {
                    name: name.into(),
                    span: self.span(p, end),
                    arguments: self.arguments(p + 1)?,
                });
            }
        }
        Ok(found)
    }
    /// Only an entire, unconditional, outermost expression statement qualifies.
    /// Nested expressions, control-flow bodies and lambda calls are not lifted.
    pub fn call(&self, body: Span, name: &str) -> Result<(Span, Vec<Span>), String> {
        let query = mixins_lexer::lex(name);
        if !query.diagnostics.is_empty() {
            return Err("invalid callee selector".into());
        }
        let words: Vec<_> = query
            .tokens
            .iter()
            .filter(|t| !matches!(t.kind, Kind::Whitespace | Kind::Comment))
            .map(|t| &name[t.start..t.end])
            .collect();
        if words.is_empty() {
            return Err("empty callee selector".into());
        }
        for (n, w) in words.iter().enumerate() {
            let ident = w.bytes().enumerate().all(|(i, b)| {
                b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit())
            });
            if !ident
                && !matches!(*w, "::" | "." | "->")
                && !(*w == "(" && words.get(n + 1) == Some(&")"))
                && !(*w == ")" && n > 0 && words[n - 1] == "(")
            {
                return Err(
                    "callee must be an identifier/member chain with optional empty getter calls"
                        .into(),
                );
            }
        }
        if (0..self.tokens.len()).any(|p| {
            self.span(p, p).start > body.start
                && self.span(p, p).end < body.end
                && self.lexed.tokens[self.tokens[p]].kind == Kind::Directive
        }) {
            return Err("conditional call query unsupported".into());
        }
        let mut candidates = vec![];
        let mut boundary = true;
        let mut p = 0;
        while p < self.tokens.len() {
            let t = self.span(p, p);
            if t.start <= body.start || t.end >= body.end {
                p += 1;
                continue;
            }
            if self.lexed.tokens[self.tokens[p]].kind == Kind::Directive {
                return Err("call query across preprocessor directives refused".into());
            }
            if boundary {
                let start = p;
                let q = p + words.len();
                if q < self.tokens.len()
                    && self.word(q) == "("
                    && words
                        .iter()
                        .enumerate()
                        .all(|(n, w)| self.word(start + n) == *w)
                {
                    let end = self.pairs[q].ok_or("unpaired call")?;
                    if end + 1 < self.tokens.len() && self.word(end + 1) == ";" {
                        candidates.push((self.span(start, end + 1), self.arguments(q)?));
                    }
                }
            }
            match self.word(p) {
                ";" => boundary = true,
                "{" | "[" | "(" => {
                    p = self.pairs[p].ok_or("unpaired statement")?;
                    boundary = false;
                }
                _ => boundary = false,
            }
            p += 1;
        }
        match candidates.as_slice() {
            [c] => Ok(c.clone()),
            _ => Err(format!(
                "outermost standalone call {name}: {} matches (nested/control-flow calls unsupported)",
                candidates.len()
            )),
        }
    }
    /// One final return statement, no earlier/nested returns or conditional
    /// compilation. This restriction also makes WRAP's explicit original block
    /// semantics honest: no inferred forwarding of arguments or return types.
    pub fn return_expression(&self, body: Span) -> Result<Span, String> {
        let positions: Vec<_> = (0..self.tokens.len())
            .filter(|&p| {
                let s = self.span(p, p);
                s.start > body.start && s.end < body.end
            })
            .collect();
        if positions
            .iter()
            .any(|&p| self.lexed.tokens[self.tokens[p]].kind == Kind::Directive)
        {
            return Err("RETURN across conditional directives unsupported".into());
        }
        let returns: Vec<_> = positions
            .iter()
            .copied()
            .filter(|&p| self.word(p) == "return")
            .collect();
        let [ret] = returns.as_slice() else {
            return Err(format!(
                "RETURN requires one final return; found {}",
                returns.len()
            ));
        };
        let mut p = *ret + 1;
        while p < self.tokens.len() && self.span(p, p).end < body.end && self.word(p) != ";" {
            if matches!(self.word(p), "(" | "[" | "{") {
                p = self.pairs[p].ok_or("unpaired return expression")?;
            }
            p += 1;
        }
        if positions.last() != Some(&p) || p == *ret + 1 {
            return Err("RETURN requires a nonempty final expression statement".into());
        }
        // Ensure the return isn't nested in a conditional/block.
        if positions
            .iter()
            .copied()
            .any(|q| q < *ret && matches!(self.word(q), "if" | "for" | "while" | "switch" | "?"))
        {
            return Err("control-flow RETURN unsupported".into());
        }
        Ok(self.span(*ret + 1, p - 1))
    }
}
