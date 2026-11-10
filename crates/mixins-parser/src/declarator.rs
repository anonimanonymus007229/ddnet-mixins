//! Recursive parenthesized declarator: name and entity function parameters are
//! distinct from suffixes describing the returned pointer/reference/array type.
use super::{Kind, Parser};
impl Parser<'_> {
    fn nested_declarator(&self, start: usize, end: usize) -> Option<(String, usize, usize)> {
        let mut p = start;
        while p < end && matches!(self.word(p), "*" | "&" | "&&" | "const" | "volatile") {
            p += 1;
        }
        let (name, mut parameters) = if p < end && self.word(p) == "(" {
            let close = self.close(p)?;
            if close >= end {
                return None;
            }
            let (name, open, last) = self.nested_declarator(p + 1, close)?;
            p = close + 1;
            (name, Some((open, last)))
        } else {
            let first = p;
            if p >= end || self.token(p).kind != Kind::Identifier {
                return None;
            }
            p += 1;
            while p + 1 < end && self.word(p) == "::" && self.token(p + 1).kind == Kind::Identifier
            {
                p += 2;
            }
            ((first..p).map(|q| self.word(q)).collect(), None)
        };
        while p < end {
            if self.word(p) == "(" {
                let close = self.close(p)?;
                if close >= end {
                    return None;
                }
                if parameters.is_none() {
                    parameters = Some((p, close));
                }
                p = close + 1;
            } else if self.word(p) == "[" {
                p = self.close(p)? + 1;
                if p > end {
                    return None;
                }
            } else {
                return None;
            }
        }
        let (open, close) = parameters?;
        Some((name, open, close))
    }
    pub(super) fn indirect_function(
        &mut self,
        begin: usize,
        body: usize,
        close: usize,
        scope: &str,
    ) -> bool {
        let mut p = self.skip_attributes(begin, body);
        while p < body && self.word(p) != "(" {
            if self.word(p) == "<" {
                let Some(end) = self.angle_end(p, body) else {
                    return false;
                };
                p = end + 1;
            } else {
                p += 1;
            }
        }
        if p >= body
            || p + 1 >= body
            || !matches!(self.word(p + 1), "*" | "&" | "&&" | "(")
            || !self.type_prefix(begin, p)
        {
            return false;
        }
        let Some((name, open, last)) = self.nested_declarator(p, body) else {
            return false;
        };
        self.record_attributes(begin, body);
        self.index.functions.push(super::Function {
            qualified_name: super::qualify(scope, &name),
            declaration: self.span(begin, close),
            parameters: self.span(open, last),
            body: self.span(body, close),
            line: self.token(begin).line,
            conditional: self.structure.conditional[self.sig[body]],
            constraints: vec![],
        });
        true
    }
}
