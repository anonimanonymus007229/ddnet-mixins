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
| Indexed function definitions | 8,121 |
| Indexed classes | 1,119 |
| Declaration-only functions/methods | 6,632 |
| Unsupported structural regions | 1,273 |
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
- `cargo test --workspace -- --ignored --nocapture`: nine explicit real-DDNet
  integration tests passed (lexer scan, parser scan, real selector/body tests,
  HEAD/TAIL/config rendering, real CMake anchors, repeated real-DDNet staging).
- Real `CGameClient::OnInit`, `OnRender` and `OnConsoleInit` resolved with exact
  paired body braces; CGameClient class found in the actual header.
- `CCollision::GetIndex` overload ambiguity rejected; exact `(int Nx, int Ny)`
  parameter selector resolves one definition.
- HEAD/TAIL rendering on the actual OnInit source preserves all original bytes
  when inserted snippets are removed.
- Config insertion resolves the real reserved comment and preserves its
  repeated-include/macro design. Duplicate identifiers/console names are checked
  after rendering; existing mutually exclusive declarations remain valid.
- `ddmix plan --ddnet workspace/ddnet --mods examples`: three patches, across
  the real gameclient.cpp and config_variables.h, without modifying upstream.
- CLI regression verifies identical plan/check output, path traversal rejection,
  and no writes to source or `.ddmix` during plan/check.
- Real `apply` run twice: identical ownership state/generated files, one managed
  include and one HEAD snippet, upstream unchanged. External overlay not copied.
- Unit tests reject manually modified or unowned staging, source/staging path
  overlap and overlays shadowing upstream. Unix-only symlink tests are in CI.
- CMake itself configures a dependency-free target probe and confirms generated
  source/include/checksum properties, including paths with spaces.
- Formatting, Clippy with warnings denied, and native release build passed.

Machine-readable local output is under `target/reports/` (not committed).
`inspect` schema version 2 reports functions/classes/unsupported regions per
file, plus source-located unsupported and structural diagnostics. A complete
scan returns failure if structural diagnostics occur, even in external code;
it still emits the full report for inspection.

GitHub Actions runs Windows/Linux/macOS validation and all explicit corpus tests
against the pinned DDNet checkout. These remote jobs have not been observed.

## Remaining gaps

Initial configure failed because bundled dependencies were absent. A separate
DDNet libraries checkout at `c0e6703fbcdbe03df2f26875427ec3951ce4ec21` was fetched
under workspace/dependencies and copied only into staging via --libraries.
MSVC 19.51.36260.0 / Python 3.12.10 successfully configured and built the real
Windows Release game-client with the declared CDdmixExample component in all
and input collections. Output: `workspace/component-build/Release/DDNet.exe`.
Logs: `target/reports/full-client-build.log` and `all-features-build.log`.
The second successful build includes every exposed injection kind, the config
entry, both overlay compilation units and the registered component. Actual
Visual Studio target and checksummed_files.txt include the mod files. Post-build
SHA-256 ownership inventory matches state.json exactly. Client binary SHA-256:
`39c21fbb539448f060214abad3a5cd06885efde342d500b5431e3f53718b3dac`.
VIDEORECORDER, VULKAN, SERVER, TOOLS and DOWNLOAD_GTEST were disabled.

The build exposed Python __pycache__ writes into source. The wrapper now sets
PYTHONDONTWRITEBYTECODE=1; generated cache files from the initial validation were
removed selectively before repeating validation. Original DDNet remains clean.

The regular suite has 27 passing tests; explicit corpus tests cover config
collisions, real component lowering/diff and real advanced selectors in addition
to earlier scans/staging. Release packaging workflow covers Windows x86_64,
Linux x86_64, macOS Intel/ARM64 but has not been run remotely. A local Windows
archive is generated under target/dist. No public tag/release is published.

Remaining: no complete C++ semantic AST, macro expansion, arbitrary nested call
rewrites or complex return control-flow transformation. Advanced aliases have
explicit conservative semantics documented in advanced-injections.md. The client
was compiled, not interactively exercised in a game session. Copy staging omits
Git metadata; concurrent mutation/hostile filesystem races remain unsupported.
Linux/macOS binaries and Rust 1.85 minimum-version compatibility have not been
tested locally. See [staging workflow](staging.md).
