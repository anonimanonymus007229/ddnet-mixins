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

Supported declarations include qualified functions, common operators/conversions,
noexcept expressions, trailing returns, standard/GNU/MSVC attributes and alignas.
Recursive parenthesized declarators cover function-pointer returns and references
to arrays. Template support includes default parameters, qualified specializations,
common base initializers, named/parenthesized requires clauses and requires
expressions. Constrained overloads can be selected by exact constraint source
text. This is not a complete implementation of all C++ template/constraint grammar.

Enums, aggregate typedef aliases, anonymous struct/union boundaries, braced
variables and common lambda initializers have separate syntax nodes. Attributes,
recognized annotation macros, constraints and linkage blocks retain source spans.
A fixed annotation policy covers common thread-safety macros and selected bundled
library annotations; arbitrary macro calls are not silently removed.

Ordinary and inline namespaces are indexed. Anonymous namespaces use a synthetic
`(anonymous@<byte-offset>)` scope to avoid merging distinct blocks. Generated
identifiers containing `$` are accepted as a compiler extension. Helpers still
have their stricter translation-unit-scope restrictions.

Uppercase invocation forms followed by a body, such as TEST/TEST_F, are indexed
as macro-body syntax—not expanded functions. Use the explicit
[macro-body selector](edits.md#macro-body-targets) to edit them. No declarations
or members produced by macro expansion are inferred.

`inspect` emits a JSON inventory of supported nodes, unsupported regions and
structural diagnostics. It returns nonzero if diagnostics occur, including in
bundled external headers. An unsupported region is not necessarily invalid C++.
Queries fail when lexical/structural errors or potentially matching unsupported
regions prevent safe selection. Overloads require a more precise selector;
`parameters` matches exact parenthesized source text, not normalized types.

Preprocessor branches are preserved, not evaluated. Delimiter analysis explores
all branch arms and merges equivalent states. Tokens with multiple possible
partners retain alternatives instead of guessed pairs. Enclosing bodies with
unique boundaries can still be selected; invalid branches or excessive state
expansion produce diagnostics. Exact paired conditional extern-C wrappers with
the same textual guard are recognized separately.

Alternative signatures sharing one source body and constructors containing
conditional/include-generated initializer fragments have explicit nodes. Ordinary
function selectors reject these forms; use [body-region targets](edits.md#conditional-and-constructor-bodies)
for edits confined to proven body boundaries. Directives inside enums/constructor
headers remain opaque: included declarations/initializers are not expanded.
Objective-C interface/implementation blocks embedded in platform headers are
recorded as foreign regions, not C++ functions; no Objective-C edit selector is
provided. Other unsupported or ambiguous macro-driven declarations can still
block a target.
A successful corpus scan is not a complete semantic AST or proof that every
construct can be edited.

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
