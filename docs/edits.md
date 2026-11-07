# Source edit planning (schema 0.1)

```sh
ddmix locate --file workspace/ddnet/src/game/client/gameclient.cpp --function CGameClient::OnInit
ddmix plan --ddnet workspace/ddnet --mods examples > plan.json
ddmix check --ddnet workspace/ddnet --mods examples
```

Both `plan` and `check` resolve all enabled mods' source edits against original
sources, detect conflicts, validate structural CMake placement/overlay mapping,
and emit the same JSON plan. Neither writes DDNet, staging, or generated state.
They do not execute CMake or a compiler or check duplicate config identifiers.
`apply` uses the plan in an isolated copy; `build` runs CMake against it. See
[staging](staging.md).

`examples/basic-hooks/mixin.toml` is a complete tested real-DDNet example.

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
  Other class operations are explicitly rejected until full declaration spans
  and uniqueness guarantees are available.

`action` is INSERT, REPLACE or REMOVE. INSERT requires `position`:
BEFORE, AFTER, BODY_START or BODY_END (the latter two require a body).
INSERT/REPLACE requires `content`; REMOVE must omit it. REPLACE/REMOVE must
omit position. Every edit has a unique lowercase/hyphenated `id`, a normalized
`src/...` file path, and optional integer `priority` (default 0).

`[[injections]]` requires `id`, `file`, fully qualified `target`, `content`,
and HEAD or TAIL `kind`; `parameters` and `priority` are optional. HEAD lowers
to INSERT/BODY_START and TAIL to INSERT/BODY_END. **TAIL is the lexical body
end, not execution before every return.** No return interception is implied.

## Ordering, conflicts and preservation

Dependency order precedes ascending numeric priority and stable edit ID within
a mod. Same-offset inserts are rendered in that order. Overlapping replacement
or removal ranges fail, as do inserts on or inside their boundaries. Edits
resolve before rendering; source spans never refer to already-modified text.
Content is verbatim, with no implicit newline, indentation or newline-style
conversion. Unchanged bytes are retained exactly. Snippets targeting CRLF files
should supply CRLF if uniform line endings are desired.

## Safety and limitations

The parser handles common qualified definitions, simple namespaces/classes,
constructors/destructors without initializers, and common member qualifiers.
It skips bodies structurally, so nested lambdas/control flow are not confused
with top-level definitions. Declaration-only functions are not indexed yet.
Templates, ctor initializer lists, operator functions, trailing returns,
complex declarators, anonymous namespaces and macro-decorated declarations
are not fully supported and are reported as source-located unsupported regions.
The count is an inventory of skipped regions, not necessarily invalid C++.

Branch-local delimiters are paired without choosing active macros. Branches
that change their inherited delimiter stack produce diagnostics; selection in
such files fails. Unsupported regions containing a queried function's bare
identifier also block selection: they might hide another overload. This is
intentionally conservative and can reject an otherwise valid target. This is
not complete semantic C++ lookup, macro expansion or a claim of universal
selector safety. Staged application never writes to the original input.
