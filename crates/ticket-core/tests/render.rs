use serde_json::json;
use ticket_core::{
    render_png, render_png_with_fonts, render_png_with_options, FontFaces, Fonts, RenderError,
    RenderOptions, TicketDoc,
};

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
fn unresolved_path_renders_empty_by_default_and_fake_in_placeholder_mode() {
    // Print mode (default): a path that doesn't resolve renders EMPTY — a typo'd
    // path must never print a believable wrong number on a customer's receipt.
    // Editor mode (placeholders): the same path renders a deterministic fake.
    let doc = sample_doc(); // references sale.total_amount
    let fonts = &Fonts::builtin().unwrap();
    let empty_data = json!({ "sale": {} });

    let print = render_png_with_options(&doc, &empty_data, fonts, &RenderOptions::default())
        .unwrap();
    let editor =
        render_png_with_options(&doc, &empty_data, fonts, &RenderOptions::placeholders()).unwrap();
    assert_ne!(
        print, editor,
        "placeholder mode must draw a fake where print mode draws nothing"
    );

    // Print mode with a missing field equals print mode with an explicitly empty
    // one — i.e. the missing path contributed NO ink.
    let explicit_blank = render_png_with_options(
        &doc,
        &json!({ "sale": { "total_amount": "" } }),
        fonts,
        &RenderOptions::default(),
    )
    .unwrap();
    assert_eq!(
        print, explicit_blank,
        "an unresolved path must render exactly like an empty value"
    );
}

#[test]
fn qr_and_barcode_from_missing_variable_draw_nothing_on_print() {
    // A QR/barcode bound to a variable that doesn't resolve: print mode draws
    // nothing (an unscannable placeholder frame would be garbage on a receipt);
    // placeholder mode still draws the fake so the editor canvas stays lively.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24, "min_rows": 14 },
        "elements": [
            { "id": "q", "row": 0, "col": 0, "type": "qr", "value": "missing.url",
              "from_variable": true, "size": 10 },
            { "id": "b", "row": 11, "col": 0, "type": "barcode", "value": "missing.code",
              "from_variable": true, "width": 20, "height": 3 }
        ]
    }))
    .unwrap();
    let blank: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24, "min_rows": 14 }, "elements": []
    }))
    .unwrap();
    let fonts = &Fonts::builtin().unwrap();
    let print =
        render_png_with_options(&doc, &json!({}), fonts, &RenderOptions::default()).unwrap();
    let empty_doc =
        render_png_with_options(&blank, &json!({}), fonts, &RenderOptions::default()).unwrap();
    assert_eq!(print, empty_doc, "no value → no QR/barcode ink on print");
    let editor =
        render_png_with_options(&doc, &json!({}), fonts, &RenderOptions::placeholders()).unwrap();
    assert_ne!(editor, print, "editor preview still draws fakes");
}

