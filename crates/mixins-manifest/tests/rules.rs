use mixins_core::progress::Event;
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
            "ddmix-rules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for d in ["mods/demo/hooks", "TClient", "BestClient"] {
            fs::create_dir_all(p.join(d)).unwrap();
        }
        Self(p)
    }
    fn write(&self, s: &str) {
        fs::write(
            self.0.join("mods/demo/mixin.toml"),
            format!("[mod]\nid='demo'\nname='Demo'\nversion='1'\n{s}"),
        )
        .unwrap();
    }
    fn load(&self, name: &str) -> Result<Vec<mixins_manifest::Mod>, String> {
        mixins_manifest::discover_for(&self.0.join("mods"), &self.0.join(name), &mut |_| {})
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn universal_filters_precede_validation_file_reads_and_duplicate_ids() {
    let f = Fixture::new();
    f.write("[[edits]]\nid='same'\nclients={include=['T*'],exclude=['Best*']}\nfile='src/test.cpp'\naction='INSERT'\ntarget={kind='file'}\nposition='START'\ncontent_file='hooks/config.cpp'\n[[edits]]\nid='same'\nclients={include=['BestClient']}\nfile='src/other.cpp'\naction='INSERT'\ntarget={kind='file'}\nposition='START'\ncontent='other'\n[[injections]]\nclients={exclude=['*']}\nhelper='hooks/missing.cpp'\n[[component]]\nclients={include=[]}\nclass='invalid'\n");
    fs::write(f.0.join("mods/demo/hooks/config.cpp"), "\r\n// config\r\n").unwrap();
    let t = f.load("TClient").unwrap();
    assert_eq!(t[0].manifest.edits.len(), 1);
    assert_eq!(
        t[0].manifest.edits[0].content.as_deref(),
        Some("\r\n// config\r\n")
    );
    assert!(t[0].manifest.injections.is_empty());
    assert!(t[0].manifest.components.is_empty());
    let b = f.load("BestClient").unwrap();
    assert_eq!(b[0].manifest.edits[0].file, "src/other.cpp");
    let mut events = vec![];
    mixins_manifest::discover_for(&f.0.join("mods"), &f.0.join("TClient"), &mut |e| {
        events.push(e)
    })
    .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Skipped(_)))
            .count(),
        3
    );
    assert!(
        mixins_manifest::load(&f.0.join("mods/demo/mixin.toml"))
            .unwrap_err()
            .contains("requires --ddnet")
    );
}
#[test]
fn grouped_members_expand_and_single_priority_remains_available() {
    let f = Fixture::new();
    f.write("[[components]]\nregister_in=['all']\nclients={include=['?Client']}\nmembers=[{class='CA',header='game/client/a.h',member='m_A'},{class='CB',header='game/client/b.h',member='m_B',clients={exclude=['TClient']}}]\n[[component]]\nclass='CC'\nheader='game/client/c.h'\nmember='m_C'\nregister_in=['all','input']\npriority=8\n");
    let t = f.load("TClient").unwrap();
    let c = &t[0].manifest.components;
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].member, "m_A");
    assert_eq!(c[0].priority, 0);
    assert_eq!(c[1].priority, 8);
    let b = f.load("BestClient").unwrap();
    assert_eq!(b[0].manifest.components.len(), 1);
}
#[test]
fn rejects_bad_filters_group_priority_and_unsafe_edit_content_paths() {
    let f = Fixture::new();
    for text in [
        "[[edits]]\nclients={include=['../TClient']}",
        "[[edits]]\nclients={includes=['TClient']}",
        "[[components]]\npriority=2\nregister_in=['all']\nmembers=[]",
        "[[components]]\nregister_in=['all']\nmembers=[]",
        "[[edits]]\nid='a'\nfile='src/a.cpp'\naction='INSERT'\ncontent='a'\ncontent_file='hooks/a.cpp'",
        "[[edits]]\nid='a'\nfile='src/a.cpp'\naction='REMOVE'\ncontent_file='hooks/a.cpp'",
        "[[edits]]\nid='a'\nfile='src/a.cpp'\naction='INSERT'\ncontent_file='../secret'",
    ] {
        f.write(text);
        assert!(f.load("TClient").is_err(), "{text}");
    }
}
