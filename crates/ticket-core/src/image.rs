//! Monochrome image preprocessing for thermal printers.
//!
//! A logo is decoded (PNG), scaled to its target pixel box, and reduced to 1-bit
//! black/white — either by a fixed threshold (crisp for logos / line art) or
//! Floyd–Steinberg dithering (better for photos). All of this runs in the one
//! renderer, so the browser preview shows the exact 1-bit result the printer
//! gets. Everything is integer/deterministic → identical native and wasm.

use base64::{engine::general_purpose::STANDARD, Engine};

use crate::schema::ImageMode;

/// Ceiling on the PNG decoder's working memory (guards decompression bombs).
pub const MAX_DECODE_BYTES: usize = 64 * 1024 * 1024;
/// Ceiling on the source image's declared pixel count.
pub const MAX_SRC_PIXELS: u64 = 32 * 1024 * 1024;

/// Rec. 601 luma. The one place the crate turns colour into ink density.
pub fn luma(r: u8, g: u8, b: u8) -> f32 {
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

/// Composite a straight-alpha grayscale value over white (the paper).
pub fn over_white(v: f32, a: f32) -> u8 {
    (v * a + 255.0 * (1.0 - a)).round() as u8
}

/// Strip an optional `data:…;base64,` prefix and decode the payload.
pub fn decode_b64(data: &str) -> Result<Vec<u8>, String> {
    let b64 = match data.rfind("base64,") {
        Some(i) => &data[i + "base64,".len()..],
        None => data,
    };
    STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("bad base64: {e}"))
}

/// Decode a base64 PNG (optionally a `data:` URI) into an 8-bit grayscale buffer.
/// Alpha is composited over white (the paper). Returns `(gray, width, height)`.
pub fn decode_png_gray(data: &str) -> Result<(Vec<u8>, u32, u32), String> {
    decode_png_gray_bytes(&decode_b64(data)?)
}

/// [`decode_png_gray`] over raw PNG bytes, for a caller that has already decoded
/// the base64 (and, in `normalize`, sniffed the format).
pub fn decode_png_gray_bytes(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    // png 0.18's Decoder requires Read + Seek; a Cursor over the bytes provides both.
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    // Cap total decode memory so a small file with a huge IHDR (a decompression
    // bomb) can't force a giant allocation before any pixel is validated.
    decoder.set_limits(png::Limits {
        bytes: MAX_DECODE_BYTES,
    });
    let mut reader = decoder.read_info().map_err(|e| format!("png: {e}"))?;
    // Reject absurd source dimensions declared in the header, up front.
    {
        let info = reader.info();
        if u64::from(info.width) * u64::from(info.height) > MAX_SRC_PIXELS {
            return Err(format!("image too large: {}x{}", info.width, info.height));
        }
    }
    // png 0.18 returns Option here (None if the size would overflow usize).
    let out_size = reader
        .output_buffer_size()
        .ok_or_else(|| "png: output buffer size unavailable".to_string())?;
    let mut buf = vec![0u8; out_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("png: {e}"))?;
    let (w, h) = (info.width, info.height);
    let px = &buf[..info.buffer_size()];

    let n = (w as usize) * (h as usize);
    let mut gray = vec![255u8; n];
    match info.color_type {
        png::ColorType::Grayscale => {
            gray[..n.min(px.len())].copy_from_slice(&px[..n.min(px.len())])
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                let (v, a) = (px[i * 2] as f32, px[i * 2 + 1] as f32 / 255.0);
                gray[i] = over_white(v, a);
            }
        }
        png::ColorType::Rgb => {
            for i in 0..n {
                gray[i] = luma(px[i * 3], px[i * 3 + 1], px[i * 3 + 2]).round() as u8;
            }
        }
        png::ColorType::Rgba => {
            for i in 0..n {
                let a = px[i * 4 + 3] as f32 / 255.0;
                gray[i] = over_white(luma(px[i * 4], px[i * 4 + 1], px[i * 4 + 2]), a);
            }
        }
        png::ColorType::Indexed => return Err("indexed png not expanded".into()),
    }
    Ok((gray, w, h))
}

