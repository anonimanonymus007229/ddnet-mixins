# DDNet Mixins

Rust tooling for independent DDNet mods without editing the upstream checkout.
**Early implementation:** conservative C++ indexing, generic edit planning,
isolated staging and CMake source-overlay integration. Full DDNet compilation
is currently blocked locally by missing dependencies; components are not yet supported.

## Build and run

```sh
cargo build --release
# Windows: target/release/ddmix.exe; Linux/macOS: target/release/ddmix
# Copy this native executable onto PATH; Cargo is not needed to run it.
ddmix --help
ddmix init mods/my-mod
ddmix list --workspace ./workspace
ddmix list --mods ./workspace/mods
ddmix inspect --ddnet ./workspace/ddnet > corpus.json
ddmix locate --file ./workspace/ddnet/src/game/client/gameclient.cpp --function CGameClient::OnInit
ddmix plan --ddnet ./workspace/ddnet --mods ./examples > plan.json
ddmix check --ddnet ./workspace/ddnet --mods ./examples
ddmix apply --workspace ./workspace --ddnet ./workspace/ddnet --mods ./examples
ddmix build --workspace ./workspace --ddnet ./workspace/ddnet --mods ./examples --build-dir ./workspace/build-ddmix
```

`init` requires an existing parent directory and refuses an existing destination.
`list`, `inspect`, `locate`, `plan` and `check` are read-only and emit JSON.
Errors return a nonzero exit status. `inspect` reports structural coverage and
source-located unsupported regions, **not** complete C++ semantic validity.

## Implemented

- Native `ddmix` executable with explicit source/workspace paths.
- Strict TOML package manifests, enabled state, wildcard compatibility and
  dependencies, stable topological ordering, missing dependency/cycle checks.
- Sorted recursive `.cpp`/`.h` overlay discovery. Other extensions are ignored.
  Only `src/game/client/** -> game-client` is supported; unknown ownership fails.
- Inputs under a discovered mod/source tree may not be symlinks. Explicit roots
  are canonicalized. The original DDNet checkout is never modified.
- Project-owned C++ lexer: exact byte spans and line/column positions, trivia,
  comments, prefixed/escaped/raw literals, numbers, operators, continued
  directives, conditional nesting validation.
- Branch-aware structural delimiter maps, conservative qualified function/class
  definitions, exact body boundaries and overload-aware lookup.
- Generic INSERT/REPLACE/REMOVE planner, function/comment targets, restricted
  class-body inserts, conflict checks and deterministic same-offset ordering.
- HEAD/TAIL aliases lower to generic edits. See [edit semantics](docs/edits.md).
- Fresh staged copies under `.ddmix/staging/ddnet`, SHA-256 ownership inventory,
  repeated-apply stability and protection against overwriting manual changes.
- Own CMake command/block scanner places a managed include after platform target
  creation; external mod sources/includes are attached to `game-client`.
- `CHECKSUM_SRC` additions outside DEV and an actual checksum-array capacity guard.
- `build` applies, configures and builds the staged client. See
  [staging/build workflow](docs/staging.md) for safeguards and current limitations.

Example `mixin.toml`:

```toml
[mod]
id = "my-mod"
name = "My mod"
version = "0.1.0"
enabled = true

[compatibility]
ddnet = "*"

[dependencies]
# other-mod = "*"
```

Only `*` version constraints are currently accepted. Unknown manifest fields
(including unimplemented components) fail rather than being silently ignored.
Edits and HEAD/TAIL injections support planning and staged application; see
`examples/basic-hooks/mixin.toml`. Mod sources remain in their own `src/` trees.

## Validation

```sh
cargo test --workspace
cargo test --workspace -- --ignored --nocapture
cargo clippy --workspace --all-targets -- -D warnings
```

The explicitly invoked integration test uses `workspace/ddnet` by default, or
`DDMIX_TEST_DDNET` for a different real checkout. It fails if that corpus is
missing; ordinary tests do not require an upstream download, but CMake is required
along with a C++ compiler for the generated-target-properties integration test. See
[validation](docs/validation.md) and [roadmap](docs/roadmap.md).
