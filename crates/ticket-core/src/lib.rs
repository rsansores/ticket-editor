//! `ticket-core` — the deterministic monospace-grid ticket renderer.
//!
//! This crate is the single source of truth for what a ticket looks like. It is
//! compiled twice:
//!   * **native**, linked into the backend, to produce the real PNG that gets
//!     printed;
//!   * **wasm** (via the sibling `ticket-wasm` crate), linked into the browser,
//!     to draw the live editor preview.
//!
//! Because both builds run *this exact code* over *these exact embedded fonts*,
//! the preview the user sees is byte-for-byte what the printer receives. That is
//! the "1:1 identical" requirement, met by construction rather than by trying to
//! keep two renderers in sync.
//!
//! ```
//! use ticket_core::{render_png, TicketDoc};
//! let doc: TicketDoc = serde_json::from_str(r#"{
//!   "version": 1,
//!   "paper": { "width_chars": 32 },
//!   "elements": [
//!     { "id": "a", "row": 0, "col": 0, "type": "text", "content": "HELLO" }
//!   ]
//! }"#).unwrap();
//! let png = render_png(&doc, &serde_json::Value::Null).unwrap();
//! assert_eq!(&png[1..4], b"PNG");
//! ```

// Production hardening gates. No `unsafe` anywhere in this crate; renderer code
// must not `unwrap`/`panic` (adversarial documents flow in). `missing_docs`
// keeps the public API documented.
#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(missing_docs)]

mod data;
mod font;
mod format;
mod image;
mod qr;
mod render;
mod schema;

pub use render::{render_png, RenderError};
pub use schema::{
    Align, CondOp, Condition, Element, ElementKind, ImageMode, NumberFormat, Paper, Region,
    Rounding, Style, TicketDoc, VAlign, SCHEMA_VERSION,
};

/// Convenience: render straight from JSON strings (the shape the wasm/HTTP
/// boundaries actually deal in). Returns PNG bytes.
pub fn render_json(
    doc_json: &str,
    variables_json: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let doc: TicketDoc = serde_json::from_str(doc_json)?;
    let variables: serde_json::Value = if variables_json.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(variables_json)?
    };
    Ok(render_png(&doc, &variables)?)
}
