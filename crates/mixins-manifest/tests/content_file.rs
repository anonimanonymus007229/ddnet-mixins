use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ddmix-hooks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("mod/hooks/nested")).unwrap();
        Self(root)
    }
    fn manifest(&self, fields: &str, kind: &str) -> PathBuf {
        let path = self.0.join("mod/mixin.toml");
        fs::write(&path, format!("[mod]\nid='test'\nname='Test'\nversion='1'\n[[injections]]\nid='hook'\nfile='src/test.cpp'\ntarget='f'\nkind='{kind}'\n{fields}\n")).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn file_is_mod_relative_verbatim_and_not_a_compilation_unit() {
    let f = Fixture::new();
    let snippet = "\r\n\t// Пример: UTF-8\r\n\t(void)this;\r\n";
    fs::write(f.0.join("mod/hooks/nested/snap input.cpp"), snippet).unwrap();
    let path = f.manifest("content_file='hooks/nested/snap input.cpp'", "HEAD");
    let m = mixins_manifest::load(&path).unwrap();
    assert_eq!(m.injections[0].content, snippet);
    assert_eq!(m.injections[0].lower().content.as_deref(), Some(snippet));
    let serialized = toml::to_string(&m).unwrap();
    assert!(serialized.contains("content_file ="));
    assert!(!serialized.contains("content ="));
    fs::write(&path, serialized).unwrap();
    assert_eq!(
        mixins_manifest::load(&path).unwrap().injections[0].content,
        snippet
    );
    let mods = mixins_manifest::discover(&f.0).unwrap();
    assert!(mods[0].files.is_empty());
    fs::write(f.0.join("mod/hooks/nested/snap input.cpp"), "new code").unwrap();
    assert_eq!(
        mixins_manifest::load(&path).unwrap().injections[0].content,
        "new code"
    );
}
#[test]
fn exactly_one_source_required_even_for_explicit_empty_content() {
    let f = Fixture::new();
    for fields in [
        "",
        "content=''\ncontent_file='hooks/test.cpp'",
        "content='code'\ncontent_file='hooks/test.cpp'",
    ] {
        let error = mixins_manifest::load(&f.manifest(fields, "HEAD")).unwrap_err();
        assert!(
            error.contains("exactly one of content or content_file"),
            "{error}"
        );
    }
    assert_eq!(
        mixins_manifest::load(&f.manifest("content=''", "HEAD"))
            .unwrap()
            .injections[0]
            .content,
        ""
    );
    fs::write(f.0.join("mod/hooks/empty.cpp"), "").unwrap();
    assert!(mixins_manifest::load(&f.manifest("content_file='hooks/empty.cpp'", "HEAD")).is_ok());
}
#[test]
fn invalid_paths_missing_files_directories_and_invalid_utf8_fail() {
    let f = Fixture::new();
    fs::write(f.0.join("mod/hooks/invalid.cpp"), [0xff]).unwrap();
    for path in [
        "../outside.cpp",
        "/hooks/test.cpp",
        "C:/hooks/test.cpp",
        "hooks/../outside.cpp",
        "hooks/./test.cpp",
        "hooks//test.cpp",
        "hooks/",
        "hooks\\test.cpp",
        "src/test.cpp",
        "hooks/missing.cpp",
        "hooks/nested",
        "hooks/invalid.cpp",
    ] {
        let error = mixins_manifest::load(&f.manifest(&format!("content_file='{path}'"), "HEAD"))
            .unwrap_err();
        assert!(
            error.contains("injection hook") && error.contains("content_file"),
            "{error}"
        );
    }
}
#[test]
fn wrap_placeholder_is_validated_after_reading_hook() {
    let f = Fixture::new();
    let path = f.manifest("content_file='hooks/wrap.cpp'", "WRAP");
    fs::write(
        f.0.join("mod/hooks/wrap.cpp"),
        "before(); {{original}} after();",
    )
    .unwrap();
    assert!(mixins_manifest::load(&path).is_ok());
    for snippet in ["missing placeholder", "{{original}} {{original}}"] {
        fs::write(f.0.join("mod/hooks/wrap.cpp"), snippet).unwrap();
        assert!(mixins_manifest::load(&path).is_err());
    }
}
#[cfg(unix)]
#[test]
fn hook_symlinks_are_rejected_even_when_they_stay_in_mod() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    fs::write(f.0.join("mod/hooks/real.cpp"), "code").unwrap();
    symlink("real.cpp", f.0.join("mod/hooks/link.cpp")).unwrap();
    let error =
        mixins_manifest::load(&f.manifest("content_file='hooks/link.cpp'", "HEAD")).unwrap_err();
    assert!(error.contains("symlink"));
    symlink("nested", f.0.join("mod/hooks/linked-dir")).unwrap();
    let error = mixins_manifest::load(&f.manifest("content_file='hooks/linked-dir/x.cpp'", "HEAD"))
        .unwrap_err();
    assert!(error.contains("symlink"));
}
