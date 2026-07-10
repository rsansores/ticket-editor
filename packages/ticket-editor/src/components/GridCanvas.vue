<script setup lang="ts">
// The structural editor. Elements are boxes on a character grid, sized by their
// real footprint (character length × size magnification, wrapped lines included),
// so the box shows exactly the space the value takes on paper.
//
// Placement is free and non-destructive (per the agreed model):
//   * dragging snaps to whole cells but is never blocked;
//   * elements past the paper edge sit in a greyed OVERFLOW ZONE, still visible
//     and draggable — nothing is ever hidden or moved on its own;
//   * overlaps are allowed but clearly flagged, never auto-resolved.
import { computed, ref } from 'vue'
import type { Element, Region, TicketDoc } from '../types'
import { footprint, overlappingIds, resolvePath, type Footprint } from '../lib/layout'
import { useT } from '../i18n'

const t = useT()

const props = defineProps<{
  doc: TicketDoc
  selectedId: string | null
  /** Currently selected band id (configured in the right drawer). */
  selectedBandId?: string | null
  zoom: number
  /** Sample data — lets a wrapped variable's box show its true line count. */
  variables?: unknown
  /** Repeatable variable groups that can drive a loop band. */
  loopSources?: { path: string; key: string }[]
  /** All leaf variables, for the condition builder. */
  allVars?: { path: string; key: string }[]
}>()
const emit = defineEmits<{
  'update:element': [el: Element]
  select: [id: string | null]
  /** Select a band (to configure it in the right drawer). */
  'select-band': [id: string | null]
  /** Insert a blank content row at this index (shifts rows below down). */
  'insert-row': [row: number, effectiveRows: number]
  /** Remove the (empty) content row at this index (pulls rows below up). */
  'delete-row': [row: number, effectiveRows: number]
  /** Create a flow band over a row range. */
  'create-region': [region: Omit<Region, 'id'>]
  /** Remove a band. */
  'remove-region': [id: string]
}>()

const cw = computed(() => (props.doc.paper.cell_width_px ?? 12) * props.zoom)
const ch = computed(() => (props.doc.paper.cell_height_px ?? 22) * props.zoom)
const width = computed(() => props.doc.paper.width_chars)
const ml = computed(() => props.doc.paper.margin_left_chars ?? 0)
const mr = computed(() => props.doc.paper.margin_right_chars ?? 0)
const mt = computed(() => props.doc.paper.margin_top_lines ?? 0)
const mb = computed(() => props.doc.paper.margin_bottom_lines ?? 0)
const contentCols = computed(() => Math.max(1, width.value - ml.value - mr.value))

function sampleValue(el: Element): string | undefined {
  if (el.type !== 'variable' || !el.path) return undefined
  return resolvePath(props.variables, el.path)
}
function computeFp(el: Element): Footprint {
  if (el.type === 'image') {
    const w = Math.max(1, el.w ?? 1)
    const h = Math.max(1, el.h ?? 1)
    return { scale: 1, bandChars: w, lines: h, cols: w, rows: h }
  }
  if (el.type === 'qr') {
    const s = Math.max(1, el.size ?? 1)
    // The QR is a pixel-square (s cells wide); its height in rows follows the cell aspect.
    const rows = Math.max(1, Math.ceil((s * cw.value) / ch.value))
    return { scale: 1, bandChars: s, lines: 1, cols: s, rows }
  }
  if (el.type === 'barcode') {
    const w = Math.max(1, el.width ?? 1)
    const h = Math.max(1, el.height ?? 1)
    return { scale: 1, bandChars: w, lines: 1, cols: w, rows: h }
  }
  return footprint(el, contentCols.value, sampleValue(el))
}
// Footprints memoized once per render — every layout computed and the O(n²)
// overlap check reuse this instead of recomputing (which thrashed on drag).
const fpMap = computed(() => {
  const m = new Map<string, Footprint>()
  for (const el of props.doc.elements) m.set(el.id, computeFp(el))
  return m
})
function fp(el: Element): Footprint {
  return fpMap.value.get(el.id) ?? computeFp(el)
}

