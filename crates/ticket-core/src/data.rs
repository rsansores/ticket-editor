//! Variable resolution and deterministic fake data.
//!
//! At render time each `Variable` element needs a concrete string. Two sources:
//!   1. Real data supplied as a JSON `variables` object (the shape the host app
//!      passes in — see the spec's `sale.total_amount` example).
//!   2. Faked data for the editor preview, so the user sees a realistic ticket
//!      before any real sale exists.
//!
//! The faker is **deterministic** (seeded only by the variable path, no RNG).
//! That keeps previews stable across keystrokes — the same path always yields
//! the same fake value, so the preview doesn't flicker with random noise on
//! every re-render, and native/wasm agree without sharing a random seed.

use serde_json::Value;

use crate::schema::{
    Computed, CondOp, Condition, ElementKind, Region, TicketDoc, RESERVED_ROW_NAMES,
};

/// A loop context: the array path being looped, the current index, and the item.
pub type LoopCtx<'a> = Option<(&'a str, usize, &'a Value)>;

/// The full name-resolution scope for an element or formula: an optional loop
/// binding plus the optional `row.*` values of the current band iteration.
///
/// `row.*` is intercepted here and **never** falls through to the document root
/// or the faker — outside its band (or in a band that defines no row values) a
/// `row.<name>` path simply doesn't resolve.
#[derive(Clone, Copy, Default)]
pub struct Scope<'a> {
    /// Current loop binding: (source path, index, item).
    pub loop_ctx: LoopCtx<'a>,
    /// Row-scoped values for the current band iteration (a JSON object holding
    /// the implicit `index`/`number`/`count`/`first`/`last` plus any declared
    /// [`Region::computed`] results).
    pub row: Option<&'a Value>,
}

/// Resolve a `path` in a full scope: `row.*` against the current band
/// iteration's row values, everything else via [`resolve_loop`].
pub fn resolve_scoped<'a>(scope: Scope<'a>, root: &'a Value, path: &str) -> Option<&'a Value> {
    if path == "row" {
        return scope.row;
    }
    if let Some(rest) = path.strip_prefix("row.") {
        return scope.row.and_then(|r| resolve(r, rest));
    }
    resolve_loop(scope.loop_ctx, root, path)
}

/// Resolve a `path` for an element, honoring loop context. Two ways an element
/// inside a loop binds to the current item, both supported so the editor never
/// has to rewrite stored paths:
///   1. **Absolute tree path** (`sale.items.0.qty`): the index segment
///      right after the loop's `source` is replaced with the current index.
///   2. **Relative path** (`qty`): resolved against the item directly.
///
/// Anything else resolves against the document root (a constant across iterations).
pub fn resolve_loop<'a>(loop_ctx: LoopCtx<'a>, root: &'a Value, path: &str) -> Option<&'a Value> {
    if let Some((source, index, item)) = loop_ctx {
        let prefix = format!("{source}.");
        if let Some(after) = path.strip_prefix(&prefix) {
            let eff = match after.split_once('.') {
                Some((_idx, rest)) => format!("{source}.{index}.{rest}"),
                None => format!("{source}.{index}"),
            };
            return resolve(root, &eff);
        }
        if let Some(v) = resolve(item, path) {
            return Some(v);
        }
    }
    resolve(root, path)
}

/// Resolve a path to its display string, applying the placeholder policy in
/// ONE place for every element kind (variable, QR, barcode, image):
///   * resolved → its string (JSON null → empty, like everywhere else);
///   * missing + placeholder mode → a deterministic fake — EXCEPT `row.*`,
///     which never fakes, so a row value referenced outside its band looks
///     exactly as empty in the editor as it prints on paper;
///   * missing otherwise → `None`: the caller draws nothing.
pub(crate) fn resolve_or_fake(
    scope: Scope,
    root: &Value,
    path: &str,
    placeholders: bool,
) -> Option<String> {
    if let Some(v) = resolve_scoped(scope, root, path) {
        return Some(value_to_string(v));
    }
    let row_scoped = path == "row" || path.starts_with("row.");
    (placeholders && !row_scoped).then(|| fake_for(path))
}

