# Logging and progress

Logs go to **stderr**. JSON and `plan --diff` output stay on stdout, so they can
be redirected without banners or progress messages mixed into the result.

```sh
ddmix plan --ddnet workspace/ddnet --mods examples > plan.json
ddmix --quiet check --ddnet workspace/ddnet --mods examples > plan.json
ddmix plan --ddnet workspace/ddnet --mods examples --verbose --color=never > plan.json 2> details.log
```

## Output modes

Global flags work before or after the subcommand:

| Mode | Output |
|---|---|
| Normal (default) | Banner/version, numbered stages, mod/file operations, progress and final summary |
| `-q`, `--quiet` | Warnings, skipped mods and errors; routine output and successful summary are hidden |
| `-v`, `--verbose` | Normal output plus edit IDs, source locations, byte spans, dependency order, copy decisions, commands and stage timings |

Quiet and verbose cannot be combined. Verbose builds also request native command
details from CMake. Help/version print without an execution banner or summary.
The banner remains visible when normal/verbose logs are redirected.

`--color auto|always|never` controls status-label colors. Auto uses color only
when stderr is a terminal, NO_COLOR is unset and TERM is not dumb. Use never
for plain logs or always to override autodetection.

## Progress and summary

Overall progress shows processing stages, not estimated time remaining. File
resolution, corpus scans and copying report counts and percentages; compilation
shows native compiler progress. The final summary includes elapsed time and
available counts for mods, injections, components, edits, modified/copied files,
warnings and skips. Failed operations may have partial counts.

Verbose messages identify mod/edit IDs and source locations. Disabled packages
are explicitly marked skipped. Parser coverage warnings do not mean that the
C++ is invalid; use `inspect` JSON or verbose output to locate unsupported regions.
See [compatibility](compatibility.md#c-parsing-and-selectors).

## Build logs and errors

Complete CMake/compiler transcripts are saved in the build directory:

- `ddmix-configure.log`
- `ddmix-build.log`

Each invocation replaces the corresponding transcript. Normal/verbose mode
streams native output to stderr with a `cmake |` prefix. Quiet mode retains full
logs, displays recognized warning/error lines with following context, and shows
the last 40 lines on failure. Warning recognition is best-effort; consult the
complete transcript when a compiler's output format is not recognized.

Errors identify the active stage and retain available mod, edit, file and
selector context. Failed configuration stops before compilation. Failure
summaries and elapsed time remain visible in quiet mode. Terminal output escapes
control characters; transcript files preserve native output bytes.
