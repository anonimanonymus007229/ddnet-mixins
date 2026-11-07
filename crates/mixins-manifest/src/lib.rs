//! Strict initial manifest schema; unsupported transformations are never ignored.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(rename = "mod")]
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    pub compatibility: Option<Compatibility>,
    #[serde(default)]
    pub edits: Vec<mixins_transform::Edit>,
    #[serde(default)]
    pub injections: Vec<mixins_transform::Injection>,
    #[serde(default)]
    pub components: Vec<Component>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub class: String,
    pub header: String,
    pub member: String,
    pub register_in: Vec<String>,
    #[serde(default)]
    pub priority: i32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    pub ddnet: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
}
fn enabled() -> bool {
    true
}
#[derive(Debug, Serialize)]
pub struct Mod {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub files: Vec<Source>,
}
#[derive(Debug, Serialize)]
pub struct Source {
    pub physical: PathBuf,
    pub logical: String,
    pub target: String,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}
pub fn load(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut m: Manifest = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if !valid_id(&m.package.id)
        || m.package.name.trim().is_empty()
        || m.package.version.trim().is_empty()
    {
        return Err(format!(
            "{}: invalid mod ID or empty package metadata",
            path.display()
        ));
    }
    if m.compatibility.as_ref().is_some_and(|c| c.ddnet != "*")
        || m.dependencies.values().any(|v| v != "*")
    {
        return Err(format!(
            "{}: only wildcard (*) compatibility/dependency versions are currently supported",
            path.display()
        ));
    }
    let mut ids = BTreeSet::new();
    for (id, file) in m
        .edits
        .iter()
        .map(|e| (&e.id, &e.file))
        .chain(m.injections.iter().map(|e| (&e.id, &e.file)))
    {
        if !valid_id(id) || !ids.insert(id) {
            return Err(format!(
                "{}: invalid or duplicate edit ID {id}",
                path.display()
            ));
        }
        if file.contains('\\')
            || !file.starts_with("src/")
            || file
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == ".." || s.contains(':'))
        {
            return Err(format!(
                "{}: edit path must be a normalized src/... path: {file}",
                path.display()
            ));
        }
    }
    for i in &mut m.injections {
        if let Some(relative) = &i.content_file {
            i.content = read_hook(path, relative).map_err(|e| {
                format!(
                    "{}: mod {}, injection {}, content_file {relative:?}: {e}",
                    path.display(),
                    m.package.id,
                    i.id
                )
            })?;
        }
        use mixins_transform::InjectionKind as K;
        if let Some(relative) = &i.helper {
            i.helper_content = read_hook(path, relative).map_err(|e| {
                format!(
                    "{}: mod {}, injection {}, helper {relative:?}: {e}",
                    path.display(),
                    m.package.id,
                    i.id
                )
            })?;
            if !matches!(i.kind, K::Head | K::Tail) || !i.file.ends_with(".cpp") {
                return Err(format!(
                    "{}: injection {}: helper requires HEAD/TAIL and a .cpp target",
                    path.display(),
                    i.id
                ));
            }
            mixins_transform::helpers::validate(&i.bindings, &i.helper_content).map_err(|e| {
                format!(
                    "{}: mod {}, injection {}: {e}",
                    path.display(),
                    m.package.id,
                    i.id
                )
            })?;
        }
        let call = matches!(i.kind, K::BeforeCall | K::AfterCall | K::ModifyArg);
        if call != i.callee.is_some()
            || matches!(i.kind, K::ModifyArg) != i.argument.is_some()
            || (matches!(i.kind, K::Wrap) && i.content.matches("{{original}}").count() != 1)
        {
            return Err(format!(
                "{}: injection {} has invalid callee/argument/original placeholder fields",
                path.display(),
                i.id
            ));
        }
    }
    for c in &m.components {
        let ident = |s: &str| {
            !s.is_empty()
                && s.bytes().enumerate().all(|(n, b)| {
                    b.is_ascii_alphabetic() || b == b'_' || (n > 0 && b.is_ascii_digit())
                })
        };
        if !ident(&c.class)
            || !ident(&c.member)
            || !c.header.starts_with("game/client/")
            || c.header.contains('\\')
            || c.header
                .split('/')
                .any(|p| p.is_empty() || p == ".." || p == "." || p.contains(':'))
            || !c.header.ends_with(".h")
            || c.register_in.is_empty()
            || !c.register_in.iter().any(|s| s == "all")
            || c.register_in.iter().any(|s| s != "all" && s != "input")
            || c.register_in.iter().collect::<BTreeSet<_>>().len() != c.register_in.len()
        {
            return Err(format!(
                "{}: invalid component declaration {} (requires all; optional input)",
                path.display(),
                c.class
            ));
        }
    }
    Ok(m)
}

