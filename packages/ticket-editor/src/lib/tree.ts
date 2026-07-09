// Turn the host app's variable data (e.g. `{ sale: { total, items: [...] } }`)
// into the tree the editor renders. Also used to guess a sensible reserved
// width for a freshly-added variable so the user rarely has to touch it.

import type { VariableType, VarNode } from '../types'

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

/** Guess a variable's type from a sample value (fallback when not declared). */
export function inferType(sample: unknown): VariableType {
  if (typeof sample === 'number') return 'number'
  if (typeof sample === 'string' && /^\d{4}-\d{2}-\d{2}([ T]\d{2}:\d{2})?/.test(sample)) return 'date'
  return 'text'
}

/** Build a VarNode tree from a sample data object. */
export function deriveTree(data: Record<string, unknown>, prefix = ''): VarNode[] {
  return Object.entries(data).map(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key
    if (Array.isArray(value)) {
      // Repeatable (loopable) collection. Expose the first element's fields so a
      // user can place `items.0.field` today; real loops come in a later pass.
      const first = value[0]
      const children = isPlainObject(first) ? deriveTree(first, `${path}.0`) : []
      return { key, path, repeatable: true, children }
    }
    if (isPlainObject(value)) {
      return { key, path, children: deriveTree(value, path) }
    }
    const sample = value as string | number | boolean
    return { key, path, sample, type: inferType(sample) }
  })
}

/** Flatten a tree into a `path -> type` map (leaves only). */
export function pathTypeMap(nodes: VarNode[], out: Record<string, VariableType> = {}) {
  for (const n of nodes) {
    if (n.children) pathTypeMap(n.children, out)
    else out[n.path] = n.type ?? 'text'
  }
  return out
}

/**
 * Produce a randomized clone of a sample data object — same shape, different leaf
 * values — so the user can spot-check the layout against varied data ("reshuffle
 * sample data"). Only meant for previewing; never for real values.
 */
export function randomizeSample<T>(data: T): T {
  const rnd = (n: number) => Math.floor(Math.random() * n)
  const walk = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(walk)
    if (isPlainObject(v)) {
      const out: Record<string, unknown> = {}
      for (const [k, val] of Object.entries(v)) out[k] = walk(val)
      return out
    }
    if (typeof v === 'number') {
      const decimals = String(v).includes('.') ? 2 : 0
      const base = rnd(100000) / (decimals ? 100 : 1)
      return decimals ? Number(base.toFixed(2)) : Math.floor(base)
    }
    if (typeof v === 'string') {
      // Keep date-looking strings date-looking, otherwise a short token.
      if (/^\d{4}-\d{2}-\d{2}/.test(v)) {
        const p2 = (n: number) => String(n).padStart(2, '0')
        return `2030-${p2(1 + rnd(12))}-${p2(1 + rnd(28))} ${p2(rnd(24))}:${p2(rnd(60))}:${p2(rnd(60))}`
      }
      const words = ['Dog Food', 'Cat Toy', 'Fish Food', 'Bird Seed', 'Leash', 'A-100294', 'Cash']
      return words[rnd(words.length)]
    }
    if (typeof v === 'boolean') return Math.random() > 0.5
    return v
  }
  return walk(data) as T
}

/** Guess a reasonable reserved character width from a sample value. */
export function guessLength(sample: unknown): number {
  if (typeof sample === 'number') {
    // Room for thousands + two decimals, e.g. "1234567.89".
    return Math.max(8, String(sample).length + 2)
  }
  if (typeof sample === 'string') {
    return Math.min(40, Math.max(6, sample.length + 2))
  }
  if (typeof sample === 'boolean') return 5
  return 12
}
