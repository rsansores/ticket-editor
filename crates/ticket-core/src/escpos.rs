//! Turn a rendered ticket into bytes a thermal printer will accept.
//!
//! [`render`](crate::render) produces a PNG — the exact image the editor preview
//! shows. A thermal printer cannot take a PNG. It wants a 1-bit raster fed with
//! the `GS v 0` bit-image command, plus a few control codes around it. This
//! module is that last mile: [`render_escpos`] goes document → printer bytes in
//! one call, and [`png_to_escpos`] does the conversion alone if you already have
//! a raster.
//!
//! # Markers are intent, the profile decides
//!
//! A document's `cut` marker says *"the ticket ends here"*. Whether that becomes
//! bytes is not the document's call — it depends on the printer standing in front
//! of you, and this is the one place in the crate where getting it wrong has
//! physical consequences.
//!
//! A cut sent to a printer whose cutter is absent or disabled is not a no-op. It
//! is silently ignored **and latches an error that stops the printer until it is
//! power-cycled** — so every subsequent ticket is lost, and nobody finds out
//! until someone asks where the receipts went. The printer answers no status
//! query, so this cannot be probed. It has to be told.
//!
//! Hence [`CutMode::None`] is the default, and a document asking to cut is never
//! enough on its own — you opt a real cutter in via [`PrinterProfile`]. The
//! reverse is deliberately harmless: an intent this printer cannot honor (a cash
//! drawer it doesn't have) is dropped, never an error, because a ticket authored
//! for a fancier device must still print here.
//!
//! Enabled by the `escpos` feature. It adds no dependencies.

use std::io::Cursor;

use crate::render::{render, RenderOptions};
use crate::schema::TicketDoc;
use crate::Fonts;

/// Luminance below this (0–255) prints as a black dot. The renderer already
/// produces near-black glyphs on white, so the exact cutoff is not sensitive.
const BLACK_THRESHOLD: u8 = 128;

/// Rows per `GS v 0` command. Banding keeps each command small enough for
/// printers that buffer a whole bit-image before printing it.
const BAND_ROWS: u32 = 128;

/// Blank lines fed after the ticket so the last line clears the print head and
/// the paper can be torn off at the bar.
const FEED_LINES: u8 = 5;

/// Dots fed before the blade fires, so the cut lands past the last printed line.
const CUT_FEED_DOTS: u8 = 100;

/// How this printer cuts, if it cuts at all.
///
/// Defaults to [`CutMode::None`] on purpose — see the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutMode {
    /// Tear-bar printer, or a cutter that is absent/disabled. Never emit a cut.
    #[default]
    None,
    /// `GS V 66 n` — feed, then partial cut (leaves a small tab).
    Partial,
    /// `GS V 65 n` — feed, then full cut.
    Full,
}

impl CutMode {
    /// Parse a configured value (`"partial"`, `"full"`; anything else is
    /// [`CutMode::None`]). Case- and whitespace-insensitive, so it can be fed
    /// straight from an environment variable or a config file.
    ///
    /// Unrecognized input deliberately means "do not cut" rather than an error:
    /// a typo in a config must not make a printer cut when nobody meant it to.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "partial" => CutMode::Partial,
            "full" => CutMode::Full,
            _ => CutMode::None,
        }
    }

    /// The ESC/POS bytes for this cut, or none when the printer cannot cut.
    ///
    /// `GS V 65|66 n` (feed-then-cut) rather than the bare `GS V 0|1`: the blade
    /// sits above the print head, so cutting without feeding first slices through
    /// the last printed lines.
    fn bytes(self) -> Option<[u8; 4]> {
        match self {
            CutMode::None => None,
            CutMode::Partial => Some([0x1D, 0x56, 0x42, CUT_FEED_DOTS]),
            CutMode::Full => Some([0x1D, 0x56, 0x41, CUT_FEED_DOTS]),
        }
    }
}

/// What the *device* can do, as opposed to what the *document* asks for.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrinterProfile {
    /// How this printer cuts. Default: not at all.
    pub cut: CutMode,
}

