//! Read-only planning and isolated, tool-owned staging publication.
use mixins_transform::{Edit, Plan};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|e| e.to_string())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Stable portable paths, not Windows verbatim-path prefixes in CMake.
pub fn cmake_path(path: &Path) -> Result<String> {
    let s = path
        .to_str()
        .ok_or("CMake paths must be UTF-8")?
        .replace('\\', "/");
    let s = if let Some(rest) = s.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else {
        s.strip_prefix("//?/").unwrap_or(&s).into()
    };
    mixins_cmake::quote_path(&s)?;
    Ok(s)
}
fn regular_tree(root: &Path, skip_git: bool) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, relative: &Path, skip_git: bool, files: &mut Vec<PathBuf>) -> Result<()> {
        let mut entries = io(fs::read_dir(root.join(relative)))?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if skip_git && relative.as_os_str().is_empty() && entry.file_name() == ".git" {
                continue;
            }
            let rel = relative.join(entry.file_name());
            let meta = io(fs::symlink_metadata(entry.path()))?;
            if meta.file_type().is_symlink() {
                return Err(format!("symlink forbidden: {}", entry.path().display()));
            }
            if meta.is_dir() {
                walk(root, &rel, false, files)?;
            } else if meta.is_file() {
                files.push(rel);
            } else {
                return Err(format!("non-regular input: {}", entry.path().display()));
            }
        }
        Ok(())
    }
    let mut files = vec![];
    walk(root, Path::new(""), skip_git, &mut files)?;
    Ok(files)
}
fn safe_file(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.contains('\\')
        || !relative.starts_with("src/")
        || relative
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.contains(':'))
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("unsafe source path {relative}"));
    }
    let mut p = root.to_path_buf();
    for c in relative.split('/') {
        p.push(c);
        if io(fs::symlink_metadata(&p))?.file_type().is_symlink() {
            return Err(format!("symlink edit path forbidden: {}", p.display()));
        }
    }
    if !io(p.canonicalize())?.starts_with(root) {
        return Err(format!("path escapes DDNet: {relative}"));
    }
    Ok(p)
}
#[derive(Serialize)]
pub struct IntegrationPlan {
    pub schema_version: u32,
    pub stage: &'static str,
    pub files: BTreeMap<String, Plan>,
    pub input_sha256: BTreeMap<String, String>,
    pub generated_cmake: String,
    /// Structural placement/generation validated, not a CMake configure run.
    pub cmake_integration_validated: bool,
    pub cmake_configure_validated: bool,
    pub compile_validated: bool,
    #[serde(skip)]
    pub rendered: BTreeMap<String, String>,
}
pub fn plan(ddnet: &Path, mods_root: &Path) -> Result<IntegrationPlan> {
    let ddnet = io(ddnet.canonicalize())?;
    let mods = mixins_manifest::discover(mods_root)?;
    let mut grouped: BTreeMap<String, Vec<(String, Edit)>> = BTreeMap::new();
    let (mut overlays, mut includes) = (Vec::new(), Vec::new());
    let mut owners = BTreeMap::new();
    for m in &mods {
        let mut edits = m.manifest.edits.clone();
        edits.extend(m.manifest.injections.iter().map(|i| i.lower()));
        edits.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
        for e in edits {
            grouped
                .entry(e.file.clone())
                .or_default()
                .push((m.manifest.package.id.clone(), e));
        }
        if !m.files.is_empty() {
            includes.push(cmake_path(&m.root.join("src"))?);
        }
        for f in &m.files {
            if let Some(previous) = owners.insert(f.logical.clone(), m.manifest.package.id.clone())
            {
                return Err(format!(
                    "overlay collision at {}: {previous} and {}",
                    f.logical, m.manifest.package.id
                ));
            }
            if ddnet.join(&f.logical).exists() {
                return Err(format!("overlay shadows upstream source: {}", f.logical));
            }
            overlays.push(cmake_path(&f.physical)?);
        }
    }
    let mut result = IntegrationPlan {
        schema_version: 2,
        stage: "staged-source-and-cmake",
        files: BTreeMap::new(),
        input_sha256: BTreeMap::new(),
        rendered: BTreeMap::new(),
        generated_cmake: mixins_cmake::generate(&overlays, &includes)?,
        cmake_integration_validated: true,
        cmake_configure_validated: false,
        compile_validated: false,
    };
    for (file, edits) in grouped {
        let source = io(fs::read_to_string(safe_file(&ddnet, &file)?))?;
        let refs: Vec<_> = edits.iter().map(|(id, e)| (id.as_str(), e)).collect();
        let plan = Plan::resolve(&source, &refs)?;
        result.rendered.insert(file.clone(), plan.render(&source)?);
        result
            .input_sha256
            .insert(file.clone(), hash(source.as_bytes()));
        result.files.insert(file, plan);
    }
    let cmake = io(fs::read_to_string(ddnet.join("CMakeLists.txt")))?;
    let capacity = mixins_cmake::checksum_capacity(&io(fs::read_to_string(safe_file(
        &ddnet,
        "src/engine/client/checksum.h",
    )?))?)?;
    let extras = io(fs::read_to_string(
        ddnet.join("cmake/checksummed_extra.txt"),
    ))?
    .lines()
    .count();
    result.rendered.insert(
        "CMakeLists.txt".into(),
        mixins_cmake::integrate(&cmake, capacity, extras)?,
    );
    result
        .input_sha256
        .insert("CMakeLists.txt".into(), hash(cmake.as_bytes()));
    Ok(result)
}
#[derive(Serialize, Deserialize)]
struct State {
    owner: String,
    version: u32,
    files: BTreeMap<String, String>,
}
fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut entries = BTreeMap::new();
    for relative in regular_tree(root, false)? {
        if relative == Path::new("state.json") {
            continue;
        }
        entries.insert(
            relative.to_string_lossy().replace('\\', "/"),
            hash(&io(fs::read(root.join(relative)))?),
        );
    }
    Ok(entries)
}
fn validate_owned(root: &Path) -> Result<()> {
    if io(fs::symlink_metadata(root))?.file_type().is_symlink() {
        return Err("tool output must not be a symlink".into());
    }
    let state_path = root.join("state.json");
    if io(fs::symlink_metadata(&state_path))?
        .file_type()
        .is_symlink()
    {
        return Err("symlink state file forbidden".into());
    }
    let state: State = serde_json::from_slice(&io(fs::read(state_path))?)
        .map_err(|e| format!("unowned .ddmix: {e}"))?;
    if state.owner != "ddnet-mixins" || state.version != 1 {
        return Err("unrecognized .ddmix ownership state".into());
    }
    if inventory(root)? != state.files {
        return Err("staging/output was modified outside ddmix; preserve your changes and remove .ddmix manually before regeneration".into());
    }
    Ok(())
}
/// Fresh copy every time; never patch an already transformed input. Publication
/// uses a locked sibling transaction and renames with rollback on rename failure.
pub fn apply(workspace: &Path, ddnet: &Path, mods: &Path) -> Result<PathBuf> {
    let workspace = io(workspace.canonicalize())?;
    let ddnet = io(ddnet.canonicalize())?;
    let output = workspace.join(".ddmix");
    if ddnet.starts_with(&output) || output.starts_with(&ddnet) {
        return Err("workspace staging must not contain or be inside the DDNet input".into());
    }
    let mods = io(mods.canonicalize())?;
    if mods.starts_with(&output) {
        return Err("mods must remain outside generated output".into());
    }
    let lock = workspace.join(".ddmix.lock");
    let handle = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|e| {
            format!(
                "cannot acquire {} (another apply or stale lock): {e}",
                lock.display()
            )
        })?;
    struct Lock(PathBuf, Option<fs::File>);
    impl Drop for Lock {
        fn drop(&mut self) {
            self.1.take();
            let _ = fs::remove_file(&self.0);
        }
    }
    let _lock = Lock(lock, Some(handle));
    let transaction = workspace.join(".ddmix-next");
    let previous = workspace.join(".ddmix-previous");
    if transaction.try_exists().map_err(|e| e.to_string())?
        || previous.try_exists().map_err(|e| e.to_string())?
    {
        return Err(
            "unfinished .ddmix-next/.ddmix-previous transaction; inspect and recover manually"
                .into(),
        );
    }
    if output.try_exists().map_err(|e| e.to_string())? {
        validate_owned(&output)?;
    }
    let plan = plan(&ddnet, &mods)?;
    let inputs = regular_tree(&ddnet, true)?; // validate entire copy before creating output
    io(fs::create_dir(&transaction))?;
    // Failed prepublication outputs remain named .ddmix-next for inspection.
    let staged = transaction.join("staging/ddnet");
    io(fs::create_dir_all(&staged))?;
    for relative in inputs {
        let dest = staged.join(&relative);
        io(fs::create_dir_all(
            dest.parent().ok_or("missing destination parent")?,
        ))?;
        let bytes = io(fs::read(ddnet.join(&relative)))?;
        if let Some(expected) = plan
            .input_sha256
            .get(&relative.to_string_lossy().replace('\\', "/"))
        {
            if hash(&bytes) != *expected {
                return Err(format!(
                    "input changed after planning: {}",
                    relative.display()
                ));
            }
        }
        io(fs::write(&dest, &bytes))?;
        // Preserve executable scripts and other source permissions.
        io(fs::set_permissions(
            &dest,
            io(fs::metadata(ddnet.join(relative)))?.permissions(),
        ))?;
    }
    for (relative, content) in &plan.rendered {
        io(fs::write(staged.join(relative), content))?;
    }
    io(fs::write(
        transaction.join("generated.cmake"),
        &plan.generated_cmake,
    ))?;
    io(fs::write(
        transaction.join("plan.json"),
        serde_json::to_vec_pretty(&plan).map_err(|e| e.to_string())?,
    ))?;
    let state = State {
        owner: "ddnet-mixins".into(),
        version: 1,
        files: inventory(&transaction)?,
    };
    io(fs::write(
        transaction.join("state.json"),
        serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
    ))?;
    let had_previous = output.exists();
    if had_previous {
        validate_owned(&output)?;
        io(fs::rename(&output, &previous))?;
    }
    if let Err(error) = fs::rename(&transaction, &output) {
        if had_previous {
            io(fs::rename(&previous, &output))?;
        }
        return Err(format!("publication failed: {error}"));
    }
    if had_previous {
        io(fs::remove_dir_all(previous))?;
    }
    Ok(output.join("staging/ddnet"))
}
