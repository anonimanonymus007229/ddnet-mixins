//! Macro bodies remain macro nodes; no function names invented by expansion.
use super::{Kind, MacroBody, Parser};
impl Parser<'_> {
    pub(super) fn macro_declaration(&mut self, start: usize, end: usize) -> Option<usize> {
        if start + 1 >= end
            || self.token(start).kind != Kind::Identifier
            || self.word(start + 1) != "("
        {
            return None;
        }
        let name = self.word(start);
        if !name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            return None;
        }
        let close = self.close(start + 1)?;
        if close + 1 >= end
            || matches!(
                self.word(close + 1),
                "{" | ";" | "(" | "[" | "=" | "," | ":" | "::"
            )
        {
            return None;
        }
        if self.token(close + 1).line <= self.token(close).line {
            return None;
        }
        let syntax = super::syntax::Syntax::new(self.source, self.lexed).ok()?;
        let p = syntax
            .tokens
            .iter()
            .position(|i| *i == self.sig[start + 1])?;
        let arguments = syntax.arguments(p).ok()?;
        self.index.macro_declarations.push(super::MacroDeclaration {
            name: name.into(),
            span: self.span(start, close),
            arguments,
        });
        Some(close)
    }
    pub(super) fn macro_body(&mut self, begin: usize, open: usize, close: usize) -> bool {
        if begin + 1 >= open
            || self.token(begin).kind != Kind::Identifier
            || self.word(begin + 1) != "("
        {
            return false;
        }
        let name = self.word(begin);
        if !name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            return false;
        }
        let Some(args_close) = self.close(begin + 1) else {
            return false;
        };
        if args_close + 1 != open {
            return false;
        }
        // Lexical argument splitter refuses angle ambiguity exactly as other queries.
        let Ok(syntax) = super::syntax::Syntax::new(self.source, self.lexed) else {
            return false;
        };
        let Some(position) = syntax.tokens.iter().position(|i| *i == self.sig[begin + 1]) else {
            return false;
        };
        let Ok(arguments) = syntax.arguments(position) else {
            return false;
        };
        self.index.macro_bodies.push(MacroBody {
            name: name.into(),
            declaration: self.span(begin, close),
            arguments,
            body: self.span(open, close),
        });
        true
    }
}
