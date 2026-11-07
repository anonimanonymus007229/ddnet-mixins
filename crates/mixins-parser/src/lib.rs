//! Conservative structural index, not a complete C++ grammar.
//! Unsupported declaration bodies are reported and never searched as declarations.
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
#[derive(Debug, Serialize)]
pub struct Index {
    pub functions: Vec<Function>,
    pub declarations: Vec<FunctionDeclaration>,
    pub classes: Vec<Class>,
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
        if t.kind != Kind::Punctuation {
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
                ";" | "{" | "}" | "=" => return None,
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
                let head = if self.word(begin) == "template" && begin + 1 < p {
                    self.angle_end(begin + 1, p).map_or(begin, |n| n + 1)
                } else {
                    begin
                };
                let header: Vec<String> = (head..p).map(|q| self.word(q).to_owned()).collect();
                let class_head = header
                    .first()
                    .is_some_and(|h| matches!(h.as_str(), "class" | "struct" | "union"));
                let namespace = header.first().is_some_and(|h| h == "namespace");
                if class_head || namespace {
                    let name_end = header
                        .iter()
                        .position(|s| s == ":" || s == "final")
                        .unwrap_or(header.len());
                    let parts = &header[1..name_end];
                    let name = if namespace {
                        parts.join("")
                    } else {
                        parts.first().cloned().unwrap_or_default()
                    };
                    // Refuse templates, attributes, macro decorations and anonymous scopes.
                    let valid = !name.is_empty()
                        && if namespace {
                            parts.iter().all(|s| {
                                s == "::" || s.chars().all(|c| c.is_alphanumeric() || c == '_')
                            })
                        } else {
                            parts.len() == 1
                        };
                    if valid {
                        let qualified = qualify(scope, &name);
                        if class_head {
                            self.index.classes.push(Class {
                                qualified_name: qualified.clone(),
                                body: self.span(p, close),
                                line: self.token(begin).line,
                            });
                        }
                        self.scope(p + 1, close, &qualified);
                    } else {
                        self.unsupported(begin, close, "unsupported class/namespace declarator");
                    }
                } else if !self.function(begin, p, close, scope) {
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
        let mut q = begin;
        if self.word(q) == "template" {
            let Some(end) = self.angle_end(q + 1, body) else {
                return false;
            };
            q = end + 1;
        }
        let declaration_start = q;
        let mut parameters = None;
        while q < body {
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
        if operator.is_none() && self.token(open - 1).kind != Kind::Identifier {
            return false;
        }
        let mut name_start = operator.unwrap_or(open - 1);
        if name_start > begin && self.word(name_start - 1) == "~" {
            name_start -= 1;
        }
        while name_start >= begin + 2
            && self.word(name_start - 1) == "::"
            && self.token(name_start - 2).kind == Kind::Identifier
        {
            name_start -= 2;
        }
        let name = (name_start..open).map(|i| self.word(i)).collect::<String>();
        if matches!(name.as_str(), "if" | "while" | "for" | "switch" | "catch") {
            return false;
        }
        let bare = name
            .rsplit("::")
            .next()
            .unwrap_or(&name)
            .trim_start_matches('~');
        let constructor =
            scope.rsplit("::").next() == Some(bare) || name.rsplit("::").nth(1) == Some(bare);
        // A bare macro-like invocation must not be mistaken for a function.
        if name_start == begin && !constructor {
            return false;
        }
        let mut suffix = param_close + 1;
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
        if (declaration_start..name_start)
            .any(|i| matches!(self.word(i), ":" | ")" | "(" | "{" | "}"))
        {
            return false;
        }
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
