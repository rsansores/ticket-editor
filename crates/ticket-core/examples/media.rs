//! Render a ticket with a logo image + a QR code, to eyeball the 1-bit output.
//! cargo run -p ticket-core --example media -- /path/out.png
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use ticket_core::{render_png, TicketDoc};

/// Build a small RGBA PNG (a diagonal-stripe "logo") and base64-encode it.
fn make_logo_b64() -> String {
    let (w, h) = (96u32, 48u32);
    let mut rgba = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            // diagonal stripes + a border → clear black/white content
            let stripe = ((x + y) / 6) % 2 == 0;
            let border = x < 2 || y < 2 || x >= w - 2 || y >= h - 2;
            let black = border || (stripe && x > w / 3);
            let v = if black { 0 } else { 255 };
            rgba[i] = v;
            rgba[i + 1] = v;
            rgba[i + 2] = v;
        }
    }
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    }
    format!("data:image/png;base64,{}", STANDARD.encode(&png))
}

fn main() {
    let logo = make_logo_b64();
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1,
        "paper": { "width_chars": 40, "margin_left_chars": 1, "margin_right_chars": 1,
                   "margin_top_lines": 1, "margin_bottom_lines": 1 },
        "elements": [
            { "id": "logo", "row": 0, "col": 12, "type": "image", "data": logo, "w": 16, "h": 4,
              "mode": { "kind": "threshold", "level": 128 } },
            { "id": "t", "row": 5, "col": 10, "type": "text", "content": "THANK YOU FOR YOUR VISIT" },
            { "id": "qr", "row": 7, "col": 13, "type": "qr", "value": "sale.receipt_url", "from_variable": true, "size": 12 },
            { "id": "c", "row": 20, "col": 8, "type": "text", "content": "Scan for your receipt", "style": { "italic": true } }
        ]
    }))
    .unwrap();

    let data = json!({ "sale": { "receipt_url": "https://example.com/r/A-100294" } });
    let out = std::env::args().nth(1).unwrap_or_else(|| "media.png".into());
    std::fs::write(&out, render_png(&doc, &data).unwrap()).unwrap();
    // Dump the exact doc+data so the native/wasm parity check feeds identical input.
    std::fs::write(
        format!("{out}.json"),
        serde_json::to_string(&json!({ "doc": doc, "data": data })).unwrap(),
    )
    .unwrap();
    eprintln!("wrote {out}");
}