#[test]
fn unresolved_paths_reports_typos_and_respects_scopes() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 40 },
        "computed": [{ "name": "total", "formula": "sale.subtotal + sale.tax" }],
        "regions": [
            { "id": "loop", "start_row": 2, "end_row": 3, "source": "movements" },
            { "id": "gone", "start_row": 5, "end_row": 6, "source": "nope.items" }
        ],
        "elements": [
            { "id": "ok",   "row": 0, "col": 0, "type": "variable", "path": "sale.customer", "length": 10 },
            { "id": "typo", "row": 0, "col": 12, "type": "variable", "path": "sale.totl", "length": 10 },
            { "id": "calc", "row": 1, "col": 0, "type": "variable", "path": "calc.total", "length": 10 },
            // Inside the loop: a real item field, a typo'd one, and implicit row vars.
            { "id": "vol",  "row": 2, "col": 0, "type": "variable", "path": "volume", "length": 8 },
            { "id": "oops", "row": 2, "col": 10, "type": "variable", "path": "volumen", "length": 8 },
            { "id": "n",    "row": 2, "col": 20, "type": "variable", "path": "row.number", "length": 3 },
            { "id": "bad",  "row": 2, "col": 24, "type": "variable", "path": "row.importe", "length": 8 },
            // row.* outside any band never resolves.
            { "id": "out",  "row": 4, "col": 0, "type": "variable", "path": "row.number", "length": 3 },
            // Inside the dead band: unverifiable, must NOT add noise.
            { "id": "dead", "row": 5, "col": 0, "type": "variable", "path": "whatever", "length": 5 },
            // is_set conditions probe absent fields legitimately — not reported.
            { "id": "cond", "row": 7, "col": 0, "type": "text", "content": "X",
              "condition": { "var": "sale.maybe", "op": "is_set", "value": "" } },
            { "id": "cond2", "row": 8, "col": 0, "type": "text", "content": "Y",
              "condition": { "var": "sale.misspelt", "op": "eq", "value": "1" } }
        ]
    }))
    .unwrap();
    let vars = json!({
        "sale": { "subtotal": 10, "tax": 1.6, "customer": "ACME" },
        "movements": [ { "volume": 5.0, "price": 21.5 } ]
    });
    let missing = doc.unresolved_paths(&vars);
    assert_eq!(
        missing,
        vec![
            "nope.items".to_string(),
            "sale.totl".to_string(),
            "volumen".to_string(),
            "row.importe".to_string(),
            "row.number".to_string(),
            "sale.misspelt".to_string(),
        ]
    );
    // A declared row-computed makes row.importe valid inside its band.
    let mut with_row: TicketDoc = doc;
    with_row.regions[0].computed = vec![ticket_core::Computed {
        name: "importe".into(),
        formula: "volume * price".into(),
    }];
    let missing = with_row.unresolved_paths(&vars);
    assert!(!missing.contains(&"row.importe".to_string()));
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
fn barcode_renders_and_varies_with_content() {
    let doc = |val: &str, sym: &str| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 2, "paper": { "width_chars": 30 },
            "elements": [{ "id": "b", "row": 0, "col": 0, "type": "barcode",
                           "value": val, "symbology": sym, "width": 24, "height": 4 }]
        }))
        .unwrap()
    };
    let a = render_png(&doc("ABC123", "code128"), &serde_json::Value::Null).unwrap();
    let b = render_png(&doc("XYZ789", "code128"), &serde_json::Value::Null).unwrap();
    assert_eq!(&a[0..4], &[0x89, b'P', b'N', b'G']);
    assert_ne!(
        a, b,
        "different barcode content must produce a different image"
    );
    // Code 39 and a numeric EAN-13 also render as valid PNGs.
    let c39 = render_png(&doc("HELLO", "code39"), &serde_json::Value::Null).unwrap();
    let ean = render_png(&doc("012345678905", "ean13"), &serde_json::Value::Null).unwrap();
    assert_eq!(&c39[0..4], &[0x89, b'P', b'N', b'G']);
    assert_eq!(&ean[0..4], &[0x89, b'P', b'N', b'G']);
}

#[test]
fn barcode_from_variable_resolves_and_invalid_falls_back() {
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 30 },
        "elements": [{ "id": "b", "row": 0, "col": 0, "type": "barcode",
                       "value": "code", "from_variable": true, "symbology": "code128", "width": 24, "height": 4 }]
    }))
    .unwrap();
    let x = render_png(&doc, &json!({ "code": "AAA" })).unwrap();
    let y = render_png(&doc, &json!({ "code": "BBB" })).unwrap();
    assert_ne!(x, y, "a barcode bound to a variable must reflect the value");
    // An EAN-13 with letters can't encode → placeholder frame, not an error.
    let bad: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 30 },
        "elements": [{ "id": "b", "row": 0, "col": 0, "type": "barcode",
                       "value": "not-digits", "symbology": "ean13", "width": 24, "height": 4 }]
    }))
    .unwrap();
    assert_eq!(
        &render_png(&bad, &serde_json::Value::Null).unwrap()[0..4],
        &[0x89, b'P', b'N', b'G']
    );
}

#[test]
fn missing_font_is_a_hard_error() {
    // A document referencing a font the renderer wasn't given must fail loudly,
    // not silently substitute — so a backend render surfaces the misconfiguration.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20 },
        "elements": [{ "id": "t", "row": 0, "col": 0, "type": "text", "content": "HI",
                       "style": { "font": "not-registered" } }]
    }))
    .unwrap();
    assert!(matches!(
        render_png(&doc, &serde_json::Value::Null),
        Err(RenderError::MissingFont(f)) if f == "not-registered"
    ));
}

