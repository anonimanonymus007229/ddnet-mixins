//! Conditional signatures sharing a body remain explicit alternatives. No active
//! configuration is guessed and ordinary selectors refuse such definitions.
use super::{ConditionalFunction, Function, Kind, Parser, Span};
impl Parser<'_> {
    pub(super) fn conditional_function(
        &mut self,
        start: usize,
        end: usize,
        scope: &str,
    ) -> Option<usize> {
        if self.token(start).kind != Kind::Directive {
            return None;
        }
        let first = self.word(start).trim_start();
        if !["#if ", "#ifdef ", "#ifndef "]
            .iter()
            .any(|s| first.starts_with(s))
        {
            return None;
        }
        let mut p = start + 1;
        let mut level = 1usize;
        let mut arms = vec![];
        let mut arm = p;
        let mut directive = start;
        while p < end {
            if self.token(p).kind == Kind::Directive {
                let text = self.word(p).trim_start();
                if text.starts_with("#if ")
                    || text.starts_with("#ifdef ")
                    || text.starts_with("#ifndef ")
                {
                    level += 1;
                }
                if text.starts_with("#endif") {
                    level -= 1;
                    if level == 0 {
                        arms.push((directive, arm, p));
                        break;
                    }
                }
                if level == 1 && (text.starts_with("#else") || text.starts_with("#elif")) {
                    arms.push((directive, arm, p));
                    directive = p;
                    arm = p + 1;
                }
            }
            p += 1;
        }
        if p + 1 >= end || self.word(p + 1) != "{" || arms.len() < 2 {
            return None;
        }
        let open = p + 1;
        let close = self.close(open)?;
        let mut alternatives = vec![];
        for (condition, first, last) in arms {
            let mut begin = first;
            // Any declarations before the branch's final signature are not silently
            // expanded: retain the complete branch span in the conditional node.
            for q in first..last {
                if self.word(q) == ";" || self.token(q).kind == Kind::Directive {
                    begin = q + 1;
                }
            }
            if begin >= last {
                continue;
            }
            let header = &self.source[self.token(begin).start..self.token(last - 1).end];
            let body = &self.source[self.token(open).start..self.token(close).end];
            let fragment = format!("{header}\n{body}");
            let lexed = mixins_lexer::lex(&fragment);
            let parsed = super::parse(&fragment, &lexed);
            if !parsed.diagnostics.is_empty() || parsed.functions.len() != 1 {
                return None;
            }
            let f = &parsed.functions[0];
            let origin = self.token(begin).start;
            let body_origin = self.token(open).start;
            let body_local = header.len() + 1;
            let map = |s: Span| Span {
                start: if s.start < body_local {
                    origin + s.start
                } else {
                    body_origin + s.start - body_local
                },
                end: if s.end <= body_local {
                    origin + s.end
                } else {
                    body_origin + s.end - body_local
                },
            };
            alternatives.push(Function {
                qualified_name: super::qualify(scope, &f.qualified_name),
                declaration: Span {
                    start: origin,
                    end: self.token(close).end,
                },
                parameters: map(f.parameters),
                body: self.span(open, close),
                line: self.token(begin).line,
                conditional: true,
                constraints: f.constraints.iter().copied().map(map).collect(),
            });
            let _ = condition;
        }
        if alternatives.is_empty() {
            return None;
        }
        self.index.conditional_functions.push(ConditionalFunction {
            declaration: self.span(start, close),
            alternatives,
        });
        Some(close)
    }
}
