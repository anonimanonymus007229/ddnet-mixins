use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let r = std::env::temp_dir().join(format!(
            "ddmix-dep-links-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for p in ["ddnet/src/engine/client", "ddnet/cmake", "mods", "libs"] {
            fs::create_dir_all(r.join(p)).unwrap();
        }
        fs::write(r.join("ddnet/CMakeLists.txt"),"add_library(game-client INTERFACE)\nconfigure_file(cmake/checksummed_extra.txt checksummed_extra.txt)\n").unwrap();
        fs::write(
            r.join("ddnet/src/engine/client/checksum.h"),
            "unsigned m_aFiles[1024];",
        )
        .unwrap();
        fs::write(r.join("ddnet/cmake/checksummed_extra.txt"), "extra\n").unwrap();
        Self(r)
    }
    fn apply(&self) -> Result<PathBuf, String> {
        mixins_build::apply_with_dependencies(
            &self.0,
            &self.0.join("ddnet"),
            &self.0.join("mods"),
            Some(&self.0.join("libs")),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn link(target: &Path, path: &Path, directory: bool) -> bool {
    #[cfg(windows)]
    let normalized = PathBuf::from(target.to_string_lossy().replace('/', "\\"));
    #[cfg(windows)]
    let target = normalized.as_path();
    #[cfg(unix)]
    let result = {
        let _ = directory;
        std::os::unix::fs::symlink(target, path)
    };
    #[cfg(windows)]
    let result = if directory {
        std::os::windows::fs::symlink_dir(target, path)
    } else {
        std::os::windows::fs::symlink_file(target, path)
    };
    match result {
        Ok(()) => true,
        Err(e) => {
            #[cfg(windows)]
            if e.raw_os_error() == Some(1314) {
                eprintln!("SKIP symlink creation requires Windows Developer Mode/privilege: {e}");
                return false;
            }
            panic!("cannot create test link: {e}");
        }
    }
}
#[test]
fn normal_dependencies_copy_and_exclude_git_metadata() {
    let f = Fixture::new();
    fs::write(f.0.join("libs/library.lib"), "bytes").unwrap();
    fs::write(f.0.join("libs/.git"), "gitdir: ../external").unwrap();
    let staged = f.apply().unwrap();
    assert_eq!(
        fs::read(staged.join("ddnet-libs/library.lib")).unwrap(),
        b"bytes"
    );
    assert!(!staged.join("ddnet-libs/.git").exists());
}
#[test]
fn framework_links_materialize_without_following_writes_and_apply_is_repeatable() {
    let f = Fixture::new();
    let libs = f.0.join("libs");
    fs::create_dir_all(libs.join("sdl/mac/SDL2.framework/Versions/A/Headers")).unwrap();
    fs::create_dir_all(libs.join("sdl/include")).unwrap();
    fs::write(
        libs.join("sdl/mac/SDL2.framework/Versions/A/Headers/SDL.h"),
        "header",
    )
    .unwrap();
    fs::write(
        libs.join("sdl/mac/SDL2.framework/Versions/A/SDL2"),
        "binary",
    )
    .unwrap();
    if !link(
        Path::new("A"),
        &libs.join("sdl/mac/SDL2.framework/Versions/Current"),
        true,
    ) {
        return;
    }
    assert!(link(
        Path::new("Versions/Current/Headers"),
        &libs.join("sdl/mac/SDL2.framework/Headers"),
        true
    ));
    assert!(link(
        Path::new("Versions/Current/SDL2"),
        &libs.join("sdl/mac/SDL2.framework/SDL2"),
        false
    ));
    assert!(link(
        Path::new("../mac/SDL2.framework/Headers/"),
        &libs.join("sdl/include/mac"),
        true
    ));
    let staged = f.apply().unwrap();
    let root = staged.join("ddnet-libs");
    assert_eq!(
        fs::read(root.join("sdl/include/mac/SDL.h")).unwrap(),
        b"header"
    );
    assert_eq!(
        fs::read(root.join("sdl/mac/SDL2.framework/SDL2")).unwrap(),
        b"binary"
    );
    for p in [
        "sdl/include/mac",
        "sdl/mac/SDL2.framework/Headers",
        "sdl/mac/SDL2.framework/Versions/Current",
        "sdl/mac/SDL2.framework/SDL2",
    ] {
        assert!(
            !fs::symlink_metadata(root.join(p))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    let state = fs::read(f.0.join(".ddmix/state.json")).unwrap();
    f.apply().unwrap();
    assert_eq!(state, fs::read(f.0.join(".ddmix/state.json")).unwrap());
    fs::write(root.join("sdl/include/mac/SDL.h"), "modified staged copy").unwrap();
    assert_eq!(
        fs::read(libs.join("sdl/include/mac/SDL.h")).unwrap(),
        b"header"
    );
    assert!(f.apply().unwrap_err().contains("modified outside"));
}
#[test]
fn external_broken_and_cyclic_links_fail_before_transaction() {
    for mode in ["outside", "broken", "cycle"] {
        let f = Fixture::new();
        let libs = f.0.join("libs");
        fs::write(f.0.join("secret"), "private").unwrap();
        let target = match mode {
            "outside" => Path::new("../secret"),
            "broken" => Path::new("missing"),
            _ => Path::new("."),
        };
        if !link(target, &libs.join("unsafe"), mode == "cycle") {
            return;
        }
        let error = f.apply().unwrap_err();
        assert!(
            match mode {
                "outside" => error.contains("escapes dependency root"),
                "broken" => error.contains("cannot resolve dependency"),
                _ => error.contains("cycle"),
            },
            "{mode}: {error}"
        );
        assert!(!f.0.join(".ddmix-next").exists());
        assert!(!f.0.join(".ddmix").exists());
    }
}
