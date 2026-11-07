# Executable distribution

`cargo build --release --locked` produces `target/release/ddmix.exe` on Windows
or `target/release/ddmix` on Linux/macOS. Copy the executable to a directory on
PATH. Cargo/Rust are not needed to run it; DDNet compilation still needs its own
compiler, CMake, Python, Rust and third-party dependencies.

`.github/workflows/release.yml` builds ZIP archives containing executable,
README and MIT license, plus archive SHA-256 checksums, for:

- Windows x86_64 MSVC;
- Linux x86_64 GNU;
- macOS x86_64;
- macOS ARM64.

A manual workflow run uploads Actions artifacts. Pushing a `v*` tag additionally
publishes those archives as GitHub Release assets. No tag/release has been
published by this development session, and remote jobs have not been observed.
Only the Windows executable has been built/tested locally. Generate its local
archive with `python scripts/package.py --binary target/release/ddmix.exe --target x86_64-pc-windows-msvc`. Other platform
support remains CI-validation pending, not a claim of tested binaries.

Archives are platform-specific; Linux builds use the runner's libc baseline.
On Unix preserve executable permissions when extracting, or use `chmod +x ddmix`.
There is no installer, automatic updater, signing or notarization yet. Review
the workflow's permissions before enabling release publication in a fork.
