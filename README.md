# DDNet Mixins

Rust tooling for independent DDNet mods without editing the upstream checkout.
**Early implementation: discovery and lexical inspection only.** This is not yet
an injection tool or a buildable mod integration release.

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
```

`init` requires an existing parent directory and refuses an existing destination.
`list` and `inspect` are read-only and emit JSON. Errors return a nonzero exit
status. `inspect` reports lexical errors and delimiter errors in unconditional
files, **not** C++ semantic validity or selector safety.

## Implemented

- Native `ddmix` executable with explicit source/workspace paths.
- Strict TOML package manifests, enabled state, wildcard compatibility and
  dependencies, stable topological ordering, missing dependency/cycle checks.
- Sorted recursive `.cpp`/`.h` overlay discovery. Other extensions are ignored.
  Only `src/game/client/** -> game-client` is supported; unknown ownership fails.
- Inputs under a discovered mod/source tree may not be symlinks. Explicit roots
  are canonicalized. No command currently modifies DDNet.
- Project-owned C++ lexer: exact byte spans and line/column positions, trivia,
  comments, prefixed/escaped/raw literals, numbers, operators, continued
  directives, conditional nesting validation.
- Symmetric delimiter maps for unconditional files. Conditional files are marked
  `conditional_pairing_deferred` and contain no delimiter pairs. No branch is
  assumed active and no cross-branch edit is considered safe.

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
(including unimplemented edits, injections and components) fail rather than being
silently ignored. Mod sources remain in their own `src/` trees.

## Validation

```sh
cargo test --workspace
cargo test -p mixins-lexer --test ddnet_corpus -- --ignored --nocapture
cargo clippy --workspace --all-targets -- -D warnings
```

The explicitly invoked integration test uses `workspace/ddnet` by default, or
`DDMIX_TEST_DDNET` for a different real checkout. It fails if that corpus is
missing; ordinary unit tests do not require an upstream download. See
[validation](docs/validation.md) and [roadmap](docs/roadmap.md).
