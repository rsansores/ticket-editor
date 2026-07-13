//! The deterministic grid → PNG renderer.
//!
//! Rendering is two stages:
//!   1. **Lay out glyph placements.** Every element expands into a list of
//!      placements — one per character — on the base character grid. Text is
//!      fitted to its reserved width (truncate/pad) or, when `wrap` is set,
//!      flowed onto extra lines. Size magnification (`scale`) makes a character
//!      occupy `scale × scale` base cells. Because every placement lands on
//!      whole-cell coordinates, nothing is ever a fraction of a cell out of
//!      alignment — the structural cure for overflow/overlap glitches.
//!   2. **Rasterize.** Each placement's glyph is drawn anchored to its cell
//!      block and clipped to the printable area (inside the paper margins).
//!      Anything past the paper edge is clipped, never wrapped — matching what
//!      the editor shows in its overflow zone.
//!
//! Pure and deterministic: same document + data → same PNG bytes, native or wasm.

use ab_glyph::{Font, PxScale, ScaleFont};
use serde_json::Value;

use crate::data;
use crate::font::Fonts;
use crate::format;
use crate::image;
use crate::qr;
use crate::schema::{Align, Element, ElementKind, TicketDoc, VAlign};

/// Something that went wrong while rendering.
///
/// `#[non_exhaustive]` so new variants can be added without a breaking change;
/// callers should include a wildcard arm.
#[derive(Debug)]
#[non_exhaustive]
pub enum RenderError {
    /// The embedded font failed to parse (should not happen in a valid build).
    Font(String),
    /// PNG encoding failed.
    Png(String),
    /// The document would produce an image larger than `MAX_PIXELS`.
    TooLarge {
        /// Pixel width that tripped the limit (`u64` — may exceed `u32`).
        width: u64,
        /// Pixel height that tripped the limit.
        height: u64,
    },
    /// The document references a font family the renderer wasn't given. The
    /// backend must register it (its TTFs) before rendering. Holds the family id.
    MissingFont(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Font(e) => write!(f, "invalid font: {e}"),
            RenderError::Png(e) => write!(f, "png encode failed: {e}"),
            RenderError::TooLarge { width, height } => {
                write!(f, "rendered image too large: {width}x{height}")
            }
            RenderError::MissingFont(id) => {
                write!(f, "renderer has no access to font '{id}'")
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// Options controlling how a document is rendered.
///
/// `Default` is the **backend / real-print** configuration. The editor opts into
/// placeholder mode explicitly.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RenderOptions {
    /// When true (the editor's live preview), a variable path that doesn't
    /// resolve in the data renders a deterministic fake value so the canvas
    /// looks alive before any real data exists. When false (the default — a
    /// backend printing a real ticket), an unresolved path renders **empty**,
    /// padded to its reserved width like any short value: a typo'd path or a
    /// legitimately-null field must never print a believable wrong value.
    pub placeholders: bool,
}

impl RenderOptions {
    /// The editor-preview configuration (fake values for unresolved paths).
    pub fn placeholders() -> Self {
        RenderOptions { placeholders: true }
    }
}

/// A [`Marker`](crate::schema::ElementKind::Marker) the layout encountered,
/// with its **absolute, post-flow** row (top margin included, loops expanded,
/// collapses and wrap shifts applied) — where in the output the consumer
/// should inject the mapped device command. `row × cell_height_px` is the
/// pixel y; markers are reported top-to-bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerHit {
    /// The marker's intent name (`cut`, `feed`, `beep`, `drawer`, or custom).
    pub name: String,
    /// Absolute grid row after the flow transform.
    pub row: u32,
}

/// Everything a render produces: the raster plus the finishing markers. A
/// consumer that ignores `markers` behaves exactly like the plain
/// [`render_png`] path.
#[derive(Debug, Clone)]
pub struct RenderOutput {
    /// The rendered ticket as PNG bytes.
    pub png: Vec<u8>,
    /// Finishing markers in top-to-bottom order.
    pub markers: Vec<MarkerHit>,
}

/// Hard ceiling on any single raster (the whole canvas, or one logo/QR block),
/// in pixels (~64 megapixels). Bounds memory against adversarial documents.
pub(crate) const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// One laid-out character to draw.
struct Placement {
    /// Absolute base-grid column (margins already applied).
    col: u32,
    /// Absolute base-grid row (margins already applied).
    row: u32,
    /// Fine vertical nudge in pixels (from the element's `y_offset`).
    y_off_px: f32,
    ch: char,
    /// Size magnification: the glyph fills `scale × scale` base cells.
    scale: u32,
    bold: bool,
    italic: bool,
    valign: VAlign,
    /// Resolved monospace family id (`None` → built-in default). `Rc` so an
    /// element's glyphs share one allocation instead of one per character.
    font: Option<std::rc::Rc<str>>,
}

/// A pre-rasterized 1-bit block (logo or QR) to composite onto the canvas.
struct RasterBlock {
    /// Top-left pixel origin.
    x: i64,
    y: i64,
    w: u32,
    h: u32,
    /// Row-major mask, `w*h` long; true = black ink.
    mask: Vec<bool>,
}

/// Render a document to PNG bytes using only the built-in font and the default
/// [`RenderOptions`] (real-print mode: unresolved paths render empty).
pub fn render_png(doc: &TicketDoc, variables: &Value) -> Result<Vec<u8>, RenderError> {
    render_png_with_fonts(doc, variables, Fonts::builtin_shared())
}

/// Render a document to PNG bytes with a caller-provided font set — the built-in
/// default plus any families registered via [`Fonts::add_family`] (e.g. from
/// lazily-loaded TTFs). Fails with [`RenderError::MissingFont`] if the document
/// references a family the set doesn't contain, so a render never silently
/// substitutes a font. Uses the default [`RenderOptions`].
pub fn render_png_with_fonts(
    doc: &TicketDoc,
    variables: &Value,
    fonts: &Fonts,
) -> Result<Vec<u8>, RenderError> {
    render_png_with_options(doc, variables, fonts, &RenderOptions::default())
}

/// Render a document to PNG bytes with explicit [`RenderOptions`]. The editor
/// preview passes [`RenderOptions::placeholders`]; a backend printing real
/// tickets uses the default.
pub fn render_png_with_options(
    doc: &TicketDoc,
    variables: &Value,
    fonts: &Fonts,
    opts: &RenderOptions,
) -> Result<Vec<u8>, RenderError> {
    Ok(render(doc, variables, fonts, opts)?.png)
}

/// Render a document to a [`RenderOutput`]: the PNG plus the finishing
/// markers (cut / feed / beep / drawer) with their resolved absolute rows.
/// This is the full-fidelity entry point for a print consumer that maps
/// marker intent to device commands; the `render_png*` family are thin
/// wrappers that drop the markers.
pub fn render(
    doc: &TicketDoc,
    variables: &Value,
    fonts: &Fonts,
    opts: &RenderOptions,
) -> Result<RenderOutput, RenderError> {
    // Evaluate calculated variables once and expose them under `calc.*`, so every
    // downstream resolver (variables, QR-from-variable, conditions, loops) treats
    // them like ordinary data. Skipped entirely when the document has none.
    let merged;
    let variables = if doc.computed.is_empty() {
        variables
    } else {
        merged = data::with_computed(variables, &doc.computed);
        &merged
    };
    let (placements, blocks, total_rows, mut markers) = lay_out(doc, variables, fonts, opts)?;
    // Top-to-bottom, then declaration order — deterministic for consumers.
    markers.sort_by_key(|m| m.row);
    let png = rasterize(doc, &placements, &blocks, total_rows, fonts)?;
    Ok(RenderOutput { png, markers })
}

/// Stage 1: apply the flow transform (loops expand, conditional bands collapse,
/// wrapped lines push content down, content below reflows) and expand every
/// visible element into character placements. Returns the placements and the
/// total grid rows needed.
///
/// The transform is a single top-down walk. `flow` carries the net rows
/// everything at the current design position has been pushed down (+) or pulled
/// up (−) by what's above it: loop repetitions, collapsed bands, and lines added
/// by `wrap`. Because rows below never affect rows above, one pass suffices —
/// region deltas and wrap deltas compose by simple accumulation.
#[allow(clippy::type_complexity)]
fn lay_out(
    doc: &TicketDoc,
    variables: &Value,
    fonts: &Fonts,
    opts: &RenderOptions,
) -> Result<(Vec<Placement>, Vec<RasterBlock>, u32, Vec<MarkerHit>), RenderError> {
    let paper = &doc.paper;
    let content_cols = paper.content_cols();
    let ml = paper.margin_left_chars;
    let mt = paper.margin_top_lines;
    let cell_w = paper.cell_width_px.max(1);
    let cell_h = paper.cell_height_px.max(1);
    let cell_w64 = u64::from(cell_w);
    let cell_h64 = u64::from(cell_h);

    // Bands sorted by start row; each element belongs to the band whose row
    // range contains it, or to the free list. All lists ordered top-down so the
    // walk accumulates offsets strictly downward.
    let mut regions: Vec<&crate::schema::Region> = doc.regions.iter().collect();
    regions.sort_by_key(|r| r.start_row);
    let mut band_els: Vec<Vec<&Element>> = vec![Vec::new(); regions.len()];
    let mut free_els: Vec<&Element> = Vec::new();
    for el in &doc.elements {
        // Regions are sorted by start_row, so only bands starting at or above
        // this row can contain it — binary-search the boundary and scan back
        // (the scan is 1 step unless bands overlap, which the editor never
        // produces; a hostile doc degrades gracefully instead of O(E×R)).
        let bound = regions.partition_point(|r| r.start_row <= el.row);
        match (0..bound).rev().find(|&i| el.row < regions[i].end_row) {
            Some(i) => band_els[i].push(el),
            None => free_els.push(el),
        }
    }
    free_els.sort_by_key(|e| e.row);
    for v in &mut band_els {
        v.sort_by_key(|e| e.row);
    }

    let mut placements = Vec::new();
    let mut blocks: Vec<RasterBlock> = Vec::new();
    let mut markers: Vec<MarkerHit> = Vec::new();
    let mut max_bottom: i64 = 0; // lowest content row reached (absolute), exclusive
                                 // ONE aggregate-row budget for the whole layout: shared across every row
                                 // formula of every loop iteration (see expr::fresh_budget).
    let budget = crate::expr::fresh_budget();

    // Emit one element at an absolute base row. Returns the rows the element
    // occupies BEYOND its design row (`(lines-1)*scale` for a wrapped variable,
    // 0 for everything else) so the caller can push content below it down.
    // Blocks with explicit cell heights (image/QR/barcode) return 0: their
    // height is visible at design time, so the author reserves rows for them.
    let mut emit = |el: &Element, scope: data::Scope, base_row: i64| -> Result<u32, RenderError> {
        // Per-element condition (evaluated in the current scope): hide in place.
        if let Some(c) = &el.condition {
            if !data::eval_condition(scope, variables, c) {
                return Ok(0);
            }
        }
        let y_off_px = el.y_offset * cell_h as f32;
        match &el.kind {
            ElementKind::Image {
                data,
                from_variable,
                w,
                h,
                mode,
            } => {
                // Bound the block in u64 BEFORE decoding/allocating (adversarial w/h).
                let w_px64 = u64::from((*w).max(1)) * cell_w64;
                let h_px64 = u64::from((*h).max(1)) * cell_h64;
                if w_px64 > MAX_PIXELS || h_px64 > MAX_PIXELS || w_px64 * h_px64 > MAX_PIXELS {
                    return Err(RenderError::TooLarge {
                        width: w_px64,
                        height: h_px64,
                    });
                }
                let (w_px, h_px) = (w_px64 as u32, h_px64 as u32);
                // A dynamic image (e.g. a signature) resolves its base64
                // bytes from a variable. In the editor a missing/empty
                // source draws the placeholder frame below (visible while
                // designing); on a real print it draws NOTHING — a hollow
                // frame where a signature should be is print corruption,
                // same rule as QR/barcode.
                let resolved;
                let src: &str = if *from_variable {
                    resolved = data::resolve_or_fake(scope, variables, data, opts.placeholders)
                        .unwrap_or_default();
                    if resolved.is_empty() && !opts.placeholders {
                        return Ok(0);
                    }
                    &resolved
                } else {
                    data
                };
                let mask = match image::decode_png_gray(src) {
                    Ok((gray, sw, sh)) => {
                        let scaled = image::resize_gray(&gray, sw, sh, w_px, h_px);
                        image::to_bw(&scaled, w_px, h_px, *mode)
                    }
                    // On a bad image, draw an outline placeholder so it's visible.
                    Err(_) => placeholder_mask(w_px, h_px),
                };
                blocks.push(RasterBlock {
                    x: i64::from(ml + el.col) * i64::from(cell_w),
                    y: base_row * i64::from(cell_h) + y_off_px as i64,
                    w: w_px,
                    h: h_px,
                    mask,
                });
                max_bottom = max_bottom.max(base_row + i64::from(*h));
                Ok(0)
            }
            ElementKind::Qr {
                value,
                from_variable,
                size,
            } => {
                // Real print: a QR whose variable is missing or empty is
                // meaningless AND unscannable — draw nothing rather than a
                // placeholder frame on a customer's receipt. The editor
                // (placeholder mode) still shows a fake so the canvas is
                // lively — except for row.* paths, which never fake.
                let text = if *from_variable {
                    match data::resolve_or_fake(scope, variables, value, opts.placeholders) {
                        Some(t) if !t.is_empty() || opts.placeholders => t,
                        _ => return Ok(0),
                    }
                } else {
                    value.clone()
                };
                let side64 = u64::from((*size).max(1)) * cell_w64; // square (scannable)
                if side64 > MAX_PIXELS || side64 * side64 > MAX_PIXELS {
                    return Err(RenderError::TooLarge {
                        width: side64,
                        height: side64,
                    });
                }
                let side = side64 as u32;
                let rows = side.div_ceil(cell_h);
                // A value too long to encode falls back to a visible placeholder.
                let mask = qr_mask(&text, side).unwrap_or_else(|| placeholder_mask(side, side));
                blocks.push(RasterBlock {
                    x: i64::from(ml + el.col) * i64::from(cell_w),
                    y: base_row * i64::from(cell_h) + y_off_px as i64,
                    w: side,
                    h: side,
                    mask,
                });
                max_bottom = max_bottom.max(base_row + i64::from(rows));
                Ok(0)
            }
            ElementKind::Barcode {
                value,
                from_variable,
                symbology,
                width,
                height,
            } => {
                // Same policy as QR: no value → no barcode on a real print.
                let text = if *from_variable {
                    match data::resolve_or_fake(scope, variables, value, opts.placeholders) {
                        Some(t) if !t.is_empty() || opts.placeholders => t,
                        _ => return Ok(0),
                    }
                } else {
                    value.clone()
                };
                let w_px64 = u64::from((*width).max(1)) * cell_w64;
                let h_px64 = u64::from((*height).max(1)) * cell_h64;
                if w_px64 > MAX_PIXELS || h_px64 > MAX_PIXELS || w_px64 * h_px64 > MAX_PIXELS {
                    return Err(RenderError::TooLarge {
                        width: w_px64,
                        height: h_px64,
                    });
                }
                let (w_px, h_px) = (w_px64 as u32, h_px64 as u32);
                // An unencodable value (e.g. letters in an EAN-13) falls back
                // to the visible placeholder frame.
                let mask = barcode_mask(&text, *symbology, w_px, h_px)
                    .unwrap_or_else(|| placeholder_mask(w_px, h_px));
                blocks.push(RasterBlock {
                    x: i64::from(ml + el.col) * i64::from(cell_w),
                    y: base_row * i64::from(cell_h) + y_off_px as i64,
                    w: w_px,
                    h: h_px,
                    mask,
                });
                max_bottom = max_bottom.max(base_row + i64::from(*height));
                Ok(0)
            }
            ElementKind::Marker { name } => {
                // Zero ink, zero cells: record the intent at its resolved
                // absolute row and move on. Doesn't touch max_bottom — a
                // marker never makes the paper longer. Rows above the top
                // margin can't happen in practice (y_offset doesn't move
                // grid rows); clamp defensively.
                markers.push(MarkerHit {
                    name: name.clone(),
                    row: base_row.clamp(0, i64::from(u32::MAX)) as u32,
                });
                Ok(0)
            }
            _ => {
                let scale = el.style.scale_clamped();
                // Resolve this element's font: its own, then the document
                // default, then the built-in. Validate up front so a missing
                // family fails the whole render rather than drawing blanks.
                let family = el.style.font.as_deref().or(doc.font.as_deref());
                if let Some(f) = family {
                    if !fonts.contains(f) {
                        return Err(RenderError::MissingFont(f.to_string()));
                    }
                }
                // One allocation per element; each glyph shares it via `Rc`.
                let family: Option<std::rc::Rc<str>> = family.map(std::rc::Rc::from);
                let display = resolve_display(el, scope, variables, opts);
                let lines = fit_lines(el, &display, content_cols, scale);
                for (li, line) in lines.iter().enumerate() {
                    let row = base_row + i64::from(li as u32 * scale);
                    if row < 0 {
                        continue;
                    }
                    for (i, ch) in line.chars().enumerate() {
                        placements.push(Placement {
                            col: ml + el.col + i as u32 * scale,
                            row: row as u32,
                            y_off_px,
                            ch,
                            scale,
                            bold: el.style.bold,
                            italic: el.style.italic,
                            valign: el.style.valign,
                            font: family.clone(),
                        });
                    }
                }
                max_bottom = max_bottom.max(base_row + i64::from(lines.len() as u32 * scale));
                // Every line past the first pushes content below down by
                // `scale` rows — the wrap half of the flow transform.
                Ok((lines.len() as u32).saturating_sub(1) * scale)
            }
        }
    };

    // Emit a run of elements sharing one design row at an absolute base row;
    // the row's wrap-extra is the MAX over its elements (two fields wrapping to
    // 3 lines on the same row need 2 extra rows, not 4).
    // Written as a macro because it must call `emit` (a capturing FnMut) —
    // a second closure could not borrow it mutably at the same time.
    macro_rules! emit_row_group {
        ($els:expr, $j:ident, $scope:expr, $base:expr) => {{
            let row = $els[$j].row;
            let mut row_extra: u32 = 0;
            while $j < $els.len() && $els[$j].row == row {
                row_extra = row_extra.max(emit($els[$j], $scope, $base)?);
                $j += 1;
            }
            row_extra
        }};
    }

    // The top-down flow walk: interleave free rows and bands in design order.
    let mut flow: i64 = 0;
    let mut f = 0usize; // cursor into free_els
    for (ri, region) in regions.iter().enumerate() {
        // Free rows strictly above this band.
        while f < free_els.len() && free_els[f].row < region.start_row {
            let base = mt as i64 + i64::from(free_els[f].row) + flow;
            flow += i64::from(emit_row_group!(free_els, f, data::Scope::default(), base));
        }

        let height = region.end_row.saturating_sub(region.start_row).max(1);
        let enabled = region
            .condition
            .as_ref()
            .map(|c| data::eval_condition(data::Scope::default(), variables, c))
            .unwrap_or(true);
        // Loop items, capped so a giant data array can't explode the placement
        // list before the pixel guard runs.
        let items: Option<&[Value]> = region.source.as_deref().and_then(|s| {
            match data::resolve(variables, s) {
                // Capped so a giant data array can't explode the placement
                // list before the pixel guard runs. No real ticket loops this
                // much; the same cap bounds preview_row's count/last chips.
                Some(Value::Array(a)) => Some(&a[..a.len().min(data::MAX_LOOP)]),
                _ => None,
            }
        });
        let count = if !enabled {
            0
        } else if region.source.is_some() {
            items.map(|a| a.len()).unwrap_or(0)
        } else {
            1
        };
        if count == 0 {
            // Collapsed band: rows vanish, content below flows up. Row formulas
            // are never evaluated (no cost, no divide-by-zero on absent data).
            flow -= i64::from(height);
            continue;
        }

        let els = &band_els[ri];
        // Row values cost one object per iteration — skip the machinery when
        // nothing in the band reads `row.*`. Formulas parse ONCE here; the
        // loop below only evaluates the compiled ASTs, and every aggregate
        // scan in every iteration draws from the single shared budget, so a
        // hostile document can't multiply per-formula allowances into a stall.
        let band_uses_row = els.iter().any(|el| element_references_row(el));
        let compiled = if band_uses_row {
            data::compile_row(&region.computed)
        } else {
            Vec::new()
        };
        let band_top = mt as i64 + i64::from(region.start_row) + flow;
        let mut cursor = band_top;
        for i in 0..count {
            let loop_ctx: data::LoopCtx = items
                .and_then(|a| a.get(i))
                .and_then(|item| region.source.as_deref().map(|src| (src, i, item)));
            // Row values: every loop iteration gets the implicit index/number/…;
            // declared formulas evaluate on loop AND shown-conditional bands.
            let row_vals: Option<Value> = band_uses_row.then(|| {
                data::eval_row(
                    variables,
                    &compiled,
                    loop_ctx,
                    loop_ctx.map(|_| (i, count)),
                    &budget,
                )
            });
            let scope = data::Scope {
                loop_ctx,
                row: row_vals.as_ref(),
            };
            // Within one iteration, wrap extras shift this iteration's later
            // rows down, exactly like free rows — so the iteration's height is
            // its design height plus everything wrap inserted.
            let mut iter_extra: i64 = 0;
            let mut j = 0usize;
            while j < els.len() {
                let base = cursor + i64::from(els[j].row - region.start_row) + iter_extra;
                iter_extra += i64::from(emit_row_group!(els, j, scope, base));
            }
            cursor += i64::from(height) + iter_extra;
        }
        // Net rows this band added versus its design height.
        flow += (cursor - band_top) - i64::from(height);
    }
    // Free rows below the last band.
    while f < free_els.len() {
        let base = mt as i64 + i64::from(free_els[f].row) + flow;
        flow += i64::from(emit_row_group!(free_els, f, data::Scope::default(), base));
    }

    // Total height honours trailing whitespace (min_rows) after the net flow delta.
    let total_delta: i64 = flow;
    let design_floor = i64::from(mt) + i64::from(paper.min_rows.max(1)) + total_delta;
    let content_bottom = max_bottom.max(design_floor).max(i64::from(mt) + 1);
    let total_rows_i =
        (content_bottom + i64::from(paper.margin_bottom_lines)).clamp(1, i64::from(u32::MAX));

    // Final canvas bound in u64 — no u32 overflow before the check.
    let width64 = u64::from(paper.width_chars.max(1)) * cell_w64;
    let height64 = total_rows_i as u64 * cell_h64;
    if width64 == 0
        || height64 == 0
        || width64 > MAX_PIXELS
        || height64 > MAX_PIXELS
        || width64 * height64 > MAX_PIXELS
    {
        return Err(RenderError::TooLarge {
            width: width64,
            height: height64,
        });
    }

    Ok((placements, blocks, total_rows_i as u32, markers))
}

/// A QR matrix scaled into a `side × side` pixel mask with a 4-module quiet zone.
/// Returns `None` when the value can't be encoded (too long) so the caller can
/// draw a visible placeholder instead of a silent blank. `side` is assumed
/// already bounded by the caller against `MAX_PIXELS`.
/// Rasterize a 1D barcode into a `w_px × h_px` mask: each encoded bar becomes a
/// full-height black column, with a horizontal quiet zone (~10% each side) so a
/// scanner can lock onto it. `None` if the value can't be encoded.
fn barcode_mask(
    value: &str,
    sym: crate::schema::Symbology,
    w_px: u32,
    h_px: u32,
) -> Option<Vec<bool>> {
    let bars = crate::barcode::bars(value, sym)?;
    let n = bars.len();
    if n == 0 {
        return None;
    }
    let w = w_px.max(1) as usize;
    let h = h_px.max(1) as usize;
    let quiet = (n / 10).max(4);
    let total = n + quiet * 2;
    let module_px = w as f32 / total as f32;
    let mut mask = vec![false; w * h];
    for px in 0..w {
        let mx = (px as f32 / module_px).floor() as isize - quiet as isize;
        if mx >= 0 && (mx as usize) < n && bars[mx as usize] {
            for py in 0..h {
                mask[py * w + px] = true;
            }
        }
    }
    Some(mask)
}

fn qr_mask(text: &str, side: u32) -> Option<Vec<bool>> {
    let (n, m) = qr::matrix(text)?;
    let side = side.max(1) as usize;
    let mut mask = vec![false; side * side];
    let total = n + 8; // quiet zone of 4 modules on each side (scannability)
    let module_px = side as f32 / total as f32;
    for py in 0..side {
        let my = (py as f32 / module_px).floor() as isize - 4;
        for px in 0..side {
            let mx = (px as f32 / module_px).floor() as isize - 4;
            if mx >= 0 && (mx as usize) < n && my >= 0 && (my as usize) < n {
                mask[py * side + px] = m[(my as usize) * n + mx as usize];
            }
        }
    }
    Some(mask)
}

/// A hollow-rectangle mask, drawn when an image fails to decode.
fn placeholder_mask(w: u32, h: u32) -> Vec<bool> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut m = vec![false; w * h];
    for x in 0..w {
        m[x] = true;
        m[(h - 1) * w + x] = true;
    }
    for y in 0..h {
        m[y * w] = true;
        m[y * w + w - 1] = true;
    }
    m
}