/// Evaluate a condition in a scope (loop item + row values). Inside a loop band
/// this lets a condition target `row.last`, `row.number`, or a row-computed
/// value as naturally as a data field.
pub fn eval_condition(scope: Scope, root: &Value, cond: &Condition) -> bool {
    let found = resolve_scoped(scope, root, &cond.var);
    let present = matches!(found, Some(v) if !is_empty_value(v));
    match cond.op {
        CondOp::IsSet => present,
        CondOp::IsEmpty => !present,
        CondOp::Eq | CondOp::Ne | CondOp::Gt | CondOp::Lt | CondOp::Gte | CondOp::Lte => {
            let lhs = found.map(value_to_string).unwrap_or_default();
            let rhs = cond.value.clone();
            let ord = compare_strs(&lhs, &rhs);
            match (cond.op, ord) {
                (CondOp::Eq, Some(o)) => o == std::cmp::Ordering::Equal,
                (CondOp::Ne, Some(o)) => o != std::cmp::Ordering::Equal,
                (CondOp::Gt, Some(o)) => o == std::cmp::Ordering::Greater,
                (CondOp::Lt, Some(o)) => o == std::cmp::Ordering::Less,
                (CondOp::Gte, Some(o)) => o != std::cmp::Ordering::Less,
                (CondOp::Lte, Some(o)) => o != std::cmp::Ordering::Greater,
                _ => false,
            }
        }
    }
}

/// Order two display strings: numerically when both parse as numbers, else
/// lexicographically. The single source of truth for both `eval_condition` and
/// the formula engine's comparisons, so `x > y` means the same in each.
pub(crate) fn compare_strs(lhs: &str, rhs: &str) -> Option<std::cmp::Ordering> {
    match (lhs.trim().parse::<f64>(), rhs.trim().parse::<f64>()) {
        (Ok(a), Ok(b)) => a.partial_cmp(&b),
        _ => Some(lhs.cmp(rhs)),
    }
}

pub(crate) fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// Resolve a dotted `path` (e.g. `sale.total_amount`) against a JSON value.
/// Returns `None` if any segment is missing.
pub fn resolve<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(map) => map.get(seg)?,
            // Support numeric indexes into arrays, e.g. `items.0.qty`.
            Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Render a resolved JSON value to its display string. Numbers keep their JSON
/// formatting for now; decimal rounding / date formatting arrive in a later pass.
pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Adversarial-input cap: at most this many computed variables per document.
const MAX_COMPUTED: usize = 256;

/// Hard ceiling on loop iterations — shared by the renderer's flow walk and
/// `preview_row`, so the editor's `row.count` / `row.last` chips agree with
/// what actually prints when a data array exceeds the cap.
pub(crate) const MAX_LOOP: usize = 100_000;

/// Return a clone of the variable data `root` with every calculated variable
/// merged in under the `calc` key — the working data the renderer resolves
/// against. Call once per render when `computed` is non-empty; after this a
/// `calc.<name>` path resolves like any other data with no special-casing.
pub fn with_computed(root: &Value, computed: &[Computed]) -> Value {
    let calc = eval_computed(root, computed);
    overlay_calc(root, calc)
}

/// Evaluate every calculated variable in declaration order, returning the object
/// exposed as `calc`. Later entries may reference earlier ones (`calc.<name>`);
/// a reference to a name not yet defined resolves to null, so cycles are
/// impossible by construction (references only ever point backwards). A formula
/// that fails to parse or evaluate resolves to null (renders blank); the editor
/// surfaces the actual error separately via [`eval_computed_report`].
pub fn eval_computed(root: &Value, computed: &[Computed]) -> Value {
    let mut calc = serde_json::Map::new();
    for r in eval_all(root, computed) {
        calc.insert(r.name, r.value);
    }
    Value::Object(calc)
}

