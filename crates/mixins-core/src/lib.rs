//! Shared diagnostics and deterministic, symlink-rejecting file discovery.
pub mod progress;
use serde::Serialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

/// Never follow symlinks (including directory links) while discovering inputs.
/// Callers canonicalize their explicitly supplied root once before traversal.
pub fn source_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "symlink is not permitted in source tree: {}",
                        path.display()
                    ),
                ));
            }
            if entry.file_type()?.is_dir() {
                walk(&path, files)?;
            } else if matches!(path.extension().and_then(|s| s.to_str()), Some("cpp" | "h")) {
                files.push(path);
            }
        }
        Ok(())
    }
    if fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("symlink source root is not permitted: {}", root.display()),
        ));
    }
    let mut files = Vec::new();
    walk(root, &mut files)?;
    files.sort();
    Ok(files)
}
