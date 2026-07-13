// Known thermal paper formats.
//
// A receipt printer's grid is not free: `width_chars × cell_width_px` has to land
// on the printer's dot width, or every dot the renderer draws is off by a
// fraction and the print no longer matches the preview at 1:1.
//
// The editor used to make you discover that backwards — set a column count, and
// get scolded if the arithmetic missed. These presets invert it: pick the paper
// you actually loaded, get a column count that fits it.
//
// # Paper width vs printable width
//
// These are NOT the same number, and mixing them up is the classic way to end up
// with a ticket that is 20% too narrow. You *buy* 58 mm or 80 mm paper; the print
// head inks a narrower strip of it (48 mm and 72 mm respectively). The presets
// are labelled by the roll — the thing in the user's hand — while the dot width
// is what the renderer needs.
//
// Dots are at 203 dpi (8 dots/mm), which is what essentially every ESC/POS
// receipt printer is. See `print.ts`.

/**
 * Thermal printers are 203 dpi — exactly 8 dots per millimetre. This is what
 * ties a raster to physical paper: a 576-dot-wide ticket is 72 mm across.
 */
export const DOTS_PER_MM = 8

export interface PaperPreset {
  /** Stable id, also the `<select>` value. */
  id: string
  /** Width of the roll you buy, in mm — how the user thinks about it. */
  paperMm: number
  /** Width the print head can actually ink, in mm. */
  printableMm: number
  /** The printer's dot width. What `width_chars × cell_width_px` must equal. */
  dots: number
  /** Columns at the default 12 px cell — a comfortable, readable receipt font. */
  cols: number
  /** Cell width in px. 12 px is the standard "Font A" cell at 203 dpi. */
  cellPx: number
}

/**
 * The two formats worth a preset. 58 mm and 80 mm are the overwhelming majority
 * of receipt printers; 82.5 mm and 112 mm exist but are rare enough that Custom
 * serves them better than a cluttered menu.
 */
export const PAPER_PRESETS: readonly PaperPreset[] = [
  { id: '58', paperMm: 58, printableMm: 48, dots: 384, cols: 32, cellPx: 12 },
  { id: '80', paperMm: 80, printableMm: 72, dots: 576, cols: 48, cellPx: 12 },
] as const

/** What a new document should be. 80 mm is the retail/POS standard. */
export const DEFAULT_PRESET = PAPER_PRESETS[1]

/** Every dot width we consider "known good". */
export const STANDARD_DOT_WIDTHS: readonly number[] = PAPER_PRESETS.map((p) => p.dots)

/** The preset a document currently matches, if any. */
export function presetForDotWidth(dotWidth: number): PaperPreset | undefined {
  return PAPER_PRESETS.find((p) => p.dots === dotWidth)
}
