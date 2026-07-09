//! Browser entry point for the ticket renderer.
//!
//! This is a thin `wasm-bindgen` shim over `ticket-core`. It exposes exactly one
//! job to JavaScript: turn a ticket document + variable data into PNG bytes,
//! using the identical code path the backend uses natively. The Vue editor calls
//! `render_png` on every (debounced) edit to paint the live preview.

use wasm_bindgen::prelude::*;

/// Render a ticket to PNG bytes.
///
/// * `doc_json` — the canonical `TicketDoc` as a JSON string.
/// * `variables_json` — the variable data as a JSON string; pass `""` or `"null"`
///   to get a preview filled with deterministic fake values.
///
/// Returns the PNG as a `Uint8Array`. On bad input it throws a JS error with a
/// human-readable message (the editor surfaces it instead of silently blanking).
#[wasm_bindgen]
pub fn render_png(doc_json: &str, variables_json: &str) -> Result<Vec<u8>, JsError> {
    ticket_core::render_json(doc_json, variables_json)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// The schema version this wasm build understands, so the JS side can guard
/// against loading documents from a newer editor.
#[wasm_bindgen]
pub fn schema_version() -> u32 {
    ticket_core::SCHEMA_VERSION
}