#[test]
fn registered_font_family_is_used_per_element_and_per_doc() {
    // Register an "alt" family whose regular face is actually the bold outlines,
    // so a field in "alt" renders differently from the built-in regular.
    let bold = include_bytes!("../assets/DejaVuSansMono-Bold.ttf").to_vec();
    let mut fonts = Fonts::builtin().unwrap();
    fonts.add_family(
        "alt",
        FontFaces::from_bytes(bold.clone(), bold.clone(), bold.clone(), bold).unwrap(),
    );

    let doc = |style: serde_json::Value, doc_font: Option<&str>| -> TicketDoc {
        let mut v = json!({
            "version": 2, "paper": { "width_chars": 20 },
            "elements": [{ "id": "t", "row": 0, "col": 0, "type": "text", "content": "HELLO", "style": style }]
        });
        if let Some(f) = doc_font {
            v["font"] = json!(f);
        }
        serde_json::from_value(v).unwrap()
    };
    let default =
        render_png_with_fonts(&doc(json!({}), None), &serde_json::Value::Null, &fonts).unwrap();
    let per_el = render_png_with_fonts(
        &doc(json!({ "font": "alt" }), None),
        &serde_json::Value::Null,
        &fonts,
    )
    .unwrap();
    let per_doc = render_png_with_fonts(
        &doc(json!({}), Some("alt")),
        &serde_json::Value::Null,
        &fonts,
    )
    .unwrap();

    assert_eq!(&per_el[0..4], &[0x89, b'P', b'N', b'G']);
    assert_ne!(
        default, per_el,
        "a different font family must change the raster"
    );
    assert_eq!(
        per_el, per_doc,
        "the doc-level default font resolves like a per-element one"
    );
}

#[cfg(feature = "bundled-fonts")]
#[test]
fn bundled_fonts_render_a_custom_family() {
    let fonts = Fonts::with_bundled().unwrap();
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20 },
        "elements": [{ "id": "t", "row": 0, "col": 0, "type": "text", "content": "HELLO",
                       "style": { "font": "vt323" } }]
    }))
    .unwrap();
    let out = render_png_with_fonts(&doc, &serde_json::Value::Null, &fonts).unwrap();
    assert_eq!(&out[0..4], &[0x89, b'P', b'N', b'G']);
    // A bundled family renders differently from the built-in default.
    let plain: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20 },
        "elements": [{ "id": "t", "row": 0, "col": 0, "type": "text", "content": "HELLO" }]
    }))
    .unwrap();
    assert_ne!(out, render_png(&plain, &serde_json::Value::Null).unwrap());
}

#[test]
fn crate_fonts_match_the_editor_copy_byte_for_byte() {
    // Native (crate) and browser (editor) must draw with the IDENTICAL font bytes
    // or preview != print. Guard against the two vendored copies drifting. Skipped
    // when the editor tree isn't present (e.g. a published crate on its own).
    use std::path::{Path, PathBuf};
    let crate_dir = Path::new("assets/fonts");
    let editor_dir = Path::new("../../packages/ticket-editor/src/assets/fonts");
    if !editor_dir.exists() {
        return;
    }
    fn ttfs(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                ttfs(&p, base, out);
            } else if p.extension().is_some_and(|x| x == "ttf") {
                out.push(p.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
    let mut rels = Vec::new();
    ttfs(crate_dir, crate_dir, &mut rels);
    assert!(!rels.is_empty(), "no bundled fonts found in the crate");
    for rel in rels {
        let a = std::fs::read(crate_dir.join(&rel)).unwrap();
        let b = std::fs::read(editor_dir.join(&rel)).unwrap_or_default();
        assert_eq!(
            a, b,
            "font drift: {rel:?} differs between the crate and the editor — parity broken"
        );
    }
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
fn image_from_variable_resolves_and_missing_draws_nothing_on_print() {
    // A dynamic image (e.g. a signature) resolves its base64 from a variable.
    // Missing source: the EDITOR (placeholder mode) draws the visible frame so
    // the designer sees the slot; a REAL PRINT draws nothing — a hollow frame
    // where a signature should be is print corruption, same rule as QR/barcode.
    // A valid 8x8 checkerboard PNG (base64), distinct from the placeholder frame.
    let png_8x8 = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAF0lEQVR42mNgYGD4//8/FhK7KAQMPh0AXXNfoWyFCAcAAAAASUVORK5CYII=";
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20, "min_rows": 6 },
        "elements": [{ "id": "img", "row": 0, "col": 0, "type": "image",
                       "data": "sale.signature", "from_variable": true, "w": 8, "h": 4 }]
    }))
    .unwrap();
    let blank: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20, "min_rows": 6 }, "elements": []
    }))
    .unwrap();
    let fonts = &Fonts::builtin().unwrap();
    let resolved = render_png(&doc, &json!({ "sale": { "signature": png_8x8 } })).unwrap();
    let missing_print = render_png(&doc, &serde_json::Value::Null).unwrap();
    let missing_editor =
        render_png_with_options(&doc, &serde_json::Value::Null, fonts, &RenderOptions::placeholders())
            .unwrap();
    let empty_doc = render_png(&blank, &serde_json::Value::Null).unwrap();
    assert_eq!(&resolved[0..4], &[0x89, b'P', b'N', b'G']);
    assert_eq!(
        missing_print, empty_doc,
        "print mode: a missing dynamic image must contribute no ink"
    );
    assert_ne!(
        missing_editor, empty_doc,
        "editor mode: the placeholder frame must stay visible"
    );
    assert_ne!(resolved, missing_editor);
}

