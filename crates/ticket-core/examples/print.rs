//! Render a ticket and send it to a thermal printer.
//!
//! This is the whole last mile: `render_escpos` turns the document into the byte
//! stream an ESC/POS printer accepts, and the "driver" is `write_all` — a USB
//! thermal printer shows up as a character device and raw bytes are the entire
//! protocol. There is no library to install.
//!
//! ```text
//! cargo run -p ticket-core --features escpos --example print -- ticket.bin
//! cargo run -p ticket-core --features escpos --example print -- /dev/usb/lp0
//! ```
//!
//! With no argument it writes `ticket.bin`, so you can try it without a printer
//! (`xxd ticket.bin | head` to see the `1B 40` reset and the `1D 76 30` raster).
//!
//! # The one thing to get right
//!
//! `PrinterProfile::cut` defaults to [`CutMode::None`], and that default is load
//! bearing. A cut sent to a printer whose cutter is absent or disabled does not
//! fail loudly — it latches an error that stops the printer until it is
//! power-cycled, so every later ticket vanishes silently. The printer answers no
//! status query, so this cannot be detected; you have to tell it. Turn cutting on
//! only for a device you know has a (DIP-enabled) cutter.

use std::io::Write;

use ticket_core::{render_escpos, CutMode, Fonts, PrinterProfile, RenderOptions, TicketDoc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A 32-char (58 mm) receipt. `cell_width_px` is what ties the grid to the
    // paper: width_chars x cell_width_px must equal the printer's dot width
    // (384 dots for 58 mm, 576 for 80 mm) or the print comes out the wrong size.
    let doc: TicketDoc = serde_json::from_value(serde_json::json!({
        "version": 2,
        "paper": { "width_chars": 32, "cell_width_px": 12, "cell_height_px": 22 },
        "computed": [
            { "name": "total", "formula": "sum(items, qty * price)" }
        ],
        "regions": [
            { "id": "lines", "start_row": 3, "end_row": 3, "source": "items",
              "computed": [ { "name": "amount", "formula": "round(qty * price, 2)" } ] }
        ],
        "elements": [
            { "id": "shop",  "row": 0, "col": 9,  "type": "text", "content": "THE CORNER CAFE",
              "style": { "bold": true } },
            { "id": "rule",  "row": 1, "col": 0,  "type": "text",
              "content": "--------------------------------" },
            { "id": "head",  "row": 2, "col": 0,  "type": "text", "content": "ITEM        QTY     AMOUNT" },

            // One row per item, repeated by the `lines` region.
            { "id": "name",  "row": 3, "col": 0,  "type": "variable", "path": "row.name",   "length": 12 },
            { "id": "qty",   "row": 3, "col": 12, "type": "variable", "path": "row.qty",    "length": 3, "align": "right" },
            { "id": "amt",   "row": 3, "col": 20, "type": "variable", "path": "row.amount", "length": 12, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": false } },

            { "id": "tlbl",  "row": 5, "col": 0,  "type": "text", "content": "TOTAL", "style": { "bold": true } },
            { "id": "tval",  "row": 5, "col": 20, "type": "variable", "path": "calc.total", "length": 12,
              "align": "right", "number": { "decimals": 2, "rounding": "half_up", "thousands": true }, "style": { "bold": true } },

            { "id": "qr",    "row": 7, "col": 11, "type": "qr", "value": "receipt.url", "from_variable": true, "size": 8 },

            // Intent, not a command: this only becomes bytes if the profile below
            // says the printer really can cut.
            { "id": "end",   "row": 16, "col": 0, "type": "marker", "name": "cut" }
        ]
    }))?;

    let data = serde_json::json!({
        "items": [
            { "name": "Espresso",  "qty": 2, "price": 2.50 },
            { "name": "Croissant", "qty": 1, "price": 3.20 },
        ],
        "receipt": { "url": "https://example.com/r/8f21c4" }
    });

    let profile = PrinterProfile {
        // Read this from your own config. `CutMode::parse` fails closed, so an
        // unrecognized value means "do not cut" rather than an error.
        cut: CutMode::parse(&std::env::var("PRINTER_CUT").unwrap_or_default()),
    };

    // `RenderOptions::default()` is print mode: a path that does not resolve
    // renders EMPTY. Never use the editor's placeholder mode here — it invents
    // believable stand-in values, which is right for a design canvas and
    // catastrophic on a customer's receipt.
    // `Fonts::with_bundled()` instead if the template uses one of the editor's
    // extra families (needs the `bundled-fonts` feature).
    let fonts = Fonts::builtin()?;
    let bytes = render_escpos(&doc, &data, &fonts, &RenderOptions::default(), &profile)?;

    // A USB thermal printer is a character device. Writing raw bytes to it is the
    // entire transport. Open per job, so a transient unplug errors this call
    // instead of leaving a dead handle behind.
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ticket.bin".into());
    let mut sink = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&target)?;
    sink.write_all(&bytes)?;
    sink.flush()?;

    println!("{} bytes -> {target}", bytes.len());
    if profile.cut == CutMode::None {
        println!("(cut disabled — set PRINTER_CUT=partial|full for a printer that has a cutter)");
    }
    Ok(())
}
