//! End-to-end check of `#[derive(Printable)]` over a realistic nested context,
//! exercising leaf features (uuid/chrono/rust_decimal), pure-denylist hiding,
//! Option/Vec composition, and the sample/real shape parity that the editor
//! relies on. Run with `--features "uuid chrono rust_decimal"`.
#![cfg(all(feature = "uuid", feature = "chrono", feature = "rust_decimal"))]

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;
use ticket_printable::{editor_var_types, Printable};
use uuid::Uuid;

// Hidden fields are intentionally never read — that is the point of the test.
#[allow(dead_code)]
#[derive(Printable)]
struct Movement {
    consecutive: i64,
    total: Option<Decimal>,
    #[printable(hidden)]
    sale_id: Uuid,
}

#[allow(dead_code)]
#[derive(Printable)]
struct Sale {
    sales_folio: Option<String>,
    total: Option<Decimal>,
    end_date: Option<chrono::DateTime<Utc>>,
    captured_sale: Option<bool>,
    #[printable(hidden)]
    equipment_id: Uuid,
    #[printable(hidden)]
    credit_card_number: Option<String>,
}

#[derive(Printable)]
struct SaleTicketContext {
    sale: Sale,
    movements: Vec<Movement>,
}

fn sample_sale() -> Sale {
    Sale {
        sales_folio: Some("A-100".into()),
        total: Some(Decimal::from_str("1234.50").unwrap()),
        end_date: Some(Utc.with_ymd_and_hms(2030, 1, 15, 10, 30, 0).unwrap()),
        captured_sale: Some(true),
        equipment_id: Uuid::nil(),
        credit_card_number: Some("4111111111111111".into()),
    }
}

#[test]
fn hidden_fields_are_absent_from_both_projections() {
    let real = sample_sale().to_value();
    assert!(real.get("equipment_id").is_none(), "hidden by attribute");
    assert!(real.get("credit_card_number").is_none(), "secret hidden");
    assert!(real.get("sales_folio").is_some());

    let sample = Sale::sample_json();
    assert!(sample.get("equipment_id").is_none());
    assert!(sample.get("credit_card_number").is_none());
}

#[test]
fn sample_and_real_share_the_same_shape() {
    let ctx = SaleTicketContext {
        sale: sample_sale(),
        movements: vec![],
    };
    let real = ctx.to_value();
    let sample = SaleTicketContext::sample_json();
    // Same top-level keys, same nested keys — the drift guarantee.
    assert_eq!(
        real["sale"].as_object().unwrap().keys().collect::<Vec<_>>(),
        sample["sale"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(real["movements"].is_array() && sample["movements"].is_array());
}

#[test]
fn var_types_report_editor_kinds() {
    let types = editor_var_types::<SaleTicketContext>();
    assert_eq!(
        types.get("sale.sales_folio").map(String::as_str),
        Some("text")
    );
    assert_eq!(types.get("sale.total").map(String::as_str), Some("number"));
    assert_eq!(types.get("sale.end_date").map(String::as_str), Some("date"));
    // Loop leaves are addressed relative to the item (no index segment).
    assert_eq!(
        types.get("movements.total").map(String::as_str),
        Some("number")
    );
    // Hidden fields never surface a type.
    assert!(!types.contains_key("sale.equipment_id"));
}

#[test]
fn real_values_serialize_as_expected() {
    let v = sample_sale().to_value();
    assert_eq!(v["sales_folio"], "A-100");
    assert_eq!(v["total"], "1234.50"); // Decimal keeps scale as a string
    assert_eq!(v["captured_sale"], true);
    assert!(v["end_date"]
        .as_str()
        .unwrap()
        .starts_with("2030-01-15T10:30:00"));
}
