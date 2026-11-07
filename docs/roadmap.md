# Implementation status and remaining work

The supplied project specification is the baseline. The supported real-DDNet
workflow is implemented and Windows compilation verified; this is still an early
release with conservative C++ grammar and explicit compatibility limits.

## Implemented

- Strict manifests, source-overlay discovery, dependency order/cycle checking.
- Own byte-span lexer, directives, branch-aware delimiter matching.
- Namespace/class/function definitions, declaration-only methods, template heads
  and return types, common operators/noexcept/trailing returns, initial ctor
  support. Lexical macro invocations with nested argument spans.
- Qualified selectors and overload disambiguation; unsupported-region checks.
- Generic INSERT/REPLACE/REMOVE plans, deterministic ordering, conflicts, diff.
- Injection content inline or from UTF-8 mod-relative hooks/ files, with strict
  containment checks and no separate snippet compilation.
- HEAD/TAIL static void helpers in the target translation unit, with explicit
  typed argument bindings, source-scope checks and safe mod-relative helper files.
- HEAD/TAIL, bounded standalone BEFORE_CALL/AFTER_CALL/MODIFY_ARG, final RETURN,
  body OVERWRITE and explicit-original-block WRAP.
- Config identifier/console-name conflict checks, exclusive branch awareness.
- Staged fresh copy, ownership inventory, repeated apply, external source CMake
  mapping, checksum integration/capacity checks, build command.
- Component include/member/all/input registration lowering to generic edits.
- Real Windows Release client compile with all example mods and pinned libs.
- Local Windows executable/ZIP/checksum; multi-platform release workflow.
- Observer-based CLI logging with banner/version, stage/count progress, elapsed
  timings, quiet/verbose/color modes, compiler transcripts and failure summaries.

## Not universal or release-validated yet

- Full C++ AST/type checking/macro expansion is not implemented; function pointer
  declarators, arbitrary template expressions, anonymous namespaces and braced
  ctor initializers still need grammar work. Corpus has unsupported regions.
- Nested/conditional expression calls and general multi-return interception are
  rejected. WRAP is an explicit nested block, not arbitrary original-function
  forwarding. CANCEL has no defined semantics and is not exposed.
- Components support the inspected DDNet layout, simple names and direct public
  CComponent inheritance. Other versions/layouts need corpus/build validation.
- Versions accept wildcard only; non-client target mappings are unavailable.
- Copy staging loses original Git metadata; crash recovery is manual. Concurrent
  source mutation/build/apply and hostile filesystem races are unsupported.
- Full client runtime/game-session behavior was not manually exercised.
- Linux/macOS binaries and CI/release upload are not observed yet; no public
  tag/release, installer, signing, notarization or updater has been produced.
- Minimum Rust version declaration has not been exercised separately locally.

See validation.md for actual executed tests/builds, advanced-injections.md for
precise alias semantics, and distribution.md for publishing instructions.
