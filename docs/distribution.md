# Installing and packaging the executable

Build from source with Rust/Cargo:

```sh
cargo build --release --locked
```

Copy `target/release/ddmix.exe` (Windows) or `target/release/ddmix` (Linux/macOS)
to a directory on PATH. Running ddmix does not require Cargo; building DDNet still
requires its own toolchain and dependencies. On Unix, preserve executable
permissions when extracting an archive, or run `chmod +x ddmix`.

## CI artifacts

Open a successful **Rust** workflow run in GitHub Actions and download the
`ddmix-<target>` artifact for Windows x86_64, Linux x86_64 or macOS ARM64.
Each artifact contains a ZIP with the ddmix executable, README and license,
plus a SHA-256 checksum file. Extract the executable and add it to PATH.
The DDNet integration build does not publish a client artifact.

## Packaging for maintainers

Package an already-built executable with its README, MIT license and archive
SHA-256 checksum:

```sh
python scripts/package.py --binary target/release/ddmix.exe --target x86_64-pc-windows-msvc
```

Output goes to `target/dist/`. Use the matching binary path and target label
when packaging another platform.

The [release workflow](../.github/workflows/release.yml) builds Windows x86_64
MSVC, Linux x86_64 GNU, macOS x86_64 and macOS ARM64 archives. Run it manually
for Actions artifacts; pushing a `v*` tag also publishes GitHub Release assets.
Check workflow results before distributing a platform's archive. Linux archives
use the runner's libc baseline. Binaries are not signed or notarized; there is
no installer or automatic updater.
