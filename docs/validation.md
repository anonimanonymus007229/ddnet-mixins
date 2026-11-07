# Development validation

## Supplied corpus

- Repository: https://github.com/ddnet/ddnet
- Commit: `176fc2f9ab85ace703c6ec10da0249b01995ba94`
- Commit subject: `Bump version to 20.2.1`
- Local root: `workspace/ddnet` (ignored, not vendored)
- Upstream working tree clean before and after validation.

| Metric | Result |
|---|---:|
| C++ headers/source files | 728 |
| Source bytes | 10,181,240 |
| Tokens including trivia | 2,355,516 |
| Files containing conditionals | 415 |
| Lexical diagnostics | 0 |
| Indexed function definitions | 7,300 |
| Indexed classes | 1,100 |
| Unsupported structural regions | 1,997 |
| Structural diagnostics | 14 |

Structural diagnostics are in bundled external headers (conditional `extern
"C"`-style scope changes); there are no structural diagnostics in DDNet-authored
files in this checkout. Unsupported-region counts are not a count of invalid
C++: they include deliberately skipped initializers/complex declarations.
Zero lexical diagnostics never meant full C++ grammar coverage.

The lexer retains its original conservative unconditional delimiter map. The
new parser's separate branch-aware map supports balanced branch-local and
enclosing delimiters. Divergent branch stacks invalidate enclosing targets
without choosing any branch active.

## Executed locally (Windows, Rust 1.99.1)

- `cargo test --workspace`: lexer, manifest, parser, transform and CLI tests.
- `cargo test --workspace -- --ignored --nocapture`: four explicit real-DDNet
  integration tests passed (lexer scan, parser scan, real selector/body tests,
  and real HEAD/TAIL/config rendering).
- Real `CGameClient::OnInit`, `OnRender` and `OnConsoleInit` resolved with exact
  paired body braces; CGameClient class found in the actual header.
- `CCollision::GetIndex` overload ambiguity rejected; exact `(int Nx, int Ny)`
  parameter selector resolves one definition.
- HEAD/TAIL rendering on the actual OnInit source preserves all original bytes
  when inserted snippets are removed.
- Config insertion resolves the real reserved comment and preserves its
  repeated-include/macro design. Duplicate identifiers are not checked yet.
- `ddmix plan --ddnet workspace/ddnet --mods examples`: three patches, across
  the real gameclient.cpp and config_variables.h, without modifying upstream.
- CLI regression verifies identical plan/check output, path traversal rejection,
  and no writes to source or `.ddmix`.
- Formatting, Clippy with warnings denied, and native release build passed.

Machine-readable local output is under `target/reports/` (not committed).
`inspect` schema version 2 reports functions/classes/unsupported regions per
file, plus source-located unsupported and structural diagnostics. A complete
scan returns failure if structural diagnostics occur, even in external code;
it still emits the full report for inspection.

GitHub Actions runs Windows/Linux/macOS validation and all explicit corpus tests
against the pinned DDNet checkout. These remote jobs have not been observed.

## Remaining gaps

No staged apply, idempotency state, generated CMake integration, component
registration, config collision analysis, complete C++ AST, or DDNet client build
exists yet. Source-only plan/check do not validate build integration or snippet
semantics. No DDNet configure/build was attempted. Linux/macOS binaries and Rust
1.85 minimum-version compatibility have not been tested locally.
