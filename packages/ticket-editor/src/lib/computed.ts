// Helpers for the calculated-variable formula editor. Evaluation itself lives in
// the wasm renderer (crates/ticket-core/src/expr.rs) so preview == print — this
// file only holds editor-side metadata: name validation and the function catalog
// that powers the "Insert function" picker and its inline documentation.

/** Names must be `^[A-Za-z_][A-Za-z0-9_]*$` so `calc.<name>` is a clean path. */
export function isValidName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name)
}

/** A function offered in the editor's picker. `insert` is what gets typed in;
 *  `caretBack` moves the cursor back that many chars (to land inside the `()`). */
export interface FnDoc {
  name: string
  /** Signature shown in the dropdown, e.g. `sumif(list, condition, value)`. */
  sig: string
  /** One-line description (i18n key resolved by the component). */
  descKey: string
  insert: string
  caretBack: number
}

// The catalog. Signatures are shown verbatim so the editor is self-documenting.
export const FUNCTIONS: FnDoc[] = [
  { name: 'concat', sig: 'concat(text, …)', descKey: 'fnConcat', insert: 'concat()', caretBack: 1 },
  { name: 'round', sig: 'round(number, decimals)', descKey: 'fnRound', insert: 'round()', caretBack: 1 },
  { name: 'min', sig: 'min(a, b, …)', descKey: 'fnMin', insert: 'min()', caretBack: 1 },
  { name: 'max', sig: 'max(a, b, …)', descKey: 'fnMax', insert: 'max()', caretBack: 1 },
  { name: 'abs', sig: 'abs(number)', descKey: 'fnAbs', insert: 'abs()', caretBack: 1 },
  { name: 'coalesce', sig: 'coalesce(a, b, …)', descKey: 'fnCoalesce', insert: 'coalesce()', caretBack: 1 },
  { name: 'count', sig: 'count(list)', descKey: 'fnCount', insert: 'count()', caretBack: 1 },
  { name: 'countif', sig: 'countif(list, condition)', descKey: 'fnCountif', insert: 'countif()', caretBack: 1 },
  { name: 'sum', sig: 'sum(list, value)', descKey: 'fnSum', insert: 'sum()', caretBack: 1 },
  { name: 'sumif', sig: 'sumif(list, condition, value)', descKey: 'fnSumif', insert: 'sumif()', caretBack: 1 },
  { name: 'avg', sig: 'avg(list, value)', descKey: 'fnAvg', insert: 'avg()', caretBack: 1 },
  { name: 'avgif', sig: 'avgif(list, condition, value)', descKey: 'fnAvgif', insert: 'avgif()', caretBack: 1 },
]
