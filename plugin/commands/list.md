# cli-anything-rs:list

**See the whole factory at a glance.** Enumerate the CLI harnesses in the
workspace — version, generated `SKILL.md`, installed binary — so an agent (or
you) knows what's already agent-native.

## Usage

```
/cli-anything-rs:list [--json]
```

## What this does

1. Read the workspace `Cargo.toml` `[workspace].members` and list each
   `crates/cli-anything-<name>` harness (excluding `cli-anything-core` and
   `cli-anything-new`).
2. For each, report: crate name, version, whether a `SKILL.md` exists, and
   whether the binary is installed (`which cli-anything-<name>`).
3. With `--json`, emit a JSON array of `{name, version, has_skill, installed}`.