/// Whether an element reads the row scope — its bound path or its condition
/// targets `row` / `row.*`. Decides if a band pays for per-iteration row
/// values at all.
fn element_references_row(el: &Element) -> bool {
    let is_row = |p: &str| p == "row" || p.starts_with("row.");
    if el.condition.as_ref().is_some_and(|c| is_row(&c.var)) {
        return true;
    }
    match &el.kind {
        ElementKind::Variable { path, .. } => is_row(path),
        ElementKind::Qr {
            value,
            from_variable,
            ..
        }
        | ElementKind::Barcode {
            value,
            from_variable,
            ..
        } => *from_variable && is_row(value),
        ElementKind::Image {
            data,
            from_variable,
            ..
        } => *from_variable && is_row(data),
        ElementKind::Text { .. } | ElementKind::Marker { .. } => false,
    }
}

/// Resolve an element to its final display string (variable lookup in the given
/// scope, then number or date formatting). An unresolved path renders empty
/// unless `opts.placeholders` is on (editor preview), in which case a
/// deterministic fake keeps the canvas lively. `row.*` paths never fake — a row
/// value referenced outside its band is empty in every mode, so the editor
/// can't paint a plausible value where print would show nothing.
fn resolve_display(el: &Element, scope: data::Scope, root: &Value, opts: &RenderOptions) -> String {
    match &el.kind {
        // Static text renders verbatim. A value derived from other variables is
        // authored as a calculated variable and placed as a `Variable` element.
        ElementKind::Text { content } => content.clone(),
        ElementKind::Variable {
            path,
            number,
            date_format,
            ..
        } => {
            let raw =
                data::resolve_or_fake(scope, root, path, opts.placeholders).unwrap_or_default();
            if let Some(nf) = number {
                format::format_number(&raw, nf)
            } else if let Some(df) = date_format {
                format::format_date(&raw, df)
            } else {
                raw
            }
        }
        _ => String::new(),
    }
}

