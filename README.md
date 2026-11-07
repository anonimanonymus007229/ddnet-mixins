# DDNet Mixins

Create DDNet mods as separate packages and apply them without editing the original DDNet checkout.

`ddmix` reads TOML manifests, inserts code into supported C++ functions or around comments, and connects mod-owned client sources to DDNet's CMake target. It prepares a separate working copy for compilation, so mod developers can keep their changes outside upstream source files.

**Early development:** transformations, component registration and isolated application are tested against real DDNet. A Windows Release client with the example mods has been compiled locally. The C++ parser remains conservative; unsupported constructs fail rather than being guessed.

## Quick start

Requires Rust/Cargo to build `ddmix`. The example below uses the DDNet revision covered by the integration tests; newer revisions may contain unsupported syntax.

```sh
git clone https://github.com/anonimanonymus007229/ddnet-mixins.git
cd ddnet-mixins
cargo build --release --locked

mkdir workspace
git clone https://github.com/ddnet/ddnet.git workspace/ddnet
git -C workspace/ddnet checkout 176fc2f9ab85ace703c6ec10da0249b01995ba94

cargo run --release -- plan --ddnet workspace/ddnet --mods examples
```

Already have a DDNet checkout? Skip the second clone and pass its path to `--ddnet` instead. The included example plans HEAD/TAIL inserts in `CGameClient::OnInit`, a config declaration, and CMake integration for an external source file. `plan` emits JSON and writes no changes.

The native executable is `target/release/ddmix.exe` on Windows or `target/release/ddmix` on Linux/macOS. Add it to `PATH` to use the shorter `ddmix` commands below. Running the executable does not require Cargo.

## Usage

Check the example, then apply it to a separate working copy:

```sh
ddmix plan --ddnet workspace/ddnet --mods examples --diff
ddmix check --ddnet workspace/ddnet --mods examples
ddmix apply --workspace workspace --ddnet workspace/ddnet --mods examples
```

The result lives in `workspace/.ddmix/staging/ddnet/`. **Staging means the generated DDNet copy with mods applied**, not a directory to edit manually. The original `workspace/ddnet/` stays unchanged, and mod source files remain in their own directories.

Each `apply` starts from the original checkout, preventing duplicate inserts. If generated files have been changed manually, `ddmix` refuses to overwrite them.

With DDNet's compiler, CMake and third-party dependencies installed, configure and build that copy:

```sh
ddmix build --workspace workspace --ddnet workspace/ddnet --mods examples --build-dir workspace/build-ddmix
```

`build` runs `apply` first, then CMake configuration and the `game-client` build. It does not install dependencies. If the original checkout lacks its submodule, `--libraries /path/to/ddnet-libs` copies a separate dependency checkout into staging only. Extra cache settings use `--cmake-arg=-DNAME=VALUE`; keep build output outside `.ddmix` and the original checkout. See the [staging and build guide](docs/staging.md).

To locate a function without changing it:

```sh
ddmix locate --file workspace/ddnet/src/game/client/gameclient.cpp --function CGameClient::OnInit
```

Use `ddmix --help` or `ddmix <command> --help` for command options.

## Mod configuration

Create a package under an existing parent directory:

```sh
mkdir workspace/mods
ddmix init workspace/mods/my-mod
```

Add an injection to `workspace/mods/my-mod/mixin.toml`:

```toml
[mod]
id = "my-mod"
name = "My mod"
version = "0.1.0"

[[injections]]
id = "init-head"
file = "src/game/client/gameclient.cpp"
target = "CGameClient::OnInit"
parameters = "()"
kind = "HEAD"
content = '''

	// My initialization hook.
'''
```

Prefer editing C++ outside TOML? Replace `content` with
`content_file = "hooks/ccontrols_snapinput.cpp"` and put the snippet in your
mod's `hooks/` directory. It is inserted into the target body, not compiled as a
separate file. Use exactly one of `content` or `content_file`. See the
[file-based hook example](examples/controls-hook) and [path rules](docs/edits.md#injection-content-from-files).

Plan your packages with `ddmix plan --ddnet workspace/ddnet --mods workspace/mods`.

Place new `.cpp` and `.h` files under `workspace/mods/my-mod/src/game/client/…`, mirroring DDNet paths. They are discovered automatically and attached to `game-client` without copying them into DDNet's source tree. Other target mappings are not supported yet; overlays cannot replace existing upstream files.

HEAD inserts at the beginning of a function body. TAIL inserts at its lexical end, **not before every return**. Generic edits support INSERT, REPLACE and REMOVE with documented target restrictions. Ambiguous matches and overlapping edits fail instead of choosing a location.

Components can declare their header, CGameClient member and `all`/`input` registration in `[[components]]`. BEFORE_CALL, AFTER_CALL, MODIFY_ARG, RETURN, OVERWRITE and explicit-block WRAP are available with conservative restrictions. Config edits are checked for identifier/console-name collisions. See the [advanced feature guide](docs/advanced-injections.md).

Dependencies have deterministic ordering; compatibility and dependency version constraints currently accept only `*`. The parser covers a subset of C++ and explicitly reports unsupported regions. `check` validates selectors, conflicts and structural CMake placement—it does not run a compiler or validate snippet semantics.

## Documentation

- [Edit syntax, selectors and limitations](docs/edits.md)
- [Staging, ownership checks, CMake and build recovery](docs/staging.md)
- [Real-DDNet validation and known gaps](docs/validation.md)
- [Development roadmap](docs/roadmap.md)
- [Advanced injections and components](docs/advanced-injections.md)
- [Binary packaging and platform status](docs/distribution.md)
- [Basic hooks](examples/basic-hooks), [file-based hook](examples/controls-hook), [advanced hooks](examples/advanced-hooks) and [component](examples/component) examples

## Development

Rust/Cargo, CMake and a C++ compiler are needed for the regular test suite. The explicit corpus tests also require a real DDNet checkout at `workspace/ddnet`, or at the path set by `DDMIX_TEST_DDNET`.

```sh
cargo test --workspace --locked
cargo test --workspace --locked -- --ignored --nocapture
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Parser changes should be checked against real DDNet files, not only synthetic examples. C++ lexing, parsing and transformation are implemented in this repository rather than delegated to an existing C++ parser.

## License

[MIT](LICENSE).
