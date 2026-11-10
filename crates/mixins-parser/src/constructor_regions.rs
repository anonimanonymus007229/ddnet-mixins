//! Constructor headers containing preprocessor fragments retain their full
//! original header and opaque directives. No include expansion is invented.
use super::{ConstructorRegion, Kind, Parser};
impl Parser<'_> {
    pub(super) fn constructor_region(
        &mut self,
        start: usize,
        end: usize,
        scope: &str,
    ) -> Option<usize> {
        let mut name_start = self.skip_attributes(start, end);
        while let Some(last) = self.annotation_end(name_start, end) {
            name_start = last + 1;
        }
        if name_start < end && self.word(name_start) == "explicit" {
            name_start += 1;
        }
        let mut open = name_start;
        while open < end && self.word(open) != "(" {
            if self.token(open).kind != Kind::Identifier && !matches!(self.word(open), "::" | "~") {
                return None;
            }
            open += 1;
        }
        if open == name_start || open >= end {
            return None;
        }
        let name = (name_start..open).map(|p| self.word(p)).collect::<String>();
        let bare = name.rsplit("::").next()?;
        let owner = if scope.is_empty() {
            name.rsplit("::").nth(1)
        } else {
            scope.rsplit("::").next()
        };
        if owner != Some(bare) {
            return None;
        }
        let parameters_end = self.close(open)?;
        let mut p = parameters_end + 1;
        let mut fragments = vec![];
        let mut colon = false;
        // Parse literal initializer entries; directives contribute unknown entries
        // but cannot hide the actual source-level compound-body boundary.
        while p < end {
            if self.token(p).kind == Kind::Directive {
                fragments.push(self.span(p, p));
                p += 1;
                continue;
            }
            if let Some(last) = self.annotation_end(p, end) {
                p = last + 1;
                continue;
            }
            match self.word(p) {
                ":" => {
                    colon = true;
                    p += 1;
                }
                "," => {
                    if !colon {
                        return None;
                    }
                    p += 1;
                }
                "{" => {
                    if fragments.is_empty() {
                        return None;
                    }
                    let close = self.close(p)?;
                    self.index.constructor_regions.push(ConstructorRegion {
                        qualified_name: super::qualify(scope, &name),
                        declaration: self.span(start, close),
                        parameters: self.span(open, parameters_end),
                        body: self.span(p, close),
                        opaque_regions: fragments,
                    });
                    return Some(close);
                }
                _ => {
                    if !colon || self.token(p).kind != Kind::Identifier {
                        return None;
                    }
                    p += 1;
                    while p + 1 < end
                        && self.word(p) == "::"
                        && self.token(p + 1).kind == Kind::Identifier
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
                    if p < end
                        && self.word(p) != "{"
                        && self.word(p) != ","
                        && self.token(p).kind != Kind::Directive
                    {
                        return None;
                    }
                }
            }
        }
        None
    }
}
