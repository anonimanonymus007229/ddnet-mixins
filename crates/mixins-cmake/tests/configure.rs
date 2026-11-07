//! Exercise generated CMake through CMake itself, independent of DDNet dependencies.
#[test]
fn generated_overlay_attaches_sources_includes_and_checksums() {
    use std::{fs, process::Command};
    let root = std::env::temp_dir().join(format!("ddmix cmake {}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let src = root.join("external source.cpp");
    fs::write(&src, "int x;").unwrap();
    let inc = root.join("external include");
    fs::create_dir(&inc).unwrap();
    let source = src.to_str().unwrap().replace('\\', "/");
    let include = inc.to_str().unwrap().replace('\\', "/");
    fs::write(
        root.join("generated.cmake"),
        mixins_cmake::generate(
            std::slice::from_ref(&source),
            std::slice::from_ref(&include),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(root.join("CMakeLists.txt"),"cmake_minimum_required(VERSION 3.16)\nproject(probe LANGUAGES CXX)\nadd_library(game-client STATIC)\ninclude(\"${CMAKE_CURRENT_SOURCE_DIR}/generated.cmake\")\nget_target_property(S game-client SOURCES)\nget_target_property(I game-client INCLUDE_DIRECTORIES)\nfile(WRITE \"${CMAKE_BINARY_DIR}/observed.txt\" \"${S}\\n${I}\\n${CHECKSUM_SRC}\\n\")\n").unwrap();
    let output = Command::new("cmake")
        .arg("-S")
        .arg(&root)
        .arg("-B")
        .arg(root.join("build"))
        .output()
        .expect("CMake must be available for this integration test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let observed = fs::read_to_string(root.join("build/observed.txt")).unwrap();
    assert_eq!(
        observed.lines().collect::<Vec<_>>(),
        [source.as_str(), include.as_str(), source.as_str()]
    );
}
