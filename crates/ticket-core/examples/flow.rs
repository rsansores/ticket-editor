//! Render a receipt exercising a loop band + a conditional band, to eyeball flow.
//! cargo run -p ticket-core --example flow -- /path/out.png
use serde_json::json;
use ticket_core::{render_png, TicketDoc};

fn main() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1,
        "paper": { "width_chars": 40, "margin_left_chars": 1, "margin_right_chars": 1,
                   "margin_top_lines": 1, "margin_bottom_lines": 1 },
        "regions": [
            // rows 3..4 repeat once per cart item
            { "id": "loop", "start_row": 3, "end_row": 4, "source": "sale.items" },
            // rows 6..7 only show when a discount exists
            { "id": "disc", "start_row": 6, "end_row": 7,
              "condition": { "var": "sale.discount", "op": "gt", "value": "0" } }
        ],
        "elements": [
            { "id": "t",  "row": 0, "col": 15, "type": "text", "content": "PET PALACE", "style": { "bold": true } },
            { "id": "h1", "row": 2, "col": 0,  "type": "text", "content": "Item" },
            { "id": "h2", "row": 2, "col": 22, "type": "text", "content": "Qty" },
            { "id": "h3", "row": 2, "col": 30, "type": "text", "content": "Amount" },
            // --- loop band (row 3): relative paths bind to each cart item ---
            { "id": "p",  "row": 3, "col": 0,  "type": "variable", "path": "product", "length": 18 },
            { "id": "q",  "row": 3, "col": 22, "type": "variable", "path": "qty", "length": 6, "align": "right" },
            { "id": "am", "row": 3, "col": 30, "type": "variable", "path": "amount", "length": 9, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            // --- after the loop: flows below all rows ---
            { "id": "sl", "row": 5, "col": 0,  "type": "text", "content": "SUBTOTAL:" },
            { "id": "sv", "row": 5, "col": 28, "type": "variable", "path": "sale.subtotal", "length": 11, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            // --- conditional band (row 6): only if discount > 0 ---
            { "id": "dl", "row": 6, "col": 0,  "type": "text", "content": "DISCOUNT:" },
            { "id": "dv", "row": 6, "col": 28, "type": "variable", "path": "sale.discount", "length": 11, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            { "id": "tl", "row": 7, "col": 0,  "type": "text", "content": "TOTAL:", "style": { "bold": true, "scale": 2 } },
            { "id": "tv", "row": 7, "col": 20, "type": "variable", "path": "sale.total", "length": 19, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true }, "style": { "bold": true, "scale": 2 } }
        ]
    }))
    .unwrap();

    let data = json!({ "sale": {
        "subtotal": 46.24, "discount": 4.62, "total": 41.62,
        "items": [
            { "product": "Dog Food",  "qty": 2, "amount": 24.50 },
            { "product": "Cat Toy",   "qty": 1, "amount": 8.99 },
            { "product": "Fish Food",  "qty": 3, "amount": 12.75 }
        ]
    }});
    let out = std::env::args().nth(1).unwrap_or_else(|| "flow.png".into());
    std::fs::write(&out, render_png(&doc, &data).unwrap()).unwrap();
    eprintln!("wrote {out}");
}
