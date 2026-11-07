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
            "ddmix-helper-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(p.join("hooks")).unwrap();
        fs::write(p.join("hooks/f.cpp"), "(void)x; return;").unwrap();
        Self(p)
    }
    fn load(&self, fields: &str) -> Result<mixins_manifest::Manifest, String> {
        let p = self.0.join("mixin.toml");
        fs::write(&p,format!("[mod]\nid='test'\nname='Test'\nversion='1'\n[[injections]]\nid='hook'\nfile='src/test.cpp'\ntarget='f'\n{fields}\n")).unwrap();
        mixins_manifest::load(&p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn helper_reads_body_serializes_and_requires_source_aware_lowering() {
    let f = Fixture::new();
    let m = f
        .load("kind='HEAD'\nhelper='hooks/f.cpp'\nbindings=[{name='x',ty='int &',value='member'}]")
        .unwrap();
    let i = &m.injections[0];
    assert_eq!(i.helper_content, "(void)x; return;");
    assert!(i.lower().is_err());
    let serialized = toml::to_string(&m).unwrap();
    assert!(serialized.contains("helper ="));
    assert!(!serialized.contains("content ="));
    let src = "void f() {}";
    let edits = mixins_transform::helpers::lower("test", i, src).unwrap();
    let refs: Vec<_> = edits.iter().map(|e| ("test", e)).collect();
    let output = mixins_transform::Plan::resolve(src, &refs)
        .unwrap()
        .render(src)
        .unwrap();
    assert!(output.contains("static void ddmix_helper_"));
    assert!(output.contains("int & x"));
    assert!(output.contains("((member));"));
    let m = mixins_manifest::load(&f.0.join("mixin.toml")).unwrap();
    assert_eq!(m.injections[0].helper_content, i.helper_content);
}
#[test]
fn invalid_sources_bindings_and_kinds_fail() {
    let f = Fixture::new();
    for fields in [
        "kind='HEAD'\nhelper='hooks/f.cpp'\ncontent='x'",
        "kind='HEAD'\nhelper='hooks/f.cpp'\ncontent_file='hooks/f.cpp'",
        "kind='OVERWRITE'\nhelper='hooks/f.cpp'",
        "kind='HEAD'\ncontent=''\nbindings=[{name='x',ty='int',value='1'}]",
        "kind='HEAD'\nhelper='../outside.cpp'",
        "kind='HEAD'\nhelper='hooks/missing.cpp'",
        "kind='HEAD'\nhelper='hooks/f.cpp'\nbindings=[{name='return',ty='int',value='1'}]",
        "kind='HEAD'\nhelper='hooks/f.cpp'\nbindings=[{name='x',ty='int',value='1'},{name='x',ty='int',value='2'}]",
        "kind='HEAD'\nhelper='hooks/f.cpp'\nbindings=[{name='x',ty='int); void bad(',value='1'}]",
        "kind='HEAD'\nhelper='hooks/f.cpp'\nbindings=[{name='x',ty='int',value='1; bad()'}]",
    ] {
        assert!(f.load(fields).is_err(), "{fields}");
    }
    for body in [
        "return 3;",
        "return",
        "this->Reset();",
        "#include <x>\n",
        "unclosed(",
    ] {
        fs::write(f.0.join("hooks/f.cpp"), body).unwrap();
        assert!(
            f.load("kind='HEAD'\nhelper='hooks/f.cpp'").is_err(),
            "{body}"
        );
    }
}
#[test]
fn rejects_inline_namespaced_conditional_targets_and_allows_multiple_helpers() {
    let f = Fixture::new();
    let m = f
        .load("kind='HEAD'\nhelper='hooks/f.cpp'\nbindings=[{name='x',ty='int',value='1'}]")
        .unwrap();
    for (name, source) in [
        ("C::f", "class C {public: void f() {} };"),
        ("N::f", "namespace N { void f() {} }"),
        ("f", "#if X\nvoid f() {}\n#endif\n"),
        ("f", "template<class T> void f(T x) {}"),
    ] {
        let mut i = m.injections[0].clone();
        i.target = name.into();
        assert!(mixins_transform::helpers::lower("test", &i, source).is_err());
    }
    let source = "void f() {}";
    let a = mixins_transform::helpers::lower("one", &m.injections[0], source).unwrap();
    let b = mixins_transform::helpers::lower("two", &m.injections[0], source).unwrap();
    assert_ne!(a[0].content, b[0].content);
    let refs: Vec<_> = a
        .iter()
        .map(|e| ("one", e))
        .chain(b.iter().map(|e| ("two", e)))
        .collect();
    let output = mixins_transform::Plan::resolve(source, &refs)
        .unwrap()
        .render(source)
        .unwrap();
    assert_eq!(output.matches("static void ddmix_helper_").count(), 2);
}
