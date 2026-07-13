//! `normalize_images` must be invisible to the renderer.
//!
//! The contract is not "close enough" — it is that a normalized document renders
//! to the *same bytes* as the document it came from. Every test here that can
//! assert that, does, via `render_png`: it is the only assertion that actually
//! protects preview/print parity, and it would catch any future drift between
//! the baking here and the rendering in `render.rs` (a different resize filter,
//! a different luma) that a size-only test would sail straight past.

#![cfg(feature = "normalize")]

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use ticket_core::{normalize_images, render_png, TicketDoc};

/// An RGBA source with a gradient, some transparency and some colour — enough
/// that a wrong luma, a wrong alpha composite or a wrong resize all show up as
/// different ink.
fn source_rgba(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let a = if x + y < 6 { 90u8 } else { 255 };
        image::Rgba([
            (x * 255 / w.max(1)) as u8,
            (y * 255 / h.max(1)) as u8,
            ((x + y) * 8 % 256) as u8,
            a,
        ])
    })
}

/// A source that deflate cannot help with — the worst case for the size claim,
/// and the closest thing to a photograph. Deterministic (an LCG), so the test
/// asserts on a fixed number of bytes rather than on luck.
fn source_noise(w: u32, h: u32) -> image::RgbaImage {
    let mut seed: u32 = 0x5eed_1234;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 24) as u8
    };
    image::RgbaImage::from_fn(w, h, |_, _| image::Rgba([next(), next(), next(), 255]))
}

/// The PNG fixture is encoded with `png`, not `image` — `image` is pulled in
/// without its PNG codec on purpose, so that the crate has exactly one PNG
/// decoder. The tests keep to the same rule.
fn png_data_uri(img: &image::RgbaImage) -> String {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, img.width(), img.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer.write_image_data(img.as_raw()).expect("png data");
    }
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

fn webp_data_uri(img: &image::RgbaImage) -> String {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut bytes, image::ImageFormat::WebP)
        .expect("encode webp fixture");
    format!(
        "data:image/webp;base64,{}",
        STANDARD.encode(bytes.into_inner())
    )
}

fn doc_with_image(data: String, mode: Value) -> TicketDoc {
    serde_json::from_value(json!({
        "version": 1,
        "paper": { "width_chars": 32 },
        "elements": [
            { "id": "hdr", "row": 0, "col": 0, "type": "text", "content": "TICKET" },
            { "id": "logo", "row": 1, "col": 2, "type": "image",
              "data": data, "w": 10, "h": 4, "mode": mode },
        ]
    }))
    .expect("fixture doc")
}

fn image_data(doc: &TicketDoc) -> String {
    image_data_of(doc, "logo")
}