// Grid extents: wide enough to show the furthest (possibly off-paper) element.
const displayCols = computed(() => {
  const furthest = props.doc.elements.reduce(
    (m, e) => Math.max(m, ml.value + e.col + fp(e).cols),
    width.value,
  )
  return furthest + 1
})
// Content height in rows: the greater of the lowest element and the explicit
// minimum (trailing whitespace). This is exactly what the renderer produces, so
// the editor grid matches the printed ticket — no phantom rows.
const lowestBottom = computed(() =>
  props.doc.elements.reduce((m, e) => Math.max(m, e.row + fp(e).rows), 0),
)
const effectiveRows = computed(() =>
  Math.max(lowestBottom.value, props.doc.paper.min_rows ?? 0, 1),
)
const displayRows = computed(() => mt.value + effectiveRows.value + mb.value)

const printableRight = computed(() => width.value - mr.value)
const overlapping = computed(() => overlappingIds(props.doc.elements, contentCols.value, fp))

// ---- row gutter: insert / remove clean lines -------------------------------
// Which content rows any element occupies (a scaled/wrapped element spans more
// than one). Used to only allow deleting genuinely empty rows.
const occupiedRows = computed(() => {
  const set = new Set<number>()
  for (const el of props.doc.elements) {
    const f = fp(el)
    for (let r = el.row; r < el.row + f.rows; r++) set.add(r)
  }
  return set
})
// The gutter shows exactly the ticket's content rows — no phantom rows. A
// single "append" slot below them lets you grow the ticket (e.g. add space for
// a signature).
const gutterRows = computed(() => effectiveRows.value)
function isRowEmpty(cr: number): boolean {
  return !occupiedRows.value.has(cr)
}

// ---- flow bands (loops / conditions) ---------------------------------------
const regionList = computed<Region[]>(() => props.doc.regions ?? [])
function regionOf(row: number): Region | undefined {
  return regionList.value.find((r) => row >= r.start_row && row < r.end_row)
}
function regionIndex(id: string): number {
  return regionList.value.findIndex((r) => r.id === id) + 1
}
const opLabels: Record<string, string> = {
  is_set: 'is set', is_empty: 'is empty', eq: '=', ne: '≠', gt: '>', lt: '<', gte: '≥', lte: '≤',
}
function leaf(path: string): string {
  return path.split('.').pop() ?? path
}
function opText(op: string): string {
  if (op === 'is_set') return t('opIsSet')
  if (op === 'is_empty') return t('opIsEmpty')
  return opLabels[op] ?? op
}
function bandLabel(r: Region): string {
  const n = regionIndex(r.id)
  const parts: string[] = []
  if (r.source) parts.push(t('bandForEach', { name: leaf(r.source) }))
  if (r.condition) {
    const c = r.condition
    const val = c.op === 'is_set' || c.op === 'is_empty' ? '' : ` ${c.value ?? ''}`
    parts.push(t('bandIf', { cond: `${leaf(c.var)} ${opText(c.op)}${val}` }))
  }
  return `${n}. ${parts.join('  ·  ')}`
}

// ---- band lane (create / select bands like git-gutter change bars) ---------
// A band is anchored at a row; its span is set later in the right drawer, so
// there's no multi-select. Clicking a bar selects the band for configuration.
function isBandStart(row: number): boolean {
  return regionList.value.some((r) => r.start_row === row)
}
function bandCellClass(row: number) {
  const r = regionOf(row)
  return {
    'in-band': !!r,
    loop: !!r?.source,
    cond: !!r && !r.source && !!r.condition,
    selected: !!r && r.id === props.selectedBandId,
  }
}
function bandIcon(row: number): string {
  const r = regionOf(row)
  if (!r) return ''
  return r.source ? '↻' : r.condition ? '?' : '▤'
}
function onBandCell(row: number) {
  const r = regionOf(row)
  if (r) {
    emit('select-band', r.id)
  } else {
    // Create a band anchored here (span 1). Default to a loop over the first
    // repeatable if any (the common case); the drawer lets you change it.
    const src = props.loopSources?.[0]?.path
    emit('create-region', { start_row: row, end_row: row + 1, source: src })
  }
}

