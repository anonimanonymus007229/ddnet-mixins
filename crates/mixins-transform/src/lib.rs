//! Resolve every edit against immutable original spans, then validate the whole
//! plan. This crate performs no filesystem writes and knows nothing about DDNet.
pub mod helpers;
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
    Start,
    End,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    File,
    Tokens {
        words: Vec<String>,
        within: Option<String>,
        #[serde(default)]
        block: bool,
    },
    Body {
        name: String,
        parameters: Option<String>,
    },
    ReturnExpression {
        function: String,
        parameters: Option<String>,
    },
    Call {
        function: String,
        parameters: Option<String>,
        callee: String,
        argument: Option<usize>,
    },
    MacroBody {
        name: String,
        arguments: Option<Vec<String>>,
    },
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
    BeforeCall,
    AfterCall,
    ModifyArg,
    Return,
    Overwrite,
    Wrap,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(try_from = "InjectionInput", into = "InjectionInput")]
pub struct Injection {
    pub id: String,
    pub file: String,
    pub target: String,
    pub parameters: Option<String>,
    pub kind: InjectionKind,
    /// Resolved snippet text. Manifest loading reads content_file before lowering.
    pub content: String,
    pub content_file: Option<String>,
    pub helper: Option<String>,
    pub bindings: Vec<Binding>,
    pub helper_content: String,
    #[serde(default)]
    pub priority: i32,
    pub callee: Option<String>,
    pub argument: Option<usize>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub name: String,
    pub ty: String,
    pub value: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InjectionInput {
    id: String,
    file: String,
    target: String,
    parameters: Option<String>,
    kind: InjectionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    helper: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bindings: Vec<Binding>,
    #[serde(default)]
    priority: i32,
    callee: Option<String>,
    argument: Option<usize>,
}
impl From<Injection> for InjectionInput {
    fn from(i: Injection) -> Self {
        Self {
            id: i.id,
            file: i.file,
            target: i.target,
            parameters: i.parameters,
            kind: i.kind,
            content: if i.content_file.is_none() && i.helper.is_none() {
                Some(i.content)
            } else {
                None
            },
            content_file: i.content_file,
            helper: i.helper,
            bindings: i.bindings,
            priority: i.priority,
            callee: i.callee,
            argument: i.argument,
        }
    }
}
impl TryFrom<InjectionInput> for Injection {
    type Error = String;
    fn try_from(input: InjectionInput) -> Result<Self, Self::Error> {
        if usize::from(input.content.is_some())
            + usize::from(input.content_file.is_some())
            + usize::from(input.helper.is_some())
            != 1
        {
            return Err(format!(
                "injection {}: specify exactly one of content or content_file or helper",
                input.id
            ));
        }
        if input.helper.is_none() && !input.bindings.is_empty() {
            return Err(format!("injection {}: bindings require helper", input.id));
        }
        Ok(Self {
            id: input.id,
            file: input.file,
            target: input.target,
            parameters: input.parameters,
            kind: input.kind,
            content: input.content.unwrap_or_default(),
            content_file: input.content_file,
            helper: input.helper,
            bindings: input.bindings,
            helper_content: String::new(),
            priority: input.priority,
            callee: input.callee,
            argument: input.argument,
        })
    }
}
impl Injection {
    pub fn lower(&self) -> Result<Edit, String> {
        if self.helper.is_some() {
            return Err("helper requires source-aware lowering".into());
        }
        let body = Target::Body {
            name: self.target.clone(),
            parameters: self.parameters.clone(),
        };
        let call = Target::Call {
            function: self.target.clone(),
            parameters: self.parameters.clone(),
            callee: self.callee.clone().unwrap_or_default(),
            argument: self.argument,
        };
        let (action, target, position) = match self.kind {
            InjectionKind::Head => (
                Action::Insert,
                Target::Function {
                    name: self.target.clone(),
                    parameters: self.parameters.clone(),
                },
                Some(Position::BodyStart),
            ),
            InjectionKind::Tail => (
                Action::Insert,
                Target::Function {
                    name: self.target.clone(),
                    parameters: self.parameters.clone(),
                },
                Some(Position::BodyEnd),
            ),
            InjectionKind::BeforeCall => (Action::Insert, call, Some(Position::Before)),
            InjectionKind::AfterCall => (Action::Insert, call, Some(Position::After)),
            InjectionKind::ModifyArg => (Action::Replace, call, None),
            InjectionKind::Return => (
                Action::Replace,
                Target::ReturnExpression {
                    function: self.target.clone(),
                    parameters: self.parameters.clone(),
                },
                None,
            ),
            InjectionKind::Overwrite | InjectionKind::Wrap => (Action::Replace, body, None),
        };
        Ok(Edit {
            id: self.id.clone(),
            file: self.file.clone(),
            action,
            target,
            position,
            content: Some(self.content.clone()),
            priority: self.priority,
        })
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
                let syntax = || mixins_parser::syntax::Syntax::new(source, &lexed);
                let (span, body) = match &edit.target {
                    Target::File => (
                        Span {
                            start: 0,
                            end: source.len(),
                        },
                        None,
                    ),
                    Target::Tokens {
                        words,
                        within,
                        block,
                    } => {
                        let within = if let Some(name) = within {
                            index.select_function(name, None, source)?.body
                        } else {
                            Span {
                                start: 0,
                                end: source.len(),
                            }
                        };
                        let selected = syntax()?.sequence(words, within, *block)?;
                        (selected, if *block { Some(selected) } else { None })
                    }
                    Target::Body { name, parameters } => {
                        let b = index
                            .select_function(name, parameters.as_deref(), source)?
                            .body;
                        (
                            Span {
                                start: b.start + 1,
                                end: b.end - 1,
                            },
                            Some(b),
                        )
                    }
                    Target::ReturnExpression {
                        function,
                        parameters,
                    } => {
                        let b = index
                            .select_function(function, parameters.as_deref(), source)?
                            .body;
                        (syntax()?.return_expression(b)?, None)
                    }
                    Target::Call {
                        function,
                        parameters,
                        callee,
                        argument,
                    } => {
                        let b = index
                            .select_function(function, parameters.as_deref(), source)?
                            .body;
                        let (call, args) = syntax()?.call(b, callee)?;
                        let selected = if let Some(n) = argument {
                            *args.get(*n).ok_or("call argument index out of range")?
                        } else {
                            call
                        };
                        (selected, None)
                    }
                    Target::MacroBody { name, arguments } => {
                        if !index.diagnostics.is_empty() {
                            return Err(
                                "macro body selection requires valid structural source".into()
                            );
                        }
                        let found: Vec<_> = index
                            .macro_bodies
                            .iter()
                            .filter(|node| {
                                &node.name == name
                                    && arguments.as_ref().is_none_or(|args| {
                                        args.len() == node.arguments.len()
                                            && args.iter().zip(&node.arguments).all(|(a, s)| {
                                                a.trim() == source[s.start..s.end].trim()
                                            })
                                    })
                            })
                            .collect();
                        match found.as_slice() {
                            [node] => (node.declaration, Some(node.body)),
                            _ => {
                                return Err(format!(
                                    "macro body {name}: {} matches; specify arguments",
                                    found.len()
                                ));
                            }
                        }
                    }
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
                let mut content = match edit.action {
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
                if content.contains("{{original}}") {
                    if !matches!(edit.target, Target::Body { .. })
                        || !matches!(edit.action, Action::Replace)
                        || content.matches("{{original}}").count() != 1
                    {
                        return Err(
                            "{{original}} requires exactly one placeholder in a body replacement"
                                .into(),
                        );
                    }
                    // An explicit nested original block, not a renamed function:
                    // allow only bodies without return/goto so post-code executes.
                    let fragment = lex(&source[span.start..span.end]);
                    if fragment.tokens.iter().any(|t| {
                        t.kind == Kind::Identifier
                            && matches!(
                                &source[span.start + t.start..span.start + t.end],
                                "return" | "goto" | "co_return"
                            )
                    }) {
                        return Err(
                            "WRAP original block cannot contain return/goto/co_return".into()
                        );
                    }
                    content = content.replace(
                        "{{original}}",
                        &format!("{{{}}}", &source[span.start..span.end]),
                    );
                }
                let span = match edit.action {
                    Action::Insert => {
                        if matches!(edit.target, Target::Class { .. })
                            && matches!(edit.position, Some(Position::Before | Position::After))
                        {
                            return Err("class BEFORE/AFTER requires full declaration spans, not implemented yet; use BODY_START/BODY_END".into());
                        }
                        let offset =
                            match edit.position.as_ref().ok_or("INSERT requires position")? {
                                Position::Start if matches!(edit.target, Target::File) => 0,
                                Position::End if matches!(edit.target, Target::File) => {
                                    source.len()
                                }
                                Position::Start | Position::End => {
                                    return Err("START/END currently require file target".into());
                                }
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
                    b.span.start < a.span.start && a.span.start < b.span.end
                } else if bi {
                    a.span.start < b.span.start && b.span.start < a.span.end
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
        // Boundary inserts precede a replacement starting at the same offset;
        // inserts at its end follow it. Stable within equal insert positions.
        patches.sort_by_key(|p| (p.span.start, usize::from(p.span.start != p.span.end)));
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