#[test]
fn row_scoped_qr_never_fakes_and_max_lines_zero_is_unbounded() {
    // A QR bound to `row.folio` OUTSIDE any band: even in placeholder mode it
    // must draw nothing — a scannable-looking fake that print drops is the
    // exact hazard the row.* rule exists to prevent.
    let qr_doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20, "min_rows": 12 },
        "elements": [{ "id": "q", "row": 0, "col": 0, "type": "qr",
                       "value": "row.folio", "from_variable": true, "size": 10 }]
    }))
    .unwrap();
    let blank: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 20, "min_rows": 12 }, "elements": []
    }))
    .unwrap();
    let fonts = &Fonts::builtin().unwrap();
    let editor =
        render_png_with_options(&qr_doc, &json!({}), fonts, &RenderOptions::placeholders())
            .unwrap();
    let empty = render_png_with_options(&blank, &json!({}), fonts, &RenderOptions::placeholders())
        .unwrap();
    assert_eq!(editor, empty, "row.* QR outside a band must never fake");

    // max_lines: 0 means unbounded (the editor's "no limit"), not "cut to 1".
    let wrap_doc = |max: u32| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 2, "paper": { "width_chars": 24 },
            "elements": [{ "id": "w", "row": 0, "col": 0, "type": "variable",
                           "path": "x", "length": 10, "wrap": true, "max_lines": max }]
        }))
        .unwrap()
    };
    let data = json!({ "x": "aaaa bbbb cccc dddd" });
    let zero = render_png(&wrap_doc(0), &data).unwrap();
    let one = render_png(&wrap_doc(1), &data).unwrap();
    assert_ne!(zero, one, "max_lines 0 must not behave like max_lines 1");
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

// ---------------------------------------------------------------------------
// Region.computed → row.* (per-iteration values inside a loop band)
// ---------------------------------------------------------------------------

/// A GasPAR-style sale: a movements loop with VOL, PRECIO and a row-computed
/// IMPORTE column, plus a doc-level total below the band.
fn row_computed_doc() -> TicketDoc {
    serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 40 },
        "computed": [{ "name": "total", "formula": "sum(movements, volume * price)" }],
        "regions": [{
            "id": "movs", "start_row": 1, "end_row": 2, "source": "movements",
            "computed": [
                { "name": "importe", "formula": "round(volume * price, 2)" },
                // References an earlier row-computed of the same band.
                { "name": "importe_iva", "formula": "round(row.importe * 1.16, 2)" }
            ]
        }],
        "elements": [
            { "id": "hdr", "row": 0, "col": 0, "type": "text", "content": "VOL   PRECIO   IMPORTE" },
            { "id": "n",   "row": 1, "col": 0, "type": "variable", "path": "row.number", "length": 2 },
            { "id": "v",   "row": 1, "col": 3, "type": "variable", "path": "volume", "length": 6,
              "number": { "decimals": 2, "rounding": "half_up", "thousands": false } },
            { "id": "p",   "row": 1, "col": 10, "type": "variable", "path": "price", "length": 7,
              "number": { "decimals": 2, "rounding": "half_up", "thousands": false } },
            { "id": "amt", "row": 1, "col": 18, "type": "variable", "path": "row.importe", "length": 10,
              "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } },
            { "id": "iva", "row": 1, "col": 29, "type": "variable", "path": "row.importe_iva", "length": 10,
              "align": "right" },
            { "id": "tot", "row": 2, "col": 18, "type": "variable", "path": "calc.total", "length": 10,
              "align": "right",
              "number": { "decimals": 2, "rounding": "half_up", "thousands": true } }
        ]
    }))
    .unwrap()
}