/// Snippets live outside the auto-compiled source overlay. Reject lexical path
/// escapes and every symlink component, then verify canonical containment.
fn read_hook(manifest: &Path, relative: &str) -> Result<String, String> {
    if !relative.starts_with("hooks/")
        || relative.contains('\\')
        || relative.chars().any(char::is_control)
        || relative
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.contains(':'))
    {
        return Err(
            "expected a normalized mod-relative hooks/... path using forward slashes".into(),
        );
    }
    let root = manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut file = root.clone();
    for part in relative.split('/') {
        file.push(part);
        let meta = fs::symlink_metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("symlink hook path forbidden: {}", file.display()));
        }
    }
    let canonical = file.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) {
        return Err("hook path escapes the mod directory".into());
    }
    if !canonical.is_file() {
        return Err("hook path must name a regular UTF-8 file, not a directory".into());
    }
    fs::read_to_string(&canonical).map_err(|e| format!("{}: {e}", canonical.display()))
}

pub fn discover(root: &Path) -> Result<Vec<Mod>, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))?;
    let mut mods = BTreeMap::new();
    let mut entries = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let p = entry.path();
        let meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "symlink mod directory is forbidden: {}",
                p.display()
            ));
        }
        if !meta.is_dir() || !p.join("mixin.toml").exists() {
            continue;
        }
        let manifest_path = p.join("mixin.toml");
        if fs::symlink_metadata(&manifest_path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err(format!(
                "symlink manifest is forbidden: {}",
                manifest_path.display()
            ));
        }
        let manifest = load(&manifest_path)?;
        if !manifest.package.enabled {
            continue;
        }
        let mut files = Vec::new();
        let src = p.join("src");
        if src.exists() {
            if fs::symlink_metadata(&src)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err(format!(
                    "symlink source root is forbidden: {}",
                    src.display()
                ));
            }
            for file in mixins_core::source_files(&src).map_err(|e| e.to_string())? {
                let relative = file.strip_prefix(&src).map_err(|e| e.to_string())?;
                if !relative.starts_with(Path::new("game/client")) {
                    return Err(format!(
                        "{}: no supported target mapping (only game/client/** is supported)",
                        file.display()
                    ));
                }
                let logical = format!("src/{}", relative.to_string_lossy().replace('\\', "/"));
                files.push(Source {
                    physical: file,
                    logical,
                    target: "game-client".into(),
                });
            }
        }
        let id = manifest.package.id.clone();
        if mods
            .insert(
                id.clone(),
                Mod {
                    root: p,
                    manifest,
                    files,
                },
            )
            .is_some()
        {
            return Err(format!("duplicate mod ID: {id}"));
        }
    }
    // Stable topological ordering: lexically smallest ready mod first.
    let mut done = BTreeSet::new();
    let mut ordered = Vec::new();
    while !mods.is_empty() {
        for m in mods.values() {
            for dependency in m.manifest.dependencies.keys() {
                if !mods.contains_key(dependency) && !done.contains(dependency) {
                    return Err(format!(
                        "{}: missing enabled dependency {dependency}",
                        m.manifest.package.id
                    ));
                }
            }
        }
        let next = mods
            .iter()
            .find(|(_, m)| m.manifest.dependencies.keys().all(|d| done.contains(d)))
            .map(|(id, _)| id.clone());
        let Some(id) = next else {
            return Err(format!(
                "dependency cycle among: {}",
                mods.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        };
        done.insert(id.clone());
        ordered.push(mods.remove(&id).unwrap());
    }
    Ok(ordered)
}
