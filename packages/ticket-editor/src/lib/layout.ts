// Editor-side mirror of the renderer's `fit_lines` (crates/ticket-core/src/render.rs).
// It computes the *footprint* of an element in base grid cells so the canvas can
// draw an accurate box (size magnification + text wrapping) and detect overlaps.
// Keep this in sync with the Rust side.
//
// `length` on a variable is its horizontal WIDTH in characters (same meaning
// wrapped or not). Without wrap a longer value is truncated; with wrap it flows
// onto more lines, each `length` columns wide. How many lines that is depends on
// the actual value, so wrapped footprints take the resolved sample value as
// input — the preview pane remains the exact source of truth.

import type { Element } from '../types'

export interface Footprint {
  scale: number
  /** Band width in characters (columns / scale). */
  bandChars: number
  /** Number of text lines the element occupies. */
  lines: number
  /** Width in base grid cells. */
  cols: number
  /** Height in base grid cells. */
  rows: number
}

/** Resolve a dotted path against sample data (mirror of Rust `data::resolve`). */
export function resolvePath(root: unknown, path: string): string | undefined {
  let cur: unknown = root
  for (const seg of path.split('.')) {
    if (Array.isArray(cur)) {
      const i = Number(seg)
      if (!Number.isInteger(i)) return undefined
      cur = cur[i]
    } else if (cur && typeof cur === 'object') {
      cur = (cur as Record<string, unknown>)[seg]
    } else {
      return undefined
    }
    if (cur == null) return undefined
  }
  if (typeof cur === 'object') return undefined
  return String(cur)
}

/** Greedy word wrap to `width` columns (mirror of Rust `wrap_text`). */
export function wrapText(s: string, width: number): string[] {
  width = Math.max(1, width)
  const lines: string[] = []
  let cur = ''
  let curLen = 0
  for (const word of s.split(/\s+/).filter(Boolean)) {
    const wlen = [...word].length
    if (wlen > width) {
      if (curLen > 0) {
        lines.push(cur)
        cur = ''
        curLen = 0
      }
      const chars = [...word]
      for (let i = 0; i < chars.length; i += width) {
        const chunk = chars.slice(i, i + width).join('')
        if (i + width < chars.length) lines.push(chunk)
        else {
          cur = chunk
          curLen = chunk.length
        }
      }
      continue
    }
    const needed = curLen === 0 ? wlen : curLen + 1 + wlen
    if (needed > width) {
      lines.push(cur)
      cur = word
      curLen = wlen
    } else {
      if (curLen > 0) {
        cur += ' '
        curLen += 1
      }
      cur += word
      curLen += wlen
    }
  }
  lines.push(cur)
  return lines
}

/**
 * Compute an element's footprint.
 * @param contentCols printable width (paper width minus horizontal margins)
 * @param value       resolved sample value; needed to size a wrapped variable
 */
export function footprint(el: Element, contentCols: number, value?: string): Footprint {
  const scale = Math.min(4, Math.max(1, el.style?.scale ?? 1))
  const avail = Math.max(1, contentCols - el.col)
  const cap = Math.max(1, Math.floor(avail / scale))

  if (el.type === 'variable') {
    const bandChars = Math.min(el.length ?? 1, cap)
    let lines = 1
    if (el.wrap && value != null) {
      lines = Math.max(1, wrapText(value, bandChars).length)
      if (el.max_lines) lines = Math.min(lines, Math.max(1, el.max_lines))
    }
    // Design-time footprint is ONE line even when wrapping: the renderer now
    // reflows content below a wrapped value, so its extra lines never collide
    // with anything — they insert rows. `lines` still reports the sample's
    // true line count for the canvas badge.
    return { scale, bandChars, lines, cols: bandChars * scale, rows: scale }
  }

  // Static text: single line, no reserved width; may run into the overflow zone.
  // Count code points (not UTF-16 units) to match the Rust renderer's char count.
  const bandChars = [...(el.content ?? '')].length || 1
  return { scale, bandChars, lines: 1, cols: bandChars * scale, rows: scale }
}

/** Do two elements' occupied cell rectangles intersect? */
function rectsOverlap(a: Element, b: Element, fa: Footprint, fb: Footprint): boolean {
  return (
    a.col < b.col + fb.cols &&
    b.col < a.col + fa.cols &&
    a.row < b.row + fb.rows &&
    b.row < a.row + fa.rows
  )
}

/** Ids of every element that overlaps at least one other. */
export function overlappingIds(
  elements: Element[],
  contentCols: number,
  fpOf: (el: Element) => Footprint,
): Set<string> {
  const hit = new Set<string>()
  for (let i = 0; i < elements.length; i++) {
    for (let j = i + 1; j < elements.length; j++) {
      if (rectsOverlap(elements[i], elements[j], fpOf(elements[i]), fpOf(elements[j]))) {
        hit.add(elements[i].id)
        hit.add(elements[j].id)
      }
    }
  }
  return hit
}