/// Break an element's display string into the grid lines it occupies.
///
/// `length` on a variable is its **horizontal width in characters** — the same
/// meaning whether or not it wraps. Without `wrap` a longer value is truncated
/// to that width; with `wrap` it flows onto as many lines as needed, each still
/// exactly `length` columns wide. Every line is aligned within that width, so
/// left/center/right work for wrapped text too.
fn fit_lines(el: &Element, display: &str, content_cols: u32, scale: u32) -> Vec<String> {
    match &el.kind {
        // Static text is a single line; it has no reserved width or alignment and
        // anything past the edge is clipped at rasterize time (it shows in the
        // editor's overflow zone). Static text never wraps.
        ElementKind::Text { .. } => vec![display.to_string()],
        ElementKind::Variable {
            length,
            align,
            wrap,
            max_lines,
            ..
        } => {
            // The band width in characters: what was asked, capped to the columns
            // that actually fit to the right of this element at this scale.
            let avail = content_cols.saturating_sub(el.col);
            let cap = (avail / scale).max(1) as usize;
            let band = (*length as usize).clamp(1, cap);

            if *wrap {
                let mut lines = wrap_text(display, band);
                // Bound a runaway value: keep at most `max_lines` lines and mark
                // the cut with a trailing `…` on the last kept line.
                // 0 (the editor's "no limit") and absent both mean unbounded.
                if let Some(m) = max_lines.filter(|m| *m > 0) {
                    let m = m as usize;
                    if lines.len() > m {
                        lines.truncate(m);
                        if let Some(last) = lines.last_mut() {
                            let mut chars: Vec<char> = last.chars().collect();
                            if chars.len() >= band {
                                chars.truncate(band.saturating_sub(1));
                            }
                            chars.push('…');
                            *last = chars.into_iter().collect();
                        }
                    }
                }
                lines
                    .into_iter()
                    .map(|line| fit_to_width(&line, band, *align))
                    .collect()
            } else {
                vec![fit_to_width(display, band, *align)]
            }
        }
        // Image / QR are handled as raster blocks, not text lines.
        _ => vec![],
    }
}

