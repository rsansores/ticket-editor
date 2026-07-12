//! Browser entry point for the ticket renderer.
//!
//! This is a thin `wasm-bindgen` shim over `ticket-core`. It exposes exactly one
//! job to JavaScript: turn a ticket document + variable data into PNG bytes,
//! using the identical code path the backend uses natively. The Vue editor calls
//! `render_png` on every (debounced) edit to paint the live preview.

use std::cell::RefCell;

use ticket_core::{FontFaces, Fonts};
use wasm_bindgen::prelude::*;

thread_local! {
    // The renderer's font set persists across calls: the built-in default plus
    // any families the editor has lazily fetched and registered. wasm is
    // single-threaded, so a thread-local is effectively a module global.
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

/// Run `f` with the (lazily built) persistent font set.
fn with_fonts<T>(f: impl FnOnce(&mut Fonts) -> Result<T, JsError>) -> Result<T, JsError> {
    FONTS.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(Fonts::builtin().map_err(|e| JsError::new(&e.to_string()))?);
        }
        f(slot.as_mut().expect("just initialized"))
    })
}

/// Render a ticket to PNG bytes.
///
/// * `doc_json` — the canonical `TicketDoc` as a JSON string.
/// * `variables_json` — the variable data as a JSON string; pass `""` or `"null"`
///   for no data.
/// * `placeholders` — when omitted or `true` (this shim serves the editor, so
///   preview mode is the default HERE; the native API defaults to `false`), a
///   variable path that doesn't resolve renders a deterministic fake value.
///   Pass `false` to render exactly what a backend print would produce
///   (unresolved paths render empty).
///
/// Returns the PNG as a `Uint8Array`. On bad input — including a document that
/// uses a font family not yet registered via [`register_font`] — it throws a JS
/// error with a human-readable message (the editor surfaces it).
#[wasm_bindgen]
pub fn render_png(
    doc_json: &str,
    variables_json: &str,
    placeholders: Option<bool>,
) -> Result<Vec<u8>, JsError> {
    let opts = if placeholders.unwrap_or(true) {
        ticket_core::RenderOptions::placeholders()
    } else {
        ticket_core::RenderOptions::default()
    };
    with_fonts(|fonts| {
        ticket_core::render_json_with_options(doc_json, variables_json, fonts, &opts)
            .map_err(|e| JsError::new(&e.to_string()))
    })
}

/// The variable paths a document references that do NOT resolve in the given
/// data — what powers the editor's "N fields missing from your sample data"
/// warning and a backend's save-time template validation. Returns a JSON array
/// of path strings. Throws only on malformed JSON input.
#[wasm_bindgen]
pub fn unresolved_paths(doc_json: &str, variables_json: &str) -> Result<String, JsError> {
    ticket_core::unresolved_paths_json(doc_json, variables_json)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Register (or replace) a monospace font family so documents can reference it by
/// `id`. The four faces are TTF/OTF byte arrays; a family that ships fewer
/// weights should pass the regular bytes for the missing faces. The built-in
/// `"mono"` family cannot be replaced. The editor calls this once per family,
/// after lazily fetching its files, before rendering a document that uses it.
#[wasm_bindgen]
pub fn register_font(
    id: &str,
    regular: Vec<u8>,
    bold: Vec<u8>,
    italic: Vec<u8>,
    bold_italic: Vec<u8>,
) -> Result<(), JsError> {
    let faces = FontFaces::from_bytes(regular, bold, italic, bold_italic)
        .map_err(|e| JsError::new(&e.to_string()))?;
    with_fonts(|fonts| {
        fonts.add_family(id, faces);
        Ok(())
    })
}

/// Whether a font family is currently registered (the built-in `"mono"` always
/// is). Lets the editor skip re-fetching a font it already loaded.
#[wasm_bindgen]
pub fn has_font(id: &str) -> bool {
    with_fonts(|fonts| Ok(fonts.contains(id))).unwrap_or(false)
}

/// The schema version this wasm build understands, so the JS side can guard
/// against loading documents from a newer editor.
#[wasm_bindgen]
pub fn schema_version() -> u32 {
    ticket_core::SCHEMA_VERSION
}

/// Evaluate calculated variables against sample data, for the editor's live
/// formula preview + error feedback.
///
/// * `computed_json` — a JSON array of `{ name, formula }`.
/// * `variables_json` — the sample data (`""`/`"null"` for none).
///
/// Returns a JSON array of `{ name, value, kind, error }`. Throws only on
/// malformed JSON input; a bad *formula* is reported per-item via its `error`.
#[wasm_bindgen]
pub fn preview_computed(computed_json: &str, variables_json: &str) -> Result<String, JsError> {
    ticket_core::preview_computed_json(computed_json, variables_json)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Evaluate a band's (draft) row-scoped formulas against the band's first data
/// item, for the editor's "calculated column" live preview + error feedback.
///
/// * `doc_json` — the current `TicketDoc` (provides `calc.*` and the band's source).
/// * `region_id` — which band the formulas belong to.
/// * `computed_json` — a JSON array of `{ name, formula }` (the draft list).
/// * `variables_json` — the sample data (`""`/`"null"` for none).
///
/// Returns a JSON array of `{ name, value, kind, error }`, same shape as
/// [`preview_computed`]. Throws only on malformed JSON input.
#[wasm_bindgen]
pub fn preview_row_computed(
    doc_json: &str,
    region_id: &str,
    computed_json: &str,
    variables_json: &str,
) -> Result<String, JsError> {
    ticket_core::preview_row_computed_json(doc_json, region_id, computed_json, variables_json)
        .map_err(|e| JsError::new(&e.to_string()))
}
