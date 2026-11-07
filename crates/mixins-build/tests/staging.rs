use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "ddmix-stage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn setup(&self) {
        for path in [
            "ddnet/src/engine/client",
            "ddnet/cmake",
            "mods/demo/src/game/client/components",
        ] {
            fs::create_dir_all(self.0.join(path)).unwrap();
        }
        fs::write(self.0.join("ddnet/CMakeLists.txt"),"cmake_minimum_required(VERSION 3.16)\nproject(test LANGUAGES NONE)\nadd_library(game-client INTERFACE)\nconfigure_file(cmake/checksummed_extra.txt checksummed_extra.txt)\n").unwrap();
        fs::write(self.0.join("ddnet/cmake/checksummed_extra.txt"), "extra\n").unwrap();
        fs::write(
            self.0.join("ddnet/src/engine/client/checksum.h"),
            "unsigned m_aFiles[1024];",
        )
        .unwrap();
        fs::write(
            self.0.join("mods/demo/mixin.toml"),
            "[mod]\nid='demo'\nname='Demo'\nversion='1'\n",
        )
        .unwrap();
        fs::write(
            self.0.join("mods/demo/src/game/client/components/demo.cpp"),
            "int example;\n",
        )
        .unwrap();
    }
    fn apply(&self) -> Result<PathBuf, String> {
        mixins_build::apply(&self.0, &self.0.join("ddnet"), &self.0.join("mods"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn isolated_repeatable_and_user_changes_are_protected() {
    let f = Fixture::new();
    f.setup();
    let original = fs::read(f.0.join("ddnet/CMakeLists.txt")).unwrap();
    let staged = f.apply().unwrap();
    assert_eq!(
        fs::read(f.0.join("ddnet/CMakeLists.txt")).unwrap(),
        original
    );
    assert!(!staged.join("src/game/client/components/demo.cpp").exists());
    let first = fs::read(staged.join("CMakeLists.txt")).unwrap();
    let state = fs::read(f.0.join(".ddmix/state.json")).unwrap();
    f.apply().unwrap();
    assert_eq!(fs::read(staged.join("CMakeLists.txt")).unwrap(), first);
    assert_eq!(fs::read(f.0.join(".ddmix/state.json")).unwrap(), state);
    fs::write(staged.join("CMakeLists.txt"), "user edit").unwrap();
    assert!(f.apply().unwrap_err().contains("modified outside"));
    assert_eq!(
        fs::read_to_string(staged.join("CMakeLists.txt")).unwrap(),
        "user edit"
    );
}
#[test]
fn unowned_output_and_overlap_are_refused() {
    let f = Fixture::new();
    f.setup();
    fs::create_dir(f.0.join(".ddmix")).unwrap();
    fs::write(f.0.join(".ddmix/valuable"), "keep").unwrap();
    assert!(f.apply().is_err());
    assert_eq!(
        fs::read_to_string(f.0.join(".ddmix/valuable")).unwrap(),
        "keep"
    );
    assert!(
        mixins_build::apply(&f.0.join("ddnet"), &f.0.join("ddnet"), &f.0.join("mods")).is_err()
    );
}
#[test]
fn overlay_shadowing_is_not_silently_accepted() {
    let f = Fixture::new();
    f.setup();
    fs::create_dir_all(f.0.join("ddnet/src/game/client/components")).unwrap();
    fs::write(
        f.0.join("ddnet/src/game/client/components/demo.cpp"),
        "upstream",
    )
    .unwrap();
    assert!(f.apply().unwrap_err().contains("shadows upstream"));
    assert!(!f.0.join(".ddmix").exists());
}
#[cfg(unix)]
#[test]
fn rejects_symlink_output_and_copy_inputs() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.setup();
    symlink(f.0.join("ddnet"), f.0.join(".ddmix")).unwrap();
    assert!(f.apply().is_err());
    fs::remove_file(f.0.join(".ddmix")).unwrap();
    symlink(f.0.join("mods"), f.0.join("ddnet/link")).unwrap();
    assert!(f.apply().unwrap_err().contains("symlink"));
}