impl PrinterProfile {
    /// Map a document marker to device bytes. `None` when this printer cannot do
    /// it — an unknown or unsupported intent is ignored, never an error.
    pub fn command_for(&self, marker: &str) -> Option<Vec<u8>> {
        match marker {
            "cut" => self.cut.bytes().map(|b| b.to_vec()),
            // ESC d 1 — one blank line, wherever the document asked for space.
            "feed" => Some(vec![0x1B, 0x64, 0x01]),
            // ESC p 0 25 250 — kick the cash drawer on pin 2.
            "drawer" => Some(vec![0x1B, 0x70, 0x00, 0x19, 0xFA]),
            // ESC B 2 3 — two beeps.
            "beep" => Some(vec![0x1B, 0x42, 0x02, 0x03]),
            _ => None,
        }
    }
}

/// Something went wrong turning a ticket into printer bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscPosError {
    /// Rendering the document failed (only from [`render_escpos`]).
    Render(String),
    /// The PNG could not be decoded.
    Decode(String),
    /// The image is wider than a single `GS v 0` row can address.
    TooWide(u32),
}

impl std::fmt::Display for EscPosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscPosError::Render(e) => write!(f, "render failed: {e}"),
            EscPosError::Decode(e) => write!(f, "png decode failed: {e}"),
            EscPosError::TooWide(w) => write!(f, "image too wide: {w}px"),
        }
    }
}

impl std::error::Error for EscPosError {}

/// A marker the document placed, resolved to the raster row it sits on.
pub struct MarkerAt {
    /// Intent name — `cut`, `drawer`, … The profile decides what it becomes.
    pub name: String,
    /// Pixel row in the raster. Commands are injected once the rows above it
    /// have been sent, so a mid-ticket `cut` separates the copies exactly there.
    pub y_px: u32,
}

/// Render a document and encode it for the printer, in one call. The usual entry
/// point.
///
/// `opts` matters more than it looks: [`RenderOptions::default`] is *print* mode,
/// where a path that does not resolve renders **empty**. Never pass the editor's
/// placeholder mode here — it invents believable stand-in values, which is right
/// for an empty design canvas and catastrophic on a customer's receipt.
pub fn render_escpos(
    doc: &TicketDoc,
    data: &serde_json::Value,
    fonts: &Fonts,
    opts: &RenderOptions,
    profile: &PrinterProfile,
) -> Result<Vec<u8>, EscPosError> {
    let out = render(doc, data, fonts, opts).map_err(|e| EscPosError::Render(e.to_string()))?;

    // Markers come back as grid rows; the raster is pixels.
    let cell_h = doc.paper.cell_height_px.max(1);
    let markers: Vec<MarkerAt> = out
        .markers
        .into_iter()
        .map(|m| MarkerAt {
            name: m.name,
            y_px: m.row.saturating_mul(cell_h),
        })
        .collect();

    png_to_escpos(&out.png, &markers, profile)
}

/// Convert an already-rendered ticket into a complete ESC/POS byte stream:
/// printer reset, the raster (banded), device commands wherever the document
/// placed a marker, and a closing feed so the ticket clears the head.
///
/// Prefer [`render_escpos`] unless you are holding a PNG from somewhere else.
pub fn png_to_escpos(
    png: &[u8],
    markers: &[MarkerAt],
    profile: &PrinterProfile,
) -> Result<Vec<u8>, EscPosError> {
    let bmp = decode_png(png)?;
    let bytes_per_row = bmp.width.div_ceil(8);
    if bytes_per_row > u32::from(u16::MAX) {
        return Err(EscPosError::TooWide(bmp.width));
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[0x1B, 0x40]); // ESC @ — initialize/reset

    // Markers already arrive top-to-bottom, and same-row ones in declaration
    // order — so a `drawer` authored before a `cut` kicks before the blade.
    let mut pending = markers.iter().peekable();

    let mut row = 0;
    while row < bmp.height {
        // Never print past the next marker: its command must land on the paper
        // where the document put it, not at the end of the nearest 128-row band.
        let next_marker = pending.peek().map(|m| m.y_px.min(bmp.height));
        let stop = next_marker.filter(|&y| y > row).unwrap_or(bmp.height);
        let band = BAND_ROWS.min(stop.saturating_sub(row)).max(1);

        // GS v 0: 0x1D 0x76 0x30 m xL xH yL yH  <data>
        out.extend_from_slice(&[0x1D, 0x76, 0x30, 0x00]);
        out.extend_from_slice(&(bytes_per_row as u16).to_le_bytes());
        out.extend_from_slice(&(band as u16).to_le_bytes());
        for r in row..row + band {
            for byte_col in 0..bytes_per_row {
                let mut b = 0u8;
                for bit in 0..8 {
                    let x = byte_col * 8 + bit;
                    if x < bmp.width && bmp.dots[(r * bmp.width + x) as usize] {
                        b |= 0x80 >> bit; // MSB = leftmost dot
                    }
                }
                out.push(b);
            }
        }
        row += band;

        while let Some(marker) = pending.next_if(|m| m.y_px <= row) {
            emit_marker(&mut out, marker, profile);
        }
    }

    // Markers below the last raster row (a `cut` at the very end is the common
    // case — the ticket ends there).
    for marker in pending {
        emit_marker(&mut out, marker, profile);
    }

    // Feed the ticket clear of the head so it can be torn (or cut) without
    // taking the last printed line with it.
    out.extend_from_slice(&[0x1B, 0x64, FEED_LINES]); // ESC d n

    Ok(out)
}

