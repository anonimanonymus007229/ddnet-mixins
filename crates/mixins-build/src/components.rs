use mixins_lexer::{Kind, lex};
use mixins_manifest::{Component, Mod};
use mixins_parser::{Span, parse, syntax::Syntax};
use mixins_transform::{Action, Edit, Position, Target};
use std::{fs, path::Path};

pub fn lower(ddnet: &Path, m: &Mod, c: &Component) -> Result<Vec<Edit>, String> {
    let header = m.root.join("src").join(&c.header);
    if !m.files.iter().any(|f| f.physical == header) {
        return Err(format!(
            "component header is not a discovered mod file: {}",
            header.display()
        ));
    }
    let source = fs::read_to_string(&header).map_err(|e| e.to_string())?;
    let l = lex(&source);
    let index = parse(&source, &l);
    let classes: Vec<_> = index
        .classes
        .iter()
        .filter(|n| n.qualified_name == c.class)
        .collect();
    let [class] = classes.as_slice() else {
        return Err(format!("component class {} missing/ambiguous", c.class));
    };
    let syn = Syntax::new(&source, &l)?;
    // Require an actual direct public CComponent base, not an incidental mention.
    syn.sequence(
        &[
            "class".into(),
            c.class.clone(),
            ":".into(),
            "public".into(),
            "CComponent".into(),
            "{".into(),
        ],
        Span {
            start: 0,
            end: class.body.start + 1,
        },
        false,
    )?;
    let h = fs::read_to_string(ddnet.join("src/game/client/gameclient.h"))
        .map_err(|e| e.to_string())?;
    let hl = lex(&h);
    let hi = parse(&h, &hl);
    let found: Vec<_> = hi
        .classes
        .iter()
        .filter(|n| n.qualified_name == "CGameClient")
        .collect();
    let [gameclient] = found.as_slice() else {
        return Err("CGameClient missing/ambiguous".into());
    };
    let hs = Syntax::new(&h, &hl)?;
    // Tokens in nested scopes do not affect selecting the first access label.
    let body = gameclient.body;
    let p = hs
        .tokens
        .iter()
        .position(|&i| hl.tokens[i].start == body.start)
        .ok_or("class body absent")?
        + 1;
    if hs.word(p) != "public" || hs.word(p + 1) != ":" {
        return Err(
            "CGameClient does not begin with public access; registration layout unsupported".into(),
        );
    }
    if hl
        .tokens
        .iter()
        .any(|t| t.kind == Kind::Identifier && h[t.start..t.end] == c.member)
    {
        return Err(format!("component member {} already exists", c.member));
    }
    // Unique token sequence includes the opening class and public label.
    let words = vec![
        "class".into(),
        "CGameClient".into(),
        ":".into(),
        "public".into(),
        "IGameClient".into(),
        "{".into(),
        "public".into(),
        ":".into(),
    ];
    hs.sequence(
        &words,
        Span {
            start: 0,
            end: h.len(),
        },
        false,
    )?;
    let make = |id: &str, file: &str, target, position, content| Edit {
        id: format!(
            "component-{}-{id}",
            c.member.to_ascii_lowercase().replace('_', "-")
        ),
        file: file.into(),
        action: Action::Insert,
        target,
        position: Some(position),
        content: Some(content),
        priority: c.priority,
    };
    let nl = if h.contains("\r\n") { "\r\n" } else { "\n" };
    let mut edits = vec![
        make(
            "include",
            "src/game/client/gameclient.h",
            Target::File,
            Position::Start,
            format!("#include <{}>{nl}", c.header),
        ),
        make(
            "member",
            "src/game/client/gameclient.h",
            Target::Tokens {
                words,
                within: None,
                block: false,
            },
            Position::After,
            format!("{nl}\t{} {};", c.class, c.member),
        ),
    ];
    for category in &c.register_in {
        let collection = if category == "all" {
            "m_vpAll"
        } else {
            "m_vpInput"
        };
        let words = [
            collection, ".", "insert", "(", collection, ".", "end", "(", ")", ",", "{",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        // Select initializer-list brace inside the known lifecycle function;
        // existing order is retained, new components append in manifest order.
        edits.push(make(
            category,
            "src/game/client/gameclient.cpp",
            Target::Tokens {
                words,
                within: Some("CGameClient::OnConsoleInit".into()),
                block: true,
            },
            Position::BodyEnd,
            format!(",{nl}\t\t&{}", c.member),
        ));
    }
    Ok(edits)
}
