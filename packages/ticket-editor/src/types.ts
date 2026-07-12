// TypeScript mirror of the `ticket-core` Rust schema (crates/ticket-core/src/schema.rs).
// Kept structurally identical so a TicketDoc round-trips through the wasm renderer
// unchanged. If you edit one side, edit the other.

export const SCHEMA_VERSION = 2

export type Align = 'left' | 'right' | 'center'
export type VAlign = 'top' | 'middle' | 'bottom'
export type Rounding = 'half_up' | 'half_even' | 'up' | 'down'

/** The kind of value a variable holds — drives which formatting the editor offers. */
export type VariableType = 'text' | 'number' | 'date'

export type CondOp = 'is_set' | 'is_empty' | 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte'

/** A simple non-programmer condition: `<var> <op> [value]`. */
export interface Condition {
  var: string
  op: CondOp
  value?: string
}

/**
 * A row-band with flow behaviour. `source` set → repeats once per item (loop);
 * `condition` set → collapses when false. Both → a conditional loop.
 */
export interface Region {
  id: string
  start_row: number
  end_row: number
  source?: string
  condition?: Condition
  /** Row-scoped formulas, evaluated once per loop iteration; values land under
   *  `row.<name>` for elements inside this band (e.g. `line_total = qty * price`).
   *  Looping bands also expose the implicit `row.index` / `row.number` /
   *  `row.count` / `row.first` / `row.last` without declaring anything. */
  computed?: Computed[]
}

/** `row.*` names every looping band provides implicitly — reserved, so a
 *  declared calculated column may not use them. Mirror of the Rust
 *  `RESERVED_ROW_NAMES`. */
export const RESERVED_ROW_NAMES = ['index', 'number', 'count', 'first', 'last'] as const

export interface Style {
  bold?: boolean
  italic?: boolean
  /** Integer size magnification 1–4 (thermal-printer style). */
  scale?: number
  /** Vertical placement of the glyph within its cell block (matters at scale>1). */
  valign?: VAlign
  /** Monospace font family id for this element; falls back to the doc default,
   *  then the built-in `"mono"`. Must be a family the renderer has been given. */
  font?: string
}

/** Numeric formatting for a variable. */
export interface NumberFormat {
  decimals: number
  rounding: Rounding
  thousands: boolean
}

/** A literal piece of text. */
export interface TextKind {
  type: 'text'
  content: string
}

/** A zero-ink finishing marker (`cut`, `feed`, `beep`, `drawer`, or custom):
 *  prints nothing; the backend receives it with its resolved row and maps it
 *  to the device command its printer actually honors. */
export interface MarkerKind {
  type: 'marker'
  name: string
}

/** The standardized marker intent names (mirror of Rust `MARKER_NAMES`).
 *  Unknown names are allowed — consumers ignore what they can't do. */
export const MARKER_NAMES = ['cut', 'feed', 'beep', 'drawer'] as const

/** How a color image is reduced to 1-bit black/white for a thermal printer. */
export type ImageMode = { kind: 'threshold'; level: number } | { kind: 'dither' }

/** A monochrome image occupying w×h cells: a static logo (base64 PNG embedded in
 *  `data`) or a dynamic image (signature, plot) whose base64 is resolved from a
 *  variable path when `from_variable` is true. */
export interface ImageKind {
  type: 'image'
  /** base64 PNG (with/without a `data:` prefix), or a variable path when from_variable. */
  data: string
  /** If true, `data` is a variable path resolved to the base64 at render time. */
  from_variable?: boolean
  w: number
  h: number
  mode?: ImageMode
}

/** A QR code from a variable path or literal, occupying size×size cells. */
export interface QrKind {
  type: 'qr'
  value: string
  from_variable?: boolean
  size: number
}

/** A 1D barcode symbology. */
export type Symbology = 'code128' | 'code39' | 'ean13'

/** A 1D barcode from a variable path or literal, occupying width×height cells. */
export interface BarcodeKind {
  type: 'barcode'
  value: string
  from_variable?: boolean
  symbology?: Symbology
  width: number
  height: number
}

