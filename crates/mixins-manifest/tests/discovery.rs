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
            "ddmix-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn add(&self, id: &str, extra: &str) {
        let p = self.0.join(id);
        fs::create_dir(&p).unwrap();
        fs::write(
            p.join("mixin.toml"),
            format!("[mod]\nid = '{id}'\nname = '{id}'\nversion = '1.0.0'\n{extra}"),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn dependencies_override_lexical_order_and_overlay_stays_external() {
    let f = Fixture::new();
    f.add("a", "[dependencies]\nz = '*'");
    f.add("z", "");
    let src = f.0.join("a/src/game/client/components");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("test.cpp"), "").unwrap();
    fs::write(src.join("ignored.txt"), "").unwrap();
    let mods = mixins_manifest::discover(&f.0).unwrap();
    assert_eq!(
        mods.iter()
            .map(|m| m.manifest.package.id.as_str())
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(mods[1].files.len(), 1);
    assert_eq!(
        mods[1].files[0].logical,
        "src/game/client/components/test.cpp"
    );
    assert_eq!(mods[1].files[0].target, "game-client");
}
#[test]
fn cycles_missing_dependencies_unknown_fields_and_versions_fail() {
    let f = Fixture::new();
    f.add("a", "[dependencies]\nb = '*'");
    assert!(
        mixins_manifest::discover(&f.0)
            .unwrap_err()
            .contains("missing")
    );
    f.add("b", "[dependencies]\na = '*'");
    assert!(
        mixins_manifest::discover(&f.0)
            .unwrap_err()
            .contains("cycle")
    );
    for extra in [
        "[[components]]\nclass = 'CTest'",
        "[[edits]]\naction = 'INSERT'",
        "[compatibility]\nddnet = '>=20'",
    ] {
        let f = Fixture::new();
        f.add("a", extra);
        assert!(mixins_manifest::discover(&f.0).is_err());
    }
}
#[test]
fn unsupported_overlay_target_fails() {
    let f = Fixture::new();
    f.add("a", "");
    fs::create_dir_all(f.0.join("a/src/engine")).unwrap();
    fs::write(f.0.join("a/src/engine/test.cpp"), "").unwrap();
    assert!(
        mixins_manifest::discover(&f.0)
            .unwrap_err()
            .contains("no supported target")
    );
}
