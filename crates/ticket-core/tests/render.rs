use serde_json::json;
use ticket_core::{render_png, RenderError, TicketDoc};

fn sample_doc() -> TicketDoc {
    serde_json::from_value(json!({
        "version": 1,
        "paper": {
            "width_chars": 32,
            "margin_left_chars": 1,
            "margin_right_chars": 1,
            "margin_top_lines": 1,
            "margin_bottom_lines": 1
        },
        "elements": [
            { "id": "title", "row": 0, "col": 8, "type": "text", "content": "MI TICKET", "style": { "bold": true } },
            { "id": "lbl",   "row": 2, "col": 0, "type": "text", "content": "TOTAL:" },
            { "id": "amt",   "row": 2, "col": 20, "type": "variable", "path": "sale.total_amount", "length": 10, "align": "right" }
        ]
    }))
    .unwrap()
}

#[test]
fn renders_valid_png() {
    let doc = sample_doc();
    let png = render_png(&doc, &serde_json::Value::Null).unwrap();
    // PNG magic number.
    assert_eq!(
        &png[0..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
    );
    assert!(png.len() > 100, "png suspiciously small");
}

#[test]
fn is_deterministic() {
    let doc = sample_doc();
    let a = render_png(&doc, &serde_json::Value::Null).unwrap();
    let b = render_png(&doc, &serde_json::Value::Null).unwrap();
    assert_eq!(
        a, b,
        "same input must yield identical bytes (parity depends on this)"
    );
}

#[test]
fn real_data_beats_fake() {
    let doc = sample_doc();
    let with_data = render_png(&doc, &json!({ "sale": { "total_amount": 100.23 } })).unwrap();
    let with_fake = render_png(&doc, &serde_json::Value::Null).unwrap();
    // The amount slot differs, so the rasters must differ.
    assert_ne!(with_data, with_fake);
}

#[test]
fn scale_grows_the_ticket() {
    // A 4x title must produce a taller image than a 1x one.
    let base: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 40 },
        "elements": [{ "id": "t", "row": 0, "col": 0, "type": "text", "content": "HI" }]
    }))
    .unwrap();
    let mut big = base.clone();
    if let Some(e) = big.elements.get_mut(0) {
        e.style.scale = 4;
    }
    let small_png = render_png(&base, &serde_json::Value::Null).unwrap();
    let big_png = render_png(&big, &serde_json::Value::Null).unwrap();
    // Different rasters; the 4x one is materially larger on disk.
    assert_ne!(small_png, big_png);
    assert!(big_png.len() > small_png.len());
}

#[test]
fn wrap_flows_onto_multiple_lines() {
    // `length` is the band WIDTH. A value far wider than the band flows onto
    // extra lines when wrapping, and is truncated to a single line when not.
    let long = "una frase larga que claramente no cabe en pocos caracteres";
    let doc = |wrap: bool| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 1, "paper": { "width_chars": 24 },
            "elements": [{ "id": "v", "row": 0, "col": 0, "type": "variable",
                           "path": "note", "length": 12, "wrap": wrap }]
        }))
        .unwrap()
    };
    let data = json!({ "note": long });
    let wrapped = render_png(&doc(true), &data).unwrap();
    let clipped = render_png(&doc(false), &data).unwrap();
    assert_ne!(wrapped, clipped);
    assert!(
        wrapped.len() > clipped.len(),
        "wrapped should occupy more rows"
    );
}

#[test]
fn alignment_affects_wrapped_text() {
    // Left vs right alignment must change wrapped output (each line is padded
    // within the band width).
    let doc = |align: &str| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 1, "paper": { "width_chars": 24 },
            "elements": [{ "id": "v", "row": 0, "col": 0, "type": "variable",
                           "path": "note", "length": 16, "wrap": true, "align": align }]
        }))
        .unwrap()
    };
    let data = json!({ "note": "alfa beta gamma delta epsilon" });
    let left = render_png(&doc("left"), &data).unwrap();
    let right = render_png(&doc("right"), &data).unwrap();
    assert_ne!(left, right, "alignment must change wrapped output");
}

