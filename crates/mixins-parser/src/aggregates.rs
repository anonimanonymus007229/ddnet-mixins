//! Typedef aggregates keep both type body and alias declarations.
use super::{Kind, Parser, Span, TypeAlias};
impl Parser<'_> {
    pub(super) fn typedef_aggregate(
        &mut self,
        begin: usize,
        open: usize,
        close: usize,
        end: usize,
        scope: &str,
    ) -> Option<usize> {
        if self.word(begin) != "typedef"
            || begin + 1 >= open
            || !matches!(self.word(begin + 1), "struct" | "union" | "enum")
        {
            return None;
        }
        let head = begin + 1;
        let mut tail = close + 1;
        let mut aliases = vec![];
        while tail < end && self.word(tail) != ";" {
            if matches!(self.word(tail), "*" | "&" | ",") {
                tail += 1;
                continue;
            }
            if self.token(tail).kind != Kind::Identifier {
                return None;
            }
            aliases.push(self.word(tail).to_owned());
            tail += 1;
            while tail < end && self.word(tail) == "[" {
                tail = self.close(tail)? + 1;
            }
        }
        if tail >= end || aliases.is_empty() {
            return None;
        }
        let name = if head + 1 < open && self.token(head + 1).kind == Kind::Identifier {
            Some(self.word(head + 1).to_owned())
        } else {
            None
        };
        let body = self.span(open, close);
        let declaration = self.span(begin, tail);
        if self.word(head) == "enum" {
            if !self.enumeration(begin, head, open, close, scope) {
                return None;
            }
        } else if let Some(name) = name {
            if head + 2 != open {
                return None;
            }
            let qualified = super::qualify(scope, &name);
            self.index.classes.push(super::Class {
                qualified_name: qualified.clone(),
                body,
                line: self.token(begin).line,
            });
            self.scope(open + 1, close, &qualified);
        } else {
            if head + 1 != open {
                return None;
            }
            self.index
                .anonymous_aggregates
                .push(super::AnonymousAggregate {
                    kind: self.word(head).into(),
                    declaration,
                    body,
                });
        }
        for name in aliases {
            self.index.type_aliases.push(TypeAlias {
                qualified_name: super::qualify(scope, &name),
                declaration,
                target: Span {
                    start: self.token(head).start,
                    end: body.end,
                },
            });
        }
        Some(tail)
    }
}