function isOffPaper(el: Element): boolean {
  const f = fp(el)
  return ml.value + el.col + f.cols > printableRight.value || ml.value + el.col < ml.value
}
// Paths that exist in the current variable catalog (host data + calc vars).
const knownPaths = computed(() => new Set((props.allVars ?? []).map((v) => v.path)))
const loopPrefixes = computed(() => (props.loopSources ?? []).map((l) => `${l.path}.`))
// A loop-relative field (`sale.items.0.qty`) counts as known whenever its list
// is — even when the sample array is empty, so it isn't in `allVars`.
function pathKnown(path: string): boolean {
  return knownPaths.value.has(path) || loopPrefixes.value.some((p) => path.startsWith(p))
}
// An element that references a variable NOT in the catalog — e.g. a design
// imported into a system with different variables. Flagged so the user can
// remove it or point it at a real variable.
function isUnavailable(el: Element): boolean {
  if (el.type === 'variable') return !!el.path && !pathKnown(el.path)
  if ((el.type === 'qr' || el.type === 'barcode') && el.from_variable)
    return !!el.value && !pathKnown(el.value)
  if (el.type === 'image' && el.from_variable) return !!el.data && !pathKnown(el.data)
  return false
}
function label(el: Element): string {
  if (isUnavailable(el)) return t('unavailable')
  return el.type === 'variable' ? el.path ?? '' : el.content ?? ''
}

// ---- drag (free, snaps to cells, never blocked) ----------------------------
const drag = ref<{ id: string; startX: number; startY: number; row: number; col: number } | null>(
  null,
)
function onPointerDown(e: PointerEvent, el: Element) {
  ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  emit('select', el.id)
  drag.value = { id: el.id, startX: e.clientX, startY: e.clientY, row: el.row, col: el.col }
}
function onPointerMove(e: PointerEvent) {
  const d = drag.value
  if (!d) return
  const dc = Math.round((e.clientX - d.startX) / cw.value)
  const dr = Math.round((e.clientY - d.startY) / ch.value)
  const el = props.doc.elements.find((x) => x.id === d.id)
  if (!el) return
  const nextCol = Math.max(0, d.col + dc)
  const nextRow = Math.max(0, d.row + dr)
  if (nextCol !== el.col || nextRow !== el.row) {
    emit('update:element', { ...el, col: nextCol, row: nextRow })
  }
}
function onPointerUp() {
  drag.value = null
}
</script>