fn image_data_of(doc: &TicketDoc, id: &str) -> String {
    let json = serde_json::to_value(doc).expect("doc to json");
    json["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .find(|e| e["id"] == id)
        .expect("element")["data"]
        .as_str()
        .expect("data")
        .to_string()
}

fn render(doc: &TicketDoc) -> Vec<u8> {
    render_png(doc, &Value::Null).expect("render")
}

#[test]
fn normalized_png_renders_to_the_same_bytes() {
    let src = png_data_uri(&source_rgba(97, 61));
    let original = doc_with_image(src, json!({ "kind": "threshold", "level": 200 }));

    let mut normalized = original.clone();
    normalize_images(&mut normalized).expect("normalize");

    assert_eq!(
        render(&original),
        render(&normalized),
        "a normalized document must render to the exact same PNG"
    );
}

#[test]
fn normalized_dither_renders_to_the_same_bytes() {
    // Floyd-Steinberg is the mode most likely to break under re-application:
    // it only stays idempotent because an already-binary input diffuses zero
    // error. Worth its own test.
    let src = png_data_uri(&source_rgba(97, 61));
    let original = doc_with_image(src, json!({ "kind": "dither" }));

    let mut normalized = original.clone();
    normalize_images(&mut normalized).expect("normalize");

    assert_eq!(render(&original), render(&normalized));
}

#[test]
fn normalizing_twice_changes_nothing() {
    let src = png_data_uri(&source_rgba(97, 61));
    let mut doc = doc_with_image(src, json!({ "kind": "threshold", "level": 128 }));

    normalize_images(&mut doc).expect("first");
    let once = image_data(&doc);

    let stats = normalize_images(&mut doc).expect("second");
    assert_eq!(image_data(&doc), once);
    assert_eq!(stats.bytes_before, stats.bytes_after);
}

#[test]
fn a_webp_source_bakes_to_the_same_ink_as_the_png_of_it() {
    // The renderer cannot decode WebP at all — it draws a placeholder frame. So
    // this is also the regression test for "a WebP logo silently prints as an
    // empty box": after normalization the renderer only ever sees PNG.
    let src = source_rgba(97, 61);
    let mode = json!({ "kind": "threshold", "level": 200 });

    let mut from_webp = doc_with_image(webp_data_uri(&src), mode.clone());
    let mut from_png = doc_with_image(png_data_uri(&src), mode);

    normalize_images(&mut from_webp).expect("normalize webp");
    normalize_images(&mut from_png).expect("normalize png");

    assert!(image_data(&from_webp).starts_with("data:image/png;base64,"));
    assert_eq!(
        render(&from_webp),
        render(&from_png),
        "the same pixels must bake to the same ink whatever container they arrived in"
    );
}

#[test]
fn a_webp_source_is_the_one_case_the_render_is_allowed_to_change() {
    // Everywhere else, normalizing must be invisible. Here it must NOT be: the
    // renderer was drawing a placeholder frame where the logo should be, and the
    // whole point is that it now draws the logo. Pinned so that a future change
    // cannot quietly restore the empty box.
    let src = source_rgba(97, 61);
    let original = doc_with_image(
        webp_data_uri(&src),
        json!({ "kind": "threshold", "level": 200 }),
    );

    let mut normalized = original.clone();
    normalize_images(&mut normalized).expect("normalize");

    assert_ne!(
        render(&original),
        render(&normalized),
        "an undecodable source renders as a placeholder until it is normalized"
    );
}

#[test]
fn the_result_is_bounded_by_the_target_box_not_by_the_source() {
    // The invariant worth pinning: whatever arrives — a 4 MP camera JPEG, a
    // lossless RGBA export — what leaves is one bit per pixel of the *target*
    // box. A high-entropy source is the adversarial case, because it is the one
    // that cannot lean on deflate.
    let mut doc = doc_with_image(
        png_data_uri(&source_noise(600, 400)),
        json!({ "kind": "threshold", "level": 128 }),
    );

    let stats = normalize_images(&mut doc).expect("normalize");

    // 10 x 4 cells at the default 12 x 22 px cell = 120 x 88 px.
    let raster_bytes = (120 * 88) / 8;
    assert_eq!(stats.images, 1);
    assert!(
        stats.bytes_after < raster_bytes * 2,
        "a {}x{} px target should not cost {} bytes",
        120,
        88,
        stats.bytes_after
    );
    assert!(
        stats.bytes_after * 20 < stats.bytes_before,
        "expected a large reduction on a photographic source, got {} -> {} bytes",
        stats.bytes_before,
        stats.bytes_after
    );
}

#[test]
fn a_from_variable_image_is_left_alone() {
    // Its bytes arrive at render time, so there is nothing to bake — and the
    // path must survive being handed a variable path instead of base64.
    let mut doc: TicketDoc = serde_json::from_value(json!({
        "version": 1,
        "paper": { "width_chars": 32 },
        "elements": [
            { "id": "sig", "row": 0, "col": 0, "type": "image",
              "data": "sale.signature", "from_variable": true, "w": 10, "h": 4 },
        ]
    }))
    .expect("fixture doc");

    let stats = normalize_images(&mut doc).expect("normalize");

    assert_eq!(stats.images, 0);
    assert_eq!(image_data_of(&doc, "sig"), "sale.signature");
}

#[test]
fn an_undecodable_image_names_the_element() {
    // The renderer would draw a placeholder frame here. A save-time check wants
    // to reject the logo instead, and to say which one.
    let mut doc = doc_with_image(
        "data:image/png;base64,bm90LWFuLWltYWdl".to_string(),
        json!({ "kind": "threshold", "level": 128 }),
    );

    let err = normalize_images(&mut doc).expect_err("must not silently pass a bad image");

    assert_eq!(err.element_id, "logo");
    assert!(err.to_string().contains("logo"), "{err}");
}
