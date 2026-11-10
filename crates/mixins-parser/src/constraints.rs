//! Bounded constraint-expression grammar; spans are retained, not evaluated.
use super::{Kind, Parser};
impl Parser<'_> {
    fn constraint_atom(&self, mut p: usize, end: usize) -> Option<usize> {
        if p >= end {
            return None;
        }
        if self.word(p) == "!" {
            return self.constraint_atom(p + 1, end);
        }
        if self.word(p) == "(" {
            return self.close(p).filter(|n| *n < end).map(|n| n + 1);
        }
        if self.word(p) == "requires" {
            p += 1;
            if p < end && self.word(p) == "(" {
                p = self.close(p)? + 1;
            }
            if p >= end || self.word(p) != "{" {
                return None;
            }
            return self.close(p).filter(|n| *n < end).map(|n| n + 1);
        }
        if self.token(p).kind != Kind::Identifier && !matches!(self.token(p).kind, Kind::Number) {
            return None;
        }
        p += 1;
        loop {
            if p < end && self.word(p) == "<" {
                p = self.angle_end(p, end)? + 1;
            }
            if p + 1 < end && self.word(p) == "::" && self.token(p + 1).kind == Kind::Identifier {
                p += 2;
            } else {
                break;
            }
        }
        Some(p)
    }
    pub(super) fn constraint_end(&self, start: usize, end: usize) -> Option<usize> {
        let mut p = self.constraint_atom(start, end)?;
        while p < end && matches!(self.word(p), "&&" | "||" | "==" | "!=" | "<=" | ">=") {
            p = self.constraint_atom(p + 1, end)?;
        }
        Some(p)
    }
    pub(super) fn requires_body(&self, begin: usize, open: usize) -> bool {
        // A requires-expression has its own `requires` inside the clause (or an
        // initializer). A sole trailing `requires (expr)` precedes the function body.
        for p in begin..open {
            if self.word(p) != "requires" {
                continue;
            }
            let prefix = (begin..p).any(|q| matches!(self.word(q), "requires" | "="));
            if !prefix {
                continue;
            }
            let mut q = p + 1;
            if q < open && self.word(q) == "(" {
                let Some(last) = self.close(q) else {
                    continue;
                };
                q = last + 1;
            }
            if q == open {
                return true;
            }
        }
        false
    }
}
