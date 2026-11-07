use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    process::{Command, ExitCode},
};

#[derive(Parser)]
#[command(
    name = "ddmix",
    version,
    about = "DDNet Mixins: deterministic mod discovery and custom C++ lexical inspection"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Create a new mod directory (refuses to overwrite an existing path).
    Init { path: PathBuf },
    /// List enabled mods, dependency order and source-to-target mappings as JSON.
    List {
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        mods: Option<PathBuf>,
    },
    /// Resolve edits, detect conflicts and emit original-span patches (no writes).
    Plan {
        #[arg(long)]
        ddnet: PathBuf,
        #[arg(long)]
        mods: PathBuf,
        /// Print a unified diff instead of JSON.
        #[arg(long)]
        diff: bool,
    },
    /// Validate exactly the same non-destructive transformation plan.
    Check {
        #[arg(long)]
        ddnet: PathBuf,
        #[arg(long)]
        mods: PathBuf,
    },
    /// Regenerate isolated .ddmix/staging/ddnet; never edit the supplied DDNet.
    Apply {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        ddnet: PathBuf,
        #[arg(long)]
        mods: PathBuf,
    },
    /// Apply, then configure and build the staged game-client (build output is external).
    Build {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        ddnet: PathBuf,
        #[arg(long)]
        mods: PathBuf,
        /// Existing-parent build directory outside .ddmix and the input checkout.
        #[arg(long)]
        build_dir: PathBuf,
        /// Separate ddnet-libs checkout to copy into staging only.
        #[arg(long)]
        libraries: Option<PathBuf>,
        /// Additional configure arguments, e.g. --cmake-arg=-DHEADLESS_CLIENT=ON.
        #[arg(long, allow_hyphen_values = true)]
        cmake_arg: Vec<String>,
    },
    /// Resolve one supported function definition in a source file, without editing it.
    Locate {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        function: String,
        /// Exact parenthesized parameters to disambiguate overloads.
        #[arg(long)]
        parameters: Option<String>,
    },
    /// Scan real DDNet sources and report conservative structural-index coverage.
    Inspect {
        #[arg(long)]
        ddnet: PathBuf,
    },
}
#[derive(Serialize)]
struct FileReport {
    file: String,
    bytes: usize,
    tokens: usize,
    paired_delimiters: usize,
    functions: usize,
    declarations: usize,
    classes: usize,
    unsupported_regions: usize,
    unsupported: Vec<mixins_core::Diagnostic>,
    diagnostics: Vec<mixins_core::Diagnostic>,
}
#[derive(Serialize)]
struct Report {
    schema_version: u32,
    stage: &'static str,
    ddnet: PathBuf,
    revision: Option<String>,
    files_scanned: usize,
    bytes: usize,
    tokens: usize,
    conditional_files: usize,
    functions: usize,
    classes: usize,
    unsupported_regions: usize,
    diagnostic_count: usize,
    files: Vec<FileReport>,
}
fn run(cli: Cli) -> Result<bool, String> {
    match cli.command {
        Commands::Init { path } => {
            let id = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("mod path needs a UTF-8 directory name")?;
            if !mixins_manifest::valid_id(id) {
                return Err(
                    "directory name must be a lowercase mod ID (letters, digits, hyphens)".into(),
                );
            }
            // create_dir, rather than create_dir_all, ensures no existing directory is overwritten.
            fs::create_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            fs::create_dir(path.join("src")).map_err(|e| e.to_string())?;
            fs::write(path.join("mixin.toml"), format!("[mod]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\n\n[compatibility]\nddnet = \"*\"\n")).map_err(|e| e.to_string())?;
            println!("Created {}", path.display());
            Ok(true)
        }
        Commands::List { workspace, mods } => {
            let mods = mixins_manifest::discover(&mods.unwrap_or_else(|| workspace.join("mods")))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&mods).map_err(|e| e.to_string())?
            );
            Ok(true)
        }
        Commands::Plan {
            ddnet,
            mods,
            diff: true,
        } => {
            let plan = mixins_build::plan(&ddnet, &mods)?;
            print!("{}", plan.diff(&ddnet)?);
            Ok(true)
        }
        Commands::Plan {
            ddnet,
            mods,
            diff: false,
        }
        | Commands::Check { ddnet, mods } => {
            let plan = mixins_build::plan(&ddnet, &mods)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
            );
            Ok(true)
        }
        Commands::Apply {
            workspace,
            ddnet,
            mods,
        } => {
            let staged = mixins_build::apply(&workspace, &ddnet, &mods)?;
            println!("{}", serde_json::json!({ "staged_ddnet": staged }));
            Ok(true)
        }
        Commands::Build {
            workspace,
            ddnet,
            mods,
            build_dir,
            libraries,
            cmake_arg,
        } => {
            let workspace = workspace.canonicalize().map_err(|e| e.to_string())?;
            let ddnet = ddnet.canonicalize().map_err(|e| e.to_string())?;
            let build_dir = if build_dir.exists() {
                build_dir.canonicalize().map_err(|e| e.to_string())?
            } else {
                let absolute = std::path::absolute(&build_dir).map_err(|e| e.to_string())?;
                absolute
                    .parent()
                    .ok_or("build directory needs a parent")?
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .join(absolute.file_name().ok_or("build directory needs a name")?)
            };
            if build_dir.starts_with(workspace.join(".ddmix"))
                || build_dir.starts_with(&ddnet)
                || ddnet.starts_with(&build_dir)
                || workspace.starts_with(&build_dir)
            {
                return Err("build directory must be outside generated staging and the input checkout, and must not contain the workspace".into());
            }
            // Do not permit command-line overrides of source/binary roots or the
            // workspace variable. Explicit cache options and generator are enough.
            if cmake_arg.iter().any(|a| {
                !(a.starts_with("-D") && a.contains('=')) || a.contains("DDMIX_WORKSPACE_ROOT")
            }) {
                return Err("unsafe/reserved CMake argument".into());
            }
            let staged = mixins_build::apply_with_dependencies(
                &workspace,
                &ddnet,
                &mods,
                libraries.as_deref(),
            )?;
            let status = Command::new("cmake")
                .arg("-S")
                .arg(mixins_build::cmake_path(&staged)?)
                .arg("-B")
                .arg(mixins_build::cmake_path(&build_dir)?)
                .args(cmake_arg)
                .env("PYTHONDONTWRITEBYTECODE", "1")
                // Avoid discovering the unrelated Mixins parent Git repository.
                .env(
                    "GIT_CEILING_DIRECTORIES",
                    mixins_build::cmake_path(&workspace.join(".ddmix/staging"))?,
                )
                .arg(format!(
                    "-DDDMIX_WORKSPACE_ROOT={}",
                    mixins_build::cmake_path(&workspace)?
                ))
                .status()
                .map_err(|e| format!("cannot run cmake: {e}"))?;
            if !status.success() {
                return Err(format!("CMake configure failed: {status}"));
            }
            let status = Command::new("cmake")
                .arg("--build")
                .arg(mixins_build::cmake_path(&build_dir)?)
                .args(["--target", "game-client", "--config", "Release"])
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .env(
                    "GIT_CEILING_DIRECTORIES",
                    mixins_build::cmake_path(&workspace.join(".ddmix/staging"))?,
                )
                .status()
                .map_err(|e| format!("cannot run cmake build: {e}"))?;
            if !status.success() {
                return Err(format!("CMake build failed: {status}"));
            }
            Ok(true)
        }
        Commands::Locate {
            file,
            function,
            parameters,
        } => {
            let source =
                fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let index = mixins_parser::parse(&source, &mixins_lexer::lex(&source));
            let node = index
                .select_function(&function, parameters.as_deref(), &source)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            println!(
                "{}",
                serde_json::to_string_pretty(node).map_err(|e| e.to_string())?
            );
            Ok(true)
        }
        Commands::Inspect { ddnet } => {
            let ddnet = ddnet
                .canonicalize()
                .map_err(|e| format!("{}: {e}", ddnet.display()))?;
            if !ddnet.join("CMakeLists.txt").is_file() {
                return Err("DDNet root must contain CMakeLists.txt and src/".into());
            }
            let revision = Command::new("git")
                .arg("-C")
                .arg(&ddnet)
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
            let mut report = Report {
                schema_version: 2,
                stage: "conservative-structural-index",
                ddnet: ddnet.clone(),
                revision,
                files_scanned: 0,
                bytes: 0,
                tokens: 0,
                conditional_files: 0,
                functions: 0,
                classes: 0,
                unsupported_regions: 0,
                diagnostic_count: 0,
                files: Vec::new(),
            };
            for file in mixins_core::source_files(&ddnet.join("src")).map_err(|e| e.to_string())? {
                let source =
                    fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                let lexed = mixins_lexer::lex(&source);
                report.bytes += source.len();
                report.tokens += lexed.tokens.len();
                report.conditional_files += usize::from(lexed.conditional);
                let index = mixins_parser::parse(&source, &lexed);
                let structure = mixins_parser::structure(&source, &lexed);
                report.diagnostic_count += index.diagnostics.len();
                report.functions += index.functions.len();
                report.classes += index.classes.len();
                report.unsupported_regions += index.unsupported.len();
                report.files.push(FileReport {
                    file: file
                        .strip_prefix(&ddnet)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    bytes: source.len(),
                    tokens: lexed.tokens.len(),
                    paired_delimiters: structure.pairs.iter().flatten().count() / 2,
                    functions: index.functions.len(),
                    declarations: index.declarations.len(),
                    classes: index.classes.len(),
                    unsupported_regions: index.unsupported.len(),
                    unsupported: index.unsupported,
                    diagnostics: index.diagnostics,
                });
            }
            report.files_scanned = report.files.len();
            let success = report.diagnostic_count == 0;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(success)
        }
    }
}
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("{}", serde_json::json!({ "error": message }));
            ExitCode::FAILURE
        }
    }
}
