use clap::ValueEnum;
use mixins_core::progress::Event;
use std::{
    collections::BTreeMap,
    io::{self, IsTerminal, Write},
    process::{Command, Stdio},
    time::Instant,
};

const BANNER: &str = r#"8888888b.  8888888b.  888b     d888 d8b          d8b
888  "Y88b 888  "Y88b 8888b   d8888 Y8P          Y8P
888    888 888    888 88888b.d88888
888    888 888    888 888Y88888P888 888 888  888 888 88888b.  .d8888b
888    888 888    888 888 Y888P 888 888 `Y8bd8P' 888 888 "88b 88K
888    888 888    888 888  Y8P  888 888   X88K   888 888  888 "Y8888b.
888  .d88P 888  .d88P 888   "   888 888 .d8""8b. 888 888  888      X88
8888888P"  8888888P"  888       888 888 888  888 888 888  888  88888P'"#;
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Color {
    Auto,
    Always,
    Never,
}
pub struct Logger {
    start: Instant,
    stage_start: Instant,
    stage: String,
    current: usize,
    stages: Vec<&'static str>,
    quiet: bool,
    verbose: bool,
    color: bool,
    stats: BTreeMap<&'static str, usize>,
    warnings: usize,
    skipped: usize,
    progress: BTreeMap<&'static str, usize>,
}
fn clean(message: &str) -> String {
    message
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                format!("\\u{{{:x}}}", c as u32).chars().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
impl Logger {
    pub fn new(stages: Vec<&'static str>, quiet: bool, verbose: bool, color: Color) -> Self {
        let enabled = match color {
            Color::Never => false,
            Color::Always => true,
            Color::Auto => {
                io::stderr().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none()
                    && std::env::var("TERM").as_deref() != Ok("dumb")
            }
        };
        Self {
            start: Instant::now(),
            stage_start: Instant::now(),
            stage: String::new(),
            current: 0,
            stages,
            quiet,
            verbose,
            color: enabled,
            stats: BTreeMap::new(),
            warnings: 0,
            skipped: 0,
            progress: BTreeMap::new(),
        }
    }
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }
    pub fn banner(&self) {
        if !self.quiet {
            let mut out = io::stderr().lock();
            let _ = writeln!(
                out,
                "{BANNER}\nDDNet Mixins v{}\n",
                env!("CARGO_PKG_VERSION")
            );
        }
    }
    fn line(&self, label: &str, message: &str, color: &str) {
        let mut out = io::stderr().lock();
        let time = format!("{:>7.2}s", self.start.elapsed().as_secs_f64());
        if self.color {
            let _ = writeln!(
                out,
                "{time} \x1b[{color}m{label:>8}\x1b[0m {}",
                clean(message)
            );
        } else {
            let _ = writeln!(out, "{time} {label:>8} {}", clean(message));
        }
    }
    pub fn event(&mut self, event: Event) {
        match event {
            Event::Stage { name, message } => {
                self.end_stage();
                self.stage = name.into();
                self.stage_start = Instant::now();
                self.current = self
                    .stages
                    .iter()
                    .position(|s| *s == name)
                    .map_or(self.current + 1, |n| n + 1);
                if !self.quiet {
                    self.line(
                        "stage",
                        &format!("[{}/{}] {name}: {message}", self.current, self.stages.len()),
                        "36",
                    );
                }
            }
            Event::Count { name, value } => {
                self.stats.insert(name, value);
            }
            Event::Warning(s) => {
                self.warnings += 1;
                self.line("warning", &s, "33");
            }
            Event::Skipped(s) => {
                self.skipped += 1;
                self.line("skipped", &s, "33");
            }
            Event::Detail(s) => {
                if self.verbose {
                    self.line("detail", &s, "90");
                }
            }
            Event::Info(s) => {
                if !self.quiet {
                    self.line("info", &s, "34");
                }
            }
            Event::Success(s) => {
                if !self.quiet {
                    self.line("ok", &s, "32");
                }
            }
            Event::Progress { name, done, total } => {
                let percent = done.saturating_mul(100).checked_div(total).unwrap_or(100);
                let bucket = percent / 10;
                let prev = self.progress.insert(name, bucket);
                if !self.quiet && (prev != Some(bucket) || done == total) {
                    self.line(
                        "progress",
                        &format!("{name}: {done}/{total} ({percent}%)"),
                        "36",
                    );
                }
            }
        }
    }
    fn end_stage(&self) {
        if self.verbose && !self.stage.is_empty() {
            self.line(
                "timing",
                &format!(
                    "{} completed in {:.3}s",
                    self.stage,
                    self.stage_start.elapsed().as_secs_f64()
                ),
                "90",
            );
        }
    }
    pub fn error(&self, message: &str) {
        self.line(
            "error",
            &format!(
                "{}: {message}",
                if self.stage.is_empty() {
                    "startup"
                } else {
                    &self.stage
                }
            ),
            "31",
        );
    }
    pub fn finish(&self, success: bool) {
        if success {
            self.end_stage();
        }
        if !self.quiet || !success {
            let stats = self
                .stats
                .iter()
                .map(|(k, v)| format!("{v} {k}"))
                .collect::<Vec<_>>()
                .join(", ");
            self.line(
                if success { "finished" } else { "failed" },
                &format!(
                    "{}{}{} warnings, {} skipped; elapsed {:.3}s",
                    stats,
                    if stats.is_empty() { "" } else { "; " },
                    self.warnings,
                    self.skipped,
                    self.start.elapsed().as_secs_f64()
                ),
                if success { "32" } else { "31" },
            );
        }
    }
    /// Child output is streamed to stderr (never JSON stdout). Quiet stores a
    /// complete transcript on disk and replays its tail on failure. Normal mode
    /// preserves native compiler progress; verbose adds full command metadata.
    pub fn command(&mut self, cmd: &mut Command, log_path: &std::path::Path) -> Result<(), String> {
        use std::io::{BufRead, BufReader};
        self.event(Event::Detail(format!("Running {cmd:?}")));
        let transcript = std::fs::File::create(log_path)
            .map_err(|e| format!("cannot create {}: {e}", log_path.display()))?;
        let transcript = std::sync::Arc::new(std::sync::Mutex::new(transcript));
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start {cmd:?}: {e}"))?;
        let stdout = child.stdout.take().ok_or("missing child stdout")?;
        let stderr = child.stderr.take().ok_or("missing child stderr")?;
        let quiet = self.quiet;
        let warning_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let spawn_reader = |stream: Box<dyn std::io::Read + Send>| {
            let file = transcript.clone();
            let warnings = warning_count.clone();
            std::thread::spawn(move || -> Result<(), String> {
                let mut input = BufReader::new(stream);
                let mut line = Vec::new();
                let mut warning_context = 0usize;
                loop {
                    line.clear();
                    let n = input
                        .read_until(b'\n', &mut line)
                        .map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    file.lock()
                        .map_err(|e| e.to_string())?
                        .write_all(&line)
                        .map_err(|e| e.to_string())?;
                    let text = String::from_utf8_lossy(&line);
                    let lower = text.to_ascii_lowercase();
                    let warning = lower.contains("warning:")
                        || lower.contains("warning ")
                        || lower.contains("cmake warning");
                    let error = lower.contains("error:")
                        || lower.contains("error ")
                        || lower.contains("fatal error");
                    if warning {
                        warnings.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    if warning || error {
                        warning_context = 8;
                    }
                    if !quiet || warning_context > 0 {
                        let mut out = io::stderr().lock();
                        writeln!(
                            out,
                            "    cmake | {}",
                            clean(text.trim_end_matches(['\r', '\n']))
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    warning_context = warning_context.saturating_sub(1);
                }
                Ok(())
            })
        };
        let a = spawn_reader(Box::new(stdout));
        let b = spawn_reader(Box::new(stderr));
        let status = child.wait().map_err(|e| e.to_string())?;
        let ar = a.join().map_err(|_| "compiler output reader panicked")?;
        let br = b.join().map_err(|_| "compiler output reader panicked")?;
        ar?;
        br?;
        self.warnings += warning_count.load(std::sync::atomic::Ordering::Relaxed);
        self.event(Event::Info(format!(
            "CMake transcript: {}",
            log_path.display()
        )));
        if !status.success() {
            if quiet {
                let input = std::fs::File::open(log_path).map_err(|e| e.to_string())?;
                let mut tail = std::collections::VecDeque::new();
                let mut reader = BufReader::new(input);
                let mut bytes = vec![];
                loop {
                    bytes.clear();
                    if reader
                        .read_until(b'\n', &mut bytes)
                        .map_err(|e| e.to_string())?
                        == 0
                    {
                        break;
                    }
                    if tail.len() == 40 {
                        tail.pop_front();
                    }
                    tail.push_back(
                        String::from_utf8_lossy(&bytes)
                            .trim_end_matches(['\r', '\n'])
                            .to_owned(),
                    );
                }
                for line in tail {
                    self.line("cmake", &line, "31");
                }
            }
            return Err(format!(
                "CMake exited with {status}; full log: {}",
                log_path.display()
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn controls_are_not_terminal_instructions() {
        assert_eq!(super::clean("a\x1b[31m\nb"), "a\\u{1b}[31m\\u{a}b");
    }
}
