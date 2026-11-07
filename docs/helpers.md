# Helper functions and bindings

Use `helper` when a hook should run as a separate function, rather than pasting
its statements directly into the target body. Currently helpers support HEAD
and TAIL with a generated `static void` function in the same `.cpp` translation
unit. No separate source file is compiled and no linker symbol is exported.

## Example: CControls::SnapInput

```text
my-mod/
├── mixin.toml
└── hooks/
    └── ccontrols_snapinput.cpp
```

```toml
[mod]
id = "my-mod"
name = "My mod"
version = "0.1.0"

[[injections]]
id = "snapinput-head"
file = "src/game/client/components/controls.cpp"
target = "CControls::SnapInput"
kind = "HEAD"
helper = "hooks/ccontrols_snapinput.cpp"
bindings = [
  { name = "controls", ty = "CControls *", value = "this" },
  { name = "data", ty = "int *", value = "pData" },
  { name = "player_flags", ty = "int &", value = "m_aInputData[g_Config.m_ClDummy].m_PlayerFlags" },
]
```

`hooks/ccontrols_snapinput.cpp` contains only the helper's body:

```cpp
(void)controls;
(void)data;
(void)player_flags;
// Assigning player_flags changes the original member through its reference.
return; // Returns from this helper; the original SnapInput continues.
```

The example is intentionally behavior-preserving. A complete package is in
[examples/controls-helper](../examples/controls-helper). Plan it alongside the
other examples:

```sh
ddmix plan --ddnet workspace/ddnet --mods examples --diff
```

Conceptually, the generated source looks like this (actual names are derived
from mod/edit IDs to avoid collisions):

```cpp
static void generated_helper(CControls *controls, int *data, int &player_flags)
{
    // Contents of hooks/ccontrols_snapinput.cpp.
}

int CControls::SnapInput(int *pData)
{
    generated_helper((this), (pData),
        (m_aInputData[g_Config.m_ClDummy].m_PlayerFlags));
    // Original body continues unchanged.
}
```

The definition is inserted immediately before the original method, after its
existing includes and earlier declarations. Both the definition and call are
ordinary INSERT edits in the same immutable-span plan. They participate in diff,
ordering, conflicts and repeatable staging just like other edits.

## What each binding means

- `name`: parameter name visible inside the helper.
- `ty`: explicit C++ parameter type. No type inference is performed.
- `value`: expression evaluated in the target method at the injection point.

Bindings appear in manifest order in the parameter and argument lists. Each
value is parenthesized and passed once; C++ determines argument evaluation order,
so avoid bindings with interdependent side effects. A by-value argument is a
copy; a pointer or reference can access/mutate the original object. Use `const`
when mutation is not intended. `bindings = []` (or no bindings) creates a
zero-argument helper.

The helper has **no implicit `this`**, target parameters or target locals.
Use the binding names instead. Passing `controls = this` does not grant access
to private/protected CControls members. If the target method can access a member,
it can explicitly pass a reference/value to that member via a binding (as in
player_flags above). Public methods are callable through a bound object pointer.
The compiler still checks access, conversions, constness and lifetimes.

HEAD can use parameters/members but not locals declared later. TAIL runs at the
lexical closing brace, not before every return; locals are usable only if their
scope actually reaches that point. HEAD/TAIL refer to the original body, not to
any other mod's generated helper body.

## Files, validation and limits

Choose exactly one of `content`, `content_file`, or `helper` per injection.
`bindings` is only valid with `helper`. Files use the same safe, mod-relative
`hooks/...` rules as content_file: UTF-8 regular files, no path traversal or
symlink components. They are read fresh during manifest loading. Bodies remain
verbatim; generated wrapper/call newlines follow the target source style.

Binding names must be unique, nonreserved C++ identifiers. Supported types are
simple named/qualified types, pointer/reference forms, and simple template types
with type arguments; use typedefs for arrays/function pointers/complex types.
Values must be balanced expression text, with no directives, comments, semicolons
or blocks. These checks are conservative syntax guards, not full type checking.

Helpers currently require an unconditional, out-of-class function definition at
translation-unit scope in a `.cpp` file. Inline class definitions, namespace or
extern blocks, template declaration heads and conditionally defined targets are
rejected. Scoped types must be fully qualified where needed because the helper
is a free function, not a class member. Preprocessor directives inside helper
bodies are rejected; add required includes to the target source through an edit.

Helpers return void. Bare `return;` exits the helper only; return values,
coroutines, goto, implicit this and cancellation are unsupported. No `cancellable`
field or automatic interception of the target's return is implemented. For a
textual in-body return, use content/content_file and understand the target's
return type; that is different from a helper.