#[test]
fn number_and_date_formatting_apply() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 40 },
        "elements": [
            { "id": "n", "row": 0, "col": 0, "type": "variable", "path": "amt", "length": 14,
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            { "id": "d", "row": 1, "col": 0, "type": "variable", "path": "when", "length": 14,
              "date_format": "DD/MM/YYYY" }
        ]
    }))
    .unwrap();
    let formatted = render_png(
        &doc,
        &json!({ "amt": 1234567.899, "when": "2030-01-02 03:04:05" }),
    )
    .unwrap();
    let raw = render_png(&doc, &json!({ "amt": "x", "when": "x" })).unwrap();
    // Formatted numbers/dates differ from the raw fallback rendering.
    assert_ne!(formatted, raw);
    assert_eq!(&formatted[0..4], &[0x89, b'P', b'N', b'G']);
}

#[test]
fn valign_and_offset_change_scaled_placement() {
    // A 3x title at top vs bottom of its block must differ, and a y_offset nudge
    // must move it too.
    let doc = |valign: &str, y: f32| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 1, "paper": { "width_chars": 24, "min_rows": 6 },
            "elements": [{ "id": "t", "row": 0, "col": 0, "type": "text", "content": "HI",
                           "y_offset": y, "style": { "scale": 3, "valign": valign } }]
        }))
        .unwrap()
    };
    let top = render_png(&doc("top", 0.0), &serde_json::Value::Null).unwrap();
    let bottom = render_png(&doc("bottom", 0.0), &serde_json::Value::Null).unwrap();
    let nudged = render_png(&doc("top", 0.5), &serde_json::Value::Null).unwrap();
    assert_ne!(top, bottom, "valign top vs bottom must differ");
    assert_ne!(top, nudged, "y_offset nudge must move the glyph");
}

#[test]
fn loop_repeats_and_flows_content_below() {
    // A loop band over `items` (rows 0..1) with a total line at row 1. More items
    // => taller ticket, and the total must be pushed below all repetitions.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 24 },
        "regions": [{ "id": "r", "start_row": 0, "end_row": 1, "source": "cart" }],
        "elements": [
            { "id": "row", "row": 0, "col": 0, "type": "variable", "path": "name", "length": 12 },
            { "id": "tot", "row": 1, "col": 0, "type": "text", "content": "TOTAL" }
        ]
    }))
    .unwrap();
    let data = |n: usize| json!({ "cart": (0..n).map(|i| json!({ "name": format!("item{i}") })).collect::<Vec<_>>() });
    let two = render_png(&doc, &data(2)).unwrap();
    let five = render_png(&doc, &data(5)).unwrap();
    assert_ne!(two, five);
    assert!(
        five.len() > two.len(),
        "more loop items should make a taller ticket"
    );
}

#[test]
fn loop_absolute_path_gets_index_substituted() {
    // The editor stores the tree's absolute path (`cart.0.name`); inside a loop
    // the index is substituted per iteration, so each row shows its own item —
    // NOT item 0 repeated. Compare against a doc that (wrongly) pins index 0 by
    // making all items identical vs distinct.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 24 },
        "regions": [{ "id": "r", "start_row": 0, "end_row": 1, "source": "cart" }],
        "elements": [
            { "id": "row", "row": 0, "col": 0, "type": "variable", "path": "cart.0.name", "length": 12 }
        ]
    }))
    .unwrap();
    let distinct = render_png(
        &doc,
        &json!({ "cart": [{ "name": "AAA" }, { "name": "BBB" }] }),
    )
    .unwrap();
    let same = render_png(
        &doc,
        &json!({ "cart": [{ "name": "AAA" }, { "name": "AAA" }] }),
    )
    .unwrap();
    // If index substitution works, distinct items render differently than two
    // identical items; if it were pinned to index 0, both would show "AAA","AAA".
    assert_ne!(
        distinct, same,
        "each loop row must show its own item via index substitution"
    );
}

#[test]
fn conditional_region_collapses_when_false() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 24 },
        "regions": [{ "id": "disc", "start_row": 0, "end_row": 1,
                      "condition": { "var": "discount", "op": "gt", "value": "0" } }],
        "elements": [
            { "id": "d", "row": 0, "col": 0, "type": "text", "content": "DESCUENTO" },
            { "id": "t", "row": 1, "col": 0, "type": "text", "content": "TOTAL" }
        ]
    }))
    .unwrap();
    let shown = render_png(&doc, &json!({ "discount": 10 })).unwrap();
    let hidden = render_png(&doc, &json!({ "discount": 0 })).unwrap();
    assert_ne!(
        shown, hidden,
        "collapsing the discount band must change the render"
    );
    assert!(shown.len() > hidden.len(), "hidden band => shorter ticket");
}