/// One computed variable's result plus any parse/evaluation error — what the
/// editor needs to show a live value and a red error line as the user types.
pub struct ComputedReport {
    /// The variable's name.
    pub name: String,
    /// The evaluated value (null when it errored).
    pub value: Value,
    /// A human-readable error, if the formula didn't parse/evaluate.
    pub error: Option<String>,
}

/// Like [`eval_computed`], but reports each formula's error instead of swallowing
/// it. Used by the editor's live preview (via the wasm `preview_computed` shim).
pub fn eval_computed_report(root: &Value, computed: &[Computed]) -> Vec<ComputedReport> {
    eval_all(root, computed)
}

/// Evaluate every calculated variable in declaration order. The data root is
/// cloned ONCE into a working object whose `calc` slot is refreshed per variable
/// (rather than re-cloning the whole root each iteration). Later entries may
/// reference earlier ones via `calc.<name>`; a reference to a name not yet
/// defined resolves to null, so cycles are impossible (references point backward).
fn eval_all(root: &Value, computed: &[Computed]) -> Vec<ComputedReport> {
    let mut working = overlay_calc(root, Value::Object(serde_json::Map::new()));
    let mut calc = serde_json::Map::new();
    let mut out = Vec::new();
    let budget = crate::expr::fresh_budget();
    for c in computed.iter().take(MAX_COMPUTED) {
        // Expose the values computed so far so this formula can build on them.
        if let Value::Object(m) = &mut working {
            m.insert("calc".to_string(), Value::Object(calc.clone()));
        }
        // One budget across ALL doc-level formulas: aggregates can't multiply
        // per-formula allowances into a stall.
        let (value, error) = match crate::expr::compile(&c.formula) {
            Ok(ast) => (
                crate::expr::eval_compiled(&ast, &working, Scope::default(), &budget),
                None,
            ),
            Err(e) => (Value::Null, Some(e)),
        };
        calc.insert(c.name.clone(), value.clone());
        out.push(ComputedReport {
            name: c.name.clone(),
            value,
            error,
        });
    }
    out
}

/// A band's row formulas, parsed once. The renderer evaluates them once per
/// loop iteration; compiling up front keeps a 100k-item loop from re-parsing
/// every formula 100k times. Reserved (implicit) names are dropped here so the
/// implicit value always wins deterministically — the editor rejects them at
/// validation time, `preview_row` reports them as errors.
pub(crate) type CompiledRow = Vec<(String, crate::expr::Compiled)>;

/// Parse a band's row formulas into a reusable [`CompiledRow`]. A formula that
/// fails to parse evaluates to null at render time (same rule as doc-level
/// computed), so it is simply omitted.
pub(crate) fn compile_row(computed: &[Computed]) -> CompiledRow {
    computed
        .iter()
        .take(MAX_COMPUTED)
        .filter(|c| !RESERVED_ROW_NAMES.contains(&c.name.as_str()))
        .filter_map(|c| {
            crate::expr::compile(&c.formula)
                .ok()
                .map(|ast| (c.name.clone(), ast))
        })
        .collect()
}

/// Seed a row object with the implicit values of iteration `i` of `count`.
/// The single definition both the renderer and the editor preview build on —
/// if the implicit set ever changes, it changes here for both.
fn seed_implicit_row(row: &mut serde_json::Map<String, Value>, iter: Option<(usize, usize)>) {
    if let Some((i, count)) = iter {
        row.insert("index".into(), Value::from(i as u64));
        row.insert("number".into(), Value::from(i as u64 + 1));
        row.insert("count".into(), Value::from(count as u64));
        row.insert("first".into(), Value::Bool(i == 0));
        row.insert("last".into(), Value::Bool(i + 1 == count));
    }
}

