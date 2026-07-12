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

mod barcode;
mod data;
mod expr;
mod font;
mod format;
mod image;
mod qr;
mod render;
mod schema;

pub use font::{FontFaces, Fonts, DEFAULT_FAMILY};
pub use render::{
    render, render_png, render_png_with_fonts, render_png_with_options, MarkerHit, RenderError,
    RenderOptions, RenderOutput,
};
pub use schema::{
    Align, Computed, CondOp, Condition, Element, ElementKind, ImageMode, NumberFormat, Paper,
    Region, Rounding, Style, Symbology, TicketDoc, VAlign, MARKER_NAMES, RESERVED_ROW_NAMES,
    SCHEMA_VERSION,
};

/// Convenience: render straight from JSON strings (the shape the wasm/HTTP
/// boundaries actually deal in). Returns PNG bytes. Uses only the built-in font
/// and the default [`RenderOptions`].
pub fn render_json(
    doc_json: &str,
    variables_json: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    render_json_with_fonts(doc_json, variables_json, Fonts::builtin_shared())
}

/// Like [`render_json`], but with a caller-provided [`Fonts`] set (built-in plus
/// any registered families). The error surfaces `MissingFont` when the document
/// uses a family the set doesn't have.
pub fn render_json_with_fonts(
    doc_json: &str,
    variables_json: &str,
    fonts: &Fonts,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    render_json_with_options(doc_json, variables_json, fonts, &RenderOptions::default())
}

/// Like [`render_json_with_fonts`], with explicit [`RenderOptions`] (the wasm
/// preview passes placeholder mode; a backend uses the default).
pub fn render_json_with_options(
    doc_json: &str,
    variables_json: &str,
    fonts: &Fonts,
    opts: &RenderOptions,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let doc: TicketDoc = serde_json::from_str(doc_json)?;
    let variables = parse_variables(variables_json)?;
    Ok(render_png_with_options(&doc, &variables, fonts, opts)?)
}

/// JSON-boundary convenience for [`TicketDoc::unresolved_paths`]: takes the
/// document and variables as JSON strings, returns a JSON array of the paths
/// that don't resolve. What the editor's "missing fields" badge and a backend's
/// save-time validation call through wasm/HTTP.
pub fn unresolved_paths_json(
    doc_json: &str,
    variables_json: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let doc: TicketDoc = serde_json::from_str(doc_json)?;
    let variables = parse_variables(variables_json)?;
    Ok(serde_json::to_string(&doc.unresolved_paths(&variables))?)
}

fn parse_variables(variables_json: &str) -> Result<serde_json::Value, serde_json::Error> {
    if variables_json.trim().is_empty() {
        Ok(serde_json::Value::Null)
    } else {
        serde_json::from_str(variables_json)
    }
}

/// Evaluate a list of calculated variables against sample data and report each
/// one's result — used by the editor for a live preview and error feedback as the
/// user types a formula. Returns a JSON array of
/// `{ name, value, kind, error }` where `kind` is `"number" | "text" | "empty"`
/// (drives the editor's default formatting) and `error` is null on success.
pub fn preview_computed_json(
    computed_json: &str,
    variables_json: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let computed: Vec<Computed> = serde_json::from_str(computed_json)?;
    let variables = parse_variables(variables_json)?;
    report_json(data::eval_computed_report(&variables, &computed))
}

/// The row-scoped counterpart of [`preview_computed_json`]: evaluate a band's
/// (draft) row formulas against the band's first item and report each result —
/// what the editor's "calculated column" dialog shows live. `doc_json` provides
/// the doc-level computed (`calc.*`) and the band's loop `source`;
/// `computed_json` is the draft `Region.computed` list being edited.
pub fn preview_row_computed_json(
    doc_json: &str,
    region_id: &str,
    computed_json: &str,
    variables_json: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let doc: TicketDoc = serde_json::from_str(doc_json)?;
    let computed: Vec<Computed> = serde_json::from_str(computed_json)?;
    let variables = parse_variables(variables_json)?;
    report_json(data::preview_row(&doc, region_id, &computed, &variables))
}

fn report_json(
    report: Vec<data::ComputedReport>,
) -> Result<String, Box<dyn std::error::Error>> {
    let arr: Vec<serde_json::Value> = report
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "value": data::value_to_string(&r.value),
                "kind": data::kind_of(&r.value),
                "error": r.error,
            })
        })
        .collect();
    Ok(serde_json::to_string(&serde_json::Value::Array(arr))?)
}