/// Append one marker's device bytes, if this printer can honor it. An intent it
/// cannot do is dropped — see the [module docs](self).
fn emit_marker(out: &mut Vec<u8>, marker: &MarkerAt, profile: &PrinterProfile) {
    if let Some(cmd) = profile.command_for(&marker.name) {
        out.extend_from_slice(&cmd);
    }
}

/// A decoded 1-bit image: `width`×`height` dots, `true` = black.
struct Bitmap {
    width: u32,
    height: u32,
    /// Row-major, `width*height` long.
    dots: Vec<bool>,
}

/// Decode PNG bytes to a thresholded 1-bit bitmap.
fn decode_png(png: &[u8]) -> Result<Bitmap, EscPosError> {
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder
        .read_info()
        .map_err(|e| EscPosError::Decode(e.to_string()))?;
    let out_size = reader
        .output_buffer_size()
        .ok_or_else(|| EscPosError::Decode("output buffer size unavailable".to_string()))?;
    let mut buf = vec![0u8; out_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| EscPosError::Decode(e.to_string()))?;

    if info.bit_depth != png::BitDepth::Eight {
        return Err(EscPosError::Decode(format!(
            "unsupported bit depth {:?} (expected 8)",
            info.bit_depth
        )));
    }
    // Samples per pixel for the 8-bit color types the renderer may emit.
    let spp = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => {
            return Err(EscPosError::Decode(format!(
                "unsupported color type {other:?}"
            )))
        }
    };
    let data = &buf[..info.buffer_size()];
    let (w, h) = (info.width, info.height);
    let mut dots = Vec::with_capacity((w * h) as usize);
    for px in data.chunks_exact(spp) {
        // Luminance from the color samples (alpha ignored — the canvas is opaque).
        let lum = match spp {
            1 | 2 => u32::from(px[0]),
            _ => (u32::from(px[0]) * 299 + u32::from(px[1]) * 587 + u32::from(px[2]) * 114) / 1000,
        };
        dots.push(lum < u32::from(BLACK_THRESHOLD));
    }
    Ok(Bitmap {
        width: w,
        height: h,
        dots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode an 8×1 grayscale row where the left 4 px are black and the right
    /// 4 white, so the expected raster byte is 0b11110000 = 0xF0.
    fn png_8x1_left_black() -> Vec<u8> {
        png_gray(8, 1, &[0, 0, 0, 0, 255, 255, 255, 255])
    }

    fn png_gray(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w, h);
            enc.set_color(png::ColorType::Grayscale);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("png header");
            writer.write_image_data(pixels).expect("png data");
        }
        png
    }

    fn marker(name: &str, y_px: u32) -> MarkerAt {
        MarkerAt {
            name: name.to_string(),
            y_px,
        }
    }

    fn has_gs_v(bytes: &[u8]) -> bool {
        bytes.windows(2).any(|w| w == [0x1D, 0x56])
    }

    #[test]
    fn emits_reset_raster_header_and_feed() {
        let bytes = png_to_escpos(&png_8x1_left_black(), &[], &PrinterProfile::default()).unwrap();
        // ESC @ reset.
        assert_eq!(&bytes[0..2], &[0x1B, 0x40]);
        // GS v 0, m=0, 1 byte/row, 1 row.
        assert_eq!(
            &bytes[2..10],
            &[0x1D, 0x76, 0x30, 0x00, 0x01, 0x00, 0x01, 0x00]
        );
        // The single raster byte: left half black.
        assert_eq!(bytes[10], 0xF0);
        // Ends with the tear-off feed.
        assert_eq!(&bytes[bytes.len() - 3..], &[0x1B, 0x64, FEED_LINES]);
    }

    /// The safety rule, pinned: a cut sent to a printer that cannot cut is
    /// silently ignored *and latches an error that stops the printer until it is
    /// power-cycled* — every later ticket then dies unnoticed. A document asking
    /// to cut must never be enough on its own.
    #[test]
    fn a_cut_marker_alone_never_cuts_without_a_capable_printer() {
        let bytes = png_to_escpos(
            &png_8x1_left_black(),
            &[marker("cut", 1)],
            &PrinterProfile::default(), // CutMode::None — the default
        )
        .unwrap();
        assert!(!has_gs_v(&bytes), "GS V (cut) must not appear");
    }

    #[test]
    fn a_capable_printer_cuts_where_the_document_asked() {
        let profile = PrinterProfile {
            cut: CutMode::Partial,
        };
        let bytes = png_to_escpos(&png_8x1_left_black(), &[marker("cut", 1)], &profile).unwrap();
        // GS V 66 n — feed, then partial cut. The bare GS V 1 slices the last
        // printed line, since the blade sits above the head.
        assert!(
            bytes.windows(3).any(|w| w == [0x1D, 0x56, 0x42]),
            "expected feed-then-partial-cut"
        );
    }

    /// A marker mid-document (customer copy | merchant copy) must land where the
    /// template put it, not at the end of the nearest raster band.
    #[test]
    fn a_mid_ticket_cut_splits_the_raster_there() {
        let png = png_gray(8, 300, &vec![255u8; 8 * 300]);
        let profile = PrinterProfile { cut: CutMode::Full };
        let bytes = png_to_escpos(&png, &[marker("cut", 150)], &profile).unwrap();

        let cut_at = bytes
            .windows(3)
            .position(|w| w == [0x1D, 0x56, 0x41])
            .expect("cut must be emitted");
        let rasters_before = bytes[..cut_at]
            .windows(3)
            .filter(|w| *w == [0x1D, 0x76, 0x30])
            .count();
        let rasters_after = bytes[cut_at..]
            .windows(3)
            .filter(|w| *w == [0x1D, 0x76, 0x30])
            .count();
        assert!(
            rasters_before >= 1,
            "content above the cut must print first"
        );
        assert!(rasters_after >= 1, "content below the cut must still print");
    }

    #[test]
    fn an_intent_this_printer_cannot_do_is_ignored_not_an_error() {
        // A ticket authored for a printer with a cash drawer must still print here.
        let bytes = png_to_escpos(
            &png_8x1_left_black(),
            &[marker("teleport", 1)],
            &PrinterProfile::default(),
        )
        .unwrap();
        assert!(!has_gs_v(&bytes));
    }

    #[test]
    fn wide_image_bands_by_row() {
        // 8px wide, 300 rows all white → 3 bands (128,128,44), each a GS v 0.
        let png = png_gray(8, 300, &vec![255u8; 8 * 300]);
        let bytes = png_to_escpos(&png, &[], &PrinterProfile::default()).unwrap();
        let bands = bytes
            .windows(3)
            .filter(|w| w == &[0x1D, 0x76, 0x30])
            .count();
        assert_eq!(bands, 3);
    }

    #[test]
    fn cut_mode_parses_leniently_and_fails_closed() {
        assert_eq!(CutMode::parse("  Partial "), CutMode::Partial);
        assert_eq!(CutMode::parse("FULL"), CutMode::Full);
        // A typo must not make a printer cut when nobody meant it to.
        assert_eq!(CutMode::parse("ful"), CutMode::None);
        assert_eq!(CutMode::parse(""), CutMode::None);
    }

    /// The one-call path must agree with the two-call path.
    #[test]
    fn render_escpos_matches_render_then_encode() {
        let doc: TicketDoc = serde_json::from_str(
            r#"{ "version": 1, "paper": { "width_chars": 32 },
                 "elements": [ { "id": "a", "row": 0, "col": 0, "type": "text", "content": "HI" } ] }"#,
        )
        .expect("doc");
        let profile = PrinterProfile::default();
        let fonts = Fonts::builtin_shared();

        let one_call = render_escpos(
            &doc,
            &serde_json::Value::Null,
            fonts,
            &RenderOptions::default(),
            &profile,
        )
        .expect("render_escpos");

        let out = render(
            &doc,
            &serde_json::Value::Null,
            fonts,
            &RenderOptions::default(),
        )
        .expect("render");
        let two_call = png_to_escpos(&out.png, &[], &profile).expect("encode");

        assert_eq!(one_call, two_call);
    }
}
