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
    let m: Manifest = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
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
    Ok(m)
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
