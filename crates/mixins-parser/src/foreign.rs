//! Explicit Objective-C regions embedded in platform-conditional C/C++ headers.
//! Record interfaces, implementations and method bodies without inventing C++ names.
use super::{ForeignRegion, Parser};
impl Parser<'_> {
    pub(super) fn foreign_region(&mut self, start: usize, end: usize) -> Option<usize> {
        if start + 2 >= end
            || self.word(start) != "@"
            || !matches!(
                self.word(start + 1),
                "interface" | "implementation" | "protocol"
            )
        {
            return None;
        }
        let kind = self.word(start + 1).to_owned();
        let name = self.word(start + 2).to_owned();
        let mut p = start + 3;
        let mut bodies = vec![];
        while p < end {
            if p + 1 < end && self.word(p) == "@" && self.word(p + 1) == "end" {
                self.index.foreign_regions.push(ForeignRegion {
                    language: "objective-c".into(),
                    kind,
                    name,
                    declaration: self.span(start, p + 1),
                    bodies,
                });
                return Some(p + 1);
            }
            if matches!(self.word(p), "{" | "(" | "[") {
                let close = self.close(p)?;
                if self.word(p) == "{" {
                    bodies.push(self.span(p, close));
                }
                p = close + 1;
            } else {
                p += 1;
            }
        }
        None
    }
}
