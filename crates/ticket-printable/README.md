# ticket-printable

Project an annotated Rust struct into the `variables` JSON a
[`ticket-core`](../ticket-core) `TicketDoc` renders against — with **no**
coupling to any database or web framework.

You compose your existing models into a small context struct, derive
`Printable`, and get three projections from one definition:

| Method | Used for |
|---|---|
| `to_value()` | the real data, at render time |
| `sample_json()` | placeholder data with the identical shape, for the editor preview + variable tree |
| `var_types()` | a `path -> VarType` map so the editor offers the right formatting |

Because `sample_json()` and `to_value()` come from the same field walk, the
paths a designer sees in the editor and the paths that carry real data at print
time **cannot drift**.

## Field policy — pure denylist

Every field becomes a variable unless annotated `#[printable(hidden)]`. A new
column on a model shows up in the editor automatically; hide the handful of
internal fields (ids, foreign keys, sync flags, secrets) explicitly.

```rust
use ticket_printable::Printable;

#[derive(Printable)]
struct Sale {
    sales_folio: Option<String>,     // -> "sale.sales_folio"  (text)
    total: Option<rust_decimal::Decimal>, // -> "sale.total"   (number)
    #[printable(hidden)] equipment_id: uuid::Uuid,        // never exposed
    #[printable(hidden)] credit_card_number: Option<String>,
}

#[derive(Printable)]
struct SaleTicketContext {
    sale: Sale,
    movements: Vec<Movement>,        // -> loopable array "movements[]"
}
```

Assembling the struct from real rows is your job (any DB, any query layer). This
crate only does the projection.

## Agnostic by construction

Leaf types map through `Printable` impls. The std types are always on; the
well-known third-party types are behind features of the same name — enable the
ones your models use and they map for free:

```toml
ticket-printable = { version = "0.2", features = ["derive", "uuid", "chrono", "rust_decimal"] }
```

| Feature | Type | Maps to |
|---|---|---|
| `uuid` | `uuid::Uuid` | text |
| `chrono` | `DateTime` / `NaiveDateTime` / `NaiveDate` | date |
| `rust_decimal` | `Decimal` | number (string-encoded, scale preserved) |

Don't like a convention? Omit the feature and `impl Printable for YourType`
(three short methods).

## Computed values live in the template, not here

There is deliberately **no** computed-field attribute. Derived values (e.g. a
Google-Maps QR built from `latitude`/`longitude`) are authored in the editor as
`{path}` templates and evaluated inside `ticket-core`, so the browser preview and
the printed ticket compute them identically. Just keep the raw ingredients
visible (don't hide `latitude`/`longitude`).

Dual-licensed under MIT OR Apache-2.0.