/// Greedy word wrap to `width` columns. Words longer than `width` are hard-broken
/// (so a giant token can't run off paper), otherwise breaks fall on spaces.
fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;

    for word in s.split_whitespace() {
        let wlen = word.chars().count();
        if wlen > width {
            // Flush whatever's pending, then hard-break the oversized word.
            if cur_len > 0 {
                lines.push(std::mem::take(&mut cur));
                cur_len = 0;
            }
            let wchars: Vec<char> = word.chars().collect();
            let chunks: Vec<&[char]> = wchars.chunks(width).collect();
            for (i, chunk) in chunks.iter().enumerate() {
                if i + 1 < chunks.len() {
                    lines.push(chunk.iter().collect());
                } else {
                    // Keep the last chunk open so following words can continue it.
                    cur = chunk.iter().collect();
                    cur_len = chunk.len();
                }
            }
            continue;
        }
        let needed = if cur_len == 0 {
            wlen
        } else {
            cur_len + 1 + wlen
        };
        if needed > width {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
            cur_len = wlen;
        } else {
            if cur_len > 0 {
                cur.push(' ');
                cur_len += 1;
            }
            cur.push_str(word);
            cur_len += wlen;
        }
    }
    lines.push(cur);
    lines
}

/// Truncate or pad a string to exactly `width` characters with the given alignment.
fn fit_to_width(s: &str, width: usize, align: Align) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= width {
        return chars[..width].iter().collect();
    }
    let pad = width - chars.len();
    match align {
        Align::Left => {
            let mut out: String = chars.iter().collect();
            out.extend(std::iter::repeat_n(' ', pad));
            out
        }
        Align::Right => {
            let mut out: String = " ".repeat(pad);
            out.extend(chars.iter());
            out
        }
        Align::Center => {
            let left = pad / 2;
            let right = pad - left;
            let mut out: String = " ".repeat(left);
            out.extend(chars.iter());
            out.extend(std::iter::repeat_n(' ', right));
            out
        }
    }
}

