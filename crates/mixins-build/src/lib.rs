//! Read-only planning and isolated, tool-owned staging publication.
mod components;
use mixins_core::progress::{Event, Observer};
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
    plan_observed(ddnet, mods_root, &mut |_| {})
}
pub fn plan_observed(
    ddnet: &Path,
    mods_root: &Path,
    observe: &mut Observer<'_>,
) -> Result<IntegrationPlan> {
    observe(Event::Stage {
        name: "discovery",
        message: format!("Loading mods from {}", mods_root.display()),
    });
    let ddnet = io(ddnet.canonicalize())?;
    let mods = mixins_manifest::discover_observed(mods_root, observe)?;
    observe(Event::Count {
        name: "mods",
        value: mods.len(),
    });
    if mods.is_empty() {
        observe(Event::Warning(
            "No enabled mods discovered; only managed CMake integration will be prepared".into(),
        ));
    }
    observe(Event::Count {
        name: "manifest edits",
        value: mods.iter().map(|m| m.manifest.edits.len()).sum(),
    });
    observe(Event::Count {
        name: "injections",
        value: mods.iter().map(|m| m.manifest.injections.len()).sum(),
    });
    observe(Event::Count {
        name: "components",
        value: mods.iter().map(|m| m.manifest.components.len()).sum(),
    });
    observe(Event::Count {
        name: "overlay files",
        value: mods.iter().map(|m| m.files.len()).sum(),
    });
    observe(Event::Detail(format!(
        "Dependency order: {}",
        mods.iter()
            .map(|m| m.manifest.package.id.as_str())
            .collect::<Vec<_>>()
            .join(" -> ")
    )));
    observe(Event::Stage {
        name: "lowering",
        message: "Lowering injections and component registrations to generic edits".into(),
    });
    let mut grouped: BTreeMap<String, Vec<(String, Edit)>> = BTreeMap::new();
    let (mut overlays, mut includes) = (Vec::new(), Vec::new());
    let mut owners = BTreeMap::new();
    for m in &mods {
        let mut edits = m.manifest.edits.clone();
        for i in &m.manifest.injections {
            observe(Event::Detail(format!(
                "mod {} injection {}: {:?} {} in {} (helper {:?}, content_file {:?})",
                m.manifest.package.id, i.id, i.kind, i.target, i.file, i.helper, i.content_file
            )));
            let source = io(fs::read_to_string(safe_file(&ddnet, &i.file)?))?;
            edits.extend(
                mixins_transform::helpers::lower(&m.manifest.package.id, i, &source).map_err(
                    |e| {
                        format!(
                            "mod {}, injection {}, file {}: {e}",
                            m.manifest.package.id, i.id, i.file
                        )
                    },
                )?,
            );
        }
        for c in &m.manifest.components {
            observe(Event::Info(format!(
                "mod {}: register {} as {} in {:?}",
                m.manifest.package.id, c.class, c.member, c.register_in
            )));
            edits.extend(components::lower(&ddnet, m, c).map_err(|e| {
                format!("mod {}, component {}: {e}", m.manifest.package.id, c.class)
            })?);
        }
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
            observe(Event::Detail(format!(
                "mod {} overlay {} -> {} ({})",
                m.manifest.package.id,
                f.logical,
                f.target,
                f.physical.display()
            )));
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
    observe(Event::Stage {
        name: "resolve",
        message: format!("Indexing and transforming {} source files", grouped.len()),
    });
    let file_count = grouped.len();
    let mut patch_count = 0;
    for (n, (file, edits)) in grouped.into_iter().enumerate() {
        let source = io(fs::read_to_string(safe_file(&ddnet, &file)?))?;
        let index = mixins_parser::parse(&source, &mixins_lexer::lex(&source));
        observe(Event::Detail(format!(
            "{file}: index contains {} functions, {} declarations, {} classes",
            index.functions.len(),
            index.declarations.len(),
            index.classes.len()
        )));
        if !index.unsupported.is_empty() {
            observe(Event::Warning(format!(
                "{file}: {} unsupported syntax regions outside resolved targets; see inspect JSON/--verbose for parser coverage",
                index.unsupported.len()
            )));
        }
        for d in &index.unsupported {
            observe(Event::Detail(format!(
                "{file}:{}:{} [{}] {}",
                d.line, d.column, d.code, d.message
            )));
        }
        let refs: Vec<_> = edits.iter().map(|(id, e)| (id.as_str(), e)).collect();
        let plan = Plan::resolve(&source, &refs)?;
        let rendered = plan.render(&source)?;
        patch_count += plan.patches.len();
        observe(Event::Info(format!(
            "{file}: resolved {} syntax edits",
            plan.patches.len()
        )));
        for p in &plan.patches {
            let line = source[..p.span.start]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1;
            let column = source[..p.span.start]
                .rsplit('\n')
                .next()
                .unwrap_or("")
                .chars()
                .count()
                + 1;
            observe(Event::Detail(format!(
                "{}/{} at {file}:{line}:{column}, bytes {}..{}, {} content bytes",
                p.mod_id,
                p.edit_id,
                p.span.start,
                p.span.end,
                p.content.len()
            )));
        }
        observe(Event::Progress {
            name: "resolve",
            done: n + 1,
            total: file_count,
        });
        if file == "src/engine/shared/config_variables.h" {
            validate_config(&rendered)?;
        }
        result.rendered.insert(file.clone(), rendered);
        result
            .input_sha256
            .insert(file.clone(), hash(source.as_bytes()));
        result.files.insert(file, plan);
    }
    observe(Event::Count {
        name: "patches",
        value: patch_count,
    });
    observe(Event::Count {
        name: "modified files",
        value: file_count + 1,
    });
    observe(Event::Stage {
        name: "cmake",
        message: "Validating target placement, overlay mapping and checksum capacity".into(),
    });
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
    // Validate all generated component member names across mod boundaries.
    let mut members = std::collections::BTreeSet::new();
    for m in &mods {
        for c in &m.manifest.components {
            if !members.insert(&c.member) {
                return Err(format!("duplicate component member {}", c.member));
            }
        }
    }
    observe(Event::Success(format!(
        "Plan validated: {patch_count} source patches; staged CMake integration ready"
    )));
    observe(Event::Detail(
        "CMake configure and C++ compilation have not been run by the planner".into(),
    ));
    Ok(result)
}
impl IntegrationPlan {
    pub fn diff(&self, ddnet: &Path) -> Result<String> {
        let mut output = String::new();
        for (file, after) in &self.rendered {
            let before = io(fs::read_to_string(ddnet.join(file)))?;
            output.push_str(
                &similar::TextDiff::from_lines(&before, after)
                    .unified_diff()
                    .context_radius(3)
                    .header(&format!("a/{file}"), &format!("b/{file}"))
                    .to_string(),
            );
        }
        output.push_str(
            &similar::TextDiff::from_lines("", &self.generated_cmake)
                .unified_diff()
                .header("/dev/null", "b/.ddmix/generated.cmake")
                .to_string(),
        );
        Ok(output)
    }
}
fn validate_config(source: &str) -> Result<()> {
    let l = mixins_lexer::lex(source);
    let syntax = mixins_parser::syntax::Syntax::new(source, &l)?;
    // Distinct arms of the same #if cannot coexist. Do not evaluate macros.
    let mut branches = vec![Vec::<(usize, usize)>::new(); l.tokens.len()];
    let mut stack: Vec<(usize, usize)> = vec![];
    for (i, t) in l.tokens.iter().enumerate() {
        branches[i] = stack.clone();
        if t.kind == mixins_lexer::Kind::Directive {
            let directive = source[t.start + 1..t.end]
                .trim_start()
                .split(|c: char| !c.is_ascii_alphabetic())
                .next()
                .unwrap_or("");
            match directive {
                "if" | "ifdef" | "ifndef" => stack.push((i, 0)),
                "else" | "elif" => {
                    if let Some((_, arm)) = stack.last_mut() {
                        *arm += 1;
                    }
                }
                "endif" => {
                    stack.pop();
                }
                _ => {}
            }
        }
    }
    type ConfigDeclaration = (String, String, Vec<(usize, usize)>);
    let mut seen: Vec<ConfigDeclaration> = vec![];
    for name in ["MACRO_CONFIG_INT", "MACRO_CONFIG_COL", "MACRO_CONFIG_STR"] {
        for invocation in syntax.invocations(name)? {
            if invocation.arguments.len() < 2 {
                return Err("config macro requires identifier and console name".into());
            }
            let arg = |n: usize| {
                let s = invocation.arguments[n];
                source[s.start..s.end].trim()
            };
            let token = l
                .tokens
                .iter()
                .position(|t| t.start == invocation.span.start)
                .ok_or("missing macro token")?;
            let branch = &branches[token];
            if seen.iter().any(|(id, name, previous)| {
                (id == arg(0) || name == arg(1))
                    && !previous
                        .iter()
                        .any(|(group, arm)| branch.iter().any(|(g, a)| g == group && a != arm))
            }) {
                return Err(format!(
                    "duplicate config identifier/console name: {} / {}",
                    arg(0),
                    arg(1)
                ));
            }
            seen.push((arg(0).into(), arg(1).into(), branch.clone()));
        }
    }
    Ok(())
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
    apply_with_dependencies(workspace, ddnet, mods, None)
}
pub fn apply_with_dependencies(
    workspace: &Path,
    ddnet: &Path,
    mods: &Path,
    libraries: Option<&Path>,
) -> Result<PathBuf> {
    apply_observed(workspace, ddnet, mods, libraries, &mut |_| {})
}
pub fn apply_observed(
    workspace: &Path,
    ddnet: &Path,
    mods: &Path,
    libraries: Option<&Path>,
    observe: &mut Observer<'_>,
) -> Result<PathBuf> {
    observe(Event::Stage {
        name: "prepare",
        message: "Validating workspace paths and dependency tree before staging".into(),
    });
    let libraries = libraries.map(|p| io(p.canonicalize())).transpose()?;
    let dependency_files = libraries
        .as_ref()
        .map(|p| regular_tree(p, true))
        .transpose()?;
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
        observe(Event::Info(
            "Verifying existing .ddmix ownership and SHA-256 inventory".into(),
        ));
        validate_owned(&output)?;
    }
    let plan = plan_observed(&ddnet, &mods, observe)?;
    observe(Event::Stage {
        name: "staging",
        message: "Copying pristine DDNet into an isolated transaction".into(),
    });
    let inputs = regular_tree(&ddnet, true)?; // validate entire copy before creating output
    if ddnet.join(".git").exists() {
        observe(Event::Info(
            "Excluding root .git metadata from the isolated copy".into(),
        ));
    }
    io(fs::create_dir(&transaction))?;
    // Failed prepublication outputs remain named .ddmix-next for inspection.
    let staged = transaction.join("staging/ddnet");
    io(fs::create_dir_all(&staged))?;
    let total = inputs.len();
    observe(Event::Count {
        name: "copied source files",
        value: total,
    });
    for (n, relative) in inputs.into_iter().enumerate() {
        observe(Event::Detail(format!("Copy {}", relative.display())));
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
        if (n + 1) % 100 == 0 || n + 1 == total {
            observe(Event::Progress {
                name: "staging",
                done: n + 1,
                total,
            });
        }
    }
    if let (Some(libs), Some(files)) = (&libraries, dependency_files) {
        let destination = staged.join("ddnet-libs");
        if destination.exists() && !regular_tree(&destination, false)?.is_empty() {
            return Err(
                "DDNet already contains bundled dependencies; refusing external override".into(),
            );
        }
        let total = files.len();
        observe(Event::Count {
            name: "dependency files",
            value: total,
        });
        observe(Event::Info(format!(
            "Copying {total} bundled dependency files"
        )));
        for (n, relative) in files.into_iter().enumerate() {
            let path = destination.join(&relative);
            io(fs::create_dir_all(
                path.parent().ok_or("dependency parent missing")?,
            ))?;
            io(fs::copy(libs.join(relative), path))?;
            if (n + 1) % 100 == 0 || n + 1 == total {
                observe(Event::Progress {
                    name: "dependencies",
                    done: n + 1,
                    total,
                });
            }
        }
    } else {
        observe(Event::Detail(
            "No external libraries supplied; retaining dependencies present in original DDNet"
                .into(),
        ));
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
    observe(Event::Stage {
        name: "publish",
        message: "Writing generated state and publishing verified staging".into(),
    });
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
    observe(Event::Success(format!(
        "Staging ready: {}",
        output.join("staging/ddnet").display()
    )));
    Ok(output.join("staging/ddnet"))
}
