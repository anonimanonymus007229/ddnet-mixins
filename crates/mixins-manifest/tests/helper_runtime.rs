//! Compile and execute generated helpers to verify binding forwarding/mutation.
use std::{fs, path::PathBuf, process::Command};
#[test]
fn compiled_head_tail_helpers_forward_references_and_return_to_target() {
    let root = std::env::temp_dir().join(format!("ddmix-helper-runtime-{}", std::process::id()));
    fs::create_dir_all(root.join("hooks")).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    fs::write(root.join("hooks/body.cpp"), "x += 3; return;\n").unwrap();
    fs::write(root.join("mixin.toml"),"[mod]\nid='runtime'\nname='Runtime'\nversion='1'\n[[injections]]\nid='head'\nfile='src/test.cpp'\ntarget='f'\nkind='HEAD'\nhelper='hooks/body.cpp'\nbindings=[{name='x',ty='int &',value='state'}]\n[[injections]]\nid='tail'\nfile='src/test.cpp'\ntarget='g'\nkind='TAIL'\nhelper='hooks/body.cpp'\nbindings=[{name='x',ty='int &',value='state'}]\n").unwrap();
    let manifest = mixins_manifest::load(&root.join("mixin.toml")).unwrap();
    let source = "int f(int &state) { return state; }\nvoid g(int &state) { state += 2; }\nint main() { int a=4,b=0; int result=f(a); g(b); return (a==7 && result==7 && b==5) ? 0 : 1; }\n";
    let mut edits = vec![];
    for i in &manifest.injections {
        edits.extend(mixins_transform::helpers::lower("runtime", i, source).unwrap());
    }
    let refs: Vec<_> = edits.iter().map(|e| ("runtime", e)).collect();
    let output = mixins_transform::Plan::resolve(source, &refs)
        .unwrap()
        .render(source)
        .unwrap();
    fs::write(root.join("main.cpp"), output).unwrap();
    fs::write(root.join("CMakeLists.txt"),"cmake_minimum_required(VERSION 3.16)\nproject(helper_runtime LANGUAGES CXX)\nadd_executable(helper_runtime main.cpp)\n").unwrap();
    let configure = Command::new("cmake")
        .arg("-S")
        .arg(&root)
        .arg("-B")
        .arg(root.join("build"))
        .output()
        .unwrap();
    assert!(
        configure.status.success(),
        "{}",
        String::from_utf8_lossy(&configure.stderr)
    );
    let build = Command::new("cmake")
        .arg("--build")
        .arg(root.join("build"))
        .args(["--config", "Release"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let binary = if cfg!(windows) {
        "helper_runtime.exe"
    } else {
        "helper_runtime"
    };
    let executable = [
        root.join("build/Release").join(binary),
        root.join("build").join(binary),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .expect("compiled runtime probe missing");
    assert!(
        Command::new(executable).status().unwrap().success(),
        "reference forwarding or helper return semantics changed"
    );
}
