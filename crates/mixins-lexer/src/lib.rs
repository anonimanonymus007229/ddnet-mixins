//! Byte-preserving C++ lexical index. No macro expansion or C++ parser dependency.
use mixins_core::Diagnostic;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Kind {
    Identifier,
    Number,
    String,
    Character,
    RawString,
    Comment,
    Directive,
    Whitespace,
    Punctuation,
    Operator,
}
#[derive(Debug, Clone, Serialize)]
pub struct Token {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}
#[derive(Debug, Serialize)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    /// Symmetric token-index pairs. Conditional files are deliberately not paired:
    /// pairing across branches without preprocessing would invent structure.
    pub pairs: Vec<Option<usize>>,
    pub diagnostics: Vec<Diagnostic>,
    pub conditional: bool,
}

pub fn lex(source: &str) -> Lexed {
    let b = source.as_bytes();
    let mut out = Lexed {
        tokens: vec![],
        pairs: vec![],
        diagnostics: vec![],
        conditional: false,
    };
    let (mut i, mut line, mut column) = (0, 1, 1);
    let mut line_start = true;
    let mut conditional_stack = Vec::new();
    while i < b.len() {
        let start = i;
        let (sl, sc) = (line, column);
        let mut error = None;
        let kind;
        if b[i].is_ascii_whitespace() {
            kind = Kind::Whitespace;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
        } else if b[i..].starts_with(b"//") {
            kind = Kind::Comment;
            i += 2;
            while i < b.len() {
                if b[i] == b'\n'
                    && (i == 0 || b[i - 1] != b'\\')
                    && !(i > 1 && b[i - 1] == b'\r' && b[i - 2] == b'\\')
                {
                    break;
                }
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            kind = Kind::Comment;
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                i += 1;
            }
            if i == b.len() {
                error = Some("unterminated block comment");
            } else {
                i += 2;
            }
        } else if b[i] == b'#' && line_start {
            kind = Kind::Directive;
            i += 1;
            while i < b.len() {
                if b[i] == b'\n'
                    && b[i - 1] != b'\\'
                    && !(i > 1 && b[i - 1] == b'\r' && b[i - 2] == b'\\')
                {
                    break;
                }
                i += 1;
            }
            let directive = source[start + 1..i]
                .trim_start()
                .split(|c: char| !c.is_ascii_alphabetic())
                .next()
                .unwrap_or("");
            match directive {
                "if" | "ifdef" | "ifndef" => {
                    out.conditional = true;
                    conditional_stack.push(false);
                }
                "elif" | "else" => match conditional_stack.last_mut() {
                    Some(seen_else) if !*seen_else => {
                        if directive == "else" {
                            *seen_else = true;
                        }
                    }
                    _ => error = Some("unexpected conditional branch directive"),
                },
                "endif" if conditional_stack.pop().is_none() => {
                    error = Some("unmatched #endif");
                }
                _ => {}
            }
        } else if let Some((prefix, raw, quote)) = literal_prefix(&b[i..]) {
            i += prefix;
            if raw {
                kind = Kind::RawString;
                let delimiter_start = i;
                while i < b.len()
                    && b[i] != b'('
                    && i - delimiter_start <= 16
                    && !b[i].is_ascii_whitespace()
                    && !matches!(b[i], b')' | b'\\')
                {
                    i += 1;
                }
                if i == b.len() || b[i] != b'(' || i - delimiter_start > 16 {
                    error = Some("invalid raw string delimiter");
                } else {
                    let delimiter = &b[delimiter_start..i];
                    i += 1;
                    let mut closed = false;
                    while i < b.len() {
                        if b[i] == b')'
                            && b[i + 1..].starts_with(delimiter)
                            && b.get(i + 1 + delimiter.len()) == Some(&b'"')
                        {
                            i += delimiter.len() + 2;
                            closed = true;
                            break;
                        }
                        i += 1;
                    }
                    if !closed {
                        error = Some("unterminated raw string");
                    }
                }
            } else {
                kind = if quote == b'"' {
                    Kind::String
                } else {
                    Kind::Character
                };
                let mut closed = false;
                while i < b.len() {
                    if b[i] == quote {
                        i += 1;
                        closed = true;
                        break;
                    }
                    if b[i] == b'\n' {
                        break;
                    }
                    if b[i] == b'\\' {
                        i += 1;
                        if b.get(i) == Some(&b'\r') && b.get(i + 1) == Some(&b'\n') {
                            i += 1;
                        }
                    }
                    if i < b.len() {
                        i += 1;
                    }
                }
                if !closed {
                    error = Some("unterminated quoted literal");
                }
            }
        } else if b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] >= 128 {
            kind = Kind::Identifier;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 128) {
                i += 1;
            }
        } else if b[i].is_ascii_digit()
            || (b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            kind = Kind::Number;
            i += 1;
            while i < b.len() {
                if b[i].is_ascii_alphanumeric()
                    || matches!(b[i], b'.' | b'_' | b'\'')
                    || (matches!(b[i], b'+' | b'-')
                        && matches!(b[i - 1], b'e' | b'E' | b'p' | b'P'))
                {
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            const OPS: &[&[u8]] = &[
                b"<=>", b"<<=", b">>=", b"->*", b"...", b"::", b"->", b".*", b"&&", b"||", b"<<",
                b">>", b"<=", b">=", b"==", b"!=", b"+=", b"-=", b"*=", b"/=", b"%=", b"++", b"--",
                b"&=", b"|=", b"^=", b"##",
            ];
            if let Some(op) = OPS.iter().find(|op| b[i..].starts_with(op)) {
                kind = Kind::Operator;
                i += op.len();
            } else {
                kind = Kind::Punctuation;
                i += 1;
            }
        }
        // A malformed raw prefix may stop at a non-ASCII byte; keep all spans slice-safe.
        while !source.is_char_boundary(i) {
            i += 1;
        }
        out.tokens.push(Token {
            kind,
            start,
            end: i,
            line: sl,
            column: sc,
        });
        if let Some(message) = error {
            out.diagnostics.push(Diagnostic {
                code: "LEX001",
                message: message.into(),
                start,
                end: i,
                line: sl,
                column: sc,
            });
        }
        for c in source[start..i].chars() {
            if c == '\n' {
                line += 1;
                column = 1;
                line_start = true;
            } else {
                column += 1;
                if !c.is_whitespace() && kind != Kind::Comment {
                    line_start = false;
                }
            }
        }
    }
    if !conditional_stack.is_empty() {
        out.diagnostics.push(Diagnostic {
            code: "PP001",
            message: "unterminated preprocessor conditional".into(),
            start: i,
            end: i,
            line,
            column,
        });
    }
    out.pairs = vec![None; out.tokens.len()];
    if !out.conditional {
        let mut stack = Vec::new();
        for (index, token) in out.tokens.iter().enumerate() {
            if token.kind != Kind::Punctuation {
                continue;
            }
            let text = &source[token.start..token.end];
            if matches!(text, "(" | "[" | "{") {
                stack.push((index, text));
            } else if matches!(text, ")" | "]" | "}") {
                if let Some(&(open, opening)) = stack.last() {
                    if matches!((opening, text), ("(", ")") | ("[", "]") | ("{", "}")) {
                        stack.pop();
                        out.pairs[open] = Some(index);
                        out.pairs[index] = Some(open);
                        continue;
                    }
                }
                out.diagnostics
                    .push(delimiter_error(token, "unmatched closing delimiter"));
            }
        }
        for (index, _) in stack {
            out.diagnostics
                .push(delimiter_error(&out.tokens[index], "unclosed delimiter"));
        }
    }
    out
}
fn delimiter_error(t: &Token, message: &str) -> Diagnostic {
    Diagnostic {
        code: "DELIM001",
        message: message.into(),
        start: t.start,
        end: t.end,
        line: t.line,
        column: t.column,
    }
}
fn literal_prefix(b: &[u8]) -> Option<(usize, bool, u8)> {
    for p in [
        "u8R\"", "uR\"", "UR\"", "LR\"", "R\"", "u8\"", "u\"", "U\"", "L\"", "u8'", "u'", "U'",
        "L'", "\"", "'",
    ] {
        if b.starts_with(p.as_bytes()) {
            return Some((p.len(), p.contains('R'), *p.as_bytes().last().unwrap()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literals_preserve_structure_and_utf8() {
        let s = "void f() { auto x = u8R\"tag({é})tag\"; /* } */ char c = '\\''; }";
        let l = lex(s);
        assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
        assert_eq!(
            l.tokens
                .iter()
                .map(|t| &s[t.start..t.end])
                .collect::<String>(),
            s
        );
        assert_eq!(l.pairs.iter().flatten().count(), 4);
    }
    #[test]
    fn refuses_cross_branch_pairs() {
        let l = lex("#if A\n{\n#else\n{\n#endif\n}\n");
        assert!(l.conditional);
        assert!(l.pairs.iter().all(Option::is_none));
        assert!(l.diagnostics.is_empty());
    }
    #[test]
    fn malformed_inputs_are_diagnostics() {
        for s in ["/*", "\"oops", "R\"tag(no end", "{]", "#endif\n", "#if X\n"] {
            assert!(!lex(s).diagnostics.is_empty(), "{s}");
        }
    }
}
