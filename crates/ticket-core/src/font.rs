//! Embedded monospace fonts.
//!
//! The fonts are compiled *into* the crate with `include_bytes!` rather than
//! loaded from the system. This is what lets the native backend and the wasm
//! browser build rasterize from the exact same glyph outlines — a prerequisite
//! for the "1:1 identical preview" guarantee. If the font were loaded from disk
//! (present on the server, absent in the browser) parity would be impossible.

use ab_glyph::FontRef;

const REGULAR: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");
const BOLD: &[u8] = include_bytes!("../assets/DejaVuSansMono-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../assets/DejaVuSansMono-Oblique.ttf");
const BOLD_ITALIC: &[u8] = include_bytes!("../assets/DejaVuSansMono-BoldOblique.ttf");

/// The four monospace faces, parsed once and reused for every cell.
pub struct FontSet {
    pub regular: FontRef<'static>,
    pub bold: FontRef<'static>,
    pub italic: FontRef<'static>,
    pub bold_italic: FontRef<'static>,
}

impl FontSet {
    /// Parse the embedded fonts. Infallible in practice (the bytes ship with the
    /// crate) but returns a Result so a corrupt build fails loudly rather than
    /// panicking deep inside rendering.
    pub fn load() -> Result<Self, ab_glyph::InvalidFont> {
        Ok(FontSet {
            regular: FontRef::try_from_slice(REGULAR)?,
            bold: FontRef::try_from_slice(BOLD)?,
            italic: FontRef::try_from_slice(ITALIC)?,
            bold_italic: FontRef::try_from_slice(BOLD_ITALIC)?,
        })
    }

    /// Pick the face for a given style combination.
    pub fn face(&self, bold: bool, italic: bool) -> &FontRef<'static> {
        match (bold, italic) {
            (true, true) => &self.bold_italic,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (false, false) => &self.regular,
        }
    }
}
