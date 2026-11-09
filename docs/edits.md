# Source edits and injections

```sh
ddmix locate --file workspace/ddnet/src/game/client/gameclient.cpp --function CGameClient::OnInit
ddmix plan --ddnet workspace/ddnet --mods examples > plan.json
ddmix check --ddnet workspace/ddnet --mods examples
```

Both `plan` and `check` resolve all enabled mods' source edits against original
sources, detect conflicts, validate structural CMake placement/overlay mapping,
and emit the same JSON plan. Neither writes DDNet, staging, or generated state.
They do not execute CMake or a compiler. Rendered config edits are checked for
identifier/console-name collisions, respecting exclusive preprocessor arms.
`apply` uses the plan in an isolated copy; `build` runs CMake against it. See
[staging](staging.md).

See [examples/basic-hooks](../examples/basic-hooks) for a complete mod package.

## Targets and actions

- `target = { kind = "function", name = "CGameClient::OnInit", parameters = "()" }`
  selects a supported definition. Optional parameters match the exact trimmed
  parenthesized source text, not a normalized C++ type signature. Overloads are
  ambiguous unless narrowed. Function BEFORE/AFTER/REPLACE/REMOVE uses the full
  definition span (comments/trivia preceding the declaration remain untouched).
- `target = { kind = "comment", text = "nonempty substring" }` selects one
  lexical comment. Duplicate and absent matches fail. This works in macro-driven
  headers without preprocessing or rewriting them.
- `target = { kind = "class", name = "N::C" }` currently supports body-boundary
  insertion only, and only in files with no unsupported structural regions.
  Other class operations are rejected.

`action` is INSERT, REPLACE or REMOVE. INSERT requires `position`:
BEFORE, AFTER, BODY_START or BODY_END (the latter two require a body).
INSERT/REPLACE requires exactly one of `content` or
`content_file = "hooks/filename.cpp"`; REMOVE must omit both. REPLACE/REMOVE must
omit position. Every edit has a unique lowercase/hyphenated `id`, a normalized
`src/...` file path, and optional integer `priority` (default 0).

`[[injections]]` requires `id`, `file`, fully qualified `target`, a `kind`, and
exactly one of inline `content`, `content_file = "hooks/filename.cpp"`, or
`helper = "hooks/filename.cpp"` (HEAD/TAIL only).
`parameters` and `priority` are optional. HEAD lowers
to INSERT/BODY_START and TAIL to INSERT/BODY_END. **TAIL is the lexical body
end, not execution before every return.** No return interception is implied.
See [advanced aliases](advanced-injections.md) for the separate RETURN, call,
body overwrite and explicit-block wrap operations.

## Injection content from files

```toml
[[injections]]
id = "snapinput-head"
file = "src/game/client/components/controls.cpp"
target = "CControls::SnapInput"
kind = "HEAD"
content_file = "hooks/ccontrols_snapinput.cpp"
```

Paths are relative to the directory containing `mixin.toml`, not the shell's
working directory or the DDNet root. Files must be regular UTF-8 files beneath
that mod's `hooks/` directory. Use forward slashes; absolute paths, `..`, `.`,
empty path components and symlink components are rejected. Nested folders and
spaces are supported. Missing/unreadable/non-UTF-8 files produce an error naming
the manifest, mod, injection and snippet path.

The file is a **code fragment**, not a complete function or compilation unit.
It is read on each manifest load and inserted verbatim, including its original
newlines/indentation, into the selected context. No extra newline is added: a
HEAD snippet should start/end with appropriate whitespace so it does not merge
with surrounding statements. `this`, target parameters and members can be used
where C++ normally permits them; locals declared later in the original body
are not available at HEAD. Avoid file-level includes/function definitions in a
body snippet. `hooks/` is not scanned as an overlay or added to target_sources.

This works for every injection kind, including WRAP (`{{original}}` is checked
after reading the file). Empty snippets are permitted, like explicit empty
inline content. Generic `[[edits]]` also accepts `content_file` for INSERT/REPLACE,
using the same hooks path/safety rules; content is resolved before planning. `content_file` performs textual insertion
at validated syntax spans; see `examples/controls-hook`. For a separate generated
function with explicit parameters use [helper and bindings](helpers.md), as in
`examples/controls-helper`. It is a different mode, not implicit helper generation
for every snippet.

## Client-specific rules

Any table-array rule can include a common `clients` include/exclude filter.
See [client folder filters](client-filters.md) for patterns, mutually exclusive
variants and skipped-rule behavior.

## Ordering, conflicts and preservation

Dependency order precedes ascending numeric priority and stable edit ID within
a mod. Same-offset inserts are rendered in that order. Overlapping replacement
or removal ranges fail, as do inserts strictly inside them. Boundary inserts
are allowed and ordered before/after the replacement at its start/end. Edits
resolve before rendering; source spans never refer to already-modified text.
Content is verbatim, with no implicit newline, indentation or newline-style
conversion. Unchanged bytes are retained exactly. Snippets targeting CRLF files
should supply CRLF if uniform line endings are desired.

## Safety and limitations

Selectors require exactly one supported target. Missing or ambiguous matches,
structural errors and potentially matching unsupported regions cause a failure,
not a guessed insertion. Function selectors choose definitions with real bodies,
not declaration-only/defaulted/deleted methods.

The tool does not type-check snippets or expand macros. See
[compatibility](compatibility.md#c-parsing-and-selectors) for grammar and
preprocessor restrictions. Staged application never writes to the original input.
