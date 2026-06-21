# cli-anything-rs:validate

Validate a Rust CLI harness against HARNESS-rs.md.

## Usage

```
/cli-anything-rs:validate <crate-name-or-path>
```

## Checklist

Read `HARNESS-rs.md`, then verify the target crate:

- [ ] `#![forbid(unsafe_code)]` present in `lib.rs`/`main.rs`.
- [ ] Every command supports `--json` and emits the uniform `{ok, action, data, error, warnings}` envelope; exit 0/1; errors on stderr.
- [ ] Bare invocation enters the reedline REPL; `emit-skill` is a hidden subcommand.
- [ ] Subprocess only via `core::security::subprocess::run` (no shell strings); timeouts set; missing backend → typed error + install hint.
- [ ] Untrusted XML/SVG via `core::security::xml::read_svg_safely`; load/save via `guard_project_path`.
- [ ] Auto-save + `--dry-run` via `core::AutoSaveGuard`; one-shot uses `commit()`.
- [ ] Output is verified by magic bytes/format (not just exit code).
- [ ] `cargo build` + `clippy -D warnings` + `fmt --check` + `cargo deny check` all green.
- [ ] `SKILL.md` exists and matches the clap tree (`emit-skill`).

Report each item as pass/fail with the specific evidence (file:line or command output).