<template>
  <div
    class="te-canvas-wrap"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
  >
    <div class="te-stage" :style="{ height: displayRows * ch + 'px' }">
      <!-- row gutter: add a blank line (+) or remove an empty one (−) -->
      <div class="te-gutter">
        <div
          v-for="cr in gutterRows"
          :key="cr - 1"
          class="te-gutter-row"
          :class="{ empty: isRowEmpty(cr - 1) }"
          :style="{ top: (mt + (cr - 1)) * ch + 'px', height: ch + 'px' }"
        >
          <button
            class="te-lane"
            type="button"
            :class="bandCellClass(cr - 1)"
            :title="regionOf(cr - 1) ? bandLabel(regionOf(cr - 1)!) + ' — ' + t('bandConfigure') : t('bandCreate')"
            :aria-label="regionOf(cr - 1) ? bandLabel(regionOf(cr - 1)!) + ' — ' + t('bandConfigure') : t('bandCreate')"
            @click="onBandCell(cr - 1)"
          >
            <span v-if="isBandStart(cr - 1)" class="te-lane-ico">{{ bandIcon(cr - 1) }}</span>
            <span v-else-if="!regionOf(cr - 1)" class="te-lane-plus">+</span>
          </button>
          <span class="te-gutter-num">{{ cr - 1 }}</span>
          <button class="te-gutter-btn ins" type="button"
            :title="t('addLine', { n: cr - 1 })" :aria-label="t('addLine', { n: cr - 1 })"
            @click="emit('insert-row', cr - 1, effectiveRows)">+</button>
          <button
            class="te-gutter-btn del"
            type="button"
            :disabled="!isRowEmpty(cr - 1)"
            :title="isRowEmpty(cr - 1) ? t('removeLine', { n: cr - 1 }) : t('rowNotEmpty')"
            :aria-label="isRowEmpty(cr - 1) ? t('removeLine', { n: cr - 1 }) : t('rowNotEmpty')"
            @click="emit('delete-row', cr - 1, effectiveRows)"
          >−</button>
        </div>
        <!-- append slot: grow the ticket at the end (space for a signature, etc.) -->
        <div
          class="te-gutter-row append"
          :style="{ top: (mt + effectiveRows) * ch + 'px', height: ch + 'px' }"
        >
          <span class="te-lane" style="cursor: default" />
          <span class="te-gutter-num">end</span>
          <button class="te-gutter-btn ins" type="button"
            :title="t('addLineEnd')" :aria-label="t('addLineEnd')"
            @click="emit('insert-row', effectiveRows, effectiveRows)">+</button>
        </div>
      </div>

      <div
        class="te-canvas"
        :style="{
          width: displayCols * cw + 'px',
          height: displayRows * ch + 'px',
          backgroundSize: cw + 'px ' + ch + 'px',
        }"
        @pointerdown.self="emit('select', null)"
      >
      <!-- the paper itself (0..width_chars); everything outside is overflow -->
      <div class="te-paper" :style="{ width: width * cw + 'px', height: '100%' }" />
      <!-- printable-area guide (inside the margins) -->
      <div
        class="te-margin"
        :style="{
          left: ml * cw + 'px',
          top: mt * ch + 'px',
          width: (width - ml - mr) * cw + 'px',
          bottom: mb * ch + 'px',
        }"
      />

      <!-- flow bands: a subtle row tint (no labels over content); the lane and
           the right drawer carry the details -->
      <div
        v-for="r in regionList"
        :key="r.id"
        class="te-band"
        :class="{ loop: !!r.source, cond: !r.source && !!r.condition, selected: r.id === selectedBandId }"
        :title="bandLabel(r)"
        :style="{
          left: ml * cw + 'px',
          top: (mt + r.start_row) * ch + 'px',
          width: (width - ml - mr) * cw + 'px',
          height: (r.end_row - r.start_row) * ch + 'px',
        }"
      />

      <div
        v-for="el in doc.elements"
        :key="el.id"
        class="te-el"
        :class="{
          selected: el.id === selectedId,
          variable: el.type === 'variable',
          media: el.type === 'image' || el.type === 'qr' || el.type === 'barcode',
          overlap: overlapping.has(el.id),
          offpaper: isOffPaper(el),
          unavailable: isUnavailable(el),
        }"
        :style="{
          left: (ml + el.col) * cw + 'px',
          top: (mt + el.row + (el.y_offset ?? 0)) * ch + 'px',
          width: fp(el).cols * cw + 'px',
          height: fp(el).rows * ch + 'px',
          alignItems:
            (el.style?.valign ?? 'middle') === 'top'
              ? 'flex-start'
              : (el.style?.valign ?? 'middle') === 'bottom'
                ? 'flex-end'
                : 'center',
          fontSize: ch * 0.6 * fp(el).scale + 'px',
          fontWeight: el.style?.bold ? 700 : 400,
          fontStyle: el.style?.italic ? 'italic' : 'normal',
        }"
        @pointerdown="onPointerDown($event, el)"
      >
        <img v-if="el.type === 'image' && el.data && !el.from_variable" :src="el.data" class="te-el-img" alt="logo" draggable="false" />
        <span v-else-if="el.type === 'image'" class="te-el-ph">{{ isUnavailable(el) ? t('unavailable') : el.from_variable ? '⟳ ' + (el.data ? leaf(el.data) : 'image') : 'image' }}</span>
        <span v-else-if="el.type === 'qr'" class="te-el-ph">▦ QR</span>
        <span v-else-if="el.type === 'barcode'" class="te-el-ph">{{ isUnavailable(el) ? t('unavailable') : '▏▍▏▍ ' + (el.symbology ?? 'code128') }}</span>
        <span v-else class="te-el-text">{{ label(el) }}</span>
        <span v-if="isUnavailable(el)" class="te-el-badge unavail" :title="t('unavailableTip')">⚠</span>
        <span v-if="el.type === 'variable' && el.wrap" class="te-el-badge wrap" title="Wraps to multiple lines">↩{{ fp(el).lines }}</span>
        <span v-if="overlapping.has(el.id)" class="te-el-badge warn" title="Overlaps another element">⚠</span>
        <span v-if="isOffPaper(el)" class="te-el-badge off" title="Extends past the paper edge">⇥</span>
      </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.te-canvas-wrap {
  overflow: auto;
  padding: 1.25rem;
  /* the overflow zone: greyed area beyond the paper */
  background:
    repeating-linear-gradient(45deg, transparent 0 6px, rgba(0, 0, 0, 0.03) 6px 12px),
    var(--te-muted);
  border-radius: var(--te-radius);
  flex: 1;
  min-height: 0;
}
.te-stage {
  display: flex;
  align-items: flex-start;
  gap: 4px;
  width: max-content;
  margin: 0 auto;
}
.te-gutter {
  position: relative;
  flex: none;
  width: 96px;
  align-self: stretch;
  background: var(--te-muted);
  border-radius: var(--te-radius) 0 0 var(--te-radius);
}
.te-gutter-row {
  position: absolute;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 5px;
}
.te-gutter-num {
  font-family: ui-monospace, 'DejaVu Sans Mono', monospace;
  font-size: 11px;
  line-height: 1;
  color: var(--te-muted-fg);
  opacity: 0.65;
  min-width: 18px;
  text-align: right;
  user-select: none;
}
/* band lane: a git-style bar column; click to create / select a band */
.te-lane {
  flex: none;
  width: 16px;
  height: 100%;
  min-height: 16px;
  align-self: stretch;
  padding: 0;
  border: 0;
  border-radius: 3px;
  background: transparent;
  color: var(--te-primary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  line-height: 1;
}
.te-lane-plus {
  opacity: 0;
  font-weight: 700;
}
.te-lane:hover .te-lane-plus {
  opacity: 0.5;
}
.te-lane.in-band {
  background: color-mix(in srgb, #d97706 30%, transparent);
}
.te-lane.in-band.loop {
  background: color-mix(in srgb, var(--te-primary) 30%, transparent);
}
.te-lane.in-band.selected {
  background: color-mix(in srgb, var(--te-primary) 55%, transparent);
  outline: 2px solid var(--te-ring);
}
.te-lane.in-band.cond.selected {
  background: color-mix(in srgb, #d97706 55%, transparent);
}
.te-lane-ico {
  font-weight: 700;
  color: var(--te-primary);
}
.te-lane.cond .te-lane-ico {
  color: #d97706;
}
.te-gutter-row.append {
  border-top: 1px dashed var(--te-border);
}
.te-gutter-row.append .te-gutter-num {
  font-size: 9px;
  opacity: 0.5;
}
.te-gutter-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: 18px;
  height: 18px;
  padding: 0;
  border: 1px solid var(--te-border);
  border-radius: 4px;
  background: var(--te-card);
  color: var(--te-muted-fg);
  cursor: pointer;
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
}
.te-gutter-btn.ins {
  color: var(--te-primary);
  border-color: color-mix(in srgb, var(--te-primary) 45%, var(--te-border));
}
.te-gutter-btn.ins:hover {
  background: color-mix(in srgb, var(--te-primary) 16%, var(--te-card));
}
.te-gutter-btn.del {
  color: #dc2626;
  border-color: color-mix(in srgb, #dc2626 45%, var(--te-border));
}
.te-gutter-btn.del:hover {
  background: color-mix(in srgb, #dc2626 14%, var(--te-card));
}
.te-gutter-btn:disabled {
  color: var(--te-border);
  border-color: var(--te-border);
  background: transparent;
  cursor: default;
}
.te-canvas {
  position: relative;
  background-image: linear-gradient(to right, rgba(0, 0, 0, 0.05) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(0, 0, 0, 0.05) 1px, transparent 1px);
}
.te-paper {
  position: absolute;
  left: 0;
  top: 0;
  background: var(--te-card);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
}
/* flow bands: a subtle row tint on the canvas (details live in the lane + drawer) */
.te-band {
  position: absolute;
  z-index: 1;
  border-radius: 3px;
  background: color-mix(in srgb, #f59e0b 7%, transparent);
  pointer-events: none;
}
.te-band.loop {
  background: color-mix(in srgb, var(--te-primary) 7%, transparent);
}
.te-band.selected {
  outline: 1px dashed var(--te-ring);
  background: color-mix(in srgb, var(--te-primary) 12%, transparent);
}
.te-margin {
  position: absolute;
  border: 1px dashed var(--te-border);
  pointer-events: none;
}
.te-el {
  position: absolute;
  display: flex;
  align-items: flex-start;
  font-family: ui-monospace, 'DejaVu Sans Mono', monospace;
  line-height: 1;
  white-space: nowrap;
  overflow: hidden;
  cursor: grab;
  border-radius: 2px;
  user-select: none;
  touch-action: none;
}
.te-el.variable {
  background: color-mix(in srgb, var(--te-primary) 12%, transparent);
  outline: 1px solid color-mix(in srgb, var(--te-primary) 45%, transparent);
  color: var(--te-primary);
}
.te-el.media {
  align-items: stretch;
  background: color-mix(in srgb, var(--te-muted-fg) 6%, transparent);
  outline: 1px dashed color-mix(in srgb, var(--te-muted-fg) 45%, transparent);
  overflow: hidden;
}
.te-el-img {
  width: 100%;
  height: 100%;
  object-fit: fill;
  display: block;
  /* let the drag land on the element box, not the native image drag */
  pointer-events: none;
  -webkit-user-drag: none;
  user-select: none;
}
.te-el-ph {
  margin: auto;
  font-size: 0.7rem;
  color: var(--te-muted-fg);
  white-space: nowrap;
}
.te-el.selected {
  outline: 2px solid var(--te-ring);
  z-index: 3;
}
.te-el.overlap {
  outline: 2px solid #d97706;
  background: color-mix(in srgb, #f59e0b 16%, transparent);
}
.te-el.offpaper {
  outline: 2px dashed #dc2626;
  opacity: 0.85;
}
/* references a variable that doesn't exist in the current catalog */
.te-el.unavailable {
  outline: 2px solid #dc2626;
  background: color-mix(in srgb, #dc2626 15%, transparent);
  color: #dc2626;
}
.te-el-badge.unavail {
  right: -2px;
  color: #dc2626;
}
.te-el:active {
  cursor: grabbing;
}
.te-el-text {
  padding: 0 1px;
  overflow: hidden;
  text-overflow: clip;
}
.te-el-badge {
  position: absolute;
  top: -2px;
  font-size: 10px;
  line-height: 1;
  padding: 1px 2px;
  border-radius: 3px;
  background: var(--te-card);
}
.te-el-badge.wrap {
  right: -2px;
  color: var(--te-primary);
}
.te-el-badge.warn {
  right: 12px;
  color: #d97706;
}
.te-el-badge.off {
  right: 26px;
  color: #dc2626;
}
</style>
