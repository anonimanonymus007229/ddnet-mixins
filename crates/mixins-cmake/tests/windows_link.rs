//! Check platform gating and compatibility with both host link signatures.
use std::{fs, path::PathBuf, process::Command};

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn configure(signature: &str, overlay: bool) {
    let root = std::env::temp_dir().join(format!(
        "ddmix-windows-link-{signature}-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let _cleanup = Cleanup(root.clone());
    fs::write(root.join("host.cpp"), "int host;").unwrap();
    fs::write(root.join("overlay.cpp"), "int overlay;").unwrap();
    let files = if overlay {
        vec![
            root.join("overlay.cpp")
                .to_str()
                .unwrap()
                .replace('\\', "/"),
        ]
    } else {
        vec![]
    };
    fs::write(
        root.join("generated.cmake"),
        mixins_cmake::generate(&files, &[]).unwrap(),
    )
    .unwrap();
    let keyword = if signature == "keyword" {
        "PRIVATE "
    } else {
        ""
    };
    fs::write(
        root.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.16)
project(native_link_probe LANGUAGES CXX)
add_library(game-client STATIC host.cpp)
add_library(host-library INTERFACE)
include("${{CMAKE_CURRENT_SOURCE_DIR}}/generated.cmake")
target_link_libraries(game-client {keyword}host-library)
get_target_property(LINKS game-client LINK_LIBRARIES)
if(WIN32 AND MSVC)
  if(NOT "synchronization" IN_LIST LINKS)
    message(FATAL_ERROR "Missing Rust Windows synchronization dependency")
  endif()
else()
  if("synchronization" IN_LIST LINKS)
    message(FATAL_ERROR "Windows MSVC dependency leaked onto another platform")
  endif()
endif()
if(NOT "host-library" IN_LIST LINKS)
  message(FATAL_ERROR "Host link dependencies were lost")
endif()
"#
        ),
    )
    .unwrap();
    let output = Command::new("cmake")
        .arg("-S")
        .arg(&root)
        .arg("-B")
        .arg(root.join("build"))
        .output()
        .expect("CMake must be available for this integration test");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn windows_rust_dependency_supports_plain_host_linking_without_overlays() {
    configure("plain", false);
}

#[test]
fn windows_rust_dependency_supports_keyword_host_linking_with_overlays() {
    configure("keyword", true);
}
