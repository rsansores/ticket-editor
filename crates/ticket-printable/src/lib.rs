//! `ticket-printable` — turn an annotated Rust struct into the `variables` JSON
//! a [`ticket-core`] `TicketDoc` renders against.
//!
//! You compose your existing models into a small context struct, derive
//! [`Printable`] on it, and get three projections from one definition:
//!
//! * [`Printable::to_value`] — the real data, at render time.
//! * [`Printable::sample_json`] — placeholder data with the identical shape, for
//!   the editor's live preview and variable tree.
//! * [`Printable::var_types`] — a `path -> VarType` map so the editor offers the
//!   right formatting (decimals for numbers, patterns for dates).
//!
//! Because `sample_json` and `to_value` are generated from the same field walk,
//! the paths a designer sees in the editor and the paths that carry real data at
//! print time cannot drift.
//!
//! ## Agnostic by construction
//!
//! This crate knows nothing about any database or web framework. It ships
//! [`Printable`] impls for the std leaf types always, and for `uuid`, `chrono`
//! and `rust_decimal` behind features of the same name — enable the ones your
//! models use and those types map for free. For anything else, implement
//! [`Printable`] (three short methods) yourself.
//!
//! ## Field policy — pure denylist
//!
//! Every field becomes a variable unless annotated `#[printable(hidden)]`. New
//! model columns show up in the editor automatically; hide the few internal
//! fields (ids, foreign keys, sync flags, secrets) explicitly.
//!
//! [`ticket-core`]: https://crates.io/crates/ticket-core

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

/// Re-exported so the derive macro can name `serde_json` without the host
/// depending on it directly.
pub use serde_json;

/// `#[derive(Printable)]`.
#[cfg(feature = "derive")]
pub use ticket_printable_derive::Printable;

use serde_json::Value;

/// The type of a leaf variable — drives which formatting the editor offers.
///
/// Mirrors `VariableType` in the `@ticket-editor/vue` package
/// (`'text' | 'number' | 'date'`); `Bool` is reported as `text` when serialized
/// for the editor, since it has no dedicated formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarType {
    /// Free text.
    Text,
    /// A number (offers decimals / rounding / thousands).
    Number,
    /// A timestamp (offers date-pattern reformatting).
    Date,
    /// A boolean (treated as text by the editor).
    Bool,
}

impl VarType {
    /// The wire string the editor's `variableTypes` map expects.
    pub fn as_editor_str(self) -> &'static str {
        match self {
            VarType::Number => "number",
            VarType::Date => "date",
            VarType::Text | VarType::Bool => "text",
        }
    }
}

/// A value that can be projected into the ticket `variables` JSON.
///
/// Leaves implement this concretely (see the impls in this crate); structs get
/// it via `#[derive(Printable)]`; `Option<T>` and `Vec<T>` compose it
/// generically. `var_types` receives the dotted path built so far (always ending
/// in `.` for a struct/leaf boundary) and inserts one entry per leaf.
pub trait Printable {
    /// The real value at render time.
    fn to_value(&self) -> Value;
    /// A placeholder value with the same shape, for the editor.
    fn sample_json() -> Value;
    /// Insert `path -> VarType` entries for every leaf reachable under `prefix`.
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>);
}

/// Convenience: the editor's `variableTypes` map (`path -> "text"|"number"|"date"`)
/// for a context type, ready to hand to `@ticket-editor/vue`.
pub fn editor_var_types<T: Printable>() -> BTreeMap<String, String> {
    let mut raw = BTreeMap::new();
    T::var_types("", &mut raw);
    raw.into_iter()
        .map(|(k, v)| (k, v.as_editor_str().to_string()))
        .collect()
}

// ---- leaf impls (std, always on) ------------------------------------------

/// Insert a single leaf type at `prefix` with its trailing `.` trimmed.
fn leaf_type(prefix: &str, out: &mut BTreeMap<String, VarType>, ty: VarType) {
    out.insert(prefix.trim_end_matches('.').to_string(), ty);
}

macro_rules! leaf_number {
    ($($t:ty => $sample:expr),* $(,)?) => {$(
        impl Printable for $t {
            fn to_value(&self) -> Value { serde_json::json!(self) }
            fn sample_json() -> Value { serde_json::json!($sample) }
            fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
                leaf_type(prefix, out, VarType::Number);
            }
        }
    )*};
}

