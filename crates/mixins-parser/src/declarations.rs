//! Declaration-specific grammar: no regex, expression braces never become scopes.
use super::{AnonymousAggregate, EnumNode, Enumerator, Kind, Parser, Variable, qualify};
impl Parser<'_> {
    /// Only type/declaration specifiers here. Parentheses are allowed inside
    /// contextual template arguments but not as an unexplained return declarator.
    pub(super) fn type_prefix(&self, start: usize, end: usize) -> bool {
        let mut p = start;
        let mut named = false;
        while p < end {
            if let Some(close) = self.attribute_end(p, end) {
                p = close + 1;
                continue;
            }
            let word = self.word(p);
            if word == "decltype" && p + 1 < end && self.word(p + 1) == "(" {
                let Some(close) = self.close(p + 1).filter(|n| *n < end) else {
                    return false;
                };
                named = true;
                p = close + 1;
                continue;
            }
            if self.token(p).kind == Kind::Identifier {
                if word == "extern" && p + 1 < end && self.token(p + 1).kind == Kind::String {
                    p += 2;
                    continue;
                }
                if word == "namespace" {
                    return false;
                }
                if matches!(word, "class" | "struct" | "union")
                    && (p + 1 >= end || self.token(p + 1).kind != Kind::Identifier)
                {
                    return false;
                }
                if matches!(
                    word,
                    "return"
                        | "if"
                        | "else"
                        | "while"
                        | "for"
                        | "switch"
                        | "case"
                        | "throw"
                        | "new"
                        | "delete"
                        | "using"
                        | "typedef"
                        | "enum"
                ) {
                    return false;
                }
                named = true;
                p += 1;
            } else if word == "<" {
                let Some(close) = self.angle_end(p, end) else {
                    return false;
                };
                p = close + 1;
            } else if matches!(word, "::" | "*" | "&" | "&&") {
                p += 1;
            } else {
                return false;
            }
        }
        named
    }
    pub(super) fn class_header_tail(&self, head: usize, end: usize) -> bool {
        let mut p = head + 1;
        while p < end {
            if let Some(close) = self
                .attribute_end(p, end)
                .or_else(|| self.annotation_end(p, end))
            {
                p = close + 1;
                continue;
            }
            if self.word(p) == "<" {
                let Some(close) = self.angle_end(p, end) else {
                    return false;
                };
                p = close + 1;
                continue;
            }
            if matches!(self.word(p), "(" | ")" | "=") {
                return false;
            }
            p += 1;
        }
        true
    }
    pub(super) fn constructor_body(
        &self,
        begin: usize,
        end: usize,
        scope: &str,
    ) -> Option<(usize, usize)> {
        let mut start = self.skip_attributes(begin, end);
        if start < end && self.word(start) == "explicit" {
            start += 1;
        }
        let mut open = start;
        while open < end && self.word(open) != "(" {
            if !matches!(self.word(open), "::" | "~") && self.token(open).kind != Kind::Identifier {
                return None;
            }
            open += 1;
        }
        if open == start || open >= end {
            return None;
        }
        let name = (start..open).map(|p| self.word(p)).collect::<String>();
        let bare = name.rsplit("::").next()?;
        if scope.rsplit("::").next() != Some(bare) && name.rsplit("::").nth(1) != Some(bare) {
            return None;
        }
        let mut p = self.close(open)? + 1;
        while let Some(last) = self.annotation_end(p, end) {
            p = last + 1;
        }
        if p >= end || self.word(p) != ":" {
            return None;
        }
        p += 1;
        loop {
            if p >= end || self.token(p).kind != Kind::Identifier {
                return None;
            }
            p += 1;
            while p + 1 < end && self.word(p) == "::" && self.token(p + 1).kind == Kind::Identifier
            {
                p += 2;
            }
            if p < end && self.word(p) == "<" {
                p = self.angle_end(p, end)? + 1;
            }
            if p >= end || !matches!(self.word(p), "(" | "{") {
                return None;
            }
            p = self.close(p)? + 1;
            if p >= end {
                return None;
            }
            if self.word(p) == "{" {
                return Some((p, self.close(p)?));
            }
            if self.word(p) != "," {
                return None;
            }
            p += 1;
        }
    }
    pub(super) fn enumeration(
        &mut self,
        begin: usize,
        head: usize,
        open: usize,
        close: usize,
        scope: &str,
    ) -> bool {
        if head >= open || self.word(head) != "enum" {
            return false;
        }
        let mut p = head + 1;
        let scoped = p < open && matches!(self.word(p), "class" | "struct");
        if scoped {
            p += 1;
        }
        let name = if p < open && self.token(p).kind == Kind::Identifier {
            let n = self.word(p).to_owned();
            p += 1;
            Some(n)
        } else {
            None
        };
        if scoped && name.is_none() {
            return false;
        }
        if p < open && (self.word(p) != ":" || p + 1 == open || !self.type_prefix(p + 1, open)) {
            return false;
        }
        let mut entries = vec![];
        let mut opaque = vec![];
        p = open + 1;
        while p < close {
            if self.token(p).kind == Kind::Directive {
                opaque.push(self.span(p, p));
                p += 1;
                continue;
            }
            if self.token(p).kind != Kind::Identifier || self.word(p) == "return" {
                return false;
            }
            let start = p;
            let name = self.word(p).to_owned();
            p += 1;
            if p < close && self.word(p) == "(" {
                return false;
            }
            if p < close && self.word(p) == "=" {
                p += 1;
                let expr = p;
                while p < close && self.word(p) != "," {
                    if self.token(p).kind == Kind::Directive
                        || matches!(self.word(p), ";" | "return")
                    {
                        return false;
                    }
                    if matches!(self.word(p), "(" | "[" | "{") {
                        let Some(end) = self.close(p).filter(|end| *end < close) else {
                            return false;
                        };
                        p = end + 1;
                    } else {
                        p += 1;
                    }
                }
                if p == expr {
                    return false;
                }
            }
            if p < close && self.word(p) != "," {
                return false;
            }
            entries.push(Enumerator {
                name,
                span: self.span(start, p - 1),
            });
            if p < close {
                p += 1;
            }
        }
        self.index.enums.push(EnumNode {
            qualified_name: name.map(|n| qualify(scope, &n)),
            scoped,
            declaration: self.span(begin, close),
            body: self.span(open, close),
            enumerators: entries,
            opaque_regions: opaque,
        });
        true
    }
    pub(super) fn anonymous_aggregate(
        &mut self,
        begin: usize,
        head: usize,
        open: usize,
        close: usize,
        end: usize,
        _scope: &str,
    ) -> Option<usize> {
        if head + 1 != open || !matches!(self.word(head), "struct" | "union") {
            return None;
        }
        // Require an ordinary declaration tail, not a guessed following scope.
        let mut tail = close + 1;
        while tail < end && self.word(tail) != ";" {
            if self.word(tail) == "[" {
                tail = self.close(tail)? + 1;
                continue;
            }
            if self.word(tail) == "=" && tail + 1 < end && self.word(tail + 1) == "{" {
                tail = self.close(tail + 1)? + 1;
                continue;
            }
            if self.token(tail).kind != Kind::Identifier
                && !matches!(self.word(tail), "," | "*" | "&")
            {
                return None;
            }
            tail += 1;
        }
        if tail >= end {
            return None;
        }
        let kind = self.word(head).to_owned();
        self.index.anonymous_aggregates.push(AnonymousAggregate {
            kind,
            declaration: self.span(begin, tail),
            body: self.span(open, close),
        });
        // Anonymous data has no globally unique qualified type name. Do not
        // expose methods under the enclosing class as that would invent lookup.
        Some(tail)
    }
    pub(super) fn braced_variable(
        &mut self,
        begin: usize,
        open: usize,
        close: usize,
        end: usize,
        scope: &str,
    ) -> Option<usize> {
        if begin >= open || close + 1 >= end || self.word(close + 1) != ";" {
            return None;
        }
        let equal = (begin..open).find(|&p| self.word(p) == "=");
        let declaration_end = equal.unwrap_or(open);
        let mut trimmed = declaration_end;
        for q in begin..declaration_end {
            if self.annotation_end(q, declaration_end) == Some(declaration_end - 1) {
                trimmed = q;
                break;
            }
        }
        let mut name_end = trimmed;
        // Fixed array bounds are balanced tokens, not part of the identifier.
        while name_end > begin && self.word(name_end - 1) == "]" {
            name_end = self.close(name_end - 1)?;
        }
        if name_end <= begin {
            return None;
        }
        let name = name_end - 1;
        let mut name_start = name;
        while name_start >= begin + 2
            && self.word(name_start - 1) == "::"
            && self.token(name_start - 2).kind == Kind::Identifier
        {
            name_start -= 2;
        }
        if matches!(self.word(begin), "class" | "struct" | "union") {
            let mut decoration = begin + 1;
            while let Some(end) = self
                .attribute_end(decoration, name)
                .or_else(|| self.annotation_end(decoration, name))
            {
                decoration = end + 1;
            }
            if self.word(name) == "final" || decoration == name {
                return None;
            }
        }
        if self.token(name).kind != Kind::Identifier || !self.type_prefix(begin, name_start) {
            return None;
        }
        // No hidden declarator/call/lambda in the typed RHS. A simple constructor
        // type (including templates) is supported; arbitrary expressions fail.
        if let Some(eq) = equal {
            if eq + 1 < open && !self.type_prefix(eq + 1, open) {
                return None;
            }
        }
        let initializer_start = equal.map_or(open, |eq| eq + 1);
        self.record_annotations(begin, open);
        self.index.variables.push(Variable {
            qualified_name: qualify(
                scope,
                &(name_start..=name)
                    .map(|p| self.word(p))
                    .collect::<String>(),
            ),
            declaration: self.span(begin, close + 1),
            initializer: self.span(initializer_start, close),
        });
        Some(close + 1)
    }
}
