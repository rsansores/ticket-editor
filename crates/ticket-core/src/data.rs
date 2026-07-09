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

use crate::schema::{CondOp, Condition};

/// A loop context: the array path being looped, the current index, and the item.
pub type LoopCtx<'a> = Option<(&'a str, usize, &'a Value)>;

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

/// Evaluate a condition in an optional loop context.
pub fn eval_condition(loop_ctx: LoopCtx, root: &Value, cond: &Condition) -> bool {
    let found = resolve_loop(loop_ctx, root, &cond.var);
    let present = matches!(found, Some(v) if !is_empty_value(v));
    match cond.op {
        CondOp::IsSet => present,
        CondOp::IsEmpty => !present,
        CondOp::Eq | CondOp::Ne | CondOp::Gt | CondOp::Lt | CondOp::Gte | CondOp::Lte => {
            let lhs = found.map(value_to_string).unwrap_or_default();
            let rhs = cond.value.clone();
            // Numeric compare when both parse; else lexicographic for eq/ne.
            let ord = match (lhs.trim().parse::<f64>(), rhs.trim().parse::<f64>()) {
                (Ok(a), Ok(b)) => a.partial_cmp(&b),
                _ => Some(lhs.cmp(&rhs)),
            };
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

fn is_empty_value(v: &Value) -> bool {
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

/// Interpolate `{dotted.path}` tokens in a template string against the data.
///
/// This is what lets a designer build a *computed* value in the editor without
/// any backend code — e.g. a QR whose value is
/// `https://maps.google.com/?q={reception_unit.latitude},{reception_unit.longitude}`,
/// or a text line `Total: {sale.total}`. Because it runs here in `ticket-core`,
/// the browser preview (wasm) and the printed ticket (native) evaluate it
/// identically — parity is preserved by construction.
///
/// Each token resolves through the same path/loop logic as a `Variable`
/// element, falling back to a deterministic fake when the path is absent (so the
/// editor preview is never blank). Literal braces are written `{{` and `}}`.
/// An unterminated `{` is emitted verbatim.
pub fn interpolate(template: &str, loop_ctx: LoopCtx, root: &Value) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut path = String::new();
                let mut closed = false;
                for pc in chars.by_ref() {
                    if pc == '}' {
                        closed = true;
                        break;
                    }
                    path.push(pc);
                }
                if !closed {
                    // Unterminated token: emit literally so nothing is silently lost.
                    out.push('{');
                    out.push_str(&path);
                    break;
                }
                let key = path.trim();
                let value = match resolve_loop(loop_ctx, root, key) {
                    Some(v) => value_to_string(v),
                    None => fake_for(key),
                };
                out.push_str(&value);
            }
            other => out.push(other),
        }
    }
    out
}

/// True when a template contains at least one `{path}` token (cheap pre-check so
/// plain literals skip interpolation entirely).
pub fn has_tokens(template: &str) -> bool {
    let bytes = template.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' {
            if bytes[i + 1] == b'{' {
                i += 2;
                continue;
            }
            return true;
        }
        i += 1;
    }
    false
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

    #[test]
    fn interpolate_resolves_tokens_and_escapes() {
        let data = json!({ "ru": { "lat": "19.4", "lng": "-99.1" }, "sale": { "total": 42 } });
        assert_eq!(
            interpolate("q={ru.lat},{ru.lng}", None, &data),
            "q=19.4,-99.1"
        );
        assert_eq!(interpolate("Total: {sale.total}", None, &data), "Total: 42");
        // Literal braces.
        assert_eq!(interpolate("{{literal}}", None, &data), "{literal}");
        // No tokens is a passthrough.
        assert_eq!(interpolate("plain text", None, &data), "plain text");
        // Unterminated token emitted verbatim, nothing lost.
        assert_eq!(interpolate("a {oops", None, &data), "a {oops");
    }

    #[test]
    fn has_tokens_distinguishes_literals() {
        assert!(has_tokens("a {b} c"));
        assert!(!has_tokens("a {{b}} c"));
        assert!(!has_tokens("plain"));
    }
}
