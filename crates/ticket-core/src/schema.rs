//! The canonical ticket description schema.
//!
//! This is the source of truth the visual editor reads and writes. It is
//! deliberately a plain data model (serde) so it can be persisted as JSON,
//! diffed, versioned and migrated. The backend later compiles it to an Askama
//! template; the renderer in this crate consumes it directly.
//!
//! Everything is expressed in **character cells**, never raw pixels. A ticket
//! is a fixed-width monospace grid: `paper.width_chars` columns wide, growing
//! downward in whole lines. Reserving space in characters (not pixels) is what
//! guarantees a variable can never overflow its slot and push the layout around
//! when a real value replaces the placeholder — the class of glitch the spec
//! explicitly calls out.

use serde::{Deserialize, Serialize};

/// Current schema version. Bump when the shape changes so persisted documents
/// can be migrated deterministically.
pub const SCHEMA_VERSION: u32 = 1;

/// A complete ticket layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketDoc {
    /// Schema version of this document.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Paper / printable-area configuration.
    pub paper: Paper,
    /// Placed elements. Order is irrelevant to rendering (position is absolute);
    /// it only matters for editor z-stacking / selection.
    #[serde(default)]
    pub elements: Vec<Element>,
    /// Row-bands with flow behaviour: loops (repeat per item) and/or conditions
    /// (collapse when false). Elements are captured by the band whose row-range
    /// contains them. See `render` for the flow transform.
    #[serde(default)]
    pub regions: Vec<Region>,
}

/// A row-band `[start_row, end_row)` (content rows) that changes how the rows it
/// covers flow. A band may repeat (loop) and/or be conditional; combining both
/// gives a loop that only appears when its condition holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    /// Stable id (used by the editor to select the band).
    pub id: String,
    /// First content row covered (inclusive).
    pub start_row: u32,
    /// One past the last content row covered (exclusive); `end_row > start_row`.
    pub end_row: u32,
    /// Repeatable array path (e.g. `sale.items`). When set, the band renders
    /// once per item; elements inside resolve their variable paths against the
    /// current item first, then the document root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// When set, the whole band (and the rows it occupies) only appears if this
    /// holds; otherwise those rows collapse and content below flows up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
}

/// A simple, non-programmer condition: `<var> <op> [value]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Variable path — relative to the loop item when inside a loop, else absolute.
    pub var: String,
    /// The comparison operator.
    pub op: CondOp,
    /// Comparison operand (ignored by `is_set` / `is_empty`). Compared numerically
    /// when both sides parse as numbers, else as text.
    #[serde(default)]
    pub value: String,
}

/// Condition operators, kept to what a non-programmer reads at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CondOp {
    /// Value is present and non-empty.
    IsSet,
    /// Value is absent or empty.
    IsEmpty,
    /// Equal to the operand.
    Eq,
    /// Not equal to the operand.
    Ne,
    /// Greater than the operand.
    Gt,
    /// Less than the operand.
    Lt,
    /// Greater than or equal to the operand.
    Gte,
    /// Less than or equal to the operand.
    Lte,
}

fn default_version() -> u32 {
    SCHEMA_VERSION
}

/// The physical constraints of the target paper and printer.
///
/// Widths are in characters because the grid is monospace. Pixel density
/// (`cell_width_px` / `cell_height_px`) is what turns the abstract grid into a
/// concrete raster; both the native and wasm renderer read these identical
/// numbers, which is what makes the two outputs byte-for-byte equal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    /// Total printable width in character columns (e.g. 32 for 58mm, 48 for 80mm).
    pub width_chars: u32,
    /// Columns of unusable margin on the left (printers cannot ink the full width).
    #[serde(default)]
    pub margin_left_chars: u32,
    /// Columns of unusable margin on the right.
    #[serde(default)]
    pub margin_right_chars: u32,
    /// Blank lines reserved at the top.
    #[serde(default)]
    pub margin_top_lines: u32,
    /// Blank lines reserved at the bottom.
    #[serde(default)]
    pub margin_bottom_lines: u32,
    /// Pixel width of a single character cell.
    #[serde(default = "default_cell_w")]
    pub cell_width_px: u32,
    /// Pixel height of a single character cell (the line height).
    #[serde(default = "default_cell_h")]
    pub cell_height_px: u32,
    /// Font size in pixels used to rasterize glyphs inside a cell.
    #[serde(default = "default_font_px")]
    pub font_px: f32,
    /// Minimum number of content lines. The ticket is at least this tall, so
    /// trailing blank lines (e.g. space for a signature or a tear-off) are
    /// preserved even though no element occupies them. The real height is
    /// `max(min_rows, lowest element)`.
    #[serde(default)]
    pub min_rows: u32,
}

fn default_cell_w() -> u32 {
    12
}
fn default_cell_h() -> u32 {
    22
}
fn default_font_px() -> f32 {
    20.0
}

impl Paper {
    /// Number of usable content columns after subtracting margins.
    pub fn content_cols(&self) -> u32 {
        self.width_chars
            .saturating_sub(self.margin_left_chars)
            .saturating_sub(self.margin_right_chars)
    }
}

/// A single placed thing on the grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    /// Stable id (used by the editor for selection / drag). Not used by the renderer.
    pub id: String,
    /// Grid row (0-based, before the top margin is applied).
    pub row: u32,
    /// Grid column (0-based, within the content area — margins are added by the renderer).
    pub col: u32,
    /// Fine vertical nudge in rows (may be fractional / negative). Shifts the
    /// element's pixels without changing its grid row — for closing or opening a
    /// gap by a fraction of a line. 0 = on the grid.
    #[serde(default)]
    pub y_offset: f32,
    /// What this element renders.
    #[serde(flatten)]
    pub kind: ElementKind,
    /// Visual styling.
    #[serde(default)]
    pub style: Style,
    /// Show this element only when the condition holds. Unlike a conditional
    /// region it does NOT collapse rows — it just hides the element in place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
}