#[test]
fn row_computed_single_item_has_correct_value() {
    // One item: row.importe = 5 * 20 = 100. The render must equal a doc where
    // the same cell holds a plain item field with value 100 — i.e. the computed
    // value is REAL data to the renderer, not a special case.
    let doc = row_computed_doc();
    let by_formula = render_png(
        &doc,
        &json!({ "movements": [ { "volume": 5, "price": 20 } ] }),
    )
    .unwrap();
    // Same doc, but importe comes precomputed in the data and the element binds
    // to it directly.
    let mut direct: TicketDoc = row_computed_doc();
    direct.regions[0].computed = vec![serde_json::from_value::<ticket_core::Computed>(
        json!({ "name": "importe_iva", "formula": "round(importe * 1.16, 2)" })).unwrap()];
    // rebind row.importe -> importe (denormalized into the item)
    let doc_json = serde_json::to_string(&direct).unwrap().replace("row.importe\"", "importe\"");
    let direct: TicketDoc = serde_json::from_str(&doc_json).unwrap();
    let denormalized = render_png(
        &direct,
        &json!({ "movements": [ { "volume": 5, "price": 20, "importe": 100 } ] }),
    )
    .unwrap();
    assert_eq!(by_formula, denormalized, "row.importe must render like real item data");
}

#[test]
fn row_computed_three_items_get_distinct_values_in_order() {
    // The bug class here is computing once and reusing: three items must render
    // three DISTINCT amounts, in item order. Swap two items -> different raster.
    let doc = row_computed_doc();
    let data = json!({ "movements": [
        { "volume": 1, "price": 10 },
        { "volume": 2, "price": 10 },
        { "volume": 3, "price": 10 }
    ]});
    let swapped = json!({ "movements": [
        { "volume": 2, "price": 10 },
        { "volume": 1, "price": 10 },
        { "volume": 3, "price": 10 }
    ]});
    let a = render_png(&doc, &data).unwrap();
    let b = render_png(&doc, &swapped).unwrap();
    assert_ne!(a, b, "per-row values must follow item order, not repeat");
}

#[test]
fn row_formula_mixes_calc_and_root_paths() {
    // A row formula referencing calc.* and an absolute root path together.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 32 },
        "computed": [{ "name": "rate", "formula": "0.16" }],
        "regions": [{
            "id": "r", "start_row": 0, "end_row": 1, "source": "items",
            "computed": [{ "name": "tax", "formula": "round(amount * calc.rate + shipping.flat, 2)" }]
        }],
        "elements": [
            { "id": "t", "row": 0, "col": 0, "type": "variable", "path": "row.tax", "length": 10 }
        ]
    }))
    .unwrap();
    let a = render_png(&doc, &json!({ "shipping": { "flat": 5 }, "items": [ { "amount": 100 } ] })).unwrap();
    let b = render_png(&doc, &json!({ "shipping": { "flat": 9 }, "items": [ { "amount": 100 } ] })).unwrap();
    assert_ne!(a, b, "root path inside a row formula must be live");
}

#[test]
fn row_paths_outside_a_band_render_empty_never_fake() {
    // row.* referenced from an element OUTSIDE any band → empty, in BOTH modes.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "elements": [
            { "id": "x", "row": 0, "col": 0, "type": "variable", "path": "row.importe", "length": 10 }
        ]
    }))
    .unwrap();
    let blank: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 }, "elements": [] })).unwrap();
    let fonts = &Fonts::builtin().unwrap();
    for opts in [RenderOptions::default(), RenderOptions::placeholders()] {
        let with_el = render_png_with_options(&doc, &json!({}), fonts, &opts).unwrap();
        let without = render_png_with_options(&blank, &json!({}), fonts, &opts).unwrap();
        assert_eq!(
            with_el, without,
            "row.* outside a band must contribute no ink (placeholders={})",
            opts.placeholders
        );
    }
}

