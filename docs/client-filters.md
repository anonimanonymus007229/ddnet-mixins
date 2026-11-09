# Client-specific rules

Add `clients` to a table-array entry to select which DDNet checkout folders use
that rule. Matching uses the **canonical DDNet root folder name**, not its Git
remote, version, branch or the generated staging directory.

```toml
[[edits]]
id = "config"
clients = { exclude = ["BestClient", "OtherClient"] }
file = "src/engine/shared/config_variables.h"
action = "INSERT"
target = { kind = "comment", text = "Add config variables for mods" }
position = "AFTER"
content_file = "hooks/config-default.cpp"

[[edits]]
id = "config"
clients = { include = ["BestClient", "OtherClient"] }
file = "src/engine/shared/config_variables.h"
action = "INSERT"
target = { kind = "file" }
position = "END"
content_file = "hooks/config-alternative.cpp"
```

The alternative snippets must suit those clients' actual config layout. The
example shows rule selection, not automatic detection of a config system.

## Matching

- No `clients`: apply everywhere.
- Omitted `include`: include every folder. `include = []`: include none.
- `exclude` defaults to empty and always wins over include.
- Patterns are case-sensitive: `*` matches any text, `?` matches one character.
- Patterns match the complete folder name. Slashes, drive prefixes and control
  characters are rejected; use names such as `BestClient` or `RushieClient-*`.

```toml
clients = { include = ["*Client", "RushieClient-*"], exclude = ["OldClient"] }
```

The same filter syntax works for all table-array rules: edits, injections,
component groups, individual components, and nested group members/bindings.
Filtering is a shared manifest pass, not a separate per-operation mechanism.
Only supported rule tables are executed; filtering does not add new operations.

Excluded entries are removed before typed validation, hook reads, duplicate-ID
checks or target resolution and are logged as skipped. An excluded parent also
excludes its children. Filter syntax/patterns themselves must be valid. IDs may
be reused for mutually exclusive variants, but matching duplicate IDs still fail.
Filtered members cannot leave an active component group empty.

`plan`, `check`, `apply` and `build` use their `--ddnet` path automatically. For
listing a mod set with filters, supply a context explicitly:

```sh
ddmix list --mods mods --ddnet workspace/BestClient
```

Without a DDNet context, a manifest containing filters reports an error rather
than showing an arbitrary variant. Folder renames change which rules match.
Filtering an edit/component does **not** disable the mod's automatic source
overlay discovery; separate incompatible source packages when necessary.
