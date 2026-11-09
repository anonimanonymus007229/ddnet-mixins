//! Lambda initializers are variables with explicit lambda/capture/body spans,
//! never declarations of named free functions.
use super::{Kind, Lambda, Parser, Variable, qualify};
impl Parser<'_> {
    pub(super) fn lambda_variable(
        &mut self,
        begin: usize,
        open: usize,
        close: usize,
        end: usize,
        scope: &str,
    ) -> Option<usize> {
        let eq = (begin..open).find(|&p| self.word(p) == "=")?;
        if eq == begin
            || eq + 1 >= open
            || self.word(eq + 1) != "["
            || self.token(eq - 1).kind != Kind::Identifier
            || !self.type_prefix(begin, eq - 1)
        {
            return None;
        }
        let capture = eq + 1;
        let capture_end = self.close(capture)?;
        let mut p = capture_end + 1;
        if p < open && self.word(p) == "(" {
            p = self.close(p)? + 1;
        }
        while p < open
            && matches!(
                self.word(p),
                "mutable" | "noexcept" | "constexpr" | "consteval"
            )
        {
            p += 1;
            if p < open && self.word(p) == "(" {
                p = self.close(p)? + 1;
            }
        }
        if p != open || close + 1 >= end || self.word(close + 1) != ";" {
            return None;
        }
        self.index.lambdas.push(Lambda {
            captures: self.span(capture, capture_end),
            body: self.span(open, close),
            expression: self.span(capture, close),
        });
        self.index.variables.push(Variable {
            qualified_name: qualify(scope, self.word(eq - 1)),
            declaration: self.span(begin, close + 1),
            initializer: self.span(capture, close),
        });
        Some(close + 1)
    }
}
