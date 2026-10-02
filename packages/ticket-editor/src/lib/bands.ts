// How a flow band reads to a person ("For each items · If discount > 0").
// Shared by the canvas lane and the row list so the two can't word it differently.
import type { TFn } from '../i18n'
import type { Region } from '../types'

const opLabels: Record<string, string> = {
  is_set: 'is set',
  is_empty: 'is empty',
  eq: '=',
  ne: '≠',
  gt: '>',
  lt: '<',
  gte: '≥',
  lte: '≤',
}

/** Last segment of a dotted path: `sale.items` → `items`. */
export function leaf(path: string): string {
  return path.split('.').pop() ?? path
}

function opText(op: string, t: TFn): string {
  if (op === 'is_set') return t('opIsSet')
  if (op === 'is_empty') return t('opIsEmpty')
  return opLabels[op] ?? op
}

export function bandDescription(r: Region, t: TFn): string {
  const parts: string[] = []
  if (r.source) parts.push(t('bandForEach', { name: leaf(r.source) }))
  if (r.condition) {
    const c = r.condition
    const val = c.op === 'is_set' || c.op === 'is_empty' ? '' : ` ${c.value ?? ''}`
    parts.push(t('bandIf', { cond: `${leaf(c.var)} ${opText(c.op, t)}${val}` }))
  }
  return parts.join('  ·  ')
}