#[test]
fn collapsed_band_never_evaluates_row_formulas() {
    // Band collapsed by its condition → row formulas never run: a formula that
    // would divide by zero on absent data must not affect the render.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "regions": [{
            "id": "r", "start_row": 0, "end_row": 1, "source": "items",
            "condition": { "var": "show", "op": "eq", "value": "1" },
            "computed": [{ "name": "boom", "formula": "1 / divisor" }]
        }],
        "elements": [
            { "id": "b", "row": 0, "col": 0, "type": "variable", "path": "row.boom", "length": 8 },
            { "id": "t", "row": 1, "col": 0, "type": "text", "content": "FOOTER" }
        ]
    }))
    .unwrap();
    // Collapsed (condition false): renders fine, footer flows up.
    let collapsed = render_png(&doc, &json!({ "show": 0, "items": [ { "divisor": 0 } ] })).unwrap();
    let no_band: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "elements": [ { "id": "t", "row": 0, "col": 0, "type": "text", "content": "FOOTER" } ]
    }))
    .unwrap();
    let flowed_up = render_png(&no_band, &json!({})).unwrap();
    assert_eq!(collapsed, flowed_up, "collapsed band leaves no trace");
}

#[test]
fn implicit_row_vars_drive_element_conditions() {
    // row.number prints line numbers; row.last drives a condition that shows a
    // marker only on the final line. 2 items vs 3 items must differ beyond just
    // height; the marker must appear exactly once (asserted indirectly: a doc
    // whose marker condition is row.first renders differently from row.last for
    // 2+ items, identically for 1).
    let doc = |cond_var: &str| -> TicketDoc {
        serde_json::from_value(json!({
            "version": 2, "paper": { "width_chars": 24 },
            "regions": [{ "id": "r", "start_row": 0, "end_row": 1, "source": "items" }],
            "elements": [
                { "id": "n", "row": 0, "col": 0, "type": "variable", "path": "row.number", "length": 3 },
                { "id": "m", "row": 0, "col": 4, "type": "text", "content": "<<",
                  "condition": { "var": cond_var, "op": "eq", "value": "true" } }
            ]
        }))
        .unwrap()
    };
    let one = json!({ "items": [ {} ] });
    let many = json!({ "items": [ {}, {}, {} ] });
    assert_eq!(
        render_png(&doc("row.first"), &one).unwrap(),
        render_png(&doc("row.last"), &one).unwrap(),
        "single item: first == last"
    );
    assert_ne!(
        render_png(&doc("row.first"), &many).unwrap(),
        render_png(&doc("row.last"), &many).unwrap(),
        "3 items: the marker sits on a different line for first vs last"
    );
}

#[test]
fn conditional_band_evaluates_declared_row_values_once() {
    // A conditional-only band (no source) with a declared row-computed: it
    // evaluates once when the band shows.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 30 },
        "regions": [{
            "id": "c", "start_row": 0, "end_row": 1,
            "condition": { "var": "sale.change", "op": "gt", "value": "0" },
            "computed": [{ "name": "line", "formula": "concat(\"CAMBIO: \", sale.change)" }]
        }],
        "elements": [
            { "id": "l", "row": 0, "col": 0, "type": "variable", "path": "row.line", "length": 20 }
        ]
    }))
    .unwrap();
    let a = render_png(&doc, &json!({ "sale": { "change": 12 } })).unwrap();
    let b = render_png(&doc, &json!({ "sale": { "change": 34 } })).unwrap();
    assert_ne!(a, b, "the declared row value must be live on a conditional band");
}

// ---------------------------------------------------------------------------
// wrap: true participates in the flow transform (no more overprint)
// ---------------------------------------------------------------------------