/** A value pulled from the variable tree at render time. */
export interface VariableKind {
  type: 'variable'
  /** Dotted path into the variables object, e.g. `sale.total_amount`. */
  path: string
  /** Reserved width in characters; the value is truncated/padded to this. */
  length: number
  align?: Align
  /** Flow long values across multiple lines instead of truncating. Content
   *  below moves down to make room. */
  wrap?: boolean
  /** With `wrap`, keep at most this many lines (a longer value is cut with a
   *  trailing `…`). Unset = unbounded. */
  max_lines?: number
  /** Numeric formatting (mutually exclusive with dateFormat). */
  number?: NumberFormat
  /** Date reshaping pattern, e.g. `DD/MM/YYYY HH:mm`. */
  date_format?: string
}

export type ElementKind = TextKind | VariableKind | ImageKind | QrKind | BarcodeKind | MarkerKind

export interface Element {
  id: string
  row: number
  col: number
  /** Fine vertical nudge in rows (fractional/negative allowed). */
  y_offset?: number
  style?: Style
  // ElementKind is flattened onto the element (serde `#[serde(flatten)]`).
  type: 'text' | 'variable' | 'image' | 'qr' | 'barcode' | 'marker'
  content?: string
  // marker
  name?: string
  path?: string
  length?: number
  align?: Align
  wrap?: boolean
  max_lines?: number
  number?: NumberFormat
  date_format?: string
  // image
  data?: string
  w?: number
  h?: number
  mode?: ImageMode
  // qr
  value?: string
  from_variable?: boolean
  size?: number
  // barcode
  symbology?: Symbology
  width?: number
  height?: number
  /** Show only when this holds (hides in place — does not collapse rows). */
  condition?: Condition
}

export interface Paper {
  width_chars: number
  margin_left_chars?: number
  margin_right_chars?: number
  margin_top_lines?: number
  margin_bottom_lines?: number
  cell_width_px?: number
  cell_height_px?: number
  font_px?: number
  /** Minimum content lines; keeps trailing blank space (e.g. for a signature). */
  min_rows?: number
}

/**
 * A calculated variable: a named value derived from other variables by a small
 * formula. Exposed to the whole document at path `calc.<name>`, so a QR /
 * variable / condition uses it exactly like host data. The formula is evaluated
 * by the wasm renderer (same engine native + browser), so preview == print.
 *
 * The formula is a spreadsheet-like expression: dotted variable paths, `"text"`
 * and number literals, `+ - * / %`, comparisons and `and`/`or`, and functions
 * incl. aggregates over loop arrays — e.g.
 * `sumif(sale.movements, payment == "CASH", qty)` or `count(sale.sales)`.
 */
export interface Computed {
  /** Unique name; the value is available at `calc.<name>`. */
  name: string
  /** The formula evaluated to produce the value. */
  formula: string
}

/** One entry offered in the formula editor's "Insert variable" picker. */
export interface VarOption {
  /** Text shown in the dropdown. */
  label: string
  /** Text inserted into the formula (an absolute path, or a bare row field). */
  insert: string
}

/** A labelled group of variable options — e.g. "Values", "Lists", or the row
 *  fields of a specific list ("in each sale.movements row"). Grouping is what
 *  teaches that inside an aggregate you use a row's short field name. */
export interface VarGroup {
  label: string
  options: VarOption[]
}

/** One calculated variable's live result, from the wasm preview endpoint. */
export interface ComputedResult {
  name: string
  /** The evaluated value as a display string (empty when it errored). */
  value: string
  /** Result kind — drives default formatting when placed on the ticket. */
  kind: 'number' | 'text' | 'empty'
  /** A parse/evaluation error message, or null when the formula is valid. */
  error: string | null
}

export interface TicketDoc {
  version: number
  paper: Paper
  elements: Element[]
  /** Flow bands: loops (repeat per item) and/or conditionals (collapse when false). */
  regions?: Region[]
  /** Calculated variables, exposed under the `calc.` namespace. */
  computed?: Computed[]
  /** Default monospace font family id for the whole ticket (elements override). */
  font?: string
}

/** A node in the variable tree the host app feeds the editor. */
export interface VarNode {
  /** Leaf key or group name. */
  key: string
  /** Full dotted path from the root. */
  path: string
  /** Present only on leaves — the sample value, used to guess type/width. */
  sample?: string | number | boolean
  /** Value type (leaves only). Inferred from the sample or set explicitly. */
  type?: VariableType
  /** Present only on groups. */
  children?: VarNode[]
  /** True when this is an array of records (loopable — e.g. `items`). */
  repeatable?: boolean
}