/// Evaluate a band's compiled row formulas for **one iteration**, returning
/// the object exposed as `row` to elements inside the band.
///
/// `iter` is `Some((index, count))` on a looping band — it fills the implicit
/// `index` / `number` / `count` / `first` / `last` — and `None` on a
/// conditional-only band (declared formulas still evaluate; implicits don't
/// exist because there is no iteration).
///
/// Formulas evaluate in declaration order; each sees the earlier ones via
/// `row.<name>` (a forward reference is null, so cycles are impossible — same
/// rule as doc-level computed). Aggregate row scans are charged to the shared
/// `budget`, bounding the whole render rather than a single formula.
pub(crate) fn eval_row(
    root: &Value,
    compiled: &CompiledRow,
    loop_ctx: LoopCtx,
    iter: Option<(usize, usize)>,
    budget: &std::cell::Cell<u64>,
) -> Value {
    let mut row = serde_json::Map::new();
    seed_implicit_row(&mut row, iter);
    for (name, ast) in compiled {
        // Lend the values so far to the formula's scope without cloning: move
        // the map into a Value for the borrow, then take it back.
        let so_far = Value::Object(std::mem::take(&mut row));
        let scope = Scope {
            loop_ctx,
            row: Some(&so_far),
        };
        let v = crate::expr::eval_compiled(ast, root, scope, budget);
        row = match so_far {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        row.insert(name.clone(), v);
    }
    Value::Object(row)
}

/// Like [`eval_row`], but for the editor: evaluates a band's (draft) row
/// formulas against the band's **first item** and reports each entry's value
/// and error — the row-scoped counterpart of [`eval_computed_report`].
///
/// `region_id` selects the band (for its `source`); `computed` is the draft
/// formula list being edited (it replaces the band's stored list, so renames
/// and reorders preview exactly as they will save). Doc-level computed are
/// merged first so `calc.*` references resolve. On a band whose source is
/// missing or empty, formulas evaluate with no row binding — field references
/// come back null, which is exactly what a render would do.
pub fn preview_row(
    doc: &TicketDoc,
    region_id: &str,
    computed: &[Computed],
    variables: &Value,
) -> Vec<ComputedReport> {
    let merged;
    let root = if doc.computed.is_empty() {
        variables
    } else {
        merged = with_computed(variables, &doc.computed);
        &merged
    };
    let src = doc
        .regions
        .iter()
        .find(|r| r.id == region_id)
        .and_then(|r| r.source.as_deref());
    let items = src.and_then(|s| match resolve(root, s) {
        Some(Value::Array(a)) if !a.is_empty() => Some(a),
        _ => None,
    });
    // Cap the count like the renderer does, so `row.count` / `row.last` chips
    // never promise something print won't do.
    let (loop_ctx, iter): (LoopCtx, Option<(usize, usize)>) = match (src, items) {
        (Some(s), Some(a)) => (Some((s, 0, &a[0])), Some((0, a.len().min(MAX_LOOP)))),
        _ => (None, None),
    };

    let mut row = serde_json::Map::new();
    seed_implicit_row(&mut row, iter);
    let budget = crate::expr::fresh_budget();
    let mut out = Vec::new();
    for c in computed.iter().take(MAX_COMPUTED) {
        if RESERVED_ROW_NAMES.contains(&c.name.as_str()) {
            out.push(ComputedReport {
                name: c.name.clone(),
                value: Value::Null,
                error: Some(format!("'{}' is a built-in row value", c.name)),
            });
            continue;
        }
        let (value, error) = match crate::expr::compile(&c.formula) {
            Ok(ast) => {
                let so_far = Value::Object(std::mem::take(&mut row));
                let scope = Scope {
                    loop_ctx,
                    row: Some(&so_far),
                };
                let v = crate::expr::eval_compiled(&ast, root, scope, &budget);
                row = match so_far {
                    Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
                (v, None)
            }
            Err(e) => (Value::Null, Some(e)),
        };
        row.insert(c.name.clone(), value.clone());
        out.push(ComputedReport {
            name: c.name.clone(),
            value,
            error,
        });
    }
    out
}

/// The editor-facing "kind" of a value: drives the default formatting when a
/// calculated variable is placed on the ticket (numbers get number formatting).
pub fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Number(_) => "number",
        Value::Null => "empty",
        _ => "text",
    }
}

