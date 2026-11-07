//! Helpers are internal free functions in the target translation unit. All
//! argument access is explicit; no privileged access to target members/locals.
use crate::{Action, Binding, Edit, Injection, InjectionKind, Position, Target};
use mixins_lexer::{Kind, lex};
use mixins_parser::{parse, syntax::Syntax};
use std::collections::BTreeSet;
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
}
pub fn validate(bindings: &[Binding], body: &str) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let keywords = [
        "this",
        "auto",
        "class",
        "struct",
        "union",
        "return",
        "if",
        "else",
        "for",
        "while",
        "switch",
        "case",
        "default",
        "void",
        "int",
        "float",
        "double",
        "bool",
        "char",
        "short",
        "long",
        "unsigned",
        "signed",
        "const",
        "volatile",
        "static",
        "public",
        "private",
        "protected",
        "template",
        "typename",
        "new",
        "delete",
        "operator",
        "namespace",
        "using",
        "virtual",
        "override",
        "final",
        "true",
        "false",
        "nullptr",
        "try",
        "catch",
        "throw",
        "noexcept",
        "sizeof",
        "decltype",
        "alignof",
        "do",
        "break",
        "continue",
        "goto",
        "extern",
        "inline",
        "constexpr",
        "consteval",
        "constinit",
        "thread_local",
        "mutable",
        "enum",
        "friend",
        "typedef",
        "register",
        "asm",
        "co_return",
        "co_await",
        "co_yield",
        "and",
        "or",
        "not",
        "xor",
        "and_eq",
        "or_eq",
        "xor_eq",
        "bitand",
        "bitor",
        "compl",
        "not_eq",
        "requires",
        "concept",
        "export",
        "explicit",
        "static_assert",
        "typeid",
        "dynamic_cast",
        "static_cast",
        "reinterpret_cast",
        "const_cast",
        "wchar_t",
        "char8_t",
        "char16_t",
        "char32_t",
    ];
    for b in bindings {
        if !identifier(&b.name)
            || keywords.contains(&b.name.as_str())
            || b.name.contains("__")
            || b.name.starts_with('_')
            || !names.insert(&b.name)
        {
            return Err(format!(
                "invalid/reserved/duplicate binding name {:?}",
                b.name
            ));
        }
        let ty = lex(&b.ty);
        let ts: Vec<_> = ty
            .tokens
            .iter()
            .filter(|t| t.kind != Kind::Whitespace)
            .collect();
        if !ty.diagnostics.is_empty() || ts.is_empty() {
            return Err(format!("invalid binding type for {}", b.name));
        }
        let mut depth = 0usize;
        for t in ts {
            let w = &b.ty[t.start..t.end];
            match w {
                "<" => depth += 1,
                ">" => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or("unbalanced binding template type")?
                }
                ">>" => {
                    depth = depth
                        .checked_sub(2)
                        .ok_or("unbalanced binding template type")?
                }
                "," if depth > 0 => {}
                "::" | "*" | "&" | "&&" => {}
                _ if t.kind == Kind::Identifier => {}
                _ => {
                    return Err(format!(
                        "unsupported binding type {:?}; use a simple named/pointer/reference type or typedef",
                        b.ty
                    ));
                }
            }
        }
        if depth != 0 {
            return Err("unbalanced binding template type".into());
        }
        let vl = lex(&b.value);
        let vs = Syntax::new(&b.value, &vl)?;
        if vs.tokens.is_empty()
            || vl
                .tokens
                .iter()
                .any(|t| matches!(t.kind, Kind::Directive | Kind::Comment))
        {
            return Err(format!("empty/unsafe binding value for {}", b.name));
        }
        if (0..vs.tokens.len()).any(|p| matches!(vs.word(p), ";" | "{" | "}")) {
            return Err("binding value must be an expression, not a statement/block".into());
        }
    }
    let l = lex(body);
    let syntax = Syntax::new(body, &l)?;
    if l.tokens.iter().any(|t| t.kind == Kind::Directive) {
        return Err("preprocessor directives in helper bodies are unsupported; place includes in the target source".into());
    }
    for p in 0..syntax.tokens.len() {
        if syntax.word(p) == "this" {
            return Err("free helper has no this; pass a named binding such as controls".into());
        }
        if syntax.word(p) == "return" && (p + 1 >= syntax.tokens.len() || syntax.word(p + 1) != ";")
        {
            return Err("void helper may only use return; (no return value)".into());
        }
        if matches!(
            syntax.word(p),
            "co_return" | "co_await" | "co_yield" | "goto"
        ) {
            return Err("coroutines/goto in helper bodies unsupported".into());
        }
    }
    Ok(())
}
fn encode(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
pub fn lower(mod_id: &str, i: &Injection, source: &str) -> Result<Vec<Edit>, String> {
    if i.helper.is_none() {
        return Ok(vec![i.lower()?]);
    }
    if !matches!(i.kind, InjectionKind::Head | InjectionKind::Tail) {
        return Err("helper currently requires HEAD or TAIL".into());
    }
    if !i.file.ends_with(".cpp") {
        return Err("helpers require a .cpp target translation unit".into());
    }
    validate(&i.bindings, &i.helper_content)?;
    let l = lex(source);
    let index = parse(source, &l);
    let f = index.select_function(&i.target, i.parameters.as_deref(), source)?;
    if f.conditional {
        return Err("helper target must not be conditionally defined".into());
    }
    if l.tokens.iter().any(|t| {
        t.start >= f.declaration.start
            && t.end <= f.body.start
            && t.kind == Kind::Identifier
            && &source[t.start..t.end] == "template"
    }) {
        return Err("helper targets with template declaration heads are unsupported".into());
    }
    let st = mixins_parser::structure(source, &l);
    let body_token = l
        .tokens
        .iter()
        .position(|t| t.start == f.body.start)
        .ok_or("target body token missing")?;
    if st.conditional[body_token] {
        return Err("helper target conditional region unsupported".into());
    }
    // A definition may not be injected inside a class/namespace/extern block:
    // members would gain access privileges; namespaces would change type lookup.
    for (n, t) in l.tokens.iter().enumerate() {
        if t.start >= f.declaration.start {
            break;
        }
        if t.kind == Kind::Punctuation
            && &source[t.start..t.end] == "{"
            && st.pairs[n].is_some_and(|end| l.tokens[end].end > f.declaration.end)
        {
            return Err(
                "helper target must be an out-of-class definition at translation-unit scope".into(),
            );
        }
    }
    let name = format!("ddmix_helper_m{}_i{}", encode(mod_id), encode(&i.id));
    if l.tokens
        .iter()
        .any(|t| t.kind == Kind::Identifier && source[t.start..t.end] == name)
    {
        return Err("generated helper name already exists in input".into());
    }
    let nl = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let params = i
        .bindings
        .iter()
        .map(|b| format!("{} {}", b.ty, b.name))
        .collect::<Vec<_>>()
        .join(", ");
    let args = i
        .bindings
        .iter()
        .map(|b| format!("({})", b.value))
        .collect::<Vec<_>>()
        .join(", ");
    let definition = format!(
        "{nl}// DDMIX helper {mod_id}/{}{nl}static void {name}({params}){nl}{{{nl}{}{nl}}}{nl}",
        i.id, i.helper_content
    );
    // BEFORE selects the original definition span through the same generic
    // selector. Its out-of-class location is checked above before generation.
    let definition = Edit {
        id: format!("{}-helper-definition", i.id),
        file: i.file.clone(),
        action: Action::Insert,
        target: Target::Function {
            name: i.target.clone(),
            parameters: i.parameters.clone(),
        },
        position: Some(Position::Before),
        content: Some(definition),
        priority: i.priority,
    };
    let mut call = i.clone();
    call.helper = None;
    call.content = format!("{nl}\t{name}({args});{nl}");
    let call = call.lower()?;
    Ok(vec![definition, call])
}
