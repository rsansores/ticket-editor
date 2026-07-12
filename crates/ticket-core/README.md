# ticket-core

Deterministic monospace-grid **receipt / ticket renderer** for thermal printers.

`ticket-core` turns a JSON `TicketDoc` plus your data into a 1-bit-friendly PNG.
It is the single source of truth behind the [Ticket Editor](https://github.com/rsansores/ticket-editor):
the same code is compiled native (this crate, for your backend) and to WebAssembly
(for the browser editor's live preview), so **what the user edits is byte-for-byte
what prints**.

- **Deterministic** — same document + same data → same PNG bytes.
- **No coupling** — takes a `TicketDoc` and a `serde_json::Value`, returns PNG bytes.
  No database, framework, or printer stack.
- **Hardened** — bounded image/QR/canvas sizes, clamped decimals, no panics on
  hostile input.

```rust
use ticket_core::{render_png, TicketDoc};

let doc: TicketDoc = serde_json::from_str(stored_json)?;
let png: Vec<u8> = render_png(&doc, &data)?;   // same renderer as the browser preview
// send `png` to your printer (CUPS, ESC/POS raster, …)
```

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

Everything is measured in **character cells**, never raw pixels: a variable
reserves a fixed number of columns and its value is truncated or padded to exactly
that width, so a real value can never overflow its slot and disturb the layout.

> **Print sizing:** set `cell_width_px` so `width_chars × cell_width_px` equals your
> printer's dot width (e.g. 384 for 58 mm, 576 for 80 mm) — then the preview is 1:1
> with the paper.

See the [full documentation and the Vue editor](https://github.com/rsansores/ticket-editor)
for the document schema, loops/conditionals, and the embeddable UI.

## License

Licensed under either of [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) at your option.
