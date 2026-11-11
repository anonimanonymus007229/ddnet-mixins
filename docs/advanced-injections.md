# Advanced injections and component declarations

All aliases resolve against original spans and lower to INSERT/REPLACE/REMOVE.
HEAD/TAIL additionally support [separate void helpers with bindings](helpers.md);
the body is loaded from hooks/ and both the free function and call are generated.
See `examples/advanced-hooks/mixin.toml` for real-DDNet examples. The restrictions
below apply to transformations, independently of complete source parsing on the
[checked clients](compatibility.md#ddnet-versions-and-build-targets).

| Kind | Meaning and restrictions |
|---|---|
| HEAD / TAIL | Insert at lexical body start/end. TAIL does not intercept returns. |
| BEFORE_CALL / AFTER_CALL | Insert around exactly one outermost standalone call statement, including its semicolon. Requires `callee`. |
| MODIFY_ARG | Replace one call argument; `argument` is zero-based. Requires `callee`. |
| RETURN | Replace the expression of exactly one final nonempty return statement. Multiple/nested/control-flow returns and conditional directives are rejected. |
| OVERWRITE | Replace the body interior, preserving braces/signature. Content is statements, not a second outer body. |
| WRAP | Replace the body interior using exactly one `{{original}}` placeholder. The original is inserted as a nested block; no inferred parameter forwarding or renamed function. Bodies containing return/goto/co_return are rejected so trailing wrapper code is not silently skipped. |

Callee selectors are identifier/member chains, optionally with empty getter
calls, e.g. `Client()->SendPackMsgActive`. Calls inside expressions, loops,
conditionals or lambdas are not selected. Templates or angle-bearing arguments
are rejected when argument boundaries cannot be established conservatively.
Balanced nested parentheses/brackets/braces preserve nested argument commas.
Conditional compilation in a call-query body is rejected.

Snippets must be valid C++ for their context; the tool is not a type checker.
Boundary inserts can coexist with replacements starting/ending there; inserts
strictly inside a replaced range and overlapping replacements fail. Same-offset
inserts retain deterministic dependency/priority/ID order.

CANCEL is not exposed: cancellation semantics depend on return types, side
effects and dispatch behavior and are not defined by this release.

## Components

```toml
[[component]]
class = "CDdmixExample"
header = "game/client/components/ddmix_example.h"
member = "m_DdmixExample"
register_in = ["all", "input"]
priority = 0
```

For a batch sharing registration categories, use `[[components]]` with `members`:

```toml
[[components]]
register_in = ["all"]
members = [
  { class = "COne", header = "game/client/components/one.h", member = "m_One" },
  { class = "CTwo", header = "game/client/components/two.h", member = "m_Two" },
]
```

Groups have no `priority`, `class`, `header` or singular `member` fields. Each
member records its own class/header/member; their array order is preserved.
For explicit priority or different categories, use `[[component]]` with a single
`member` and optional `priority`. Old singular `[[components]]` declarations must
be renamed to `[[component]]`. Groups expand before individual declarations;
priority orders components within a mod, with declaration order breaking ties.
Both forms and individual group members support [client filters](client-filters.md).

The header must be discovered in the mod's src tree and declare the named class
with direct `public CComponent` inheritance. `all` is mandatory (interfaces and
lifecycle initialization); `input` is optional. The class/member identifiers
must be simple C++ identifiers. See `examples/component` for an implementation
of DDNet's actual `Sizeof()` and `OnInit()` callbacks.

The adapter verifies the actual CGameClient class and OnConsoleInit collection
structures, then emits generic include/member/initializer-block inserts.
Existing render/input order remains unchanged; new members append to selected
collections in deterministic mod/priority/declaration order. Dependencies order
mods; priority orders individual component declarations within a mod. Missing/ambiguous or changed
registration structures fail. Existing member names and duplicate declarations
across mods fail. No raw upstream line offsets are hardcoded.

Headers are included before the existing gameclient header content (mod headers
must have their own guards). Component declarations are currently tied to the
supplied DDNet registration layout, not a promise of compatibility with all
older/future versions. Only the compiler confirms complete base-class/lifecycle
compatibility.

## Config conflict checks

After rendering edits to config_variables.h, MACRO_CONFIG_INT/COL/STR identifiers
and console names are checked for collisions. Distinct arms of the same #if are
mutually exclusive and do not conflict; no macro expression is evaluated. Two
separate #if expressions are conservatively considered able to coexist.
Macro argument splitting uses tokens and delimiter pairs, not comma splitting.

For checked clients, source coverage and target restrictions, see
[client compatibility and edit limits](compatibility.md).
