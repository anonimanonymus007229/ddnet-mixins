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
    /// Scan real DDNet sources. Outputs lexical diagnostics, not an AST or safe edit plan.
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
    conditional_pairing_deferred: bool,
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
                schema_version: 1,
                stage: "lexer-only",
                ddnet: ddnet.clone(),
                revision,
                files_scanned: 0,
                bytes: 0,
                tokens: 0,
                conditional_files: 0,
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
                report.diagnostic_count += lexed.diagnostics.len();
                report.files.push(FileReport {
                    file: file
                        .strip_prefix(&ddnet)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    bytes: source.len(),
                    tokens: lexed.tokens.len(),
                    paired_delimiters: lexed.pairs.iter().flatten().count() / 2,
                    conditional_pairing_deferred: lexed.conditional,
                    diagnostics: lexed.diagnostics,
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