#[test]
fn wrapped_lines_push_content_below_down() {
    // The GasPAR field bug: a long customer name in a wrapped `length: 39`
    // field at row 0, ">> FACTURA SOLICITADA <<" at row 1. With the fix the
    // marker moves DOWN one row instead of being overprinted. Asserted exactly:
    // the render equals a doc with the two wrapped lines as static text at rows
    // 0-1 and the marker at row 2 (alignment padding draws no ink).
    let wrapped: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 44 },
        "elements": [
            { "id": "c", "row": 0, "col": 0, "type": "variable", "path": "customer",
              "length": 39, "wrap": true },
            { "id": "m", "row": 1, "col": 0, "type": "text",
              "content": ">> FACTURA SOLICITADA <<" }
        ]
    }))
    .unwrap();
    let expected: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 44 },
        "elements": [
            { "id": "l1", "row": 0, "col": 0, "type": "text",
              "content": "Transportes y Mudanzas del Bajio SA de" },
            { "id": "l2", "row": 1, "col": 0, "type": "text", "content": "CV" },
            { "id": "m", "row": 2, "col": 0, "type": "text",
              "content": ">> FACTURA SOLICITADA <<" }
        ]
    }))
    .unwrap();
    let data = json!({ "customer": "Transportes y Mudanzas del Bajio SA de CV" });
    assert_eq!(
        render_png(&wrapped, &data).unwrap(),
        render_png(&expected, &json!({})).unwrap(),
        "a 2-line wrap must push the next row down by exactly 1 (no overprint)"
    );
    // A short (single-line) value leaves the design untouched: marker stays at row 1.
    let untouched: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 44 },
        "elements": [
            { "id": "l1", "row": 0, "col": 0, "type": "text", "content": "ACME" },
            { "id": "m", "row": 1, "col": 0, "type": "text",
              "content": ">> FACTURA SOLICITADA <<" }
        ]
    }))
    .unwrap();
    assert_eq!(
        render_png(&wrapped, &json!({ "customer": "ACME" })).unwrap(),
        render_png(&untouched, &json!({})).unwrap(),
        "a single-line value must not move anything"
    );
}

#[test]
fn two_wrapped_elements_on_one_row_push_by_the_max() {
    // Elements wrapping to 2 and 3 lines on the same row: the next row moves by
    // 2 (max-1), not 3 (sum-1).
    let wrapped: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 40 },
        "elements": [
            { "id": "a", "row": 0, "col": 0, "type": "variable", "path": "x",
              "length": 10, "wrap": true },
            { "id": "b", "row": 0, "col": 12, "type": "variable", "path": "y",
              "length": 10, "wrap": true },
            { "id": "m", "row": 1, "col": 0, "type": "text", "content": "MARK" }
        ]
    }))
    .unwrap();
    // x → "aaaa aaaa" / "bbbb"; y → "cccc cccc" / "dddd dddd" / "eeee".
    let data = json!({ "x": "aaaa aaaa bbbb", "y": "cccc cccc dddd dddd eeee" });
    let expected: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 40 },
        "elements": [
            { "id": "x1", "row": 0, "col": 0, "type": "text", "content": "aaaa aaaa" },
            { "id": "x2", "row": 1, "col": 0, "type": "text", "content": "bbbb" },
            { "id": "y1", "row": 0, "col": 12, "type": "text", "content": "cccc cccc" },
            { "id": "y2", "row": 1, "col": 12, "type": "text", "content": "dddd dddd" },
            { "id": "y3", "row": 2, "col": 12, "type": "text", "content": "eeee" },
            { "id": "m", "row": 3, "col": 0, "type": "text", "content": "MARK" }
        ]
    }))
    .unwrap();
    assert_eq!(
        render_png(&wrapped, &data).unwrap(),
        render_png(&expected, &json!({})).unwrap(),
        "same-row wraps compose by MAX, not sum"
    );
}

#[test]
fn wrap_inside_a_loop_band_shifts_later_iterations() {
    // Item 2's long name pushes items 3..n down: the band height is
    // per-iteration. Equivalent check: [long, short] vs [short, long] renders
    // differently (the wrapped row sits at a different y), and both are taller
    // than [short, short].
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "regions": [{ "id": "r", "start_row": 0, "end_row": 1, "source": "items" }],
        "elements": [
            { "id": "n", "row": 0, "col": 0, "type": "variable", "path": "name",
              "length": 12, "wrap": true },
            { "id": "f", "row": 1, "col": 0, "type": "text", "content": "FOOTER" }
        ]
    }))
    .unwrap();
    let long = "un nombre muy largo que se envuelve";
    let hi = |names: &[&str]| {
        let items: Vec<_> = names.iter().map(|n| json!({ "name": n })).collect();
        render_png(&doc, &json!({ "items": items })).unwrap()
    };
    let long_first = hi(&[long, "corto"]);
    let long_second = hi(&["corto", long]);
    let both_short = hi(&["corto", "corto"]);
    assert_ne!(long_first, long_second, "which iteration wraps must matter");
    assert!(long_first.len() != both_short.len() || long_first != both_short);
    // Total band height is iteration-additive: [long, long] is taller than
    // [long, short], which is taller than [short, short].
    let both_long = hi(&[long, long]);
    let h = |png: &[u8]| {
        // PNG IHDR height: bytes 20..24 big-endian.
        u32::from_be_bytes([png[20], png[21], png[22], png[23]])
    };
    assert!(h(&long_first) > h(&both_short));
    assert!(h(&both_long) > h(&long_first));
}

