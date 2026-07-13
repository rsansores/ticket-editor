# Ticket Editor

A monospace-grid **receipt / ticket editor** for thermal printers, with a live
preview that is **byte-for-byte identical** to what prints.

**▶ [Try the editor live](https://rsansores.github.io/ticket-editor/)** — it runs
entirely in your browser (the renderer is compiled to WebAssembly; no server).

![The editor: dragging and resizing an element while the preview updates live](docs/demo.gif)

Design on a grid — drag variables, static text, logos and QR codes; add loops
and conditionals — and the panel on the right shows exactly what the printer will
produce. Because the preview and the print come from the **same renderer**, it's
not an approximation:

<p>
  <img src="docs/receipt.png" alt="A rendered receipt with looped line items, a conditional discount, and a large total" width="330">
  &nbsp;&nbsp;
  <img src="docs/media.png" alt="A rendered receipt with a logo image and a QR code" width="330">
</p>

There is one renderer, written in Rust, compiled twice:

```
                    ticket-core  (the only renderer)
                    ┌───────────────────────────────────┐
                    │  JSON schema → grid layout → PNG   │
                    └───────────────┬───────────────────┘
              native │                              │ wasm
        ┌────────────▼───────────┐      ┌───────────▼────────────┐
        │ your backend           │      │ browser (Vue editor)    │
        │ render PNG → printer    │      │ live 1:1 preview         │
        └────────────────────────┘      └─────────────────────────┘
                    └──────────── identical bytes ──────────┘
```

Because both builds run the same code over the same embedded fonts, the preview
the user edits **is** the print output — parity is a compile target, not two
codebases kept in sync by hand.

## Packages

| Package | What it is | Registry |
|---------|-----------|----------|
| **`ticket-core`** | The renderer + the canonical document schema. Deterministic: same document + data → same PNG bytes. Optional features take it the rest of the way to the printer — see [below](#the-rest-of-the-pipeline). | crates.io |
| **`ticket-printable`** | `#[derive(Printable)]` on your own model structs → the variables JSON, and a matching sample tree for the editor. | crates.io |
| **`ticket-printable-derive`** | The proc-macro behind it. You depend on `ticket-printable`, not this. | crates.io |
| **`@ticket-editor/vue`** | The embeddable Vue 3 editor. Bundles the wasm renderer, so consumers need no Rust toolchain. | npm |

The Vue package embeds a WebAssembly build of `ticket-core` for its preview, so
the editor's preview and your backend's print output come from the exact same
renderer. All four ship at the **same version**, always — a renderer skew between
the preview and the print is precisely the bug this project exists to prevent.

## The document format

The editor reads and writes a versioned **JSON `TicketDoc`** — the single source
of truth. Everything is measured in **character cells**, never raw pixels: a
variable reserves a fixed number of columns, and its value is truncated or padded
to exactly that width, so a real value can never overflow its slot and shove the
layout around.

```jsonc
{
  "version": 1,
  "paper": { "width_chars": 40, "cell_width_px": 12, "cell_height_px": 22, "font_px": 20 },
  "regions": [
    { "id": "loop", "start_row": 3, "end_row": 4, "source": "sale.items" },
    { "id": "disc", "start_row": 6, "end_row": 7,
      "condition": { "var": "sale.discount", "op": "gt", "value": "0" } }
  ],
  "elements": [
    { "id": "t",  "row": 0, "col": 15, "type": "text", "content": "PET PALACE", "style": { "bold": true } },
    { "id": "p",  "row": 3, "col": 0,  "type": "variable", "path": "product", "length": 18 },
    { "id": "tv", "row": 7, "col": 20, "type": "variable", "path": "sale.total", "length": 19, "align": "right",
      "number": { "decimals": 2, "rounding": "half_up", "thousands": true }, "style": { "scale": 2 } }
  ]
}
```

- **elements** — placed at a `(row, col)`. A `text` element is literal; a
  `variable` pulls a value from your data at render time; there are also `image`
  (PNG logo, reduced to 1-bit) and `qr` (from a value) elements.
- **regions** — row-bands with flow behaviour: a `source` makes the band a
  **loop** (repeats once per array item, everything below reflows down); a
  `condition` makes it **conditional** (collapses when false).

## Frontend: the Vue editor

```bash
npm i @ticket-editor/vue vue vue-i18n
```

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { TicketEditor } from '@ticket-editor/vue'
import '@ticket-editor/vue/styles.css'
import type { TicketDoc } from '@ticket-editor/vue'

const doc = ref<TicketDoc>({ version: 1, paper: { width_chars: 40 }, elements: [] })

// Sample (or real) data — drives the variable tree and the preview values.
const sale = {
  sale: {
    subtotal: 46.24, discount: 4.62, total: 41.62,
    items: [
      { product: 'Dog Food', qty: 2, amount: 24.5 },
      { product: 'Cat Toy',  qty: 1, amount: 8.99 },
    ],
  },
}

async function persist(d: TicketDoc) {
  // Store the JSON however you like (this is the library user's responsibility).
  await fetch('/api/ticket-templates', { method: 'POST', body: JSON.stringify(d) })
}
</script>

<template>
  <TicketEditor v-model="doc" :variables="sale" :on-save="persist" />
</template>
```

### `<TicketEditor>` props

| Prop | Type | Purpose |
|------|------|---------|
| `v-model` (`modelValue`) | `TicketDoc` | The document. The editor keeps a private copy and emits snapshots — it never mutates your object. |
| `variables` | `Record<string, unknown>` | Sample or real data. Builds the variable tree and fills the live preview. |
| `variableTypes` | `Record<string, 'text'\|'number'\|'date'>` | Authoritative variable types keyed by dotted path. Gates which formatting the editor offers (numbers as numbers, dates as dates). Anything omitted is inferred from the sample. |
| `locale` | `string` | Force a UI language (`'en'`, `'es'`). If omitted, follows the host's `vue-i18n` locale. |
| `messages` | `Messages` | Override or extend the built-in UI strings, keyed by locale. |
| `on-save` | `(doc: TicketDoc) => void \| Promise<void>` | Called by the Save button. Persist the JSON wherever you want. |

### Theming

The editor styles itself entirely through the **shadcn CSS-variable contract**
(`--color-primary`, `--color-background`, `--radius`, …) with neutral fallbacks.
Embedded in an app that defines those tokens, it inherits the host look
automatically; standalone it uses the defaults. It has **no dependency on any
host UI kit**.

### Internationalization

Built-in English and Spanish, in a local `vue-i18n` scope that **follows the
host's locale** automatically (no wiring). Override any string via the
`messages` prop. `vue-i18n` is a peer dependency (shared with your app, not
bundled twice).

## Backend: rendering to a PNG

```toml
# Cargo.toml
ticket-core = "0.3"
```

```rust
use ticket_core::{render_png, TicketDoc};

// `doc` is the JSON you stored; `data` is the real sale as a serde_json::Value.
let doc: TicketDoc = serde_json::from_str(stored_json)?;
let png: Vec<u8> = render_png(&doc, &data)?;   // same renderer as the browser preview
```

That PNG is the deliverable if you print through CUPS or show the ticket on a
screen. If you are talking to a thermal printer directly, don't stop here — the
[`escpos`](#printing-it-the-escpos-feature) feature emits the bytes it actually
accepts.

One deliberate asymmetry: `render_png` runs in **print mode** — a variable path
that doesn't resolve in `data` renders *empty*, so a typo or a null field can
never print a plausible wrong value. The editor preview opts into *placeholder
mode* (deterministic fake values for missing paths) so the canvas stays lively
while designing; pass `RenderOptions::placeholders()` to
`render_png_with_options` if you want that behavior natively. With fully
resolved data the two modes are byte-for-byte identical. Use
`TicketDoc::unresolved_paths(&data)` to reject a template that references
fields your data doesn't have — the editor surfaces the same list while
designing.

`ticket-core` takes plain data (a `TicketDoc` and a `serde_json::Value`) and
returns PNG bytes — no database, framework, or printing coupling. It defends
against adversarial documents (bounded image/QR/canvas sizes, clamped decimals,
no panics on hostile input).

> **Print sizing:** set `cell_width_px` so `width_chars × cell_width_px` equals
> your printer's dot width (e.g. 384 dots for 58 mm, 576 for 80 mm) — then the
> preview is 1:1 with the paper.

## Features

- Monospace grid; **char-length reservation** so values never overflow their slot.
- **Text size** (1×–4× integer magnification, thermal-printer style), bold /
  italic, vertical align + a fractional **nudge** (super/subscript, ™/©).
- **Number** formatting (decimals, rounding method, thousands) and **date**
  reshaping; type-aware (offered only where it applies).
- **Text wrap** (word-aware) within a fixed column band.
- **Logos** (PNG, reduced to 1-bit by threshold or Floyd–Steinberg dithering) and
  **QR codes** (from a literal or a variable).
- **Straight to the printer** (`escpos` feature): ESC/POS bytes, not just a PNG —
  with a printer profile that keeps a `cut` marker from bricking a cutter-less
  device. See [below](#printing-it-the-escpos-feature).
- **Loops** over repeatable arrays and **conditionals**, with content reflow —
  authored via a git-style lane and configured in the side drawer.
- Non-destructive editing: free placement, insert/remove rows, overflow zone,
  overlap flags, "fit to width".

Loops and conditionals are marked in a git-style lane next to the rows and
configured in the drawer — no template language to learn:

![Configuring a loop: the band lane on the left, its settings in the right drawer](docs/loops.png)

## Repository layout

```
crates/ticket-core            # the renderer + schema (native + wasm), the crates.io package
                              #   features: normalize (shrink for transport), escpos (print), bundled-fonts
crates/ticket-wasm            # wasm-bindgen wrapper (build tool; produces the browser bundle)
crates/ticket-printable       # derive an annotated model into the variables JSON (DB/framework-agnostic)
crates/ticket-printable-derive# the #[derive(Printable)] proc-macro
packages/ticket-editor        # the Vue editor (@ticket-editor/vue), embeds the wasm
scripts/build-wasm.sh         # rebuild the browser wasm bundle from ticket-core
```

## The rest of the pipeline

The renderer is the middle of the story, not the whole of it. A ticket has to be
*fed* data, and it has to *get* somewhere — and both of those turn out to have a
sharp edge that isn't obvious until it cuts you. Each is a separate crate or an
off-by-default feature, so you only pay for the ones you use.

```
   your models                                      a thermal printer
        │                                                   ▲
        │ ticket-printable          ticket-core             │ escpos
        │ (derive)                  (the renderer)          │ (feature)
        ▼                                                   │
   variables JSON ──────────► TicketDoc ──► PNG ────────────┘
                                  │
                                  │ normalize (feature)
                                  ▼
                          a document small enough to ship
```

### Feeding it: `ticket-printable`

The renderer wants a `variables` JSON tree. You could hand-write it — and then
hand-write a *second*, fake one so the editor has something to preview against.
Now you have two hand-written trees that must agree with each other and with your
real model, forever, and nothing enforces it. They drift. When they drift, the
editor happily lets someone design against a field that the print path does not
supply, and the failure is silent: a blank space on a customer's receipt, found
weeks later.

`#[derive(Printable)]` on your existing structs removes the second and third
copies. One annotated definition yields both the real render data and the sample
tree the editor previews with, so they cannot disagree. It is DB- and
framework-agnostic: annotate a struct, get JSON. See
[`crates/ticket-printable`](crates/ticket-printable).

### Shipping it: the `normalize` feature

A logo pasted into the editor is an 8-bit RGBA PNG — roughly 24 bits per pixel,
of which a monochrome printer keeps exactly one. That waste is free if the
document renders on the machine it is stored on. It is not free if the document
has to travel to the device on every save, over a link you don't control.

`normalize_images` does the renderer's image work up front — decode, scale to the
target box, threshold or dither — and writes each result back as a 1-bit PNG. The
renderer's own pass then degenerates to a no-op (the scale is identity, and
thresholding an already-binary image is idempotent), so the ticket renders
**pixel-for-pixel the same** from a far smaller document. A real 48-char receipt
went from 102 kB to 13 kB; the images in it, 95 kB to 3.3 kB.

```rust
let mut doc: TicketDoc = serde_json::from_str(&json)?;
let stats = ticket_core::normalize_images(&mut doc)?;   // 102 kB -> 13 kB
send_to_device(&serde_json::to_string(&doc)?);
```

Compression is not a substitute, incidentally, and it's worth knowing why before
you reach for it: the payload is base64 of *already-deflated* PNG, so gzip on the
original document buys about 1.4× and leaves you at 73 kB. Normalizing is what
makes the document compressible again — what's left is repetitive JSON.

It also accepts the formats the renderer cannot (JPEG, WebP) and canonicalizes
them to PNG. `render` reads PNG only and draws a placeholder frame for anything
else, so a WebP logo prints as an empty box; normalize it and the logo appears.

Keep your original document if you want to re-tune a threshold later — baking is
one-way. Normalize on the way out.

Off by default: it pulls in `image` for the JPEG/WebP decoding, which a build
that only draws the preview has no use for.

### Printing it: the `escpos` feature

A renderer that stops at a PNG is half a library. No thermal printer takes a PNG;
it wants a 1-bit raster fed with `GS v 0`, wrapped in control codes. Every user
would otherwise write that same conversion, and the interesting part is not the
raster — it's the part that bites.

```rust
use ticket_core::{render_escpos, CutMode, Fonts, PrinterProfile, RenderOptions};

let profile = PrinterProfile { cut: CutMode::Partial };  // only if it really has a cutter
let bytes = render_escpos(&doc, &data, &fonts, &RenderOptions::default(), &profile)?;
std::fs::OpenOptions::new().write(true).open("/dev/usb/lp0")?.write_all(&bytes)?;
```

A USB thermal printer is a character device; raw bytes are the entire transport.
There is no driver to install. See [`examples/print.rs`](crates/ticket-core/examples/print.rs).

**Markers are intent; the profile decides.** A document's `cut` marker means "the
ticket ends here" — not "cut now". Whether it becomes bytes depends on the printer
in front of you, and this is the one place in the crate where being wrong has
physical consequences:

> A cut sent to a printer whose cutter is absent or disabled is **not** a no-op.
> It is silently ignored *and latches an error that stops the printer until it is
> power-cycled* — so every subsequent ticket is lost, and nobody notices until
> someone asks where the receipts went. The printer answers no status query, so
> this cannot be probed. It has to be configured.

Hence `CutMode::None` is the default and a document asking to cut is never enough
on its own. The reverse is deliberately harmless: an intent this printer cannot
honor (a cash drawer it doesn't have) is dropped, never an error — a ticket
authored for a fancier device must still print here.

Off by default, but it adds **no dependencies** — it's `png` (already required)
plus std. It's opt-in only because a build that just draws the preview has no
printer to talk to.

## Calculated variables (e.g. a maps QR, per-payment totals)

Derived values are authored in the editor, not the backend. In the **Calculated**
panel you give a value a name and a small **formula** over your existing
variables. It appears under the `calc.` namespace and behaves like any other
variable from then on: use it as a field, in a QR (**From a variable →
`calc.<name>`**), or in a condition. The editor's formula box has an **Insert
variable** and **Insert function** picker (so you never memorise names or
syntax), a self-documenting function list, and a live preview + error line
evaluated by the same engine that prints — so preview == print by construction.

The formula language is spreadsheet-like:

- dotted variable paths (`sale.total`, `calc.subtotal`), `"text"` and number
  literals, `+ - * / %` with parentheses, comparisons (`== != < <= > >=`) and
  `and` / `or`;
- functions: `concat`, `round`, `min`, `max`, `abs`, `coalesce`;
- **aggregates over a loop array**: `count`, `countif`, `sum`, `sumif`, `avg`,
  `avgif` — bare field names inside them refer to the current row.

```text
maps_link  = concat("https://maps.google.com/?q=", store.lat, ",", store.lng)
cash_total = sumif(sale.movements, payment == "CASH", amount)
sales_line = concat(count(sale.movements), " payments in the cut")
```

That last group is how a POS "cut" ticket shows per-payment totals after a loop —
declaratively, with no accumulators. The document stores each as
`{ name, formula }` under `computed` (see `TicketDoc`). Parsing and evaluation
live in `ticket-core` with strict bounds and no panics; a formula that fails to
parse renders blank (and the editor shows why), arithmetic falls back to blank on
a missing/non-numeric operand or divide-by-zero (never NaN), and results are
cleaned of floating-point noise.

## Develop

```bash
# Renderer (native): tests + a sample PNG
cargo test -p ticket-core
cargo test -p ticket-core --all-features        # incl. normalize + escpos
cargo run -p ticket-core --example sample -- /tmp/sample.png   # basic receipt
cargo run -p ticket-core --example flow   -- /tmp/flow.png     # loop + condition
cargo run -p ticket-core --example media  -- /tmp/media.png    # logo + QR

# Printing: render straight to ESC/POS bytes (no printer needed — writes a file)
cargo run -p ticket-core --features escpos --example print -- /tmp/ticket.bin
cargo run -p ticket-core --features escpos --example print -- /dev/usb/lp0

# Renderer (wasm): the browser bundle is a GENERATED artifact, not committed to
# git — build it once before running the editor, and again after changing ticket-core:
./scripts/build-wasm.sh    # needs `rustup target add wasm32-unknown-unknown`
                           # and `cargo install wasm-bindgen-cli --version <matches wasm-bindgen>`

# Editor (standalone demo with hot reload)
cd packages/ticket-editor
npm install
npm run build:wasm    # first-time bootstrap (needs the Rust toolchain above)
npm run dev           # http://localhost:5199

# Lint / typecheck
cargo clippy --all-targets            # (crate denies clippy::all)
cd packages/ticket-editor && npm run lint && npm run type-check
```

The wasm bundle is built from `ticket-core` and is **not checked in** — it's a
derived artifact. CI rebuilds it for the live demo, and `npm publish` bundles a
fresh build automatically (the package's `prepack` script runs `build:wasm`), so
a published release can never ship a stale renderer. When releasing, publish the
crate and the npm package **from the same `ticket-core` version** so your backend
(native) and the editor (wasm) render identically.

## License

MIT OR Apache-2.0.
