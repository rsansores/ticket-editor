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
use crate::font::FontSet;
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
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Font(e) => write!(f, "invalid font: {e}"),
            RenderError::Png(e) => write!(f, "png encode failed: {e}"),
            RenderError::TooLarge { width, height } => {
                write!(f, "rendered image too large: {width}x{height}")
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// Hard ceiling on any single raster (the whole canvas, or one logo/QR block),
/// in pixels (~64 megapixels). Bounds memory against adversarial documents.
const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// Hard ceiling on loop iterations, so a giant data array can't explode the
/// placement list before the pixel guard runs. No real ticket loops this much.
const MAX_LOOP: u32 = 100_000;

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

/// Render a document to PNG bytes.
pub fn render_png(doc: &TicketDoc, variables: &Value) -> Result<Vec<u8>, RenderError> {
    let fonts = FontSet::load().map_err(|e| RenderError::Font(e.to_string()))?;
    let (placements, blocks, total_rows) = lay_out(doc, variables)?;
    rasterize(doc, &placements, &blocks, total_rows, &fonts)
}

/// A region resolved against the data: how many times it renders and how many
/// rows it adds to (or removes from) everything below it.
struct RegionPlan<'a> {
    region: &'a crate::schema::Region,
    height: u32,
    /// Loop item count (1 for a plain conditional band that is shown, 0 if the
    /// band is collapsed — condition false or an empty/absent loop source).
    count: u32,
    /// Extra rows added after `end_row`: `(count-1)*height`, or `-height` when the
    /// band collapses.
    delta_after: i64,
}

/// Stage 1: apply the flow transform (loops expand, conditional bands collapse,
/// content below reflows) and expand every visible element into character
/// placements. Returns the placements and the total grid rows needed.
#[allow(clippy::type_complexity)]
fn lay_out(
    doc: &TicketDoc,
    variables: &Value,
) -> Result<(Vec<Placement>, Vec<RasterBlock>, u32), RenderError> {
    let paper = &doc.paper;
    let content_cols = paper.content_cols();
    let ml = paper.margin_left_chars;
    let mt = paper.margin_top_lines;
    let cell_w = paper.cell_width_px.max(1);
    let cell_h = paper.cell_height_px.max(1);
    let cell_w64 = u64::from(cell_w);
    let cell_h64 = u64::from(cell_h);

    // Resolve each band against the data (sorted by start so offsets accumulate).
    let mut regions: Vec<&crate::schema::Region> = doc.regions.iter().collect();
    regions.sort_by_key(|r| r.start_row);
    let plans: Vec<RegionPlan> = regions
        .iter()
        .map(|region| {
            let height = region.end_row.saturating_sub(region.start_row).max(1);
            let enabled = region
                .condition
                .as_ref()
                .map(|c| data::eval_condition(None, variables, c))
                .unwrap_or(true);
            let count = if !enabled {
                0
            } else if let Some(src) = &region.source {
                match data::resolve(variables, src) {
                    // Cap iterations so a giant data array can't explode memory.
                    Some(Value::Array(a)) => a.len().min(MAX_LOOP as usize) as u32,
                    _ => 0,
                }
            } else {
                1
            };
            let delta_after = if count == 0 {
                -i64::from(height)
            } else {
                (i64::from(count) - 1) * i64::from(height)
            };
            RegionPlan { region, height, count, delta_after }
        })
        .collect();

    // Rows added/removed by every band whose end_row is at or above `row`.
    let offset_before = |row: u32| -> i64 {
        plans
            .iter()
            .filter(|p| p.region.end_row <= row)
            .map(|p| p.delta_after)
            .sum()
    };
    // The band (if any) that captures a given design row.
    let region_of = |row: u32| -> Option<&RegionPlan> {
        plans
            .iter()
            .find(|p| row >= p.region.start_row && row < p.region.end_row)
    };

    let mut placements = Vec::new();
    let mut blocks: Vec<RasterBlock> = Vec::new();
    let mut max_bottom: i64 = 0; // lowest content row reached (absolute), exclusive

    let mut emit =
        |el: &Element, loop_ctx: data::LoopCtx, base_row: i64| -> Result<(), RenderError> {
            // Per-element condition (evaluated in the current context): hide in place.
            if let Some(c) = &el.condition {
                if !data::eval_condition(loop_ctx, variables, c) {
                    return Ok(());
                }
            }
            let y_off_px = el.y_offset * cell_h as f32;
            match &el.kind {
                ElementKind::Image { data, w, h, mode } => {
                    // Bound the block in u64 BEFORE decoding/allocating (adversarial w/h).
                    let w_px64 = u64::from((*w).max(1)) * cell_w64;
                    let h_px64 = u64::from((*h).max(1)) * cell_h64;
                    if w_px64 > MAX_PIXELS || h_px64 > MAX_PIXELS || w_px64 * h_px64 > MAX_PIXELS {
                        return Err(RenderError::TooLarge { width: w_px64, height: h_px64 });
                    }
                    let (w_px, h_px) = (w_px64 as u32, h_px64 as u32);
                    let mask = match image::decode_png_gray(data) {
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
                }
                ElementKind::Qr { value, from_variable, size } => {
                    let text = if *from_variable {
                        match data::resolve_loop(loop_ctx, variables, value) {
                            Some(v) => data::value_to_string(v),
                            None => data::fake_for(value),
                        }
                    } else {
                        value.clone()
                    };
                    let side64 = u64::from((*size).max(1)) * cell_w64; // square (scannable)
                    if side64 > MAX_PIXELS || side64 * side64 > MAX_PIXELS {
                        return Err(RenderError::TooLarge { width: side64, height: side64 });
                    }
                    let side = side64 as u32;
                    let rows = side.div_ceil(cell_h);
                    // A value too long to encode falls back to a visible placeholder.
                    let mask =
                        qr_mask(&text, side).unwrap_or_else(|| placeholder_mask(side, side));
                    blocks.push(RasterBlock {
                        x: i64::from(ml + el.col) * i64::from(cell_w),
                        y: base_row * i64::from(cell_h) + y_off_px as i64,
                        w: side,
                        h: side,
                        mask,
                    });
                    max_bottom = max_bottom.max(base_row + i64::from(rows));
                }
                _ => {
                    let scale = el.style.scale_clamped();
                    let display = resolve_display(el, loop_ctx, variables);
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
                            });
                        }
                    }
                    max_bottom = max_bottom.max(base_row + i64::from(lines.len() as u32 * scale));
                }
            }
            Ok(())
        };

    for el in &doc.elements {
        match region_of(el.row) {
            Some(plan) if plan.region.source.is_some() => {
                // Loop band: render once per item, offset by the band height.
                if plan.count == 0 {
                    continue;
                }
                let Some(src) = plan.region.source.as_deref() else {
                    continue;
                };
                let items = data::resolve(variables, src);
                let base = mt as i64 + i64::from(el.row) + offset_before(plan.region.start_row);
                for i in 0..plan.count {
                    let item = items
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.get(i as usize));
                    let loop_ctx = item.map(|it| (src, i as usize, it));
                    emit(el, loop_ctx, base + i64::from(i) * i64::from(plan.height))?;
                }
            }
            Some(plan) => {
                // Conditional (non-loop) band: render only when shown.
                if plan.count == 0 {
                    continue;
                }
                emit(el, None, mt as i64 + i64::from(el.row) + offset_before(el.row))?;
            }
            None => {
                emit(el, None, mt as i64 + i64::from(el.row) + offset_before(el.row))?;
            }
        }
    }

    // Total height honours trailing whitespace (min_rows) after the net flow delta.
    let total_delta: i64 = plans.iter().map(|p| p.delta_after).sum();
    let design_floor = i64::from(mt) + i64::from(paper.min_rows.max(1)) + total_delta;
    let content_bottom = max_bottom.max(design_floor).max(i64::from(mt) + 1);
    let total_rows_i =
        (content_bottom + i64::from(paper.margin_bottom_lines)).clamp(1, i64::from(u32::MAX));

    // Final canvas bound in u64 — no u32 overflow before the check.
    let width64 = u64::from(paper.width_chars.max(1)) * cell_w64;
    let height64 = total_rows_i as u64 * cell_h64;
    if width64 == 0 || height64 == 0 || width64 > MAX_PIXELS || height64 > MAX_PIXELS
        || width64 * height64 > MAX_PIXELS
    {
        return Err(RenderError::TooLarge { width: width64, height: height64 });
    }

    Ok((placements, blocks, total_rows_i as u32))
}

