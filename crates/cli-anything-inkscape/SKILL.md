---
name: cli-anything-inkscape
description: Build SVG documents, export via real inkscape, and safely import untrusted SVG.
---

# cli-anything-inkscape

Build SVG documents, export via real inkscape, and safely import untrusted SVG.

## Installation

```bash
cargo install --path crates/cli-anything-inkscape
```

Requires the real backend: inkscape (for PNG/PDF export). Install with `install Inkscape from https://inkscape.org`.

## Conventions

- Every command accepts `--json`. JSON output is a uniform envelope: `{ok, action, data, error, warnings}` (`ok=false` plus `error.kind`/`error.message`/`error.hint` on failure).
- Run with no subcommand to enter an interactive REPL.
- Global flags: `--project <path>`, `--dry-run`.

## Commands

### document

Document lifecycle, canvas, and safe SVG import

- `document new` — Create a new document
  - `--width <WIDTH>`
  - `--height <HEIGHT>`
  - `--units <UNITS>`
  - `--background <BACKGROUND>`
  - `--name <NAME>`
  - `--output <OUTPUT>`
- `document open` — Open an existing `.inkscape-cli.json` document
  - `<PATH>` (required)
- `document save` — Save the current document
  - `<PATH>`
- `document info` — Show document + session status
- `document json` — Print the document model as JSON
- `document canvas-size` — Resize the canvas
  - `--width <WIDTH>` (required)
  - `--height <HEIGHT>` (required)
- `document units` — Set the canvas units
  - `<UNITS>` (required)
- `document import` — Safely import (validate) an untrusted SVG file — the security showcase
  - `<PATH>` (required)

### shape

Add and manage shapes

- `shape add-rect` — Add a rectangle
  - `<X>` (required)
  - `<Y>` (required)
  - `<WIDTH>` (required)
  - `<HEIGHT>` (required)
  - `--rx <RX>`
- `shape add-circle` — Add a circle
  - `<CX>` (required)
  - `<CY>` (required)
  - `<R>` (required)
- `shape add-ellipse` — Add an ellipse
  - `<CX>` (required)
  - `<CY>` (required)
  - `<RX>` (required)
  - `<RY>` (required)
- `shape add-line` — Add a line
  - `<X1>` (required)
  - `<Y1>` (required)
  - `<X2>` (required)
  - `<Y2>` (required)
- `shape add-polygon` — Add a polygon (SVG points string)
  - `<POINTS>` (required)
- `shape add-path` — Add a path (SVG `d`)
  - `<D>` (required)
- `shape add-star` — Add an N-pointed star
  - `<CX>` (required)
  - `<CY>` (required)
  - `<R>` (required)
  - `<POINTS>`
- `shape remove` — Remove an object by index
  - `<INDEX>` (required)
- `shape duplicate` — Duplicate an object by index
  - `<INDEX>` (required)
- `shape list` — List all objects
- `shape get` — Show one object by index
  - `<INDEX>` (required)

### text

Add and manage text

- `text add` — Add a text object
  - `<X>` (required)
  - `<Y>` (required)
  - `<CONTENT>` (required)
  - `--font-size <FONT_SIZE>`
- `text list` — List text objects

### style

Fill / stroke / opacity

- `style set-fill` — Set the fill color of an object
  - `<INDEX>` (required)
  - `<COLOR>` (required)
- `style set-stroke` — Set the stroke color and width
  - `<INDEX>` (required)
  - `<COLOR>` (required)
  - `--width <WIDTH>`
- `style set-opacity` — Set the opacity (0.0–1.0)
  - `<INDEX>` (required)
  - `<OPACITY>` (required)
- `style get` — Show an object's style
  - `<INDEX>` (required)

### transform

Translate / rotate / scale objects

- `transform translate` — Translate an object
  - `<INDEX>` (required)
  - `<DX>` (required)
  - `<DY>` (required)
- `transform rotate` — Rotate an object (degrees, about its origin or a point)
  - `<INDEX>` (required)
  - `<DEGREES>` (required)
  - `--cx <CX>`
  - `--cy <CY>`
- `transform scale` — Scale an object
  - `<INDEX>` (required)
  - `<SX>` (required)
  - `<SY>` (required)
- `transform get` — Show an object's transform
  - `<INDEX>` (required)
- `transform clear` — Clear an object's transform
  - `<INDEX>` (required)

### layer

Layer management

- `layer add` — Add a layer
  - `<LABEL>` (required)
- `layer list` — List layers
- `layer move-object` — Move an object to a layer index
  - `<INDEX>` (required)
  - `<LAYER>` (required)

### gradient

Gradients

- `gradient add-linear` — Add a linear gradient (stops as `offset:color` pairs)
  - `<X1>` (required)
  - `<Y1>` (required)
  - `<X2>` (required)
  - `<Y2>` (required)
  - `--stops <STOPS>`
- `gradient add-radial` — Add a radial gradient
  - `<CX>` (required)
  - `<CY>` (required)
  - `<R>` (required)
  - `--stops <STOPS>`
- `gradient apply` — Apply a gradient id to an object's fill or stroke
  - `<GRADIENT>` (required)
  - `<INDEX>` (required)
  - `--target <TARGET>`
- `gradient list` — List gradients

### path

Path operations

- `path list-operations` — List supported path operations
- `path convert` — Convert an object to a path (records intent)
  - `<INDEX>` (required)
- `path union` — Boolean union (recorded; true geometry needs a geometry backend)
  - `<A>` (required)
  - `<B>` (required)
- `path difference` — Boolean difference (recorded)
  - `<A>` (required)
  - `<B>` (required)

### export

Export to SVG / PNG / PDF

- `export svg` — Export to SVG (generated locally; no inkscape needed)
  - `<OUTPUT>` (required)
  - `--overwrite`
- `export png` — Export to PNG via the real inkscape
  - `<OUTPUT>` (required)
  - `--dpi <DPI>`
  - `--width <WIDTH>`
  - `--height <HEIGHT>`
  - `--overwrite`
- `export pdf` — Export to PDF via the real inkscape
  - `<OUTPUT>` (required)
  - `--overwrite`
- `export presets` — List export presets

### session

Undo/redo session control

- `session status` — Show undo/redo status
- `session undo` — Undo the last change
- `session redo` — Redo the last undone change
- `session history` — Show the operation history

## Agent guidance

- Always pass `--json` for machine-readable output.
- Check the exit code: `0` success, `1` error.
- On error, read `error.kind` (stable) and `error.hint`.
- Use absolute paths for `--project` and outputs.
- Verify produced files (e.g. magic bytes) rather than trusting exit code alone.

