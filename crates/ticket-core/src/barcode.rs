//! 1D barcode generation (pure Rust → wasm-safe) via `barcoders`.
//!
//! We use only the crate's encoder — `.encode()` returns a run of module bits —
//! and rasterize the bars into the ticket's 1-bit grid ourselves (mirroring the
//! QR path), so no image dependency is pulled in and native/wasm stay in parity.

use crate::schema::Symbology;

/// Encode `value` into a run of bars: one bool per horizontal module
/// (true = black bar). Returns `None` when the value is invalid for the chosen
/// symbology (e.g. non-digits for EAN-13), so the renderer can fall back to a
/// visible placeholder instead of failing.
pub fn bars(value: &str, sym: Symbology) -> Option<Vec<bool>> {
    let encoded: Vec<u8> = match sym {
        // Code 128 requires a code-set prefix; `\u{0181}` (Ɓ) selects set B,
        // which covers the full printable ASCII range.
        Symbology::Code128 => barcoders::sym::code128::Code128::new(format!("\u{0181}{value}"))
            .ok()?
            .encode(),
        Symbology::Code39 => barcoders::sym::code39::Code39::new(value).ok()?.encode(),
        Symbology::Ean13 => barcoders::sym::ean13::EAN13::new(value).ok()?.encode(),
    };
    if encoded.is_empty() {
        return None;
    }
    Some(encoded.into_iter().map(|b| b != 0).collect())
}
