//! Conservative structural index, not a complete C++ grammar.
//! Unsupported declaration bodies are reported and never searched as declarations.
mod aggregates;
mod annotations;
mod attributes;
mod conditional;
mod declarations;
mod declarator;
mod lambdas;
mod macros;
mod preprocessor;
pub mod syntax;
use mixins_core::Diagnostic;
use mixins_lexer::{Kind, Lexed, Token};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct Function {
    pub qualified_name: String,
    pub declaration: Span,
    pub parameters: Span,
    pub body: Span,
    pub line: u32,
    pub conditional: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDeclaration {
    pub qualified_name: String,
    pub declaration: Span,
    pub parameters: Span,
    pub line: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct Class {
    pub qualified_name: String,
    pub body: Span,
    pub line: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct EnumNode {
    pub qualified_name: Option<String>,
    pub scoped: bool,
    pub declaration: Span,
    pub body: Span,
    pub enumerators: Vec<Enumerator>,
    /// Directives whose expansion may add more enumerators; not evaluated.
    pub opaque_regions: Vec<Span>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Enumerator {
    pub name: String,
    pub span: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnonymousAggregate {
    pub kind: String,
    pub declaration: Span,
    pub body: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct Variable {
    pub qualified_name: String,
    pub declaration: Span,
    pub initializer: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct TypeAlias {
    pub qualified_name: String,
    pub declaration: Span,
    pub target: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct MacroDeclaration {
    pub name: String,
    pub span: Span,
    pub arguments: Vec<Span>,
}
#[derive(Debug, Clone, Serialize)]
pub struct MacroBody {
    pub name: String,
    pub declaration: Span,
    pub arguments: Vec<Span>,
    pub body: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct ConditionalFunction {
    pub declaration: Span,
    pub alternatives: Vec<Function>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Namespace {
    pub qualified_name: String,
    pub anonymous: bool,
    pub inline: bool,
    pub declaration: Span,
    pub body: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct Lambda {
    pub captures: Span,
    pub body: Span,
    pub expression: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct Constraint {
    pub span: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct Annotation {
    pub name: String,
    pub span: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct Attribute {
    pub syntax: String,
    pub span: Span,
}
#[derive(Debug, Clone, Serialize)]
pub struct Linkage {
    pub language: String,
    pub declaration: Span,
    pub body: Span,
}
#[derive(Debug, Serialize)]
pub struct Index {
    pub functions: Vec<Function>,
    pub declarations: Vec<FunctionDeclaration>,
    pub classes: Vec<Class>,
    pub enums: Vec<EnumNode>,
    pub anonymous_aggregates: Vec<AnonymousAggregate>,
    pub variables: Vec<Variable>,
    pub attributes: Vec<Attribute>,
    pub linkages: Vec<Linkage>,
    pub type_aliases: Vec<TypeAlias>,
    pub macro_bodies: Vec<MacroBody>,
    pub macro_declarations: Vec<MacroDeclaration>,
    pub annotations: Vec<Annotation>,
    pub constraints: Vec<Constraint>,
    pub lambdas: Vec<Lambda>,
    pub namespaces: Vec<Namespace>,
    pub conditional_functions: Vec<ConditionalFunction>,
    pub diagnostics: Vec<Diagnostic>,
    pub unsupported: Vec<Diagnostic>,
}
#[derive(Debug)]
pub struct Structure {
    pub pairs: Vec<Option<usize>>,
    pub conditional: Vec<bool>,
    pub diagnostics: Vec<Diagnostic>,
}
fn text<'a>(s: &'a str, t: &Token) -> &'a str {
    &s[t.start..t.end]
}
fn diag(t: &Token, code: &'static str, message: &str) -> Diagnostic {
    Diagnostic {
        code,
        message: message.into(),
        start: t.start,
        end: t.end,
        line: t.line,
        column: t.column,
    }
}

/// Pair only unambiguous delimiters. Branch-local pairs survive; pairs crossing
/// divergent branch boundaries do not. No preprocessor branch is chosen active.
pub fn structure(source: &str, lexed: &Lexed) -> Structure {
    let mut out = Structure {
        pairs: vec![None; lexed.tokens.len()],
        conditional: vec![false; lexed.tokens.len()],
        diagnostics: Vec::new(),
    };
    let linkage_pairs = preprocessor::linkage_pairs(source, lexed);
    for (&open, &close) in &linkage_pairs {
        out.pairs[open] = Some(close);
    }
    let mut stack: Vec<usize> = Vec::new();
    let mut frames: Vec<Vec<usize>> = Vec::new();
    let mut invalid = BTreeSet::new();
    for (i, t) in lexed.tokens.iter().enumerate() {
        out.conditional[i] = !frames.is_empty();
        let word = text(source, t);
        if t.kind == Kind::Directive {
            let directive = word[1..]
                .trim_start()
                .split(|c: char| !c.is_ascii_alphabetic())
                .next()
                .unwrap_or("");
            match directive {
                "if" | "ifdef" | "ifndef" => frames.push(stack.clone()),
                "elif" | "else" | "endif" => {
                    if let Some(base) = frames.last() {
                        if &stack != base {
                            invalid.extend(stack.iter().chain(base.iter()).copied());
                            out.diagnostics.push(diag(t, "STRUCT001", "delimiter structure diverges across preprocessor branches; enclosing targets unavailable"));
                        }
                        stack = base.clone();
                    }
                    if directive == "endif" {
                        frames.pop();
                    }
                }
                _ => {}
            }
            continue;
        }
        if t.kind != Kind::Punctuation || linkage_pairs.contains_key(&i) {
            continue;
        }
        if matches!(word, "(" | "[" | "{") {
            stack.push(i);
        } else if matches!(word, ")" | "]" | "}") {
            let base_depth = frames.last().map_or(0, Vec::len);
            if let Some(&open) = stack.last() {
                if stack.len() <= base_depth {
                    invalid.insert(open);
                    out.diagnostics.push(diag(
                        t,
                        "STRUCT001",
                        "branch closes a delimiter opened outside its conditional",
                    ));
                } else if matches!(
                    (text(source, &lexed.tokens[open]), word),
                    ("(", ")") | ("[", "]") | ("{", "}")
                ) {
                    stack.pop();
                    if !invalid.contains(&open) {
                        out.pairs[open] = Some(i);
                        out.pairs[i] = Some(open);
                    }
                } else {
                    out.diagnostics
                        .push(diag(t, "STRUCT002", "mismatched closing delimiter"));
                }
            } else {
                out.diagnostics
                    .push(diag(t, "STRUCT002", "unmatched closing delimiter"));
            }
        }
    }
    for i in stack {
        if !invalid.contains(&i) {
            out.diagnostics
                .push(diag(&lexed.tokens[i], "STRUCT002", "unclosed delimiter"));
        }
    }
    for i in invalid {
        if let Some(j) = out.pairs[i].take() {
            out.pairs[j] = None;
        }
    }
    out
}

struct Parser<'a> {
    source: &'a str,
    lexed: &'a Lexed,
    structure: Structure,
    /// Significant tokens; original token indices retained for spans and pairing.
    sig: Vec<usize>,
    positions: Vec<Option<usize>>,
    index: Index,
}
impl Parser<'_> {
    fn word(&self, p: usize) -> &str {
        text(self.source, &self.lexed.tokens[self.sig[p]])
    }
    fn token(&self, p: usize) -> &Token {
        &self.lexed.tokens[self.sig[p]]
    }
    fn close(&self, p: usize) -> Option<usize> {
        self.structure.pairs[self.sig[p]].and_then(|i| self.positions[i])
    }
    fn span(&self, start: usize, end: usize) -> Span {
        Span {
            start: self.token(start).start,
            end: self.token(end).end,
        }
    }
    fn unsupported(&mut self, p: usize, end: usize, message: &str) {
        let mut diagnostic = diag(self.token(p), "PARSE001", message);
        diagnostic.end = self.token(end).end;
        self.index.unsupported.push(diagnostic);
    }
    /// Contextual template/type argument list; never used on expressions.
    fn angle_end(&self, start: usize, end: usize) -> Option<usize> {
        if self.word(start) != "<" {
            return None;
        }
        let mut depth = 0usize;
        let mut p = start;
        while p < end {
            match self.word(p) {
                "<" => depth += 1,
                ">" => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(p);
                    }
                }
                ">>" => {
                    depth = depth.checked_sub(2)?;
                    if depth == 0 {
                        return Some(p);
                    }
                }
                "(" | "[" => {
                    p = self.close(p)?;
                }
                "=" => {}
                ";" | "{" | "}" => return None,
                _ => {}
            }
            p += 1;
        }
        None
    }
    fn scope(&mut self, start: usize, end: usize, scope: &str) {
        let mut p = start;
        let mut begin = start;
        while p < end {
            if p == begin {
                if let Some(close) = self.macro_declaration(begin, end) {
                    p = close + 1;
                    begin = p;
                    continue;
                }
            }
            if let Some(close) = self.conditional_function(p, end, scope) {
                p = close + 1;
                begin = p;
                continue;
            }
            let w = self.word(p);
            if w == ";" && self.function(begin, p, p, scope) {
                if let Some(f) = self.index.functions.pop() {
                    self.index.declarations.push(FunctionDeclaration {
                        qualified_name: f.qualified_name,
                        declaration: f.declaration,
                        parameters: f.parameters,
                        line: f.line,
                    });
                }
            }
            let w = self.word(p);
            if self.token(p).kind == Kind::Directive
                || w == ";"
                || matches!(w, "public" | "private" | "protected")
                    && p + 1 < end
                    && self.word(p + 1) == ":"
            {
                p += if matches!(w, "public" | "private" | "protected") {
                    2
                } else {
                    1
                };
                begin = p;
                continue;
            }
            if w == "{" {
                // Constructor member initializers can themselves contain braces.
                // Resolve the full initializer grammar before declaring a body.
                if let Some((body, close)) = self.constructor_body(begin, end, scope) {
                    if close < end && self.function(begin, body, close, scope) {
                        p = close + 1;
                        begin = p;
                        continue;
                    }
                }
                let Some(close) = self.close(p).filter(|c| *c < end) else {
                    self.unsupported(
                        p,
                        end - 1,
                        "unresolved body delimiter; declaration not indexed",
                    );
                    // Do not descend into an unresolved scope: nested text might
                    // otherwise masquerade as top-level functions/classes.
                    return;
                };
                let first = self.skip_attributes(begin, p);
                let mut head = if first < p && self.word(first) == "template" && first + 1 < p {
                    self.angle_end(first + 1, p)
                        .map_or(first, |n| self.skip_attributes(n + 1, p))
                } else {
                    first
                };
                if head + 2 == p
                    && self.word(head) == "extern"
                    && self.token(head + 1).kind == Kind::String
                {
                    self.record_attributes(begin, p);
                    self.index.linkages.push(Linkage {
                        language: self.word(head + 1).into(),
                        declaration: self.span(begin, close),
                        body: self.span(p, close),
                    });
                    self.scope(p + 1, close, scope);
                    p = close + 1;
                    begin = p;
                    continue;
                }
                // Recognize declarators before interpreting leading `class` or
                // `struct` as definitions (they can be elaborated return types).
                if self.function(begin, p, close, scope) {
                    p = close + 1;
                    begin = p;
                    continue;
                }
                if let Some(last) = self.typedef_aggregate(begin, p, close, end, scope) {
                    p = last + 1;
                    begin = p;
                    continue;
                }
                if self.macro_body(begin, p, close) {
                    p = close + 1;
                    begin = p;
                    continue;
                }
                if self.enumeration(begin, head, p, close, scope) {
                    p = close + 1;
                    begin = p;
                    continue;
                }
                if let Some(declaration_end) =
                    self.anonymous_aggregate(begin, head, p, close, end, scope)
                {
                    p = declaration_end + 1;
                    begin = p;
                    continue;
                }
                if let Some(last) = self.lambda_variable(begin, p, close, end, scope) {
                    p = last + 1;
                    begin = p;
                    continue;
                }
                if let Some(declaration_end) = self.braced_variable(begin, p, close, end, scope) {
                    p = declaration_end + 1;
                    begin = p;
                    continue;
                }
                let mut header = Vec::new();
                let mut q = head;
                while q < p {
                    if let Some(end) = self
                        .annotation_end(q, p)
                        .or_else(|| self.attribute_end(q, p))
                    {
                        q = end + 1;
                    } else {
                        header.push(self.word(q).to_owned());
                        q += 1;
                    }
                }
                if header
                    .first()
                    .is_some_and(|s| matches!(s.as_str(), "static" | "inline"))
                    && header
                        .get(1)
                        .is_some_and(|s| matches!(s.as_str(), "class" | "struct" | "union"))
                {
                    header.remove(0);
                    head += 1;
                }
                let class_head = header
                    .first()
                    .is_some_and(|h| matches!(h.as_str(), "class" | "struct" | "union"));
                let inline_namespace = header.first().is_some_and(|h| h == "inline")
                    && header.get(1).is_some_and(|h| h == "namespace");
                let namespace =
                    header.first().is_some_and(|h| h == "namespace") || inline_namespace;
                if class_head || namespace {
                    let name_end = header
                        .iter()
                        .position(|s| s == ":" || s == "final")
                        .unwrap_or(header.len());
                    let parts = &header[if inline_namespace { 2 } else { 1 }..name_end];
                    let anonymous = namespace && parts.is_empty();
                    let name = if anonymous {
                        format!("(anonymous@{})", self.token(head).start)
                    } else {
                        parts.join("")
                    };
                    // Refuse templates, attributes, macro decorations and anonymous scopes.
                    let valid = !name.is_empty()
                        && (namespace || self.class_header_tail(head, p))
                        && if namespace {
                            parts.iter().all(|s| {
                                s == "::" || s.chars().all(|c| c.is_alphanumeric() || c == '_')
                            })
                        } else {
                            let mut cursor = head + 1;
                            while let Some(end) = self
                                .annotation_end(cursor, p)
                                .or_else(|| self.attribute_end(cursor, p))
                            {
                                cursor = end + 1;
                            }
                            let mut valid =
                                cursor < p && self.token(cursor).kind == Kind::Identifier;
                            if valid {
                                cursor += 1;
                            }
                            while valid && cursor < p && self.word(cursor) == "::" {
                                cursor += 1;
                                valid = cursor < p && self.token(cursor).kind == Kind::Identifier;
                                if valid {
                                    cursor += 1;
                                }
                            }
                            if valid && cursor < p && self.word(cursor) == "<" {
                                if let Some(end) = self.angle_end(cursor, p) {
                                    cursor = end + 1;
                                } else {
                                    valid = false;
                                }
                            }
                            valid && (cursor == p || matches!(self.word(cursor), ":" | "final"))
                        };
                    if valid {
                        self.record_attributes(begin, p);
                        self.record_annotations(begin, p);
                        let qualified = qualify(scope, &name);
                        if class_head {
                            self.index.classes.push(Class {
                                qualified_name: qualified.clone(),
                                body: self.span(p, close),
                                line: self.token(begin).line,
                            });
                        }
                        if namespace {
                            self.index.namespaces.push(Namespace {
                                qualified_name: qualified.clone(),
                                anonymous,
                                inline: inline_namespace,
                                declaration: self.span(begin, close),
                                body: self.span(p, close),
                            });
                        }
                        self.scope(p + 1, close, &qualified);
                    } else {
                        self.unsupported(begin, close, "unsupported class/namespace declarator");
                    }
                } else {
                    self.unsupported(begin.min(p), close, "unsupported declaration body (initializer, lambda, template, macro or complex declarator)");
                }
                p = close + 1;
                begin = p;
                continue;
            }
            // Skip parentheses/brackets as structural units, not their contents.
            if matches!(w, "(" | "[") {
                if let Some(close) = self.close(p) {
                    p = close + 1;
                    continue;
                }
            }
            p += 1;
        }
    }
    fn function(&mut self, begin: usize, body: usize, close: usize, scope: &str) -> bool {
        if begin >= body {
            return false;
        }
        if self.indirect_function(begin, body, close, scope) {
            return true;
        }
        let mut q = self.skip_attributes(begin, body);
        if q >= body {
            return false;
        }
        if self.word(q) == "template" {
            let Some(end) = self.angle_end(q + 1, body) else {
                return false;
            };
            q = self.skip_attributes(end + 1, body);
        }
        let mut constraint_spans = vec![];
        if q < body && self.word(q) == "requires" {
            let first = q;
            q += 1;
            if q >= body || self.word(q) != "(" {
                return false;
            }
            let Some(end) = self.close(q) else {
                return false;
            };
            constraint_spans.push(self.span(first, end));
            q = self.skip_attributes(end + 1, body);
        }
        let declaration_start = q;
        let mut parameters = None;
        while q < body {
            if let Some(end) = self.attribute_end(q, body) {
                q = end + 1;
                continue;
            }
            if matches!(
                self.word(q),
                "=" | "[" | ">" | "template" | "typedef" | "using" | "return"
            ) {
                return false;
            }
            if self.word(q) == "<" {
                let Some(end) = self.angle_end(q, body) else {
                    return false;
                };
                q = end + 1;
                continue;
            }
            if self.word(q) == "operator" {
                q += 1;
                if q >= body {
                    return false;
                }
                if matches!(self.word(q), "(" | "[") {
                    let Some(end) = self.close(q) else {
                        return false;
                    };
                    q = end + 1;
                } else if self.token(q).kind == Kind::Identifier {
                    while q < body && self.word(q) != "(" {
                        if self.word(q) == "<" {
                            let Some(end) = self.angle_end(q, body) else {
                                return false;
                            };
                            q = end + 1;
                        } else if self.token(q).kind == Kind::Identifier
                            || matches!(self.word(q), "::" | "*" | "&" | "&&")
                        {
                            q += 1;
                        } else {
                            return false;
                        }
                    }
                } else {
                    q += 1;
                }
                if q >= body || self.word(q) != "(" {
                    return false;
                }
            }
            if self.word(q) == "(" {
                parameters = Some(q);
                break;
            }
            q += 1;
        }
        let Some(open) = parameters else {
            return false;
        };
        let Some(param_close) = self.close(open).filter(|c| *c < body) else {
            return false;
        };
        if open == declaration_start {
            return false;
        }
        let operator = (declaration_start..open).find(|&i| self.word(i) == "operator");
        let conversion =
            operator.is_some_and(|op| op + 1 < open && self.token(op + 1).kind == Kind::Identifier);
        if operator.is_none() && self.token(open - 1).kind != Kind::Identifier {
            return false;
        }
        let mut name_start = operator.unwrap_or(open - 1);
        if name_start > begin && self.word(name_start - 1) == "~" {
            name_start -= 1;
        }
        while name_start >= begin + 2 && self.word(name_start - 1) == "::" {
            if self.token(name_start - 2).kind == Kind::Identifier {
                name_start -= 2;
                continue;
            }
            if matches!(self.word(name_start - 2), ">" | ">>") {
                let finish = name_start - 2;
                let candidate = (begin..finish)
                    .rev()
                    .find(|&p| self.word(p) == "<" && self.angle_end(p, open) == Some(finish));
                if let Some(angle) = candidate {
                    if angle > begin && self.token(angle - 1).kind == Kind::Identifier {
                        name_start = angle - 1;
                        continue;
                    }
                }
            }
            break;
        }
        let name = if conversion {
            let op = operator.unwrap();
            format!(
                "{}operator {}",
                (name_start..op).map(|i| self.word(i)).collect::<String>(),
                (op + 1..open)
                    .map(|i| self.word(i))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        } else {
            (name_start..open).map(|i| self.word(i)).collect::<String>()
        };
        if matches!(name.as_str(), "if" | "while" | "for" | "switch" | "catch") {
            return false;
        }
        let bare = name
            .rsplit("::")
            .next()
            .unwrap_or(&name)
            .trim_start_matches('~');
        let constructor = scope
            .rsplit("::")
            .next()
            .map(|s| s.split('<').next().unwrap_or(s))
            == Some(bare)
            || name
                .rsplit("::")
                .nth(1)
                .map(|s| s.split('<').next().unwrap_or(s))
                == Some(bare);
        // A bare macro-like invocation must not be mistaken for a function.
        if (name_start == begin && !constructor && !conversion)
            || (name_start > begin
                && !constructor
                && !self.type_prefix(declaration_start, name_start))
        {
            return false;
        }
        let mut suffix = self.skip_attributes(param_close + 1, body);
        while let Some(end) = self.annotation_end(suffix, body) {
            suffix = self.skip_attributes(end + 1, body);
        }
        while suffix < body
            && matches!(
                self.word(suffix),
                "const" | "volatile" | "noexcept" | "override" | "final" | "&" | "&&"
            )
        {
            let noexcept = self.word(suffix) == "noexcept";
            suffix += 1;
            if noexcept && suffix < body && self.word(suffix) == "(" {
                let Some(end) = self.close(suffix) else {
                    return false;
                };
                suffix = end + 1;
            }
            suffix = self.skip_attributes(suffix, body);
            while let Some(end) = self.annotation_end(suffix, body) {
                suffix = self.skip_attributes(end + 1, body);
            }
        }
        if suffix < body && self.word(suffix) == "requires" {
            let first = suffix;
            suffix += 1;
            if suffix >= body || self.word(suffix) != "(" {
                return false;
            }
            let Some(end) = self.close(suffix) else {
                return false;
            };
            constraint_spans.push(self.span(first, end));
            suffix = end + 1;
        }
        if suffix < body && self.word(suffix) == "->" {
            suffix += 1;
            while suffix < body {
                if self.word(suffix) == "<" {
                    let Some(end) = self.angle_end(suffix, body) else {
                        return false;
                    };
                    suffix = end + 1;
                } else if self.token(suffix).kind == Kind::Identifier
                    || matches!(self.word(suffix), "::" | "*" | "&" | "&&")
                {
                    suffix += 1;
                } else {
                    return false;
                }
            }
        }
        if suffix < body && constructor && self.word(suffix) == ":" {
            suffix += 1;
            loop {
                if suffix >= body || self.token(suffix).kind != Kind::Identifier {
                    return false;
                }
                suffix += 1;
                while suffix < body && self.word(suffix) == "::" {
                    suffix += 2;
                }
                if suffix < body && self.word(suffix) == "<" {
                    let Some(end) = self.angle_end(suffix, body) else {
                        return false;
                    };
                    suffix = end + 1;
                }
                if suffix >= body || !matches!(self.word(suffix), "(" | "{") {
                    return false;
                }
                let Some(end) = self.close(suffix) else {
                    return false;
                };
                suffix = end + 1;
                if suffix == body {
                    break;
                }
                if self.word(suffix) != "," {
                    return false;
                }
                suffix += 1;
            }
        }
        if body == close
            && suffix + 2 == body
            && self.word(suffix) == "="
            && matches!(self.word(suffix + 1), "0" | "default" | "delete")
        {
            suffix = body;
        }
        if suffix != body {
            return false;
        }
        if declaration_start < name_start && !self.type_prefix(declaration_start, name_start) {
            return false;
        }
        self.record_attributes(begin, body);
        self.record_annotations(begin, body);
        self.index
            .constraints
            .extend(constraint_spans.into_iter().map(|span| Constraint { span }));
        self.index.functions.push(Function {
            qualified_name: qualify(scope, &name),
            declaration: self.span(begin, close),
            parameters: self.span(open, param_close),
            body: self.span(body, close),
            line: self.token(begin).line,
            conditional: self.structure.conditional[self.sig[body]],
        });
        true
    }
}
fn qualify(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.into()
    } else {
        format!("{scope}::{name}")
    }
}

pub fn parse(source: &str, lexed: &Lexed) -> Index {
    let structure = structure(source, lexed);
    let sig: Vec<_> = lexed
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !matches!(t.kind, Kind::Whitespace | Kind::Comment))
        .map(|(i, _)| i)
        .collect();
    let mut positions = vec![None; lexed.tokens.len()];
    for (p, &i) in sig.iter().enumerate() {
        positions[i] = Some(p);
    }
    let mut diagnostics = lexed.diagnostics.clone();
    diagnostics.extend(structure.diagnostics.clone());
    let mut parser = Parser {
        source,
        lexed,
        structure,
        sig,
        positions,
        index: Index {
            functions: vec![],
            declarations: vec![],
            classes: vec![],
            enums: vec![],
            anonymous_aggregates: vec![],
            variables: vec![],
            attributes: vec![],
            linkages: vec![],
            type_aliases: vec![],
            macro_bodies: vec![],
            macro_declarations: vec![],
            annotations: vec![],
            constraints: vec![],
            lambdas: vec![],
            namespaces: vec![],
            conditional_functions: vec![],
            diagnostics,
            unsupported: vec![],
        },
    };
    // Lexical corruption makes every structural query unsafe.
    if lexed.diagnostics.is_empty() {
        parser.scope(0, parser.sig.len(), "");
    }
    parser.index
}

impl Index {
    /// No guessed fallback or occurrence selection: overloads require parameters.
    pub fn select_function(
        &self,
        name: &str,
        parameters: Option<&str>,
        source: &str,
    ) -> Result<&Function, String> {
        if !self.diagnostics.is_empty() {
            return Err(
                "source has structural/lexical diagnostics; function selection refused".into(),
            );
        }
        if self
            .conditional_functions
            .iter()
            .any(|node| node.alternatives.iter().any(|f| f.qualified_name == name))
        {
            return Err(format!(
                "target {name}: conditional signature alternatives share a body; explicit branch-aware editing not yet supported"
            ));
        }
        // An unsupported region may contain another overload. Never infer
        // uniqueness just because the supported subset happens to contain one.
        let bare = name
            .rsplit("::")
            .next()
            .unwrap_or(name)
            .trim_start_matches('~');
        for region in &self.unsupported {
            let fragment = mixins_lexer::lex(&source[region.start..region.end]);
            if fragment.tokens.iter().any(|t| {
                t.kind == Kind::Identifier
                    && &source[region.start + t.start..region.start + t.end] == bare
            }) {
                return Err(format!(
                    "target {name}: potentially matching unsupported region at {}:{}; selection refused",
                    region.line, region.column
                ));
            }
        }
        let matches: Vec<_> = self
            .functions
            .iter()
            .filter(|f| {
                f.qualified_name == name
                    && parameters.is_none_or(|p| {
                        source[f.parameters.start..f.parameters.end].trim() == p.trim()
                    })
            })
            .collect();
        match matches.as_slice() {
            [f] => Ok(f),
            [] => Err(format!(
                "target {name}: 0 supported matches ({} unsupported regions); inspect the file or refine the selector",
                self.unsupported.len()
            )),
            _ => Err(format!(
                "target {name}: {} matches; supply exact parenthesized parameters",
                matches.len()
            )),
        }
    }
}