#[test]
fn wrap_and_region_collapse_compose() {
    // A wrapped field at row 0 (+1 row), a collapsible band at row 1 (−1 when
    // hidden), a footer at row 2. Both active: the deltas cancel and the footer
    // lands back at its design row, with the wrap's second line where the band
    // used to be.
    let doc: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "regions": [{ "id": "c", "start_row": 1, "end_row": 2,
                      "condition": { "var": "show", "op": "eq", "value": "1" } }],
        "elements": [
            { "id": "w", "row": 0, "col": 0, "type": "variable", "path": "note",
              "length": 10, "wrap": true },
            { "id": "b", "row": 1, "col": 0, "type": "text", "content": "BAND" },
            { "id": "f", "row": 2, "col": 0, "type": "text", "content": "FOOTER" }
        ]
    }))
    .unwrap();
    let expected: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 24 },
        "elements": [
            { "id": "l1", "row": 0, "col": 0, "type": "text", "content": "aaaa aaaa" },
            { "id": "l2", "row": 1, "col": 0, "type": "text", "content": "bbbb" },
            { "id": "f", "row": 2, "col": 0, "type": "text", "content": "FOOTER" }
        ]
    }))
    .unwrap();
    assert_eq!(
        render_png(&doc, &json!({ "show": 0, "note": "aaaa aaaa bbbb" })).unwrap(),
        render_png(&expected, &json!({})).unwrap(),
        "wrap (+1) and collapse (−1) must compose"
    );
}

#[test]
fn scaled_wrap_pushes_by_scale_per_extra_line() {
    // scale: 2 + wrap: each wrapped line is 2 rows tall. A 2-line value's
    // second line starts 2 rows down, and the next element moves down 2 rows.
    let wrapped: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 40 },
        "elements": [
            { "id": "w", "row": 0, "col": 0, "type": "variable", "path": "x",
              "length": 8, "wrap": true, "style": { "scale": 2 } },
            { "id": "m", "row": 2, "col": 0, "type": "text", "content": "MARK" }
        ]
    }))
    .unwrap();
    // x → "aaaa aaa" / "bbbb" at scale 2: line 2 occupies rows 2-3, marker → row 4.
    let expected: TicketDoc = serde_json::from_value(json!({
        "version": 2, "paper": { "width_chars": 40 },
        "elements": [
            { "id": "l1", "row": 0, "col": 0, "type": "text", "content": "aaaa aaa",
              "style": { "scale": 2 } },
            { "id": "l2", "row": 2, "col": 0, "type": "text", "content": "bbbb",
              "style": { "scale": 2 } },
            { "id": "m", "row": 4, "col": 0, "type": "text", "content": "MARK" }
        ]
    }))
    .unwrap();
    assert_eq!(
        render_png(&wrapped, &json!({ "x": "aaaa aaa bbbb" })).unwrap(),
        render_png(&expected, &json!({})).unwrap(),
        "one extra wrapped line at scale 2 = 2 rows of push"
    );
}

#[test]
fn max_lines_truncates_with_ellipsis() {
    let make = |max: Option<u32>| -> TicketDoc {
        let mut el = json!({ "id": "w", "row": 0, "col": 0, "type": "variable",
                             "path": "x", "length": 10, "wrap": true });
        if let Some(m) = max { el["max_lines"] = json!(m); }
        serde_json::from_value(json!({
            "version": 2, "paper": { "width_chars": 24 },
            "elements": [ el, { "id": "m", "row": 1, "col": 0, "type": "text", "content": "MARK" } ]
        }))
        .unwrap()
    };
    let data = json!({ "x": "aaaa bbbb cccc dddd eeee ffff" }); // many lines
    let unbounded = render_png(&make(None), &data).unwrap();
    let bounded = render_png(&make(Some(2)), &data).unwrap();
    assert_ne!(unbounded, bounded, "max_lines must cut the value");
    let h = |png: &[u8]| u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert!(h(&unbounded) > h(&bounded), "bounded wrap must be shorter");
    // The bounded render still differs from a hard 2-line value without the
    // ellipsis — i.e. the `…` is actually drawn.
    let two_lines_exact = render_png(&make(Some(2)), &json!({ "x": "aaaa bbbb cccc" })).unwrap();
    assert_ne!(bounded, two_lines_exact);
}