#[test]
fn qr_renders_and_varies_with_content() {
    let doc = |val: &str| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 1, "paper": { "width_chars": 20 },
            "elements": [{ "id": "q", "row": 0, "col": 0, "type": "qr",
                           "value": val, "from_variable": false, "size": 12 }]
        }))
        .unwrap()
    };
    let a = render_png(&doc("https://example.com/a"), &serde_json::Value::Null).unwrap();
    let b = render_png(&doc("https://example.com/b"), &serde_json::Value::Null).unwrap();
    assert_eq!(&a[0..4], &[0x89, b'P', b'N', b'G']);
    assert_ne!(a, b, "different QR content must produce a different image");
}

#[test]
fn qr_from_variable_resolves() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 20 },
        "elements": [{ "id": "q", "row": 0, "col": 0, "type": "qr",
                       "value": "url", "from_variable": true, "size": 12 }]
    }))
    .unwrap();
    let x = render_png(&doc, &json!({ "url": "AAA" })).unwrap();
    let y = render_png(&doc, &json!({ "url": "BBB" })).unwrap();
    assert_ne!(x, y, "QR bound to a variable must reflect the value");
}

#[test]
fn qr_from_calculated_variable_matches_the_literal_url() {
    // A calculated variable joins a base URL with lat/lng; a QR bound to it must
    // render byte-for-byte identically to the same URL typed as a literal QR.
    // This is the whole point of computed values: author once, resolve by path.
    let data = json!({ "ru": { "lat": "19.4326", "lng": "-99.1332" } });
    let literal_url = "https://maps.google.com/?q=19.4326,-99.1332";

    let computed_doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20 },
        "computed": [{
            "name": "maps_url",
            "formula": "concat(\"https://maps.google.com/?q=\", ru.lat, \",\", ru.lng)"
        }],
        "elements": [{ "id": "q", "row": 0, "col": 0, "type": "qr",
                       "value": "calc.maps_url", "from_variable": true, "size": 12 }]
    }))
    .unwrap();
    let literal_doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20 },
        "elements": [{ "id": "q", "row": 0, "col": 0, "type": "qr",
                       "value": literal_url, "from_variable": false, "size": 12 }]
    }))
    .unwrap();

    let from_calc = render_png(&computed_doc, &data).unwrap();
    let from_literal = render_png(&literal_doc, &serde_json::Value::Null).unwrap();
    assert_eq!(
        from_calc, from_literal,
        "a QR bound to calc.maps_url must equal the same URL as a literal QR"
    );
}

#[test]
fn computed_arithmetic_feeds_a_variable_element() {
    // total = subtotal + tax, shown through a Variable element with formatting.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "computed": [{ "name": "total", "formula": "subtotal + tax" }],
        "elements": [{ "id": "t", "row": 0, "col": 0, "type": "variable", "path": "calc.total",
                       "length": 12, "align": "right",
                       "number": { "decimals": 2, "rounding": "half_up", "thousands": true } }]
    }))
    .unwrap();
    // 100 + 16 = 116 renders differently than 100 + 4 = 104.
    let a = render_png(&doc, &json!({ "subtotal": 100, "tax": 16 })).unwrap();
    let b = render_png(&doc, &json!({ "subtotal": 100, "tax": 4 })).unwrap();
    assert_ne!(a, b, "the computed total must reflect its operands");
    assert_eq!(&a[0..4], &[0x89, b'P', b'N', b'G']);
}

#[test]
fn computed_conditional_aggregate_totals_by_category() {
    // A POS "cut": cash_total = sumif over movements where payment == "CASH".
    // The footer shows it as a Variable element referencing calc.cash_total.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 32 },
        "computed": [
            { "name": "cash_total", "formula": "sumif(sale.movements, payment == \"CASH\", qty)" },
            { "name": "sales_line", "formula": "concat(count(sale.movements), \" movements\")" }
        ],
        "elements": [
            { "id": "c", "row": 0, "col": 0, "type": "variable", "path": "calc.cash_total",
              "length": 12, "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            { "id": "n", "row": 1, "col": 0, "type": "variable", "path": "calc.sales_line", "length": 20 }
        ]
    }))
    .unwrap();
    let data = json!({ "sale": { "movements": [
        { "payment": "CASH", "qty": 10 },
        { "payment": "CARD", "qty": 99 },
        { "payment": "CASH", "qty": 5 }
    ]}});
    // Changing a CARD row must NOT change cash_total; changing a CASH row must.
    let base = render_png(&doc, &data).unwrap();
    let card_changed = render_png(&doc, &json!({ "sale": { "movements": [
        { "payment": "CASH", "qty": 10 }, { "payment": "CARD", "qty": 1 }, { "payment": "CASH", "qty": 5 }
    ]}})).unwrap();
    let cash_changed = render_png(&doc, &json!({ "sale": { "movements": [
        { "payment": "CASH", "qty": 10 }, { "payment": "CARD", "qty": 99 }, { "payment": "CASH", "qty": 7 }
    ]}})).unwrap();
    assert_eq!(
        base, card_changed,
        "a CARD movement must not affect the cash total"
    );
    assert_ne!(
        base, cash_changed,
        "a CASH movement must affect the cash total"
    );
}