/// Encode a 1-bit ink mask (`true` = black, as produced by [`to_bw`]) as a
/// grayscale bit-depth-1 PNG — the most compact form the renderer can read back
/// without losing a single pixel. What [`crate::normalize_images`] rewrites a
/// document's embedded images into.
#[cfg(feature = "normalize")]
pub fn encode_1bit_png(mask: &[bool], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let n = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| format!("dimensions overflow: {w}x{h}"))?;
    if mask.len() != n {
        return Err(format!(
            "mask has {} px, expected {n} ({w}x{h})",
            mask.len()
        ));
    }
    // Bit set = white. Ink leaves the bit clear, which is what the 1-bit
    // grayscale colour type means (0 = black at depth 1).
    let row_bytes = (w as usize).div_ceil(8);
    let mut packed = vec![0u8; row_bytes * h as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            if !mask[y * w as usize + x] {
                packed[y * row_bytes + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::One);
        encoder.set_compression(png::Compression::High);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("png encode: {e}"))?;
        writer
            .write_image_data(&packed)
            .map_err(|e| format!("png encode: {e}"))?;
    }
    Ok(out)
}

/// Bilinear resize of a grayscale buffer to `dw × dh`.
pub fn resize_gray(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let (sw, sh, dw, dh) = (
        sw as usize,
        sh as usize,
        dw.max(1) as usize,
        dh.max(1) as usize,
    );
    if sw == 0 || sh == 0 {
        return vec![255u8; dw * dh];
    }
    let mut out = vec![0u8; dw * dh];
    let sx = sw as f32 / dw as f32;
    let sy = sh as f32 / dh as f32;
    for y in 0..dh {
        let fy = ((y as f32 + 0.5) * sy - 0.5).max(0.0);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let wy = fy - y0 as f32;
        for x in 0..dw {
            let fx = ((x as f32 + 0.5) * sx - 0.5).max(0.0);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let wx = fx - x0 as f32;
            let p = |xx: usize, yy: usize| src[yy * sw + xx] as f32;
            let top = p(x0, y0) * (1.0 - wx) + p(x1, y0) * wx;
            let bot = p(x0, y1) * (1.0 - wx) + p(x1, y1) * wx;
            out[y * dw + x] = (top * (1.0 - wy) + bot * wy).round() as u8;
        }
    }
    out
}

/// Reduce a grayscale buffer to a 1-bit mask (`true` = black ink).
pub fn to_bw(gray: &[u8], w: u32, h: u32, mode: ImageMode) -> Vec<bool> {
    let (w, h) = (w as usize, h as usize);
    match mode {
        ImageMode::Threshold { level } => gray.iter().map(|&v| v < level).collect(),
        ImageMode::Dither => {
            // Floyd–Steinberg error diffusion over a working float buffer.
            let mut buf: Vec<f32> = gray.iter().map(|&v| v as f32).collect();
            let mut out = vec![false; w * h];
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let old = buf[i];
                    let black = old < 128.0;
                    out[i] = black;
                    let err = old - if black { 0.0 } else { 255.0 };
                    let mut add = |xx: isize, yy: isize, f: f32| {
                        if xx >= 0 && (xx as usize) < w && (yy as usize) < h {
                            buf[(yy as usize) * w + xx as usize] += err * f;
                        }
                    };
                    add(x as isize + 1, y as isize, 7.0 / 16.0);
                    add(x as isize - 1, y as isize + 1, 3.0 / 16.0);
                    add(x as isize, y as isize + 1, 5.0 / 16.0);
                    add(x as isize + 1, y as isize + 1, 1.0 / 16.0);
                }
            }
            out
        }
    }
}
