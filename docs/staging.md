# Isolated staging and build

```sh
ddmix plan --ddnet workspace/ddnet --mods examples > plan.json
ddmix check --ddnet workspace/ddnet --mods examples
ddmix apply --workspace workspace --ddnet workspace/ddnet --mods examples
ddmix build --workspace workspace --ddnet workspace/ddnet --mods examples --build-dir workspace/build-ddmix --cmake-arg=-DHEADLESS_CLIENT=ON --cmake-arg=-DSERVER=OFF --cmake-arg=-DTOOLS=OFF --cmake-arg=-DDOWNLOAD_GTEST=OFF
```

All supplied roots must exist. Plan/check are read-only and validate source edit
resolution, overlay ownership and CMake structural placement, not a compiler or
CMake configure run. JSON explicitly distinguishes structural CMake validation
from configure/compile validation. Input hashes cover the edited source files;
this is not a lockfile for the complete external mod/source environment.

## Generated ownership

Apply copies original DDNet files (excluding root `.git`) into a fresh sibling
transaction, renders edits there, then publishes `.ddmix` with renames. No hard
links or symlinks are used. All input symlinks/nonregular files fail before
copying. Mod files remain in their original directories. Conflicting logical
overlay paths and overlays shadowing original DDNet files fail explicitly.

`.ddmix` contains:

- `staging/ddnet/`: transformed source copy;
- `generated.cmake`: external target sources/includes and checksum additions;
- `plan.json`: source patch plan, input hashes and generated CMake;
- `state.json`: ownership version and SHA-256 inventory of generated regular files.

Every apply starts from original DDNet, preventing duplicate injections. Before
replacing an existing output, its ownership and full inventory are verified.
Manual modifications/additions/deletions in staging or generated output cause a
failure: preserve those changes elsewhere and remove `.ddmix` yourself if you
intend to regenerate. The state file is ownership metadata, not a tamper-proof
security boundary. Never use it to authorize untrusted output.

A create-new `.ddmix.lock` serializes apply transactions. Interrupted transactions
may leave `.ddmix-next`, `.ddmix-previous` or the lock; the next apply refuses to
silently delete them. Inspect/recover/remove those paths manually. Publication
rolls back on a normal rename failure; it is not power-loss-atomic across both
renames. Source/mod inputs must not be changed concurrently. This version does
not defend against hostile concurrent filesystem replacement.

## CMake integration

The project-owned scanner handles quoted/bracket arguments, line/bracket
comments, nested parentheses and block nesting. It finds literal `game-client`
creation commands and their common platform conditional end. Complex unknown
structures fail instead of using line numbers. The staged root gets one marked
include before later target configuration. The generated file guards
`if(TARGET game-client)` and uses scoped PRIVATE sources/includes. Paths are
absolute and quoted; spaces are supported; semicolons, dollar expressions and
control-line breaks are rejected rather than ambiguously escaped.

Configure must pass `-DDDMIX_WORKSPACE_ROOT=<workspace>`; the build command does
so automatically. Mod directories may live outside the workspace. The include
is not based on the staged directory. Regeneration operates on a pristine input;
managed markers in the original CMake are rejected, not appended again.

Inspection of supplied DDNet found `set_src()` appends ordinary source files to
`CHECKSUM_SRC`; `DEV` later clears this list. Mixins appends external files when
NOT DEV, preserving that policy. Before `configure_file(checksummed_extra.txt)`
it checks source count plus actual extra-template line count against the parsed
`m_aFiles[N]` capacity. Capacity overflow is a configure error, not generated
out-of-bounds C++. Unsupported checksum layouts fail planning.

## Build and environment

Build outputs go into an explicit directory outside `.ddmix` and original DDNet.
CMake may write arbitrary outputs there; use a dedicated directory. Only extra
`-DNAME=VALUE` cache settings are accepted. Reserved workspace overrides and
alternate source/binary/script modes are rejected. Default generator discovery
is used. `build` always configures then builds target game-client, Release for
multi-config generators. Source files must stay immutable between apply and
build; no concurrent build/apply support is promised yet.

The build wrapper caps Git parent discovery so DDNet staging does not mistake
the enclosing Mixins repository for its own checkout. This copy-based mode does
not preserve original Git revision metadata; a future worktree/metadata mode is
needed for that. Manual CMake users should set `GIT_CEILING_DIRECTORIES` to the
staging parent for equivalent behavior.

Full Release configuration/build was validated locally with MSVC and a separate
pinned ddnet-libs checkout, including a declared component. To reproduce without
modifying the original checkout's empty submodule directory:

```sh
git clone --depth 1 https://github.com/ddnet/ddnet-libs.git workspace/dependencies/ddnet-libs
# Tested dependency revision: c0e6703fbcdbe03df2f26875427ec3951ce4ec21
ddmix build --workspace workspace --ddnet workspace/ddnet --mods examples --libraries workspace/dependencies/ddnet-libs --build-dir workspace/build-ddmix --cmake-arg=-DVIDEORECORDER=OFF --cmake-arg=-DSERVER=OFF --cmake-arg=-DTOOLS=OFF --cmake-arg=-DDOWNLOAD_GTEST=OFF --cmake-arg=-DVULKAN=OFF --cmake-arg=-DPREFER_BUNDLED_LIBS=ON
```

The optional libraries directory is validated and copied into staged ddnet-libs,
not symlinked or installed globally. Existing populated dependencies are not
overridden. Build subprocesses disable Python bytecode writes so upstream build
scripts do not pollute the staged inventory with __pycache__. A separate CMake
probe verifies generated source/include/checksum properties and paths with spaces.