/// A QR matrix scaled into a `side × side` pixel mask with a 4-module quiet zone.
/// Returns `None` when the value can't be encoded (too long) so the caller can
/// draw a visible placeholder instead of a silent blank. `side` is assumed
/// already bounded by the caller against `MAX_PIXELS`.
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

/// Resolve an element to its final display string (variable lookup in the given
/// loop context / faker, then number or date formatting).
fn resolve_display(el: &Element, loop_ctx: data::LoopCtx, root: &Value) -> String {
    match &el.kind {
        ElementKind::Text { content } => content.clone(),
        ElementKind::Variable {
            path,
            number,
            date_format,
            ..
        } => {
            let raw = match data::resolve_loop(loop_ctx, root, path) {
                Some(v) => data::value_to_string(v),
                None => data::fake_for(path),
            };
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
            ..
        } => {
            // The band width in characters: what was asked, capped to the columns
            // that actually fit to the right of this element at this scale.
            let avail = content_cols.saturating_sub(el.col);
            let cap = (avail / scale).max(1) as usize;
            let band = (*length as usize).clamp(1, cap);

            if *wrap {
                wrap_text(display, band)
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
        let needed = if cur_len == 0 { wlen } else { cur_len + 1 + wlen };
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
    fonts: &FontSet,
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
        return Err(RenderError::TooLarge { width: width64, height: height64 });
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
        if px < clip_x0 as i64 || px >= clip_x1 as i64 || py < clip_y0 as i64 || py >= clip_y1 as i64
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
        let face = fonts.face(p.bold, p.italic);
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
        let ref_scaled = fonts.regular.as_scaled(px_scale);
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