leaf_number! {
    i8 => 12i8, i16 => 1234i16, i32 => 12345i32, i64 => 12345i64, i128 => 12345i128, isize => 12345isize,
    u8 => 12u8, u16 => 1234u16, u32 => 12345u32, u64 => 12345u64, u128 => 12345u128, usize => 12345usize,
    f32 => 1234.56f32, f64 => 1234.56f64,
}

impl Printable for String {
    fn to_value(&self) -> Value {
        Value::String(self.clone())
    }
    fn sample_json() -> Value {
        Value::String("Texto".to_string())
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Text);
    }
}

impl Printable for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn sample_json() -> Value {
        Value::Bool(true)
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Bool);
    }
}

// ---- generic composition ---------------------------------------------------

impl<T: Printable> Printable for Option<T> {
    fn to_value(&self) -> Value {
        match self {
            Some(v) => v.to_value(),
            None => Value::Null,
        }
    }
    fn sample_json() -> Value {
        T::sample_json()
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        T::var_types(prefix, out);
    }
}

impl<T: Printable> Printable for Vec<T> {
    fn to_value(&self) -> Value {
        Value::Array(self.iter().map(Printable::to_value).collect())
    }
    fn sample_json() -> Value {
        // A single representative item so the editor sees a loopable array.
        Value::Array(vec![T::sample_json()])
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        // Leaf paths inside a loop are addressed relative to the item, i.e.
        // without an index segment — the same prefix the editor uses.
        T::var_types(prefix, out);
    }
}

impl<T: Printable + ?Sized> Printable for Box<T> {
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
    fn sample_json() -> Value {
        T::sample_json()
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        T::var_types(prefix, out);
    }
}

// ---- feature-gated leaf impls for well-known crates ------------------------

#[cfg(feature = "uuid")]
impl Printable for uuid::Uuid {
    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
    fn sample_json() -> Value {
        Value::String("00000000-0000-0000-0000-000000000000".to_string())
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Text);
    }
}

#[cfg(feature = "chrono")]
impl<Tz: chrono::TimeZone> Printable for chrono::DateTime<Tz>
where
    Tz::Offset: std::fmt::Display,
{
    fn to_value(&self) -> Value {
        Value::String(self.to_rfc3339())
    }
    fn sample_json() -> Value {
        Value::String("2030-01-15T10:30:00Z".to_string())
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Date);
    }
}

#[cfg(feature = "chrono")]
impl Printable for chrono::NaiveDateTime {
    fn to_value(&self) -> Value {
        Value::String(self.format("%Y-%m-%dT%H:%M:%S").to_string())
    }
    fn sample_json() -> Value {
        Value::String("2030-01-15T10:30:00".to_string())
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Date);
    }
}

#[cfg(feature = "chrono")]
impl Printable for chrono::NaiveDate {
    fn to_value(&self) -> Value {
        Value::String(self.format("%Y-%m-%d").to_string())
    }
    fn sample_json() -> Value {
        Value::String("2030-01-15".to_string())
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Date);
    }
}

#[cfg(feature = "rust_decimal")]
impl Printable for rust_decimal::Decimal {
    fn to_value(&self) -> Value {
        // A string preserves scale exactly; the editor's number formatter parses
        // it back when the designer applies decimals/rounding.
        Value::String(self.to_string())
    }
    fn sample_json() -> Value {
        Value::String("1234.56".to_string())
    }
    fn var_types(prefix: &str, out: &mut BTreeMap<String, VarType>) {
        leaf_type(prefix, out, VarType::Number);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_paths_and_types() {
        let mut out = BTreeMap::new();
        String::var_types("sale.folio.", &mut out);
        assert_eq!(out.get("sale.folio"), Some(&VarType::Text));
    }

    #[test]
    fn option_and_vec_compose() {
        assert_eq!(
            <Option<String>>::sample_json(),
            Value::String("Texto".into())
        );
        assert!(<Vec<i64>>::sample_json().is_array());
        assert_eq!(Some(3i64).to_value(), serde_json::json!(3));
        assert_eq!(None::<i64>.to_value(), Value::Null);
    }
}
