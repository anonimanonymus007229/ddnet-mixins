//! Common table-array filtering runs before typed rule loading and file reads.
use mixins_core::progress::{Event, Observer};
use serde::Deserialize;
use std::path::Path;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Clients {
    #[serde(default)]
    include: Option<Vec<String>>,
    #[serde(default)]
    exclude: Vec<String>,
}
fn matches(pattern: &str, name: &str) -> bool {
    let p: Vec<_> = pattern.chars().collect();
    let n: Vec<_> = name.chars().collect();
    let mut row = vec![false; n.len() + 1];
    row[0] = true;
    for c in p {
        let mut next = vec![false; n.len() + 1];
        if c == '*' {
            next[0] = row[0];
        }
        for j in 1..=n.len() {
            next[j] = if c == '*' {
                row[j] || next[j - 1]
            } else {
                row[j - 1] && (c == '?' || c == n[j - 1])
            };
        }
        row = next;
    }
    row[n.len()]
}
pub fn folder(ddnet: &Path) -> Result<String, String> {
    ddnet
        .canonicalize()
        .map_err(|e| format!("{}: {e}", ddnet.display()))?
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .ok_or("DDNet directory needs a UTF-8 name".into())
}
pub fn filter(
    value: &mut toml::Value,
    folder: Option<&str>,
    path: &str,
    observe: &mut Observer<'_>,
) -> Result<(), String> {
    match value {
        toml::Value::Table(t) => {
            for (key, child) in t.iter_mut() {
                filter(child, folder, &format!("{path}.{key}"), observe)?;
            }
        }
        toml::Value::Array(items) => {
            let mut kept = vec![];
            for (n, mut item) in std::mem::take(items).into_iter().enumerate() {
                let label = format!("{path}[{}]", n + 1);
                if let Some(t) = item.as_table_mut() {
                    if let Some(clients) = t.remove("clients") {
                        let clients: Clients = clients
                            .try_into()
                            .map_err(|e| format!("{label}.clients: {e}"))?;
                        for pattern in clients
                            .include
                            .iter()
                            .flatten()
                            .chain(clients.exclude.iter())
                        {
                            if pattern.is_empty()
                                || pattern
                                    .chars()
                                    .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
                            {
                                return Err(format!("{label}: invalid folder pattern {pattern:?}"));
                            }
                        }
                        let name = folder.ok_or_else(|| {
                            format!("{label}: clients filter requires --ddnet / a DDNet context")
                        })?;
                        let included = clients
                            .include
                            .as_ref()
                            .is_none_or(|patterns| patterns.iter().any(|p| matches(p, name)));
                        let excluded = clients.exclude.iter().any(|p| matches(p, name));
                        if !included || excluded {
                            observe(Event::Skipped(format!(
                                "{label} (id {}): clients filter excludes DDNet folder {name}",
                                t.get("id").and_then(|v| v.as_str()).unwrap_or("unnamed")
                            )));
                            continue;
                        }
                        observe(Event::Detail(format!(
                            "{label}: clients filter includes DDNet folder {name}"
                        )));
                    }
                }
                filter(&mut item, folder, &label, observe)?;
                kept.push(item);
            }
            *items = kept;
        }
        _ => {}
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    register_in: Vec<String>,
    members: Vec<Member>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    class: String,
    header: String,
    member: String,
}
pub fn expand(value: &mut toml::Value) -> Result<(), String> {
    let table = value.as_table_mut().ok_or("manifest must be a table")?;
    let Some(groups) = table.remove("components") else {
        return Ok(());
    };
    let groups = groups
        .as_array()
        .ok_or("components must use [[components]]")?;
    let mut expanded = vec![];
    for group in groups {
        let group:Group=group.clone().try_into().map_err(|e|format!("[[components]]: {e}; use members=[{{class,header,member}}], or [[component]] for a priority"))?;
        if group.members.is_empty() {
            return Err("[[components]] members cannot be empty".into());
        }
        for m in group.members {
            expanded.push(
                toml::Value::try_from(super::Component {
                    class: m.class,
                    header: m.header,
                    member: m.member,
                    register_in: group.register_in.clone(),
                    priority: 0,
                })
                .map_err(|e| e.to_string())?,
            );
        }
    }
    let singles = table
        .entry("component")
        .or_insert_with(|| toml::Value::Array(vec![]))
        .as_array_mut()
        .ok_or("component must use [[component]]")?;
    // Group order and member order are retained; explicit priorities still sort.
    expanded.append(singles);
    *singles = expanded;
    Ok(())
}
