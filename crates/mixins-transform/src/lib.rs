//! Resolve every edit against immutable original spans, then validate the whole
//! plan. This crate performs no filesystem writes and knows nothing about DDNet.
use mixins_lexer::{Kind, lex};
use mixins_parser::{Span, parse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Insert,
    Replace,
    Remove,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Position {
    Before,
    After,
    BodyStart,
    BodyEnd,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Function {
        name: String,
        parameters: Option<String>,
    },
    Class {
        name: String,
    },
    Comment {
        text: String,
    },
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub id: String,
    pub file: String,
    pub action: Action,
    pub target: Target,
    pub position: Option<Position>,
    pub content: Option<String>,
    #[serde(default)]
    pub priority: i32,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InjectionKind {
    Head,
    Tail,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Injection {
    pub id: String,
    pub file: String,
    pub target: String,
    pub parameters: Option<String>,
    pub kind: InjectionKind,
    pub content: String,
    #[serde(default)]
    pub priority: i32,
}
impl Injection {
    pub fn lower(&self) -> Edit {
        Edit {
            id: self.id.clone(),
            file: self.file.clone(),
            action: Action::Insert,
            target: Target::Function {
                name: self.target.clone(),
                parameters: self.parameters.clone(),
            },
            position: Some(match self.kind {
                InjectionKind::Head => Position::BodyStart,
                InjectionKind::Tail => Position::BodyEnd,
            }),
            content: Some(self.content.clone()),
            priority: self.priority,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Patch {
    pub mod_id: String,
    pub edit_id: String,
    pub span: Span,
    pub content: String,
}
#[derive(Debug, Serialize)]
pub struct Plan {
    pub patches: Vec<Patch>,
}

impl Plan {
    /// Input order is dependency order, then ascending priority and stable IDs.
    /// Equal-offset inserts appear in that same order in output.
    pub fn resolve(source: &str, edits: &[(&str, &Edit)]) -> Result<Self, String> {
        let lexed = lex(source);
        let index = parse(source, &lexed);
        let mut patches = Vec::new();
        for &(mod_id, edit) in edits {
            let resolve = || -> Result<Patch, String> {
                let (span, body) = match &edit.target {
                    Target::Function { name, parameters } => {
                        let f = index.select_function(name, parameters.as_deref(), source)?;
                        (f.declaration, Some(f.body))
                    }
                    Target::Class { name } => {
                        if !index.diagnostics.is_empty() || !index.unsupported.is_empty() {
                            return Err("class edits require a completely supported structural file in this version".into());
                        }
                        let found: Vec<_> = index
                            .classes
                            .iter()
                            .filter(|c| &c.qualified_name == name)
                            .collect();
                        match found.as_slice() {
                            [c] => (c.body, Some(c.body)),
                            _ => return Err(format!("class {name}: {} matches", found.len())),
                        }
                    }
                    Target::Comment { text } => {
                        if !lexed.diagnostics.is_empty() || text.is_empty() {
                            return Err(
                                "comment selection requires valid tokens and nonempty text".into(),
                            );
                        }
                        let found: Vec<_> = lexed
                            .tokens
                            .iter()
                            .filter(|t| {
                                t.kind == Kind::Comment && source[t.start..t.end].contains(text)
                            })
                            .collect();
                        match found.as_slice() {
                            [t] => (
                                Span {
                                    start: t.start,
                                    end: t.end,
                                },
                                None,
                            ),
                            _ => return Err(format!("comment {text:?}: {} matches", found.len())),
                        }
                    }
                };
                let content = match edit.action {
                    Action::Remove => {
                        if edit.content.is_some() {
                            return Err("REMOVE must not specify content".into());
                        }
                        String::new()
                    }
                    _ => edit
                        .content
                        .clone()
                        .ok_or("INSERT/REPLACE requires content")?,
                };
                let span = match edit.action {
                    Action::Insert => {
                        if matches!(edit.target, Target::Class { .. })
                            && matches!(edit.position, Some(Position::Before | Position::After))
                        {
                            return Err("class BEFORE/AFTER requires full declaration spans, not implemented yet; use BODY_START/BODY_END".into());
                        }
                        let offset =
                            match edit.position.as_ref().ok_or("INSERT requires position")? {
                                Position::Before => span.start,
                                Position::After => span.end,
                                Position::BodyStart => body.ok_or("target has no body")?.start + 1,
                                Position::BodyEnd => body.ok_or("target has no body")?.end - 1,
                            };
                        Span {
                            start: offset,
                            end: offset,
                        }
                    }
                    _ => {
                        if edit.position.is_some() {
                            return Err("REPLACE/REMOVE must not specify position".into());
                        }
                        // Class spans currently describe bodies only: replacing/removing
                        // them could leave an invalid class declaration. Fail explicitly.
                        if matches!(edit.target, Target::Class { .. }) {
                            return Err("class REPLACE/REMOVE not supported yet".into());
                        }
                        span
                    }
                };
                Ok(Patch {
                    mod_id: mod_id.into(),
                    edit_id: edit.id.clone(),
                    span,
                    content,
                })
            };
            patches.push(resolve().map_err(|e| {
                format!(
                    "mod {mod_id}, edit {}, file {}, target {:?}: {e}",
                    edit.id, edit.file, edit.target
                )
            })?);
        }
        for (i, a) in patches.iter().enumerate() {
            for b in &patches[i + 1..] {
                let ai = a.span.start == a.span.end;
                let bi = b.span.start == b.span.end;
                let conflict = if ai && bi {
                    false
                } else if ai {
                    b.span.start <= a.span.start && a.span.start <= b.span.end
                } else if bi {
                    a.span.start <= b.span.start && b.span.start <= a.span.end
                } else {
                    a.span.start < b.span.end && b.span.start < a.span.end
                };
                if conflict {
                    return Err(format!(
                        "overlapping edits: {}/{} and {}/{}",
                        a.mod_id, a.edit_id, b.mod_id, b.edit_id
                    ));
                }
            }
        }
        patches.sort_by_key(|p| p.span.start); // stable: preserve input order for equal offsets
        Ok(Self { patches })
    }
    /// No formatting or newline normalization: snippets are inserted verbatim.
    pub fn render(&self, source: &str) -> Result<String, String> {
        let mut result = String::new();
        let mut offset = 0;
        for p in &self.patches {
            if p.span.start < offset
                || p.span.end < p.span.start
                || !source.is_char_boundary(p.span.start)
                || !source.is_char_boundary(p.span.end)
            {
                return Err("invalid or conflicting patch spans".into());
            }
            result.push_str(&source[offset..p.span.start]);
            result.push_str(&p.content);
            offset = p.span.end;
        }
        result.push_str(&source[offset..]);
        Ok(result)
    }
}