#[test]
fn adversarial_inputs_do_not_panic() {
    // Absurd decimals must clamp, not overflow/divide-by-zero.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 30 },
        "elements": [{ "id": "n", "row": 0, "col": 0, "type": "variable", "path": "x", "length": 20,
                       "number": { "decimals": 200, "rounding": "half_up", "thousands": true } }]
    }))
    .unwrap();
    assert!(render_png(&doc, &json!({ "x": 12.5 })).is_ok());

    // A giant image / QR / paper must return TooLarge, not OOM/panic.
    let big_img: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 30 },
        "elements": [{ "id": "i", "row": 0, "col": 0, "type": "image", "data": "x", "w": 100000, "h": 100000 }]
    }))
    .unwrap();
    assert!(matches!(
        render_png(&big_img, &serde_json::Value::Null),
        Err(RenderError::TooLarge { .. })
    ));

    let big_qr: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 30 },
        "elements": [{ "id": "q", "row": 0, "col": 0, "type": "qr", "value": "hi", "size": 100000 }]
    }))
    .unwrap();
    assert!(matches!(
        render_png(&big_qr, &serde_json::Value::Null),
        Err(RenderError::TooLarge { .. })
    ));

    let huge_paper: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 4000000000u32 }
    }))
    .unwrap();
    assert!(matches!(
        render_png(&huge_paper, &serde_json::Value::Null),
        Err(RenderError::TooLarge { .. })
    ));
}

#[test]
fn image_from_variable_resolves_and_falls_back_to_placeholder() {
    // A dynamic image (e.g. a signature) resolves its base64 from a variable; a
    // missing source draws the placeholder frame, and the two must differ.
    // A valid 8x8 checkerboard PNG (base64), distinct from the placeholder frame.
    let png_8x8 = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAF0lEQVR42mNgYGD4//8/FhK7KAQMPh0AXXNfoWyFCAcAAAAASUVORK5CYII=";
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20 },
        "elements": [{ "id": "img", "row": 0, "col": 0, "type": "image",
                       "data": "sale.signature", "from_variable": true, "w": 8, "h": 4 }]
    }))
    .unwrap();
    let resolved = render_png(&doc, &json!({ "sale": { "signature": png_8x8 } })).unwrap();
    let missing = render_png(&doc, &serde_json::Value::Null).unwrap(); // → placeholder
    assert_eq!(&resolved[0..4], &[0x89, b'P', b'N', b'G']);
    assert_ne!(
        resolved, missing,
        "a resolved image must differ from the placeholder"
    );
}

#[test]
fn bad_image_bytes_render_placeholder_not_error() {
    // Undecodable base64/PNG draws a placeholder frame — still a valid PNG.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 1, "paper": { "width_chars": 30 },
        "elements": [{ "id": "i", "row": 0, "col": 0, "type": "image", "data": "not-a-png", "w": 10, "h": 4 }]
    }))
    .unwrap();
    let png = render_png(&doc, &serde_json::Value::Null).unwrap();
    assert_eq!(&png[0..4], &[0x89, b'P', b'N', b'G']);
}

#[test]
fn reserved_width_never_overflows() {
    // A value far longer than its slot must be truncated, not allowed to bleed
    // into neighboring cells — render must still succeed and stay bounded.
    let doc = sample_doc();
    let png = render_png(
        &doc,
        &json!({ "sale": { "total_amount": "999999999999999999999999" } }),
    )
    .unwrap();
    assert_eq!(&png[0..4], &[0x89, b'P', b'N', b'G']);
}
