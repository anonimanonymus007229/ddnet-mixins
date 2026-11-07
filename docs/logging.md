# CLI logging and progress

Human-readable logging goes to **stderr**, not stdout. JSON from list, locate,
inspect, plan/check and apply remains available for tooling; `plan --diff` keeps
stdout a plain unified diff. Build subprocess output also goes to stderr.
Libraries are silent by default and expose observer-based progress events;
there is no global logger or shared mutable logging configuration.

## Modes

```sh
ddmix plan --ddnet workspace/ddnet --mods examples > plan.json
ddmix --quiet check --ddnet workspace/ddnet --mods examples > plan.json
ddmix plan --ddnet workspace/ddnet --mods examples --verbose --color=never > plan.json 2> details.log
```

Global flags work before or after the subcommand:

- Normal (default): supplied ASCII banner/version, stage `[n/total]`, per-mod and
  per-file operations, throttled count/percentage progress, successes and summary.
- `-q` / `--quiet`: suppress banner, routine operations/progress and successful
  summary. Warnings, intentionally disabled mods and errors still appear.
- `-v` / `--verbose`: add dependency order, overlay mapping, injection/helper
  decisions, patch source locations/byte spans/content sizes, copy paths, parser
  unsupported-region details, subprocess commands and per-stage timings. Build
  also passes `--verbose` to CMake's build driver for native command details.
- `--color auto|always|never`: auto colors status labels only when stderr is a
  terminal, NO_COLOR is absent and TERM is not dumb. Explicit always overrides
  autodetection; never produces plain text. Quiet and verbose conflict.

Banner is printed once per executed command in normal/verbose modes, including
when stderr is redirected. Help/version use Clap's normal output without a
banner or execution summary. Command-line syntax failures use Clap diagnostics.
All status lines include elapsed wall-clock time. A final summary reports the
statistics actually collected: enabled mods, manifest edits, injections, components, overlays,
resolved source patches, modified files and copied source/dependency files for
apply/build; corpus functions/classes/tokens for inspect. Counts on failure may
be partial. CMake edits are counted as one modified CMake file separately from
source patch count; generated.cmake is output metadata, not a source patch.

## Stages and multi-mod scale

Plan/check: discovery -> lowering -> resolve -> cmake.
Apply starts with prepare (path/dependency/ownership validation), then those
four planning stages and staging -> publish; build adds configure -> compile
(9 stages).
Inspect and locate use their own discovery/index/lookup stages. Overall progress
is stage-based, not an estimate of remaining time. Within corpus scans, source
resolution and copying, count/percentage messages are throttled in ~10% buckets;
compiler-native progress is streamed rather than inferred from output line count.

Each enabled mod gets a compact manifest summary. Verbose operations name both
mod and edit IDs, preserving dependency/priority order. Disabled packages emit
an explicit skipped status; non-package directory entries are explained in
verbose mode. Non-C++ overlay extensions follow the documented ignore policy,
not a failed transformation. Failed selectors/edits are never reclassified as
skips. Parser regions outside supported targets produce warnings; inspect lists
structural diagnostics with file:line:column and preserves the JSON inventory.
This reports partial parser coverage rather than suggesting a full C++ AST.

## CMake output and failures

Build writes complete merged stdout/stderr transcripts in the build directory:

- `ddmix-configure.log`
- `ddmix-build.log`

These files are replaced per invocation, not appended. Normal/verbose streams
are prefixed `cmake |` on stderr. Quiet retains the complete transcript on disk,
still displays recognized compiler/CMake warning/error lines with a small
following context window, and replays the
last 40 lines when the subprocess fails. Recognition is best-effort (English
warning/error forms), not a complete compiler diagnostic parser; the transcript
is authoritative. Separate pipe readers prevent stdout/stderr deadlock; order
across the two streams depends on scheduling. Transcripts preserve native bytes;
terminal output decodes non-UTF-8 lossily and escapes control characters.

A nonzero process exit or failure to start is a command failure. Errors identify
the active stage, preserve mod/edit/file selector context where available, and
point to the full build transcript. Configure failure stops before compilation.
Warnings/skips are visible even in quiet mode and tallied in the final summary.
Quiet failures still print failure summary and elapsed time.

No transcript retention/rotation or interactive progress animation is implemented.
The append-only log format is intentional for redirected output and CI readability.
