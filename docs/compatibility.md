# Compatibility and limitations

DDNet Mixins is in early development. It parses a subset of C++ and does not
expand macros or type-check snippets. A successful `check` means selectors,
conflicts and structural CMake placement are valid—not that the client compiles.
Use `build` to validate the result with DDNet's toolchain.

## DDNet versions and build targets

The reference DDNet revision is
[`176fc2f9ab85ace703c6ec10da0249b01995ba94`](https://github.com/ddnet/ddnet/commit/176fc2f9ab85ace703c6ec10da0249b01995ba94).
For a separate dependency checkout, the corresponding ddnet-libs revision is
[`c0e6703fbcdbe03df2f26875427ec3951ce4ec21`](https://github.com/ddnet/ddnet-libs/commit/c0e6703fbcdbe03df2f26875427ec3951ce4ec21).
Other revisions may work, but changes to declarations, component collections or
CMake structure can make selectors fail. Compatibility and dependency version
constraints currently accept only `*`; this does not guarantee compatibility.

Only mod files under `src/game/client/**` map automatically to `game-client`.
Server, engine, shared and tool target mappings are not supported. Mod overlays
cannot replace existing upstream files or each other's logical paths.
Components require the supported CGameClient registration layout and direct
public CComponent inheritance; see [component declarations](advanced-injections.md#components).

## C++ parsing and selectors

Supported declaration patterns include simple namespaces/classes, qualified
function definitions, common qualifiers/operators, simple template heads/types,
noexcept expressions, trailing returns and parenthesized constructor initializers.
Function-pointer declarators, arbitrary template expressions, anonymous namespaces,
braced constructor initializers and many macro-decorated declarations are not
supported reliably.

`inspect` emits a JSON inventory of supported nodes, unsupported regions and
structural diagnostics. It returns nonzero if diagnostics occur, including in
bundled external headers. An unsupported region is not necessarily invalid C++.
Queries fail when lexical/structural errors or potentially matching unsupported
regions prevent safe selection. Overloads require a more precise selector;
`parameters` matches exact parenthesized source text, not normalized types.

Preprocessor branches are preserved, not evaluated. Delimiter structure that
diverges across branches can make a target unavailable. A successful corpus scan
is not a complete semantic AST or proof that every construct can be edited.

## Injection limits

- TAIL is the lexical body end, not interception of every return.
- Call edits select one outermost standalone call, not nested expressions,
  conditional bodies or lambda calls. Ambiguous argument boundaries fail.
- RETURN supports one final nonempty return expression, not general control flow.
- WRAP uses an explicit nested original block and rejects early-return/goto cases.
- Helpers support HEAD/TAIL, return void and receive only explicit bindings.
  They do not gain private access or cancel the target function.
- CANCEL is not supported.

See [edit selectors](edits.md), [advanced injection semantics](advanced-injections.md)
and [helper restrictions](helpers.md#files-validation-and-limits) before choosing
an operation. The compiler checks snippet types, access and lifetimes.

## Filesystem and build limits

Do not change source/mod inputs during apply or build. Concurrent build/apply and
hostile filesystem replacement are unsupported. Copy staging excludes Git metadata;
it does not preserve the original checkout's revision information in the build.
Interrupted staging transactions require manual recovery; see
[staging and build](staging.md).

CMake paths may contain spaces, but semicolons, dollar expressions and control
characters are rejected. Source/mod symlinks are forbidden; the external dependency
copier permits contained links and materializes independent copies.

A successful compilation does not verify gameplay behavior. Test mod behavior in
the client, and check the repository's CI results for platform-specific build status.
