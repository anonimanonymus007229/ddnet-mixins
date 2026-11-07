//! Dependency-only dereferencing: destination files are always independent copies.
//! Never use this policy for source edits, mod inputs or generated output inventory.
use crate::{Result, io};
use mixins_core::progress::{Event, Observer};
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    path::{Component, Path, PathBuf},
};

pub(crate) struct Entry {
    pub relative: PathBuf,
    pub source: PathBuf,
    pub directory: bool,
}
pub(crate) fn snapshot(root: &Path, observe: &mut Observer<'_>) -> Result<Vec<Entry>> {
    let root = io(root.canonicalize())?;
    let mut output = vec![];
    walk(
        &root,
        &root,
        Path::new(""),
        &mut BTreeSet::new(),
        &mut output,
        observe,
    )?;
    Ok(output)
}
/// Resolve each link component ourselves. Git for Windows can create a link
/// with a trailing separator that Win32 canonicalize cannot follow; Path
/// components remove that separator without relaxing containment or cycles.
fn resolve(root: &Path, path: &Path) -> Result<PathBuf> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("dependency path outside root: {}", path.display()))?;
    let mut pending: VecDeque<_> = relative
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    let mut current = root.to_path_buf();
    let mut links = BTreeSet::new();
    while let Some(part) = pending.pop_front() {
        let part_path = Path::new(&part);
        match part_path.components().next() {
            Some(Component::CurDir) => continue,
            Some(Component::ParentDir) => {
                if current == root {
                    return Err(format!(
                        "dependency link escapes dependency root: {}",
                        path.display()
                    ));
                }
                current.pop();
                continue;
            }
            Some(Component::Normal(_)) => {}
            _ => {
                return Err(format!(
                    "unsupported dependency path component at {}",
                    path.display()
                ));
            }
        }
        current.push(part);
        let meta = fs::symlink_metadata(&current)
            .map_err(|e| format!("cannot resolve dependency {}: {e}", current.display()))?;
        if meta.file_type().is_symlink() {
            if !links.insert(current.clone()) || links.len() > 128 {
                return Err(format!("dependency link cycle at {}", current.display()));
            }
            let target = io(fs::read_link(&current))?;
            current.pop();
            let target = if target.is_absolute() {
                let rel = target.strip_prefix(root).map_err(|_| {
                    format!(
                        "dependency link escapes dependency root: {} -> {}",
                        path.display(),
                        target.display()
                    )
                })?;
                current = root.to_path_buf();
                rel.to_path_buf()
            } else {
                target
            };
            for c in target.components().rev() {
                pending.push_front(c.as_os_str().to_os_string());
            }
        }
    }
    let result = current
        .canonicalize()
        .map_err(|e| format!("cannot resolve dependency {}: {e}", current.display()))?;
    if !result.starts_with(root) {
        return Err(format!(
            "dependency link escapes dependency root: {}",
            path.display()
        ));
    }
    Ok(result)
}
fn walk(
    root: &Path,
    path: &Path,
    relative: &Path,
    ancestors: &mut BTreeSet<PathBuf>,
    output: &mut Vec<Entry>,
    observe: &mut Observer<'_>,
) -> Result<()> {
    let meta =
        fs::symlink_metadata(path).map_err(|e| format!("dependency {}: {e}", path.display()))?;
    let resolved = resolve(root, path)?;
    if !resolved.starts_with(root) {
        return Err(format!(
            "dependency link escapes dependency root: {} -> {}",
            path.display(),
            resolved.display()
        ));
    }
    // Also reject directory junctions/reparse aliases that resolve to ancestors.
    let meta = if meta.file_type().is_symlink() {
        observe(Event::Info(format!(
            "Materializing dependency link {} -> {}",
            relative.display(),
            resolved.display()
        )));
        fs::metadata(&resolved).map_err(|e| format!("dependency {}: {e}", resolved.display()))?
    } else {
        meta
    };
    if meta.is_dir() {
        if !ancestors.insert(resolved.clone()) {
            return Err(format!(
                "dependency directory link cycle at {} -> {}",
                relative.display(),
                resolved.display()
            ));
        }
        if ancestors.len() > 128 {
            return Err(format!(
                "dependency directory nesting exceeds safety limit at {}",
                relative.display()
            ));
        }
        if !relative.as_os_str().is_empty() {
            output.push(Entry {
                relative: relative.into(),
                source: resolved.clone(),
                directory: true,
            });
        }
        let mut children = io(fs::read_dir(&resolved))?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        children.sort_by_key(|e| e.file_name());
        for entry in children {
            // Git metadata is not a dependency and may refer outside a submodule.
            if entry.file_name() == ".git" {
                continue;
            }
            walk(
                root,
                &entry.path(),
                &relative.join(entry.file_name()),
                ancestors,
                output,
                observe,
            )?;
        }
        ancestors.remove(&resolved);
    } else if meta.is_file() {
        output.push(Entry {
            relative: relative.into(),
            source: resolved,
            directory: false,
        });
    } else {
        return Err(format!("non-regular dependency input: {}", path.display()));
    }
    if output.len() > 1_000_000 {
        return Err("expanded dependency tree exceeds safety limit".into());
    }
    Ok(())
}
