//! Minimal project-owned CMake command/block scanner. No line-number anchors.
use mixins_parser::Span;
use mixins_transform::{Patch, Plan};

#[derive(Debug)]
struct Command {
    name: String,
    args: Vec<String>,
    start: usize,
    end: usize,
    blocks: Vec<usize>,
}
fn bracket(b: &[u8], i: usize) -> Option<(usize, usize)> {
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let mut j = i + 1;
    while b.get(j) == Some(&b'=') {
        j += 1;
    }
    (b.get(j) == Some(&b'[')).then_some((j + 1, j - i - 1))
}
fn skip_bracket(b: &[u8], i: usize) -> Result<usize, String> {
    let (mut p, equals) = bracket(b, i).ok_or("invalid bracket argument")?;
    while p < b.len() {
        if b[p] == b']'
            && b.get(p + 1..p + 1 + equals)
                .is_some_and(|s| s.iter().all(|c| *c == b'='))
            && b.get(p + 1 + equals) == Some(&b']')
        {
            return Ok(p + equals + 2);
        }
        p += 1;
    }
    Err("unterminated CMake bracket argument/comment".into())
}
fn trivia(b: &[u8], mut p: usize) -> Result<usize, String> {
    loop {
        while b.get(p).is_some_and(u8::is_ascii_whitespace) {
            p += 1;
        }
        if b.get(p) != Some(&b'#') {
            return Ok(p);
        }
        if bracket(b, p + 1).is_some() {
            p = skip_bracket(b, p + 1)?;
        } else {
            while p < b.len() && b[p] != b'\n' {
                p += 1;
            }
        }
    }
}
fn commands(source: &str) -> Result<Vec<Command>, String> {
    let b = source.as_bytes();
    let mut p = 0;
    let mut out = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    while {
        p = trivia(b, p)?;
        p < b.len()
    } {
        let start = p;
        while b
            .get(p)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            p += 1;
        }
        if p == start {
            return Err(format!("unsupported CMake syntax at byte {p}"));
        }
        let name = source[start..p].to_ascii_lowercase();
        p = trivia(b, p)?;
        if b.get(p) != Some(&b'(') {
            return Err(format!("expected '(' after {name}"));
        }
        p += 1;
        let mut depth = 1;
        let mut args = Vec::new();
        while depth > 0 {
            p = trivia(b, p)?;
            let Some(&c) = b.get(p) else {
                return Err("unclosed CMake command".into());
            };
            if c == b')' {
                depth -= 1;
                p += 1;
                continue;
            }
            if c == b'(' {
                depth += 1;
                p += 1;
                continue;
            }
            if bracket(b, p).is_some() {
                let end = skip_bracket(b, p)?;
                args.push(source[p..end].into());
                p = end;
                continue;
            }
            if c == b'"' {
                p += 1;
                let begin = p;
                while p < b.len() && b[p] != b'"' {
                    if b[p] == b'\\' {
                        p += 1;
                    }
                    p += 1;
                }
                if p >= b.len() {
                    return Err("unclosed CMake quoted argument".into());
                }
                args.push(source[begin..p].into());
                p += 1;
            } else {
                let begin = p;
                while p < b.len()
                    && !b[p].is_ascii_whitespace()
                    && !matches!(b[p], b'(' | b')' | b'#')
                {
                    if b[p] == b'\\' {
                        p += 1;
                    }
                    p += 1;
                }
                if p > b.len() {
                    return Err("truncated CMake escape".into());
                }
                args.push(source[begin..p].into());
            }
        }
        let i = out.len();
        if matches!(
            name.as_str(),
            "endif" | "endforeach" | "endwhile" | "endfunction" | "endmacro" | "endblock"
        ) {
            let open = stack.pop().ok_or("unmatched CMake block end")?;
            let expected = match name.as_str() {
                "endif" => "if",
                "endforeach" => "foreach",
                "endwhile" => "while",
                "endfunction" => "function",
                "endmacro" => "macro",
                _ => "block",
            };
            if out.get(open).is_some_and(|c: &Command| c.name != expected) {
                return Err("mismatched CMake blocks".into());
            }
            out.push(Command {
                name,
                args,
                start,
                end: p,
                blocks: vec![open],
            });
        } else {
            let opening = matches!(
                name.as_str(),
                "if" | "foreach" | "while" | "function" | "macro" | "block"
            );
            out.push(Command {
                name,
                args,
                start,
                end: p,
                blocks: stack.clone(),
            });
            if opening {
                stack.push(i);
            }
        }
    }
    if !stack.is_empty() {
        return Err("unclosed CMake block".into());
    }
    Ok(out)
}
fn insert(offset: usize, id: &str, content: String) -> Patch {
    Patch {
        mod_id: "ddmix".into(),
        edit_id: id.into(),
        span: Span {
            start: offset,
            end: offset,
        },
        content,
    }
}
/// Only accepts a pristine upstream CMake file; staging is regenerated each time.
pub fn integrate(source: &str, capacity: usize, extra_lines: usize) -> Result<String, String> {
    if source.contains("DDMIX-BEGIN") || source.contains("DDMIX-END") {
        return Err(
            "input CMake already contains managed markers; use the original checkout".into(),
        );
    }
    let cmds = commands(source)?;
    let creators: Vec<_> = cmds
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            matches!(c.name.as_str(), "add_library" | "add_executable")
                && c.args.first().is_some_and(|a| a == "game-client")
        })
        .collect();
    if creators.is_empty() {
        return Err("no literal game-client target creation found".into());
    }
    if creators
        .iter()
        .any(|(_, c)| c.blocks.iter().any(|&i| cmds[i].name != "if"))
    {
        return Err("target creation inside functions/loops is unsupported".into());
    }
    let placement = if creators.len() == 1 {
        creators[0].1.end
    } else {
        let common = creators[0]
            .1
            .blocks
            .iter()
            .rev()
            .find(|block| creators.iter().all(|(_, c)| c.blocks.contains(block)))
            .copied()
            .ok_or("game-client creations have no common conditional block")?;
        if cmds[common].name != "if" {
            return Err("target creation inside function/loop is unsupported".into());
        }
        let ending = cmds
            .iter()
            .find(|c| c.name == "endif" && c.blocks == [common])
            .ok_or("missing target-creation endif")?;
        // A common outer CLIENT block would be too late; require the creation
        // statements to be immediately inside the same platform conditional.
        if creators
            .iter()
            .any(|(_, c)| c.blocks.last() != Some(&common))
        {
            return Err("complex nested target creation unsupported".into());
        }
        let mut branches = std::collections::BTreeSet::new();
        for (i, _) in &creators {
            let branch = (common..*i)
                .rev()
                .find(|&j| {
                    matches!(cmds[j].name.as_str(), "else" | "elseif")
                        && cmds[j].blocks.last() == Some(&common)
                })
                .unwrap_or(common);
            if !branches.insert(branch) {
                return Err("multiple game-client creations in one conditional branch".into());
            }
        }
        ending.end
    };
    let checksum: Vec<_> = cmds
        .iter()
        .filter(|c| {
            c.name == "configure_file"
                && c.args
                    .first()
                    .is_some_and(|a| a == "cmake/checksummed_extra.txt")
        })
        .collect();
    let [checksum] = checksum.as_slice() else {
        return Err("expected unique checksum metadata configure_file anchor".into());
    };
    if checksum.start <= placement {
        return Err("checksum processing precedes target integration".into());
    }
    let nl = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let include="\n# DDMIX-BEGIN: generated integration\nif(NOT DEFINED DDMIX_WORKSPACE_ROOT)\n  message(FATAL_ERROR \"Set DDMIX_WORKSPACE_ROOT when configuring staged DDNet\")\nendif()\ninclude(\"${DDMIX_WORKSPACE_ROOT}/.ddmix/generated.cmake\")\n# DDMIX-END: generated integration\n".replace('\n',nl);
    let guard=format!("# DDMIX-BEGIN: checksum capacity\nlist(LENGTH CHECKSUM_SRC DDMIX_CHECKSUM_COUNT)\nmath(EXPR DDMIX_CHECKSUM_COUNT \"${{DDMIX_CHECKSUM_COUNT}} + {extra_lines}\")\nif(DDMIX_CHECKSUM_COUNT GREATER {capacity})\n  message(FATAL_ERROR \"Modded checksum metadata exceeds CChecksumData capacity\")\nendif()\n# DDMIX-END: checksum capacity\n").replace('\n',nl);
    Plan {
        patches: vec![
            insert(placement, "cmake-include", include),
            insert(checksum.start, "checksum-capacity", guard),
        ],
    }
    .render(source)
}
/// Quoted CMake argument, rejecting list/expression injection rather than
/// pretending paths with semicolons or generator expressions are portable.
pub fn quote_path(path: &str) -> Result<String, String> {
    if path
        .chars()
        .any(|c| matches!(c, ';' | '\n' | '\r' | '\0' | '$'))
    {
        return Err(format!("unsupported CMake path: {path:?}"));
    }
    Ok(format!(
        "\"{}\"",
        path.replace('\\', "/").replace('"', "\\\"")
    ))
}
pub fn generate(files: &[String], includes: &[String]) -> Result<String, String> {
    let mut files = files.to_vec();
    files.sort();
    files.dedup();
    let mut includes = includes.to_vec();
    includes.sort();
    includes.dedup();
    let mut s = String::from("# Generated by ddmix; do not edit.\nif(TARGET game-client)\n");
    if !files.is_empty() {
        s.push_str("  target_sources(game-client PRIVATE\n");
        for f in &files {
            s.push_str(&format!("    {}\n", quote_path(f)?));
        }
        s.push_str("  )\n");
        s.push_str("  target_include_directories(game-client PRIVATE\n");
        for f in &includes {
            s.push_str(&format!("    {}\n", quote_path(f)?));
        }
        s.push_str("  )\n");
        s.push_str("  if(NOT DEV)\n    list(APPEND CHECKSUM_SRC\n");
        for f in &files {
            s.push_str(&format!("      {}\n", quote_path(f)?));
        }
        s.push_str("    )\n    list(REMOVE_DUPLICATES CHECKSUM_SRC)\n  endif()\n");
    }
    s.push_str("endif()\n");
    Ok(s)
}
/// Read the actual fixed checksum array capacity using lexer tokens, not regex.
pub fn checksum_capacity(header: &str) -> Result<usize, String> {
    let l = mixins_lexer::lex(header);
    if !l.diagnostics.is_empty() {
        return Err("invalid checksum header".into());
    }
    let tokens: Vec<_> = l
        .tokens
        .iter()
        .filter(|t| {
            !matches!(
                t.kind,
                mixins_lexer::Kind::Whitespace | mixins_lexer::Kind::Comment
            )
        })
        .map(|t| &header[t.start..t.end])
        .collect();
    let found: Vec<_> = tokens
        .windows(4)
        .filter(|w| w[0] == "m_aFiles" && w[1] == "[" && w[3] == "]")
        .collect();
    match found.as_slice() {
        [w] => w[2]
            .parse()
            .map_err(|_| "unsupported checksum array size".into()),
        _ => Err("checksum array capacity missing or ambiguous".into()),
    }
}
