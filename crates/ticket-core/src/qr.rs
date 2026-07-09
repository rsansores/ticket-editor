//! QR code generation (dependency-light, pure Rust → wasm-safe).

use qrcodegen::{QrCode, QrCodeEcc};

/// Build a QR matrix for `text`. Returns `(module_count, modules)` where
/// `modules[y*n + x]` is true for a black module. `None` if the text is too long
/// to encode.
pub fn matrix(text: &str) -> Option<(usize, Vec<bool>)> {
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let n = qr.size() as usize;
    let mut m = vec![false; n * n];
    for y in 0..n {
        for x in 0..n {
            m[y * n + x] = qr.get_module(x as i32, y as i32);
        }
    }
    Some((n, m))
}
