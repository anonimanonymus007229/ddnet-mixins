//! Balanced attribute syntax, preserved as source nodes rather than discarded.
use super::{Attribute, Kind, Parser};
impl Parser<'_> {
    pub(super) fn attribute_end(&self, p: usize, end: usize) -> Option<usize> {
        if p + 1 >= end {
            return None;
        }
        if self.word(p) == "[" && self.word(p + 1) == "[" {
            let outer = self.close(p)?;
            let inner = self.close(p + 1)?;
            return (outer < end && inner + 1 == outer).then_some(outer);
        }
        if self.token(p).kind == Kind::Identifier
            && matches!(
                self.word(p),
                "__attribute__" | "__attribute" | "__declspec" | "alignas"
            )
            && self.word(p + 1) == "("
        {
            return self.close(p + 1).filter(|n| *n < end);
        }
        None
    }
    pub(super) fn skip_attributes(&self, mut p: usize, end: usize) -> usize {
        while let Some(close) = self.attribute_end(p, end) {
            p = close + 1;
        }
        p
    }
    pub(super) fn record_attributes(&mut self, start: usize, end: usize) {
        let mut p = start;
        while p < end {
            if let Some(close) = self.attribute_end(p, end) {
                let span = self.span(p, close);
                if !self.index.attributes.iter().any(|a| a.span == span) {
                    self.index.attributes.push(Attribute {
                        syntax: if self.word(p) == "[" {
                            "standard"
                        } else if self.word(p) == "alignas" {
                            "alignment"
                        } else if self.word(p) == "__declspec" {
                            "msvc"
                        } else {
                            "gnu"
                        }
                        .into(),
                        span,
                    });
                }
                p = close + 1;
            } else {
                p += 1;
            }
        }
    }
}
