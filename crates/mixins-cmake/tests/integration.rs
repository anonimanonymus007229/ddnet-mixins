use mixins_cmake::{generate, integrate, quote_path};
#[test]
fn branches_comments_strings_and_crlf_do_not_shift_anchor() {
    let source = "#[=[ add_executable(game-client fake) ]=]\r\nif(CLIENT)\r\n if(ANDROID)\r\n add_library(game-client SHARED \"paren).cpp\")\r\n else()\r\n add_executable(game-client main.cpp)\r\n endif()\r\n add_dependencies(game-client generated)\r\nendif()\r\nconfigure_file(cmake/checksummed_extra.txt checksummed_extra.txt)\r\n";
    let out = integrate(source, 1024, 22).unwrap();
    let include = out.find("# DDMIX-BEGIN: generated").unwrap();
    assert!(include > out.find("add_executable(game-client main").unwrap());
    assert!(include < out.find("add_dependencies").unwrap());
    assert!(!out.replace("\r\n", "").contains('\n'));
    assert_eq!(
        out.matches("# DDMIX-BEGIN: generated integration").count(),
        1
    );
    assert!(integrate(&out, 1024, 22).is_err());
}
#[test]
fn rejects_missing_or_ambiguous_creation_and_injected_paths() {
    for source in [
        "# absent",
        "if(X)\nadd_library(game-client SHARED x)\nendif()\nadd_executable(game-client x)\n",
        "add_executable(game-client \"unterminated)",
    ] {
        assert!(integrate(source, 1024, 22).is_err());
    }
    for path in ["a;b.cpp", "a${x}.cpp", "a\nb.cpp"] {
        assert!(quote_path(path).is_err());
    }
    assert_eq!(
        quote_path("C:\\some path\\x.cpp").unwrap(),
        "\"C:/some path/x.cpp\""
    );
    let generated = generate(
        &["b.cpp".into(), "a.cpp".into(), "b.cpp".into()],
        &["inc".into()],
    )
    .unwrap();
    assert!(generated.find("a.cpp").unwrap() < generated.find("b.cpp").unwrap());
    assert!(generated.contains("if(NOT DEV)"));
}
#[test]
#[ignore = "requires supplied DDNet checkout"]
fn actual_ddnet_platform_target_and_checksum_anchors() {
    let root = std::env::var_os("DDMIX_TEST_DDNET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspace/ddnet")
        });
    let source = std::fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    let capacity = mixins_cmake::checksum_capacity(
        &std::fs::read_to_string(root.join("src/engine/client/checksum.h")).unwrap(),
    )
    .unwrap();
    assert_eq!(capacity, 1024);
    let output = integrate(&source, capacity, 22).unwrap();
    let marker = output.find("# DDMIX-BEGIN: generated integration").unwrap();
    assert!(marker > output.find("add_executable(game-client WIN32").unwrap());
    assert!(
        marker
            < output
                .find("add_dependencies(game-client generate_content_types)")
                .unwrap()
    );
    assert!(
        output.find("# DDMIX-BEGIN: checksum capacity").unwrap()
            < output
                .find("configure_file(cmake/checksummed_extra.txt")
                .unwrap()
    );
}
