# Staging and build

Staging is the generated copy of DDNet with mods applied. Edit your mod files
and original checkout, not this copy. The original DDNet checkout stays unchanged.

## Plan, apply and build

Run from the Mixins repository, or replace the paths with your own workspace:

```sh
ddmix plan --ddnet workspace/ddnet --mods examples --diff
ddmix check --ddnet workspace/ddnet --mods examples
ddmix apply --workspace workspace --ddnet workspace/ddnet --mods examples
ddmix build --workspace workspace --ddnet workspace/ddnet --mods examples --build-dir workspace/build-ddmix
```

`plan` and `check` are read-only. They validate source selectors, conflicts,
overlay mapping and CMake placement, but do not execute CMake or a compiler.
`apply` creates the modified copy; `build` applies first, then configures CMake
and builds `game-client` (Release for multi-config generators).

The workspace, DDNet and mods roots must exist. Keep the build directory outside
`.ddmix` and the original checkout. DDNet's compiler, CMake, Python, Rust and
third-party dependencies must be installed; `ddmix` does not install them.
Extra CMake cache settings use `--cmake-arg=-DNAME=VALUE`. Source/build root
overrides, script modes and overrides of DDMIX_WORKSPACE_ROOT are rejected.

## Separate dependency checkout

If the original DDNet lacks its dependencies, supply a separate ddnet-libs
checkout. It is copied only into staging; populated dependencies are not replaced.
See [reference revisions](compatibility.md#ddnet-versions-and-build-targets).

```sh
git clone https://github.com/ddnet/ddnet-libs.git workspace/dependencies/ddnet-libs
git -C workspace/dependencies/ddnet-libs checkout c0e6703fbcdbe03df2f26875427ec3951ce4ec21
ddmix build --workspace workspace --ddnet workspace/ddnet --mods examples --libraries workspace/dependencies/ddnet-libs --build-dir workspace/build-ddmix --cmake-arg=-DVIDEORECORDER=OFF --cmake-arg=-DVULKAN=OFF --cmake-arg=-DSERVER=OFF --cmake-arg=-DTOOLS=OFF --cmake-arg=-DDOWNLOAD_GTEST=OFF --cmake-arg=-DPREFER_BUNDLED_LIBS=ON
```

Source/mod symlinks are rejected. Internal dependency links are materialized as
ordinary files/directories; external, broken or cyclic links fail. Staging uses
independent copies, not links to files in the original tree.

## Generated files and repeated application

Under the selected workspace, `.ddmix/` contains:

| Path | Purpose |
|---|---|
| `staging/ddnet/` | DDNet copy with source and CMake edits |
| `generated.cmake` | Mod source files and include paths for game-client |
| `plan.json` | Resolved edits and input hashes |
| `state.json` | Ownership metadata and generated-file hashes |

Every apply starts from the original DDNet, so inserts do not accumulate.
Mod source files stay in their packages and are referenced by CMake. Hashes in
the plan are not a lockfile for all external source files or dependencies.

Before replacing `.ddmix`, the tool checks its ownership and file inventory.
Manual additions, changes or deletions cause an error instead of being overwritten.
Save any changes you need elsewhere, then remove `.ddmix` if you intend to
regenerate it. Do not edit state.json to bypass the check.

## Recovering from interrupted apply

Only one apply can hold `.ddmix.lock`. An interrupted operation can leave
`.ddmix-next` (new transaction), `.ddmix-previous` (previous output) or the lock.
The next apply refuses to delete them automatically.

1. Ensure no apply/build process is running.
2. Inspect these directories and preserve any files you need outside generated state.
3. If publication was interrupted and `.ddmix` is missing, restore
   `.ddmix-previous` to `.ddmix` to retain the previous output, or remove generated
   state and regenerate from the original checkout.
4. Remove abandoned transaction directories and the stale lock before retrying.

Do not remove a lock held by a running process or change inputs during build/apply.

## CMake and diagnostics

Mixins adds a marked include after `game-client` creation and attaches external
sources/includes to that target. Unknown target-creation or checksum layouts
fail planning. Files contribute to source checksums outside DEV mode; exceeding
DDNet's checksum capacity fails configuration.

`build` passes DDMIX_WORKSPACE_ROOT automatically. For manual configuration of
staged sources, pass `-DDDMIX_WORKSPACE_ROOT=<absolute-workspace-path>` and set
GIT_CEILING_DIRECTORIES to the staging parent to avoid Git discovering an
unrelated parent repository. Copy staging excludes original Git metadata.

Build output and the complete `ddmix-configure.log` / `ddmix-build.log` transcripts
live in the build directory. See [logging](logging.md) for verbosity and error
output, and [compatibility](compatibility.md) for unsupported constructs.
