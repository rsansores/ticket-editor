//! A fuel-sale receipt exercising the v0.3 features end to end: a loop band
//! with per-row calculated columns (`row.importe`), implicit row values
//! (`row.number`, a marker on `row.last`), a wrapped customer name that
//! reflows content below it (bounded by `max_lines`), a conditional band, a
//! doc-level aggregate total — and print-mode rendering, where a path that
//! does not resolve prints empty (`sale.totl` at the bottom) and is reported
//! by `unresolved_paths`.
//!
//! Usage: cargo run -p ticket-core --example sale_receipt out.png
fn main() {
    let doc = serde_json::json!({
        "version": 2,
        "paper": { "width_chars": 44, "margin_left_chars": 1, "margin_right_chars": 1,
                   "margin_top_lines": 1, "margin_bottom_lines": 1 },
        "computed": [
            { "name": "total", "formula": "sum(movements, volume * price)" }
        ],
        "regions": [
            { "id": "movs", "start_row": 4, "end_row": 5, "source": "movements",
              "computed": [ { "name": "importe", "formula": "round(volume * price, 2)" } ] },
            { "id": "fact", "start_row": 6, "end_row": 7,
              "condition": { "var": "wants_invoice", "op": "eq", "value": "1" } }
        ],
        "elements": [
            { "id": "t",  "row": 0, "col": 10, "type": "text", "content": "GASPAR EDGE", "style": { "bold": true } },
            { "id": "cl", "row": 1, "col": 0, "type": "text", "content": "Cliente:" },
            // #2: wrap with reflow, bounded by max_lines
            { "id": "cn", "row": 1, "col": 9, "type": "variable", "path": "customer",
              "length": 33, "wrap": true, "max_lines": 3 },
            { "id": "h1", "row": 3, "col": 0, "type": "text", "content": "#  VOL     PRECIO       IMPORTE" },
            // #1: row.* columns inside the loop
            { "id": "n",  "row": 4, "col": 0, "type": "variable", "path": "row.number", "length": 2 },
            { "id": "v",  "row": 4, "col": 3, "type": "variable", "path": "volume", "length": 7, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": false } },
            { "id": "p",  "row": 4, "col": 11, "type": "variable", "path": "price", "length": 7, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": false } },
            { "id": "a",  "row": 4, "col": 22, "type": "variable", "path": "row.importe", "length": 11, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            // element condition on row.last: a rule under the final item only
            { "id": "rl", "row": 4, "col": 34, "type": "text", "content": "<",
              "condition": { "var": "row.last", "op": "eq", "value": "true" } },
            { "id": "fx", "row": 6, "col": 6, "type": "text", "content": ">> FACTURA SOLICITADA <<" },
            { "id": "tl", "row": 8, "col": 8, "type": "text", "content": "TOTAL:" },
            { "id": "tv", "row": 8, "col": 22, "type": "variable", "path": "calc.total", "length": 11, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            // #3: a typo'd path — must print EMPTY, not fake
            { "id": "oops", "row": 9, "col": 0, "type": "variable", "path": "sale.totl", "length": 12 }
        ]
    });
    let data = serde_json::json!({
        "customer": "Transportes y Mudanzas del Bajio SA de CV",
        "wants_invoice": 1,
        "movements": [
            { "volume": 20.5, "price": 24.99 },
            { "volume": 5.0,  "price": 21.50 },
            { "volume": 100.0, "price": 19.75 }
        ]
    });
    let doc: ticket_core::TicketDoc = serde_json::from_value(doc).unwrap();
    // Backend mode (placeholders off): what actually prints.
    let png = ticket_core::render_png(&doc, &data).unwrap();
    std::fs::write(std::env::args().nth(1).unwrap(), png).unwrap();
    // Also report unresolved paths — should flag exactly sale.totl.
    println!("unresolved: {:?}", doc.unresolved_paths(&data));
}
