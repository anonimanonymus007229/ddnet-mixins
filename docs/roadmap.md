# Engineering roadmap

The supplied project specification (`LLM.md`, November 2026 development session)
is the design baseline. This first commit establishes phases 1–2 only, not the
specification's first useful release.

## Next: structural parsing

1. Token cursor with lookahead, marks and rewind.
2. Explicit preprocessor-region model and branch-local delimiter maps. Handle
   shared delimiters outside conditionals without guessing active branches.
3. Namespace/class/declaration/function index on real DDNet. Distinguish
   constructors, declarations, lambdas, braced initializers and macro syntax.
4. Fully qualified and signature selectors; reject missing/ambiguous/unsupported
   targets with source-located diagnostics. Index comments and macro arguments.

## Then: universal transformations

- Resolve all targets against immutable original spans.
- Only INSERT / REPLACE / REMOVE primitives; HEAD/TAIL lower to body insertions.
- Stable dependency/priority/ID ordering, explicit same-offset ordering,
  overlap detection, JSON plans and diffs. No writes during plan/check.
- Preserve newline style and untouched bytes.

## Staging and integration

- Tool-owned `.ddmix/staging/ddnet` copy/worktree; never transform upstream.
- Canonicalized path containment, no symlink writes, atomic state publication,
  ownership checks, repeatable regeneration from pristine inputs.
- Parse actual CMake command/block structure to locate a common point after
  platform-specific game-client creation. Never append at a guessed line.
- External mod sources via generated target_sources/target_include_directories;
  explicit DDMIX_WORKSPACE_ROOT. Inspect source checksum metadata before choosing
  its integration mechanism.
- Component declarations lower to generic include/member/registration edits
  discovered from actual CGameClient structures and lifecycle.
- Config insertions use the parsed reserved comment; detect duplicate IDs.
- Configure and build the staged client, including a real declared component.

Only init/list/inspect are exposed today. plan/check/apply/build must not be
advertised until their safety contracts are implemented and integration-tested.
Advanced aliases, semver constraints, non-client target mapping, distribution
artifacts and comprehensive platform testing remain future work.
