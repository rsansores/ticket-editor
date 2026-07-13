//! Bake a document's embedded images down to what the renderer would have made
//! of them anyway.
//!
//! [`render`](crate::render) treats an image element as a source to be decoded,
//! bilinear-scaled to its target box of `w × cell_width_px` by
//! `h × cell_height_px` pixels, and reduced to 1-bit ink. None of that work
//! depends on the data — it is the same every print. [`normalize_images`] does
//! it once, up front, and writes the result back into the document as a 1-bit
//! PNG.
//!
//! The renderer's own pass then becomes a no-op: the scale is identity (the
//! source is already at its target size) and the threshold / dither is
//! idempotent (the input is already 0 or 255). So a normalized document renders
//! *pixel-for-pixel* the same as the one it came from — while typically being an
//! order of magnitude smaller, because an 8-bit RGBA logo carries ~24 bits per
//! pixel that a monochrome printer was always going to throw away.
//!
//! There is exactly one case where the output legitimately changes, and it is an
//! improvement: a source the renderer cannot decode at all. `render` only reads
//! PNG, and draws a placeholder frame for anything else — so a JPEG or WebP logo
//! prints as an empty box. Normalizing decodes it properly and hands the renderer
//! the PNG it can read, so the ticket gains the logo it was always supposed to
//! have.
//!
//! That size is the point. A document that lives in a database and renders on
//! the same machine can afford to carry its originals; one that has to reach a
//! device over a constrained link — a metered connection, a serial bus, a
//! phone-relayed BLE tunnel — pays for every one of those bytes, on every push.
//!
//! Two things are deliberately left alone:
//!
//! * **`from_variable` images.** Their bytes aren't known until render time (a
//!   per-sale signature the backend supplies), so there is nothing to bake.
//! * **`mode`.** It stays on the element, even though re-applying it to an
//!   already-binary image is a no-op. Normalization is a one-way transform: keep
//!   the original document if you want to re-tune the threshold later, and
//!   normalize on the way out.
//!
//! Unlike the renderer, which draws a placeholder frame for an image it cannot
//! decode, this returns an [`Error`] naming the element — a save-time check
//! wants to reject a bad logo, not print a hollow box around it.
//!
//! Enabled by the `normalize` feature (off by default: it pulls in `image` for
//! JPEG/WebP decoding, which a renderer-only build has no use for).
//!
//! ```
//! # use ticket_core::{normalize_images, TicketDoc};
//! # let json = r#"{ "version": 1, "paper": { "width_chars": 32 }, "elements": [] }"#;
//! let mut doc: TicketDoc = serde_json::from_str(json).unwrap();
//! let stats = normalize_images(&mut doc).unwrap();
//! println!("{} images, {} -> {} bytes", stats.images, stats.bytes_before, stats.bytes_after);
//! ```

use base64::{engine::general_purpose::STANDARD, Engine};

use crate::image::{
    decode_b64, decode_png_gray_bytes, encode_1bit_png, luma, over_white, resize_gray, to_bw,
    MAX_DECODE_BYTES, MAX_SRC_PIXELS,
};
use crate::render::MAX_PIXELS;
use crate::schema::{ElementKind, TicketDoc};

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const DATA_URI_PREFIX: &str = "data:image/png;base64,";

/// What [`normalize_images`] did, for a caller that wants to log it or enforce a
/// size budget on the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// How many images were rewritten (`from_variable` ones are not counted).
    pub images: usize,
    /// Total length of those images' `data` strings before.
    pub bytes_before: usize,
    /// Total length of those images' `data` strings after.
    pub bytes_after: usize,
}

/// An image in the document could not be normalized. Names the element so a
/// save-time validation can point the user at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// [`Element::id`](crate::Element::id) of the offending image.
    pub element_id: String,
    /// Why it failed — a bad base64 payload, an unsupported format, an
    /// unreasonable size.
    pub reason: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "image '{}': {}", self.element_id, self.reason)
    }
}

impl std::error::Error for Error {}

/// Rewrite every static image in `doc` as a 1-bit PNG at its exact target pixel
/// box. The document renders identically afterwards; see the [module
/// docs](self) for why, and for what is left alone.
pub fn normalize_images(doc: &mut TicketDoc) -> Result<Stats, Error> {
    let cell_w = u64::from(doc.paper.cell_width_px.max(1));
    let cell_h = u64::from(doc.paper.cell_height_px.max(1));
    let mut stats = Stats::default();

    for element in &mut doc.elements {
        let id = &element.id;
        let ElementKind::Image {
            data,
            from_variable: false,
            w,
            h,
            mode,
        } = &mut element.kind
        else {
            continue;
        };

        let fail = |reason: String| Error {
            element_id: id.clone(),
            reason,
        };

        // Bound the target box before decoding or allocating, on the same ceiling
        // the renderer enforces — an adversarial `w`/`h` must not get further here
        // than it would there.
        let w_px = u64::from((*w).max(1)) * cell_w;
        let h_px = u64::from((*h).max(1)) * cell_h;
        if w_px > MAX_PIXELS || h_px > MAX_PIXELS || w_px * h_px > MAX_PIXELS {
            return Err(fail(format!("target {w_px}x{h_px} px exceeds the ceiling")));
        }
        let (w_px, h_px) = (w_px as u32, h_px as u32);

        let (gray, src_w, src_h) = decode_gray(data).map_err(fail)?;
        let scaled = resize_gray(&gray, src_w, src_h, w_px, h_px);
        let mask = to_bw(&scaled, w_px, h_px, *mode);
        let png = encode_1bit_png(&mask, w_px, h_px).map_err(fail)?;

        stats.images += 1;
        stats.bytes_before += data.len();
        *data = format!("{DATA_URI_PREFIX}{}", STANDARD.encode(&png));
        stats.bytes_after += data.len();
    }

    Ok(stats)
}

/// Decode any supported source to 8-bit gray. PNG goes through the renderer's
/// own decoder, so a PNG normalizes to exactly the pixels the renderer would
/// have derived from it; everything else is routed through `image` and
/// converted with the same luma and the same composite-over-white.
fn decode_gray(data: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let bytes = decode_b64(data)?;
    if bytes.starts_with(PNG_MAGIC) {
        return decode_png_gray_bytes(&bytes);
    }

    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| format!("image: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES as u64);
    reader.limits(limits);

    let decoded = reader.decode().map_err(|e| format!("image: {e}"))?;
    let (w, h) = (decoded.width(), decoded.height());
    if u64::from(w) * u64::from(h) > MAX_SRC_PIXELS {
        return Err(format!("image too large: {w}x{h}"));
    }

    let gray = decoded
        .to_rgba8()
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            over_white(luma(r, g, b), a as f32 / 255.0)
        })
        .collect();
    Ok((gray, w, h))
}
