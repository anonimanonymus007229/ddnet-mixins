# Development validation

## Supplied corpus

- Repository: https://github.com/ddnet/ddnet
- Commit: `176fc2f9ab85ace703c6ec10da0249b01995ba94`
- Commit subject: `Bump version to 20.2.1`
- Local root: `workspace/ddnet` (ignored, not vendored)
- Upstream working tree clean before and after validation.

`ddmix inspect --ddnet workspace/ddnet` results:

| Metric | Result |
|---|---:|
| C++ headers/source files | 728 |
| Source bytes | 10,181,240 |
| Tokens including trivia | 2,355,516 |
| Files containing conditionals | 415 |
| Lexical/preprocessor/delimiter diagnostics | 0 |

Zero diagnostics does **not** mean complete C++ parsing. This milestone does
not implement a structural parser. Conditional files deliberately have no
delimiter map until branch-aware structural analysis is implemented.

## Executed locally (Windows, Rust 1.99.1)

- `cargo test --workspace`: six unit/integration tests passed; explicit corpus
  test ignored in the ordinary suite by design.
- `cargo test -p mixins-lexer --test ddnet_corpus -- --ignored --nocapture`:
  passed against all 728 files, including contiguous UTF-8-safe token spans and
  symmetric delimiter-map checks.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo build --release`: native `target/release/ddmix.exe` generated.
- Native CLI help and corpus inspection exercised.

Machine-readable local output is under `target/reports/` (not committed).
GitHub Actions includes Windows/Linux/macOS Rust validation and a separate
pinned real-DDNet corpus job; these remote jobs have not been observed running.

## Not yet verified / not implemented

No transformed source, staging copy, CMake integration, component registration,
HEAD/TAIL injections, or DDNet client build exists yet. No DDNet configure/build
was attempted: it would not validate an integration engine that is not present.
No claim of first-release completeness is made. Linux/macOS binaries and Rust
1.85 minimum-version compatibility have not been tested locally.
