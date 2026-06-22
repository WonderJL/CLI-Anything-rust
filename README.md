<h1 align="center">CLI-Anything-rust</h1>

<p align="center"><strong>Today's Software Serves Humans 👨‍💻. Tomorrow's Users will be Agents 🤖.</strong><br>
Turn real software into agent-native CLIs — in Rust, with memory safety and safe-by-default I/O as first-class goals.</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.89-orange?logo=rust&logoColor=white" alt="Rust 1.89">
  <img src="https://img.shields.io/badge/unsafe-forbidden-success" alt="forbid(unsafe_code)">
  <img src="https://img.shields.io/badge/tests-91_passing_+_7_gated_(E2E_proven)-brightgreen" alt="tests">
  <img src="https://img.shields.io/badge/gate-build·clippy·fmt·deny-blue" alt="gate">
  <img src="https://img.shields.io/badge/supply_chain-cargo--deny-blueviolet" alt="cargo-deny">
  <img src="https://img.shields.io/badge/output-JSON_+_Human-9cf" alt="output">
  <img src="https://img.shields.io/badge/License-Apache_2.0-yellow" alt="License">
</p>

> A focused, **security-first Rust reimplementation** of HKUDS'
> [CLI-Anything](https://github.com/HKUDS/CLI-Anything) — the *one-shot CLI
> generation factory* plus **2 reference CLIs** that prove it end-to-end. This is
> a faithful reproduction of *how* the factory works (not the upstream's 26-CLI
> catalog), with Rust's guarantees as the motivation. See
> [Credits & prior art](#credits--prior-art).

---

## Why a CLI?

The command line is the universal interface for **both humans and AI agents**:

- **Structured & composable** — text commands match the way LLMs emit actions and chain into workflows.
- **Lightweight & universal** — minimal overhead; a single static binary, no runtime to install.
- **Self-describing** — `--help` is automatic documentation an agent can discover.
- **Agent-first output** — a built-in `--json` flag delivers structured data; humans get colored tables.
- **Deterministic & reliable** — consistent results enable predictable agent behavior.

The goal is to build **structured interfaces _to_ real software — not dumbed-down
reimplementations** that miss most of its functionality.

## The agent–software gap

| The gap | CLI-Anything-rust's answer |
|---|---|
| Agents reason well but can't drive real professional software | A thin CLI **to the real tool** (mmdc, inkscape, …) — never a fake reimplementation |
| GUI automation is fragile; bespoke APIs are partial | Structured subcommands **plus a stateful REPL** over the real backend |
| Agents need parseable, predictable output | A **uniform `--json` envelope** on every command |
| Untrusted input + automation is a security risk | A **safe-by-default core**: no-shell subprocess, entity-safe XML, path/symlink guards, `#![forbid(unsafe_code)]` |

That last row is what this port adds: the Rust version treats **security as the
headline feature**, not an afterthought.

## What this is — a three-layer factory

1. **Agent brain** — the [`HARNESS-rs.md`](HARNESS-rs.md) methodology + a Claude
   Code plugin (`/cli-anything-rs`) that reads a target software's source and
   designs the command surface.
2. **Deterministic scaffolder** — [`cli-anything-new`](crates/cli-anything-new)
   stamps a *compilable* CLI crate (clap skeleton, reedline REPL, `--json`
   envelope, auto-save, `emit-skill` hook, tests). The agent then fills **only**
   the domain logic.
3. **Shared substrate** — [`cli-anything-core`](crates/cli-anything-core):
   session/undo, the `--json` envelope, the reedline skin, the full
   preview/trajectory subsystem, and the safe-by-default utilities — written once,
   `#![forbid(unsafe_code)]`.

## The security thesis (made concrete)

Not "it's Rust" hand-waving — every item below is implemented and unit-tested in
`cli-anything-core`:

- **`#![forbid(unsafe_code)]`** across the core and both proof CLIs.
- **No-shell subprocess** — every external tool runs via `Command::new(exe).args(&[…])`, never a shell string (a `$(…)` argument is passed literally, proven by test). Spawns the PATH-resolved binary in its own process group.
- **Bounded execution** — a timeout on every subprocess call; stdout capped (256 MiB → typed error), stderr tailed; a grace window so a pipe-holding grandchild can't hang the call.
- **Entity-safe XML** (the Rust analog of `defusedxml`) — rejects `DOCTYPE`/DTD (kills billion-laughs *before* expansion), processing instructions, non-builtin entity references, and external `href`/`xlink:href`/`src` (`http(s)`/`file`/`ftp`/`jar`) for SSRF/LFI defense; 25 MiB / depth-256 / 1M-element caps; single well-formed root.
- **Path-traversal + symlink guards** on every project load/save.
- **Crash-atomic saves** — write-temp → `fsync` → atomic `rename`, under an advisory lock (std `File::lock`, no extra crate); a crash mid-write can never truncate your project.
- **Fail loudly** — a missing/broken backend returns a typed error with an install hint; **never a silent fake-render fallback**.
- **Supply chain** — `cargo deny` (advisories, licenses, bans, sources) gates every build.

> These weren't free: an adversarial review pass over the core found **12 real
> latent bugs (3 HIGH-severity — silent data-loss on save, a swallowed auto-save
> failure, and a subprocess-timeout hang)** — all fixed with regression tests.

## How the factory builds a CLI

```bash
# 1. Scaffold a compilable crate (the deterministic half).
cargo run -p cli-anything-new -- --software <name>

# 2. The agent — driven by HARNESS-rs.md / the /cli-anything-rs command —
#    fills ONLY the domain logic: src/domain/*, src/cli.rs, src/backend.rs, src/repl_cmds.rs

# 3. Regenerate the agent-discoverable skill from the live clap tree.
cargo run -p cli-anything-<name> -- emit-skill -o crates/cli-anything-<name>/SKILL.md

# 4. Test + install.
cargo test -p cli-anything-<name>                  # offline
cargo test -p cli-anything-<name> -- --ignored     # real backend (E2E)
cargo install --path crates/cli-anything-<name>
```

## Dual-mode UX — see it work

Every command runs **one-shot** (for scripts/agents) or drops into a **styled
REPL** when invoked bare. Output is human-readable by default, structured under
`--json`:

```console
$ cli-anything-mermaid --project demo.mermaid.json --json export render diagram.svg -f svg
{"ok":true,"action":"export.render","data":{"output":"diagram.svg","format":"svg",
 "method":"http","file_size":11095,"url":"https://mermaid.ink/svg/pako:eNp…"},"error":null,"warnings":[]}
```

The **security showcase** — a malicious SVG is rejected with a structured error
an agent can act on:

```console
$ cli-anything-inkscape --json document import billion-laughs.svg
{"ok":false,"action":"document.import","data":null,
 "error":{"kind":"unsafe_svg","message":"xml contains a forbidden construct: DOCTYPE/DTD declaration","hint":null},
 "warnings":[]}
```

## The two proof CLIs

<table>
<tr><th>CLI</th><th>What it proves</th><th>Backend</th></tr>
<tr>
<td><b><code>cli-anything-mermaid</code></b></td>
<td>The full pipeline end-to-end: author a diagram, render it, and <b>verify the output by magic bytes</b>. One-shot + REPL, <code>--json</code> everywhere, auto-save + <code>--dry-run</code>. Wires the <b>preview subsystem</b>: <code>preview capture</code> renders into an immutable, content-addressed bundle and advances a live session + trajectory an agent can poll cheaply.</td>
<td>local <code>mmdc</code> → HTTP fallback to mermaid.ink (pako-encoded). <i>Verified live: real SVG + PNG + a full preview round-trip rendered and magic-byte-checked.</i></td>
</tr>
<tr>
<td><b><code>cli-anything-inkscape</code></b></td>
<td>The pipeline <b>plus the security showcase</b>: a full SVG document model (shapes/text/style/transform/layers/gradients), SVG built via the <code>quick-xml</code> writer (untrusted text escaped), and <b>safe import of untrusted SVG</b> (rejects DOCTYPE/billion-laughs/SSRF).</td>
<td>real <code>inkscape</code> (default) or librsvg's <code>rsvg-convert</code> (<code>--renderer rsvg</code>) for PNG/PDF — a real, selectable alternative renderer, never a fake fallback. Fails loudly if the chosen backend is absent. <i>Verified live via rsvg.</i></td>
</tr>
</table>

<details>
<summary>Command surfaces</summary>

- **mermaid** — `project new|open|save|info|samples` · `diagram set|show` · `export render|share` · `preview capture|status|list` · `session status|undo|redo` · (bare → REPL) · hidden `emit-skill`.
- **inkscape** — `document new|open|save|info|json|canvas-size|units|import` · `shape add-rect|add-circle|add-ellipse|add-line|add-polygon|add-path|add-star|remove|duplicate|list|get` · `text add|list` · `style set-fill|set-stroke|set-opacity|get` · `transform translate|rotate|scale|get|clear` · `layer add|list|move-object` · `gradient add-linear|add-radial|apply|list` · `path list-operations|convert|union|difference` · `export svg|png|pdf|presets` (png/pdf take `--renderer inkscape|rsvg`) · `session status|undo|redo|history` · (bare → REPL) · hidden `emit-skill`.
</details>

## Core design principles

- **Authentic integration** — drive the real tool and verify the real output; never fake a render.
- **Dual mode** — one-shot subcommands *and* a styled REPL; bare command → REPL.
- **Consistent UX** — one shared core crate (the reedline skin), not copy-pasted boilerplate.
- **Agent-native** — `--json` on everything; discoverable via `--help` and `emit-skill` → `SKILL.md` (generated from the live clap tree, so it never drifts).
- **Safe by default** — see [the security thesis](#the-security-thesis-made-concrete).
- **Deterministic boilerplate** — the scaffolder stamps the skeleton; the agent writes only domain logic.

## Project structure

```
crates/cli-anything-core/      # shared substrate (#![forbid(unsafe_code)])
crates/cli-anything-new/       # the scaffolder (+ embedded minijinja templates/)
crates/cli-anything-mermaid/   # proof CLI #1 — render + verify
crates/cli-anything-inkscape/  # proof CLI #2 — SVG model + export + security showcase
plugin/                        # Claude Code plugin (/cli-anything-rs + refine/test/validate/list)
HARNESS-rs.md                  # the agent methodology (7 phases + a security checklist)
scripts/check.sh               # local gate: build + clippy -D warnings + fmt + cargo-deny
```

## Plugin commands

Load the [`plugin/`](plugin) directory as a Claude Code plugin, then:

| Command | Purpose |
|---|---|
| `/cli-anything-rs <software-path-or-repo>` | Build a new harness from a software's source |
| `/cli-anything-rs:refine <crate> [focus]` | Expand an existing harness's coverage |
| `/cli-anything-rs:test <crate>` | Run its tests (incl. `--ignored` E2E) |
| `/cli-anything-rs:validate <crate>` | Check it against `HARNESS-rs.md` |
| `/cli-anything-rs:list` | List the harnesses in the workspace |

## Develop

```bash
scripts/check.sh                       # build + clippy -D warnings + fmt + cargo-deny
cargo test --workspace                 # 91 offline tests
cargo test --workspace -- --ignored    # 7 E2E: mermaid render (2) + preview round-trip (1) pass live;
                                       #        inkscape rsvg PNG/PDF (2) pass live; inkscape-native PNG/PDF (2) need a working local inkscape
```

`cargo-deny` is required for the gate: `brew install cargo-deny` (or `cargo install cargo-deny --locked`).
Distribution is local-only for now (`cargo install --path …`); crate metadata is publish-ready with `publish = false` until the API settles.

## Status, limitations & roadmap

**Status:** the factory + both proof CLIs are complete; the gate (build · clippy `-D warnings` · fmt · cargo-deny) is green with **91 offline tests passing**. Seven more E2E tests are gated behind `--ignored` and **all the runnable ones pass live**: mermaid render (SVG + PNG) and the full preview round-trip pass via mermaid.ink; inkscape PNG/PDF export passes via the real `rsvg-convert`. The 2 inkscape-native (Inkscape binary) E2E tests remain gated because the local Inkscape cask is broken here.

Honest limitations (this is a proof, not the upstream ecosystem):

- [x] Shared core, scaffolder, and **2** reference CLIs — **not** a 26-CLI catalog.
- [x] **Real-backend export E2E is proven** — SVG→PNG/PDF runs live through the real `rsvg-convert` (librsvg) backend and is verified by magic bytes.
- [ ] **inkscape-native PNG/PDF E2E is gated** — the local Inkscape cask is broken (its wrapper points at a missing binary), so the 2 Inkscape-specific E2E tests are `#[ignore]`d. The code path exists and verifies magic bytes; `--renderer rsvg` is the proven path here.
- [ ] **mermaid `mmdc` path needs a browser** — without one, rendering uses the HTTP fallback (mermaid.ink); both paths are implemented and the fallback is exercised live.
- [ ] **Path-boolean geometry is a recorded stub** — true 2D geometry needs a geometry backend (out of scope for the proof).
- [x] **Preview/trajectory subsystem is wired into `cli-anything-mermaid`** (`preview capture|status|list`) on top of the core bundle/session/trajectory layer; inkscape does not expose it yet.
- [ ] **No CI** — the gate runs locally (`scripts/check.sh`), not yet on every push.
- [ ] **crates.io publishing deferred** (`publish = false`); a `cli-hub`-style consumer is documented, not built.

Roadmap: add CI to enforce the gate on push · fix/containerize Inkscape to un-gate the native export E2E · wire the preview group into inkscape too · add more reference CLIs to stress generality · real path-boolean geometry · publish once the API settles.

## Credits & prior art

This project is an independent **Rust reimplementation of the CLI-Anything
methodology** created by **HKUDS** — see the upstream repository
([HKUDS/CLI-Anything](https://github.com/HKUDS/CLI-Anything)) and its technical
report ([arXiv:2606.03854](https://arxiv.org/abs/2606.03854)). The "agent-native
software", `HARNESS.md`, and SKILL.md ideas originate there; this port reproduces
that design in Rust and adds the safe-by-default security substrate. All upstream
trademarks, test counts, demos, and the CLI-Hub ecosystem belong to the original
project, not to this port.

## License

Apache-2.0.