/// Clone the data root and set its top-level `calc` key to the given object.
fn overlay_calc(root: &Value, calc: Value) -> Value {
    let mut obj = match root {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    obj.insert("calc".to_string(), calc);
    Value::Object(obj)
}

/// Coerce a value to f64 the same way `eval_condition` compares numerically, so
/// computes and comparisons agree. Non-numeric → None.
pub(crate) fn to_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Convert a computed f64 back to a JSON number, preferring an integer when the
/// value is whole (so `100 + 16` prints `116`, not `116.0`). Non-finite → null.
pub(crate) fn number_value(x: f64) -> Value {
    if !x.is_finite() {
        return Value::Null;
    }
    // Kill floating-point noise from chained arithmetic — e.g. `46.24 - 4.62`
    // comes out as 41.620000000000005 — by rounding to 10 decimal places. Values
    // that legitimately need more precision than that are vanishingly rare on a
    // ticket, and parity holds because the same rounding runs native and in wasm.
    let x = if x.abs() < 1e10 {
        (x * 1e10).round() / 1e10
    } else {
        x
    };
    if x.fract() == 0.0 && x.abs() < 1e15 {
        Value::Number((x as i64).into())
    } else {
        serde_json::Number::from_f64(x)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

impl TicketDoc {
    /// Every variable path this document references that does **not** resolve in
    /// `variables` (with calculated variables merged under `calc.*` first) —
    /// duplicates removed, in document order.
    ///
    /// This is what turns a silent field corruption into a visible error: the
    /// editor can show "3 fields don't exist in your sample data", and a backend
    /// can refuse to save a template referencing paths outside its variable tree.
    ///
    /// Checked: `Variable` paths, `from_variable` QR / barcode / image sources,
    /// element and region conditions (except `is_set` / `is_empty`, whose whole
    /// point is probing a maybe-absent field), and loop `source`s. Paths inside a
    /// loop band resolve against the band's first item, exactly like a render;
    /// when the loop source itself is missing or empty, its inner paths are
    /// skipped (unverifiable) rather than reported as noise. `row.<name>` is
    /// checked against the band's declared and implicit row values. Formula
    /// *bodies* are not scanned — a formula's unresolved path is already
    /// null-safe by design and surfaced by the editor's formula error line.
    pub fn unresolved_paths(&self, variables: &Value) -> Vec<String> {
        let merged;
        let root = if self.computed.is_empty() {
            variables
        } else {
            merged = with_computed(variables, &self.computed);
            &merged
        };

        let mut out: Vec<String> = Vec::new();
        let add = |out: &mut Vec<String>, p: &str| {
            if !p.is_empty() && !out.iter().any(|x| x == p) {
                out.push(p.to_string());
            }
        };

        for r in &self.regions {
            if let Some(src) = &r.source {
                if !matches!(resolve(root, src), Some(Value::Array(_))) {
                    add(&mut out, src);
                }
            }
            if let Some(c) = &r.condition {
                // Region conditions evaluate at root scope (no loop item).
                if probes_value(c) && resolve(root, &c.var).is_none() {
                    add(&mut out, &c.var);
                }
            }
        }

        for el in &self.elements {
            let band = self
                .regions
                .iter()
                .find(|r| el.row >= r.start_row && el.row < r.end_row);
            // Loop bands bind inner paths to their first item, like a render. A
            // source that is missing/empty makes inner paths unverifiable — skip
            // them (the bad source itself was already reported above).
            let loop_ctx: LoopCtx = match band.and_then(|r| r.source.as_deref()) {
                Some(src) => match resolve(root, src) {
                    Some(Value::Array(a)) if !a.is_empty() => Some((src, 0, &a[0])),
                    _ => continue,
                },
                None => None,
            };
            let check = |out: &mut Vec<String>, path: &str| {
                if path.is_empty() {
                    return;
                }
                if let Some(name) = path.strip_prefix("row.") {
                    if !row_name_exists(band, name) {
                        add(out, path);
                    }
                    return;
                }
                if path == "row" || resolve_loop(loop_ctx, root, path).is_none() {
                    add(out, path);
                }
            };
            match &el.kind {
                ElementKind::Variable { path, .. } => check(&mut out, path),
                ElementKind::Qr {
                    value,
                    from_variable,
                    ..
                }
                | ElementKind::Barcode {
                    value,
                    from_variable,
                    ..
                } => {
                    if *from_variable {
                        check(&mut out, value);
                    }
                }
                ElementKind::Image {
                    data,
                    from_variable,
                    ..
                } => {
                    if *from_variable {
                        check(&mut out, data);
                    }
                }
                ElementKind::Text { .. } => {}
            }
            if let Some(c) = &el.condition {
                if probes_value(c) {
                    check(&mut out, &c.var);
                }
            }
        }
        out
    }
}

/// Whether a condition reads the field's *value* (a typo would silently compare
/// against nothing). `is_set` / `is_empty` legitimately probe absent fields.
fn probes_value(c: &Condition) -> bool {
    !matches!(c.op, CondOp::IsSet | CondOp::IsEmpty)
}

/// Whether `row.<name>` is defined for elements inside `band`: a declared
/// row-computed value, or (on a looping band) one of the implicit names.
fn row_name_exists(band: Option<&Region>, name: &str) -> bool {
    let Some(r) = band else { return false };
    if r.computed.iter().any(|c| c.name == name) {
        return true;
    }
    r.source.is_some() && RESERVED_ROW_NAMES.contains(&name)
}

/// Produce a stable fake value for a path when no real data is available.
/// The last path segment hints at the type ("amount"/"total" -> money,
/// "time"/"date" -> timestamp, otherwise a short token).
pub fn fake_for(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path).to_lowercase();
    let seed = stable_hash(path);
    if leaf.contains("amount") || leaf.contains("total") || leaf.contains("price") {
        let cents = seed % 100_000;
        format!("{}.{:02}", cents / 100, cents % 100)
    } else if leaf.contains("volume") || leaf.contains("qty") || leaf.contains("quantity") {
        let x = seed % 10_000;
        format!("{}.{:02}", x / 100, x % 100)
    } else if leaf.contains("time") || leaf.contains("date") {
        // Deterministic-looking timestamp derived from the seed.
        let mm = 1 + (seed % 12);
        let dd = 1 + (seed / 12 % 28);
        let hh = seed / 60 % 24;
        let mi = seed % 60;
        format!("2030-{:02}-{:02} {:02}:{:02}", mm, dd, hh, mi)
    } else {
        format!("{}-{:04}", leaf.to_uppercase(), seed % 10_000)
    }
}

/// A tiny FNV-1a hash so fakes are stable without pulling in a hashing crate
/// (and without `std::collections::hash_map::RandomState`, which is non-deterministic).
fn stable_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn c(name: &str, formula: &str) -> Computed {
        Computed {
            name: name.into(),
            formula: formula.into(),
        }
    }

    #[test]
    fn computed_evaluate_in_order_under_calc_namespace() {
        let data = json!({ "sale": { "subtotal": 100, "tax": 16 } });
        let computed = vec![
            c("total", "sale.subtotal + sale.tax"),
            // References the earlier `calc.total`.
            c("label", r#"concat("Total: ", calc.total)"#),
        ];
        assert_eq!(
            eval_computed(&data, &computed),
            json!({ "total": 116, "label": "Total: 116" })
        );
        // `with_computed` exposes them so a plain `calc.total` path resolves.
        let merged = with_computed(&data, &computed);
        assert_eq!(resolve(&merged, "calc.total"), Some(&json!(116)));
    }

    #[test]
    fn forward_reference_to_a_later_calc_is_null() {
        // `a` points at `calc.b`, defined later — resolves to null, no cycle, no fake.
        let computed = vec![c("a", "calc.b"), c("b", "1")];
        assert_eq!(
            eval_computed(&Value::Null, &computed),
            json!({ "a": null, "b": 1 })
        );
    }

    #[test]
    fn report_surfaces_parse_errors() {
        let rep = eval_computed_report(&Value::Null, &[c("bad", "1 +")]);
        assert!(
            rep[0].error.is_some(),
            "a broken formula must report an error"
        );
        assert_eq!(rep[0].value, Value::Null);
    }
}