/// Stage 2: draw placements into an RGBA raster (clipped to the printable area)
/// and encode PNG.
fn rasterize(
    doc: &TicketDoc,
    placements: &[Placement],
    blocks: &[RasterBlock],
    total_rows: u32,
    fonts: &Fonts,
) -> Result<Vec<u8>, RenderError> {
    let paper = &doc.paper;
    let cell_w = paper.cell_width_px.max(1);
    let cell_h = paper.cell_height_px.max(1);
    let cols = paper.width_chars.max(1);
    // Defense in depth: `lay_out` already bounded these against MAX_PIXELS in u64,
    // so the u32 products here cannot overflow — but re-check rather than trust it.
    let width64 = u64::from(cols) * u64::from(cell_w);
    let height64 = u64::from(total_rows.max(1)) * u64::from(cell_h);
    if width64 == 0 || height64 == 0 || width64 * height64 > MAX_PIXELS {
        return Err(RenderError::TooLarge {
            width: width64,
            height: height64,
        });
    }
    let width = width64 as u32;
    let height = height64 as u32;

    // Printable clip rectangle in pixels (inside the paper margins).
    let clip_x0 = paper.margin_left_chars * cell_w;
    let clip_x1 = width.saturating_sub(paper.margin_right_chars * cell_w);
    let clip_y0 = paper.margin_top_lines * cell_h;
    let clip_y1 = height.saturating_sub(paper.margin_bottom_lines * cell_h);

    // White RGBA canvas.
    let mut buf = vec![255u8; (width as usize) * (height as usize) * 4];

    // Raster blocks (logos / QR) first, so glyphs can sit on top if they overlap.
    let paint = |buf: &mut [u8], px: i64, py: i64| {
        if px < clip_x0 as i64
            || px >= clip_x1 as i64
            || py < clip_y0 as i64
            || py >= clip_y1 as i64
        {
            return;
        }
        let idx = ((py as usize) * (width as usize) + px as usize) * 4;
        buf[idx] = 0;
        buf[idx + 1] = 0;
        buf[idx + 2] = 0;
    };
    for b in blocks {
        for row in 0..b.h {
            for col in 0..b.w {
                if b.mask[(row * b.w + col) as usize] {
                    paint(&mut buf, b.x + col as i64, b.y + row as i64);
                }
            }
        }
    }

    for p in placements {
        if p.ch == ' ' {
            continue;
        }
        // Validated in `lay_out`, so this only errors on a truly corrupt state.
        let face = fonts
            .face(p.font.as_deref(), p.bold, p.italic)
            .map_err(RenderError::MissingFont)?;
        let px_scale = PxScale::from(paper.font_px * p.scale as f32);
        let scaled = face.as_scaled(px_scale);
        let glyph_id = face.glyph_id(p.ch);

        // The glyph occupies a scale×scale block of base cells.
        let block_w = (cell_w * p.scale) as f32;
        let block_h = (cell_h * p.scale) as f32;
        let cell_left = p.col as f32 * cell_w as f32;
        let cell_top = p.row as f32 * cell_h as f32 + p.y_off_px;

        // Center horizontally; sit on a baseline shared by every face at this scale.
        let advance = scaled.h_advance(glyph_id);
        let h_pad = (block_w - advance) / 2.0;
        let ref_scaled = fonts.reference().as_scaled(px_scale);
        let line_height = ref_scaled.ascent() - ref_scaled.descent();
        // Vertical placement of the text line inside the (taller) block.
        let top_pad = match p.valign {
            VAlign::Top => 0.0,
            VAlign::Middle => (block_h - line_height) / 2.0,
            VAlign::Bottom => block_h - line_height,
        };
        let pen_x = cell_left + h_pad;
        let baseline_y = cell_top + top_pad + ref_scaled.ascent();

        let glyph = glyph_id.with_scale_and_position(px_scale, ab_glyph::point(pen_x, baseline_y));
        if let Some(outline) = face.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                // Clip to the printable area, not just the canvas.
                if px < clip_x0 as i32
                    || px >= clip_x1 as i32
                    || py < clip_y0 as i32
                    || py >= clip_y1 as i32
                {
                    return;
                }
                let idx = ((py as usize) * (width as usize) + px as usize) * 4;
                let inv = 1.0 - coverage;
                for c in 0..3 {
                    let old = buf[idx + c] as f32;
                    buf[idx + c] = (old * inv).round() as u8;
                }
            });
        }
    }

    encode_png(width, height, &buf)
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, RenderError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| RenderError::Png(e.to_string()))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| RenderError::Png(e.to_string()))?;
    }
    Ok(out)
}
