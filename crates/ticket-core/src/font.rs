//! Monospace font families available to the renderer.
//!
//! The built-in family (DejaVu Sans Mono) is compiled *into* the crate with
//! `include_bytes!` so the native backend and the wasm build rasterize from the
//! exact same outlines — the prerequisite for the "1:1 identical preview"
//! guarantee. Additional families are supplied by the host at render time (from
//! separately-shipped, lazily-loaded TTFs) via [`Fonts::add_family`], so the
//! wasm bundle stays small and a font is only downloaded when a ticket uses it.
//!
//! The renderer resolves each element's font against the [`Fonts`] it was given
//! and **fails loudly** (`RenderError::MissingFont`) if a referenced family is
//! absent — a backend render never silently substitutes a font.
//!
//! Only monospace fonts belong here: the whole layout is a fixed-width character
//! grid, so a proportional face would sit unevenly in its cells.

use std::collections::HashMap;

use ab_glyph::FontVec;

const REGULAR: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");
const BOLD: &[u8] = include_bytes!("../assets/DejaVuSansMono-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../assets/DejaVuSansMono-Oblique.ttf");
const BOLD_ITALIC: &[u8] = include_bytes!("../assets/DejaVuSansMono-BoldOblique.ttf");

/// The built-in family id — always available, and the face used when an element
/// (and the document) name no font.
pub const DEFAULT_FAMILY: &str = "mono";

/// The four faces of one monospace family.
pub struct FontFaces {
    regular: FontVec,
    bold: FontVec,
    italic: FontVec,
    bold_italic: FontVec,
}

impl FontFaces {
    /// Parse a family from its four faces' bytes. A family that ships fewer
    /// weights can pass the regular bytes for the missing faces (the host decides
    /// the fallback; the renderer just draws what it is given).
    pub fn from_bytes(
        regular: Vec<u8>,
        bold: Vec<u8>,
        italic: Vec<u8>,
        bold_italic: Vec<u8>,
    ) -> Result<Self, ab_glyph::InvalidFont> {
        Ok(FontFaces {
            regular: FontVec::try_from_vec(regular)?,
            bold: FontVec::try_from_vec(bold)?,
            italic: FontVec::try_from_vec(italic)?,
            bold_italic: FontVec::try_from_vec(bold_italic)?,
        })
    }

    fn face(&self, bold: bool, italic: bool) -> &FontVec {
        match (bold, italic) {
            (true, true) => &self.bold_italic,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (false, false) => &self.regular,
        }
    }
}

/// The set of monospace families the renderer may draw with. Always contains the
/// built-in default; the host adds more before rendering a document that uses
/// them.
pub struct Fonts {
    default: FontFaces,
    extra: HashMap<String, FontFaces>,
}

impl Fonts {
    /// A process-wide, parse-once built-in font set. The convenience renderers use
    /// this so they don't re-copy and re-parse the ~1.2 MB of embedded faces on
    /// every call. Panics only if the *embedded* fonts are corrupt — a broken
    /// build, not runtime input — which the test suite would catch immediately.
    pub(crate) fn builtin_shared() -> &'static Fonts {
        use std::sync::OnceLock;
        static BUILTIN: OnceLock<Fonts> = OnceLock::new();
        BUILTIN.get_or_init(|| Fonts::builtin().expect("embedded fonts must be valid"))
    }

    /// Just the built-in family (id [`DEFAULT_FAMILY`]). Parsing is infallible in
    /// practice — the bytes ship with the crate — but returns a `Result` so a
    /// corrupt build fails loudly rather than panicking inside rendering.
    pub fn builtin() -> Result<Self, ab_glyph::InvalidFont> {
        Ok(Fonts {
            default: FontFaces::from_bytes(
                REGULAR.to_vec(),
                BOLD.to_vec(),
                ITALIC.to_vec(),
                BOLD_ITALIC.to_vec(),
            )?,
            extra: HashMap::new(),
        })
    }

    /// Register another monospace family under `id`. Replaces any family already
    /// registered under that id (`DEFAULT_FAMILY` cannot be replaced).
    pub fn add_family(&mut self, id: impl Into<String>, faces: FontFaces) {
        let id = id.into();
        if id != DEFAULT_FAMILY {
            self.extra.insert(id, faces);
        }
    }

    /// Whether `family` is available (the default always is).
    pub fn contains(&self, family: &str) -> bool {
        family == DEFAULT_FAMILY || self.extra.contains_key(family)
    }

    /// The face for `family` (`None` → default) at the given weight/slant, or the
    /// missing family id as `Err` for the caller to surface.
    pub(crate) fn face(
        &self,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) -> Result<&FontVec, String> {
        match family {
            None => Ok(self.default.face(bold, italic)),
            Some(id) if id == DEFAULT_FAMILY => Ok(self.default.face(bold, italic)),
            Some(id) => self
                .extra
                .get(id)
                .map(|f| f.face(bold, italic))
                .ok_or_else(|| id.to_string()),
        }
    }

    /// The reference regular face (the default family) used for a single shared
    /// baseline, so glyphs of different families still line up on the grid.
    pub(crate) fn reference(&self) -> &FontVec {
        &self.default.regular
    }

    /// The built-in family plus **every** editor font family, embedded in the
    /// crate. Requires the `bundled-fonts` feature. Build once and reuse across
    /// renders (parsing all faces isn't free). This is how a backend renders a
    /// ticket authored in any editor font with the *exact same bytes* the browser
    /// preview used — the crate's fonts are a byte-identical copy of the editor's.
    #[cfg(feature = "bundled-fonts")]
    pub fn with_bundled() -> Result<Self, ab_glyph::InvalidFont> {
        let mut fonts = Fonts::builtin()?;
        macro_rules! bundled {
            ($($id:literal),* $(,)?) => {$(
                fonts.add_family(
                    $id,
                    FontFaces::from_bytes(
                        include_bytes!(concat!("../assets/fonts/", $id, "/regular.ttf")).to_vec(),
                        include_bytes!(concat!("../assets/fonts/", $id, "/bold.ttf")).to_vec(),
                        include_bytes!(concat!("../assets/fonts/", $id, "/italic.ttf")).to_vec(),
                        include_bytes!(concat!("../assets/fonts/", $id, "/bold-italic.ttf")).to_vec(),
                    )?,
                );
            )*};
        }
        bundled!(
            "jetbrains-mono",
            "ibm-plex-mono",
            "source-code-pro",
            "fira-mono",
            "roboto-mono",
            "inconsolata",
            "space-mono",
            "b612-mono",
            "courier-prime",
            "cutive-mono",
            "share-tech-mono",
            "nova-mono",
            "syne-mono",
            "major-mono-display",
            "vt323",
        );
        Ok(fonts)
    }
}
