mod logging;
use clap::{Parser, Subcommand};
use logging::{Color, Logger};
use mixins_core::progress::Event;
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
    /// Suppress banner, progress and success summaries (warnings/errors remain).
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,
    /// Show transformations, locations and stage timings.
    #[arg(short, long, global = true)]
    verbose: bool,
    #[arg(long, global = true, value_enum, default_value = "auto")]
    color: Color,
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
        /// DDNet context for clients include/exclude filters.
        #[arg(long)]
        ddnet: Option<PathBuf>,
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
impl Commands {
    fn stages(&self) -> Vec<&'static str> {
        match self {
            Self::Init { .. } => vec!["init"],
            Self::List { .. } => vec!["discovery"],
            Self::Locate { .. } => vec!["index", "locate"],
            Self::Inspect { .. } => vec!["discovery", "index"],
            Self::Plan { .. } | Self::Check { .. } => {
                vec!["discovery", "lowering", "resolve", "cmake"]
            }
            Self::Apply { .. } => vec![
                "prepare",
                "discovery",
                "lowering",
                "resolve",
                "cmake",
                "staging",
                "publish",
            ],
            Self::Build { .. } => vec![
                "prepare",
                "discovery",
                "lowering",
                "resolve",
                "cmake",
                "staging",
                "publish",
                "configure",
                "compile",
            ],
        }
    }
}
fn run(cli: Cli, log: &mut Logger) -> Result<bool, String> {
    match cli.command {
        Commands::Init { path } => {
            log.event(Event::Stage {
                name: "init",
                message: format!("Creating mod {}", path.display()),
            });
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
            log.event(Event::Success(format!("Created {}", path.display())));
            Ok(true)
        }
        Commands::List {
            workspace,
            mods,
            ddnet,
        } => {
            log.event(Event::Stage {
                name: "discovery",
                message: "Discovering and ordering mod packages".into(),
            });
            let root = mods.unwrap_or_else(|| workspace.join("mods"));
            let mods = if let Some(ddnet) = ddnet {
                mixins_manifest::discover_for(&root, &ddnet, &mut |e| log.event(e))?
            } else {
                mixins_manifest::discover_observed(&root, &mut |e| log.event(e))?
            };
            log.event(Event::Count {
                name: "mods",
                value: mods.len(),
            });
            log.event(Event::Count {
                name: "overlay files",
                value: mods.iter().map(|m| m.files.len()).sum(),
            });
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
            let plan = mixins_build::plan_observed(&ddnet, &mods, &mut |e| log.event(e))?;
            print!("{}", plan.diff(&ddnet)?);
            Ok(true)
        }
        Commands::Plan {
            ddnet,
            mods,
            diff: false,
        }
        | Commands::Check { ddnet, mods } => {
            let plan = mixins_build::plan_observed(&ddnet, &mods, &mut |e| log.event(e))?;
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
            let staged = mixins_build::apply_observed(&workspace, &ddnet, &mods, None, &mut |e| {
                log.event(e)
            })?;
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
            let staged = mixins_build::apply_observed(
                &workspace,
                &ddnet,
                &mods,
                libraries.as_deref(),
                &mut |e| log.event(e),
            )?;
            log.event(Event::Stage {
                name: "configure",
                message: format!("Configuring staged DDNet in {}", build_dir.display()),
            });
            fs::create_dir_all(&build_dir).map_err(|e| format!("{}: {e}", build_dir.display()))?;
            let mut configure = Command::new("cmake");
            configure
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
                ));
            log.command(&mut configure, &build_dir.join("ddmix-configure.log"))?;
            log.event(Event::Success("CMake configure succeeded".into()));
            log.event(Event::Stage {
                name: "compile",
                message: "Building game-client (Release)".into(),
            });
            let mut build = Command::new("cmake");
            build
                .arg("--build")
                .arg(mixins_build::cmake_path(&build_dir)?)
                .args(["--target", "game-client", "--config", "Release"])
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .env(
                    "GIT_CEILING_DIRECTORIES",
                    mixins_build::cmake_path(&workspace.join(".ddmix/staging"))?,
                );
            if log.is_verbose() {
                build.arg("--verbose");
            }
            log.command(&mut build, &build_dir.join("ddmix-build.log"))?;
            log.event(Event::Success(format!(
                "game-client compiled successfully; build output: {}",
                build_dir.display()
            )));
            Ok(true)
        }
        Commands::Locate {
            file,
            function,
            parameters,
        } => {
            log.event(Event::Stage {
                name: "index",
                message: format!("Indexing {}", file.display()),
            });
            let source =
                fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let index = mixins_parser::parse(&source, &mixins_lexer::lex(&source));
            log.event(Event::Stage {
                name: "locate",
                message: format!("Resolving {function}"),
            });
            let node = index
                .select_function(&function, parameters.as_deref(), &source)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            println!(
                "{}",
                serde_json::to_string_pretty(node).map_err(|e| e.to_string())?
            );
            log.event(Event::Success(format!(
                "{}:{}: {} body bytes {}..{}",
                file.display(),
                node.line,
                node.qualified_name,
                node.body.start,
                node.body.end
            )));
            Ok(true)
        }
        Commands::Inspect { ddnet } => {
            log.event(Event::Stage {
                name: "discovery",
                message: format!("Discovering DDNet corpus {}", ddnet.display()),
            });
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
            let files = mixins_core::source_files(&ddnet.join("src")).map_err(|e| e.to_string())?;
            let total = files.len();
            log.event(Event::Stage {
                name: "index",
                message: format!("Lexing and structurally indexing {total} files"),
            });
            for (n, file) in files.into_iter().enumerate() {
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
                log.event(Event::Detail(format!(
                    "{}: {} functions, {} classes, {} unsupported regions",
                    file.display(),
                    index.functions.len(),
                    index.classes.len(),
                    index.unsupported.len()
                )));
                for d in &index.diagnostics {
                    log.event(Event::Warning(format!(
                        "{}:{}:{} [{}] {}",
                        file.display(),
                        d.line,
                        d.column,
                        d.code,
                        d.message
                    )));
                }
                for d in &index.unsupported {
                    log.event(Event::Detail(format!(
                        "Unsupported {}:{}:{} [{}] {}",
                        file.display(),
                        d.line,
                        d.column,
                        d.code,
                        d.message
                    )));
                }
                if (n + 1) % 25 == 0 || n + 1 == total {
                    log.event(Event::Progress {
                        name: "index",
                        done: n + 1,
                        total,
                    });
                }
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
            for (name, value) in [
                ("indexed files", report.files_scanned),
                ("functions", report.functions),
                ("classes", report.classes),
                ("tokens", report.tokens),
            ] {
                log.event(Event::Count { name, value });
            }
            if report.unsupported_regions > 0 {
                log.event(Event::Warning(format!(
                    "{} unsupported syntax regions remain; see JSON inventory or --verbose",
                    report.unsupported_regions
                )));
            }
            if !success {
                log.error(&format!(
                    "{} corpus diagnostics; review file locations above and JSON report",
                    report.diagnostic_count
                ));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(success)
        }
    }
}
fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut log = Logger::new(cli.command.stages(), cli.quiet, cli.verbose, cli.color);
    log.banner();
    match run(cli, &mut log) {
        Ok(success) => {
            log.finish(success);
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(message) => {
            log.error(&message);
            log.finish(false);
            ExitCode::FAILURE
        }
    }
}