/// The payload of an element. `type`-tagged so JSON reads naturally:
/// `{ "type": "text", "content": "TOTAL" }` or
/// `{ "type": "variable", "path": "sale.total_amount", "length": 8 }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    /// Literal text that renders as-is.
    Text {
        /// The literal text.
        content: String,
    },
    /// A monochrome image (logo). Provided as base64 PNG (optionally a
    /// `data:` URI). Occupies `w × h` character cells; the renderer scales it to
    /// that pixel box and reduces it to 1-bit black/white the same way the
    /// printer will — so the preview is exactly what prints.
    Image {
        /// base64-encoded PNG bytes (with or without a `data:image/png;base64,` prefix).
        data: String,
        /// Width in character cells.
        w: u32,
        /// Height in character cells.
        h: u32,
        /// How the image is reduced to black/white.
        #[serde(default)]
        mode: ImageMode,
    },
    /// A QR code generated from a value (a variable path or a literal string).
    /// Occupies a `size × size` square of character cells.
    Qr {
        /// Variable path (when `from_variable`) or literal text to encode.
        value: String,
        /// If true, `value` is resolved from the data; else used literally.
        #[serde(default)]
        from_variable: bool,
        /// Side length in character cells.
        size: u32,
    },
    /// A value pulled from the variable tree at render time.
    Variable {
        /// Dotted path into the variables object, e.g. `sale.total_amount`.
        path: String,
        /// Horizontal width in characters. The value is fitted to exactly this
        /// many columns per line, so the layout never shifts. With `wrap` off a
        /// longer value is truncated; with `wrap` on it flows onto more lines,
        /// each still `length` columns wide.
        length: u32,
        /// How the value is placed inside its reserved width (per line).
        #[serde(default)]
        align: Align,
        /// When true, a value wider than `length` flows onto extra lines
        /// (word-aware) instead of being truncated.
        #[serde(default)]
        wrap: bool,
        /// Numeric formatting (decimals, rounding, thousands). Mutually exclusive
        /// with `date_format`; if both are set, `number` wins.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        number: Option<NumberFormat>,
        /// Date reformatting pattern (e.g. `DD/MM/YYYY HH:mm`). Applied to a value
        /// parsed as an ISO-ish timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date_format: Option<String>,
    },
}

/// How a numeric value is rendered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberFormat {
    /// Number of decimal places to keep.
    #[serde(default = "default_decimals")]
    pub decimals: u8,
    /// How the last kept digit is rounded.
    #[serde(default)]
    pub rounding: Rounding,
    /// Insert a thousands separator in the integer part.
    #[serde(default)]
    pub thousands: bool,
}

fn default_decimals() -> u8 {
    2
}

/// Rounding strategy for the final decimal. `HalfUp` is the everyday default;
/// `HalfEven` (banker's) is what accounting usually wants; `Down` truncates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Rounding {
    /// Round half away from zero (the everyday default).
    #[default]
    HalfUp,
    /// Round half to even (banker's rounding).
    HalfEven,
    /// Always round up (away from zero).
    Up,
    /// Always round down (toward zero — truncate).
    Down,
}

/// Horizontal alignment of a value within its reserved character width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    /// Left-aligned within the reserved width.
    #[default]
    Left,
    /// Right-aligned within the reserved width.
    Right,
    /// Centered within the reserved width.
    Center,
}

/// How a color image is reduced to 1-bit black/white for a thermal printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageMode {
    /// Every pixel darker than `level` becomes black. Crisp for logos / line art.
    Threshold {
        /// Grayscale cutoff (0–255): pixels darker than this become black.
        level: u8,
    },
    /// Floyd–Steinberg error diffusion. Better for photos / gradients.
    Dither,
}

impl Default for ImageMode {
    fn default() -> Self {
        ImageMode::Threshold { level: 128 }
    }
}

/// Vertical placement of the glyph within its (possibly magnified) cell block.
/// Only visible when `scale > 1`, where the block is taller than the ink: it
/// controls whether a big title hugs the top, centre, or bottom of its rows —
/// the lever for tightening the gap to an adjacent line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    /// Glyph hugs the top of its block.
    Top,
    /// Glyph centered in its block (the default).
    #[default]
    Middle,
    /// Glyph hugs the bottom of its block.
    Bottom,
}

/// Per-element visual styling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    /// Render with the bold face.
    #[serde(default)]
    pub bold: bool,
    /// Render with the italic (oblique) face.
    #[serde(default)]
    pub italic: bool,
    /// Integer size magnification (1–4), the way thermal printers actually scale
    /// text. A char at scale `s` occupies `s` columns × `s` rows, so the grid
    /// stays exact at any size — no sub-cell fractions to cause overlap glitches.
    #[serde(default = "default_scale")]
    pub scale: u8,
    /// Vertical placement of the glyph within its cell block (matters at scale > 1).
    #[serde(default)]
    pub valign: VAlign,
}

fn default_scale() -> u8 {
    1
}

impl Default for Style {
    fn default() -> Self {
        Style {
            bold: false,
            italic: false,
            scale: 1,
            valign: VAlign::Middle,
        }
    }
}

impl Style {
    /// Scale clamped to the supported 1–4 range.
    pub fn scale_clamped(&self) -> u32 {
        self.scale.clamp(1, 4) as u32
    }
}
