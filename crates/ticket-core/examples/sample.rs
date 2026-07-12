//! Render a sample receipt to a PNG file so the output can be eyeballed.
//! `cargo run -p ticket-core --example sample -- /path/out.png`
use serde_json::json;
use ticket_core::{render_png_with_options, Fonts, RenderOptions, TicketDoc};

fn main() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1,
        "paper": { "width_chars": 40, "margin_left_chars": 1, "margin_right_chars": 1,
                   "margin_top_lines": 1, "margin_bottom_lines": 1 },
        "elements": [
            { "id": "t",  "row": 0, "col": 13, "type": "text", "content": "PET PALACE", "style": { "bold": true } },
            { "id": "s",  "row": 1, "col": 9,  "type": "text", "content": "-- Sales receipt --" },
            { "id": "l1", "row": 3, "col": 0,  "type": "text", "content": "Product:" },
            { "id": "v1", "row": 3, "col": 20, "type": "variable", "path": "sale.product",  "length": 18, "align": "right" },
            { "id": "l2", "row": 4, "col": 0,  "type": "text", "content": "Quantity:" },
            { "id": "v2", "row": 4, "col": 20, "type": "variable", "path": "sale.qty",      "length": 18, "align": "right" },
            { "id": "l3", "row": 5, "col": 0,  "type": "text", "content": "Cashier:" },
            { "id": "v3", "row": 5, "col": 20, "type": "variable", "path": "sale.cashier",  "length": 18, "align": "right" },
            { "id": "l4", "row": 7, "col": 0,  "type": "text", "content": "TOTAL:", "style": { "bold": true, "scale": 2 } },
            { "id": "v4", "row": 7, "col": 20, "type": "variable", "path": "sale.total_amount", "length": 20, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true }, "style": { "bold": true, "scale": 2 } },
            { "id": "nt", "row": 12, "col": 0, "type": "variable", "path": "sale.legend", "length": 110, "wrap": true, "style": { "italic": true } }
        ]
    }))
    .unwrap();

    // Half the fields provided as real data, the rest fall back to faker —
    // which is now opt-in (placeholder mode, what the editor preview uses).
    // The plain render_png default is print mode: missing fields render empty.
    let data = json!({ "sale": {
        "product": "Dog Food",
        "total_amount": 1294.505,
        "legend": "This is not a fiscal receipt. Please keep your ticket for any inquiry about your purchase.",
    } });
    let fonts = Fonts::builtin().unwrap();
    let png =
        render_png_with_options(&doc, &data, &fonts, &RenderOptions::placeholders()).unwrap();

    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sample.png".into());
    std::fs::write(&out, &png).unwrap();
    eprintln!("wrote {} ({} bytes)", out, png.len());
}
