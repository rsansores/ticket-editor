<script setup lang="ts">
// The ticket as a list of lines — the phone's editor. A receipt is mostly
// one-dimensional (lines, with a few things side by side), so on a narrow
// screen it edits as a list instead of a 2D grid: tap an element to edit it
// (row, column and alignment live in its properties). Line actions hide behind
// a swipe so the line keeps the whole width: swipe left for add object / add
// line / loop, swipe right for delete — which still takes a tap, so a stray
// swipe can't lose a line. Arrow keys do the same for keyboard users.
//
// A list can't show two elements colliding the way the canvas does, so chips
// carry the canvas's warnings instead: overlapping another element, or running
// past the paper edge. Nothing here moves an element on its own.
import { computed, ref } from 'vue'
import {
  contentRows,
  elementFootprint,
  overlappingIds,
  printableCols,
  type Footprint,
} from '../lib/layout'
import { bandDescription, leaf } from '../lib/bands'
import { useTicketEditorContext } from '../core/useTicketEditor'
import type { Element, Region } from '../types'

const {
  t,
  doc,
  selectedId,
  selectedBandId,
  sampleOf,
  showFields,
  loopSources,
  selectElement,
  selectBand,
  insertRow,
  removeLine,
  createRegion,
  addTarget,
} = useTicketEditorContext()

const fps = computed(() => {
  const m = new Map<string, Footprint>()
  for (const el of doc.value.elements)
    m.set(el.id, elementFootprint(el, doc.value.paper, sampleOf(el)))
  return m
})
const fp = (el: Element) => fps.value.get(el.id) ?? elementFootprint(el, doc.value.paper)
const rowCount = computed(() => contentRows(doc.value, fp))

const overlapping = computed(() =>
  overlappingIds(
    doc.value.elements.filter((e) => e.type !== 'marker'),
    printableCols(doc.value.paper),
    fp,
  ),
)
function offPaper(el: Element): boolean {
  return el.type !== 'marker' && el.col + fp(el).cols > printableCols(doc.value.paper)
}
function chipWarning(el: Element): string | undefined {
  if (overlapping.value.has(el.id)) return t('chipOverlap')
  if (offPaper(el)) return t('chipOffPaper')
  return undefined
}

// Elements by the row they start on, left to right.
const byRow = computed(() => {
  const m = new Map<number, Element[]>()
  for (const el of doc.value.elements) {
    const list = m.get(el.row) ?? []
    list.push(el)
    m.set(el.row, list)
  }
  for (const list of m.values()) list.sort((a, b) => a.col - b.col)
  return m
})
// Rows covered by a taller element that started above: not separate lines.
const continued = computed(() => {
  const set = new Set<number>()
  for (const el of doc.value.elements)
    for (let r = el.row + 1; r < el.row + fp(el).rows; r++) set.add(r)
  return set
})
const lines = computed(() =>
  Array.from({ length: rowCount.value }, (_, r) => r).filter(
    (r) => byRow.value.has(r) || !continued.value.has(r),
  ),
)
function isEmpty(r: number): boolean {
  return !byRow.value.has(r) && !continued.value.has(r)
}

const regions = computed<Region[]>(() => doc.value.regions ?? [])
function bandOf(r: number): Region | undefined {
  return regions.value.find((g) => r >= g.start_row && r < g.end_row)
}
function bandStarting(r: number): Region | undefined {
  return regions.value.find((g) => g.start_row === r)
}

function chipText(el: Element): string {
  switch (el.type) {
    case 'text':
      return el.content || '""'
    case 'variable':
      return (!showFields.value && sampleOf(el)) || el.path || ''
    case 'qr':
      return '▦ QR'
    case 'barcode':
      return `▥ ${el.symbology ?? 'code128'}`
    case 'image':
      return el.from_variable && el.data ? `▣ ${leaf(el.data)}` : '▣'
    case 'marker':
      return `✂ ${el.name ?? ''}`
  }
  return ''
}

// ---- swipe ------------------------------------------------------------------
// Widths of the action trays behind a line (px), measured from the trays (their
// labels vary by language) when a swipe starts. Swiping past half opens one.
const trayW = ref({ actions: 228, delete: 96 })
function measureTrays(line: HTMLElement) {
  const tray = (side: string) =>
    line.parentElement?.querySelector<HTMLElement>(`.te-rows-tray.${side}`)?.offsetWidth
  trayW.value = {
    actions: tray('actions') ?? trayW.value.actions,
    delete: tray('delete') ?? trayW.value.delete,
  }
}
// The line whose tray is open: 'actions' is revealed by a left swipe (the tray
// sits on the right), 'delete' by a right swipe. One line at a time.
const open = ref<{ row: number; side: 'actions' | 'delete' } | null>(null)
const drag = ref<{ row: number; x0: number; y0: number; base: number; dx: number } | null>(null)
// Horizontal intent is only decided after a few pixels, so a vertical scroll
// that starts on a line stays a scroll.
let axis: 'x' | 'y' | null = null
let swiped = false

function restOffset(r: number): number {
  if (open.value?.row !== r) return 0
  return open.value.side === 'actions' ? -trayW.value.actions : trayW.value.delete
}
function offset(r: number): number {
  const d = drag.value
  if (d?.row !== r || axis !== 'x') return restOffset(r)
  return Math.min(trayW.value.delete, Math.max(-trayW.value.actions, d.base + d.dx))
}
function onDown(e: PointerEvent, r: number) {
  if (e.pointerType === 'mouse' && e.button !== 0) return
  axis = null
  swiped = false
  measureTrays(e.currentTarget as HTMLElement)
  drag.value = { row: r, x0: e.clientX, y0: e.clientY, base: restOffset(r), dx: 0 }
}
function onMove(e: PointerEvent) {
  const d = drag.value
  if (!d) return
  const dx = e.clientX - d.x0
  const dy = e.clientY - d.y0
  if (!axis && Math.max(Math.abs(dx), Math.abs(dy)) > 8) {
    axis = Math.abs(dx) > Math.abs(dy) ? 'x' : 'y'
    if (axis === 'x') (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }
  if (axis === 'x') {
    swiped = true
    d.dx = dx
  }
}
function onUp() {
  const d = drag.value
  if (d && axis === 'x') {
    const at = Math.min(trayW.value.delete, Math.max(-trayW.value.actions, d.base + d.dx))
    if (at <= -trayW.value.actions / 2) open.value = { row: d.row, side: 'actions' }
    else if (at >= trayW.value.delete / 2) open.value = { row: d.row, side: 'delete' }
    else open.value = null
  }
  drag.value = null
  axis = null
}
// A swipe ends in a click on whatever chip it started on; swallow that one.
// A tap on a line whose tray is open just closes the tray.
function onClickCapture(e: MouseEvent, r: number) {
  if (swiped || open.value?.row === r) {
    e.stopPropagation()
    e.preventDefault()
    if (!swiped) open.value = null
  }
  swiped = false
}
function onKey(e: KeyboardEvent, r: number) {
  if (e.target !== e.currentTarget) return
  measureTrays(e.currentTarget as HTMLElement)
  if (e.key === 'ArrowLeft') open.value = { row: r, side: 'actions' }
  else if (e.key === 'ArrowRight') open.value = { row: r, side: 'delete' }
  else if (e.key === 'Escape') open.value = null
  else return
  e.preventDefault()
}

// Chrome drops the click of a tap that comes too soon (~0.6 s) after a swipe —
// exactly the "swipe, tap Delete" rhythm. So tray buttons act on the pointer
// itself (press and release on the same button); click still serves keyboards,
// whose synthetic clicks carry detail 0.
let pressedTray: EventTarget | null = null
function trayDown(e: PointerEvent) {
  pressedTray = e.currentTarget
}
function trayUp(e: PointerEvent, run: () => void) {
  if (pressedTray === e.currentTarget) run()
  pressedTray = null
}
function trayClick(e: MouseEvent, run: () => void) {
  if (e.detail === 0) run()
}

function addObject(r: number) {
  open.value = null
  addTarget.value = r
}
function addLineBelow(r: number) {
  open.value = null
  insertRow(r + 1, rowCount.value)
}
function makeBand(r: number) {
  open.value = null
  createRegion({ start_row: r, end_row: r + 1, source: loopSources.value[0]?.path })
}
function deleteLine(r: number) {
  open.value = null
  removeLine(r, rowCount.value)
}
</script>

<template>
  <div class="te-part te-rows">
    <div class="te-rows-top">
      <p class="te-rows-hint">{{ t('rowsSwipeHint') }}</p>
      <button
        class="te-rows-fields"
        :class="{ on: showFields }"
        type="button"
        :aria-pressed="showFields"
        :title="t('showFieldsTip')"
        @click="showFields = !showFields"
      >
        {{ t('showFields') }}
      </button>
    </div>
    <div class="te-rows-list">
      <template v-for="r in lines" :key="r">
        <button
          v-if="bandStarting(r)"
          class="te-rows-band"
          :class="{
            loop: !!bandStarting(r)!.source,
            selected: bandStarting(r)!.id === selectedBandId,
          }"
          type="button"
          @click="selectBand(bandStarting(r)!.id)"
        >
          {{ bandDescription(bandStarting(r)!, t) }}
        </button>
        <div class="te-rows-swipe" :class="{ 'in-band': !!bandOf(r), loop: !!bandOf(r)?.source }">
          <div class="te-rows-tray delete" :inert="open?.row !== r || open.side !== 'delete'">
            <button
              class="te-rows-act"
              type="button"
              @pointerdown="trayDown"
              @pointerup="trayUp($event, () => deleteLine(r))"
              @click="trayClick($event, () => deleteLine(r))"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
              </svg>
              {{ t('rowDelete') }}
            </button>
          </div>
          <div class="te-rows-tray actions" :inert="open?.row !== r || open.side !== 'actions'">
            <button
              class="te-rows-act"
              type="button"
              @pointerdown="trayDown"
              @pointerup="trayUp($event, () => addObject(r))"
              @click="trayClick($event, () => addObject(r))"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <rect x="4" y="4" width="16" height="16" rx="3" />
                <path d="M12 8v8M8 12h8" />
              </svg>
              {{ t('rowAddObject') }}
            </button>
            <button
              class="te-rows-act"
              type="button"
              @pointerdown="trayDown"
              @pointerup="trayUp($event, () => addLineBelow(r))"
              @click="trayClick($event, () => addLineBelow(r))"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 6h16M4 11h16M4 16h8M18 14v6M15 17h6" />
              </svg>
              {{ t('rowAddLine') }}
            </button>
            <button
              v-if="!bandOf(r)"
              class="te-rows-act"
              type="button"
              @pointerdown="trayDown"
              @pointerup="trayUp($event, () => makeBand(r))"
              @click="trayClick($event, () => makeBand(r))"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M20 12a8 8 0 1 1-2.34-5.66" />
                <path d="M20 4v4h-4" />
              </svg>
              {{ t('rowMakeBand') }}
            </button>
          </div>
          <div
            class="te-rows-line"
            :class="{ empty: isEmpty(r), dragging: drag?.row === r }"
            :style="{ transform: `translateX(${offset(r)}px)` }"
            tabindex="0"
            :aria-label="t('rowLine', { n: r })"
            @pointerdown="onDown($event, r)"
            @pointermove="onMove"
            @pointerup="onUp"
            @pointercancel="onUp"
            @click.capture="onClickCapture($event, r)"
            @keydown="onKey($event, r)"
          >
            <button
              v-for="el in byRow.get(r) ?? []"
              :key="el.id"
              class="te-rows-chip"
              :class="[
                el.type,
                {
                  selected: el.id === selectedId,
                  bold: el.style?.bold,
                  warn: !!chipWarning(el),
                },
              ]"
              type="button"
              :title="chipWarning(el)"
              @click="selectElement(el.id)"
            >
              <span v-if="chipWarning(el)" aria-hidden="true">⚠ </span>{{ chipText(el) }}
            </button>
            <!-- striped like the canvas's empty space; the words are for screen readers -->
            <span v-if="isEmpty(r)" class="te-sr-only">{{ t('rowBlank') }}</span>
          </div>
        </div>
      </template>
      <button class="te-rows-append" type="button" @click="insertRow(rowCount, rowCount)">
        {{ t('rowsAppend') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
/* One surface with hairline dividers (a mail-list), not a stack of cards: rows
   touch, and inside a row only data carries a tint. Boxes inside boxes were
   what made the list read busy. */
.te-rows {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.te-rows-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.te-rows-hint {
  margin: 0;
  font-size: 0.72rem;
  line-height: 1.3;
  color: var(--te-muted-fg);
}
.te-rows-fields {
  flex: none;
  min-height: 2rem;
  padding: 0 0.7rem;
  border: 1px solid var(--te-input);
  border-radius: 999px;
  background: var(--te-card);
  color: var(--te-muted-fg);
  font: inherit;
  font-size: 0.78rem;
  cursor: pointer;
}
.te-rows-fields.on {
  border-color: color-mix(in srgb, var(--te-primary) 45%, transparent);
  background: color-mix(in srgb, var(--te-primary) 14%, transparent);
  color: var(--te-primary);
}
.te-rows-list {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
  overflow: hidden;
}
/* a band reads as a section header inside the list */
.te-rows-band {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  min-height: 2rem;
  padding: 0 0.9rem;
  border: 0;
  border-bottom: 1px solid var(--te-border);
  background: color-mix(in srgb, #f59e0b 9%, var(--te-card));
  color: #b45309;
  font: inherit;
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.02em;
  text-align: left;
  cursor: pointer;
}
.te-rows-band.loop {
  background: color-mix(in srgb, var(--te-primary) 7%, var(--te-card));
  color: var(--te-primary);
}
.te-rows-band.selected {
  box-shadow: inset 0 0 0 2px var(--te-ring);
}
/* a line: its content slides over the action slabs behind it */
.te-rows-swipe {
  position: relative;
  overflow: hidden;
  border-bottom: 1px solid var(--te-border);
}
.te-rows-tray {
  position: absolute;
  top: 0;
  bottom: 0;
  display: flex;
}
.te-rows-tray.delete {
  left: 0;
  background: #dc2626;
}
.te-rows-tray.actions {
  right: 0;
  background: var(--te-primary);
}
.te-rows-act {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.2rem;
  min-width: 4.6rem;
  padding: 0 0.6rem;
  border: 0;
  background: transparent;
  color: #fff;
  font: inherit;
  font-size: 0.68rem;
  font-weight: 500;
  white-space: nowrap;
  cursor: pointer;
}
.te-rows-tray.actions .te-rows-act + .te-rows-act {
  box-shadow: inset 1px 0 0 rgb(255 255 255 / 0.18);
}
.te-rows-act:active {
  background: rgb(0 0 0 / 0.12);
}
.te-rows-act svg {
  width: 1.15rem;
  height: 1.15rem;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.te-rows-line {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.35rem 0.5rem;
  min-height: 3rem;
  padding: 0.45rem 0.9rem;
  background: var(--te-card);
  transition: transform 0.2s ease-out;
  /* vertical pans scroll the list; horizontal ones are the swipe */
  touch-action: pan-y;
  user-select: none;
}
.te-rows-line.dragging {
  transition: none;
}
.te-rows-line:focus-visible {
  outline: 2px solid var(--te-ring);
  outline-offset: -2px;
}
.te-rows-swipe.in-band .te-rows-line {
  box-shadow: inset 3px 0 0 #f59e0b;
}
.te-rows-swipe.in-band.loop .te-rows-line {
  box-shadow: inset 3px 0 0 var(--te-primary);
}
/* an empty line: the canvas's overflow stripes, no words */
.te-rows-line.empty {
  /* shorter than a filled line, but tall enough for the swipe slabs' icon and label */
  min-height: 2.6rem;
  background:
    repeating-linear-gradient(45deg, transparent 0 6px, rgba(0, 0, 0, 0.035) 6px 12px),
    var(--te-muted);
}
.te-rows-chip {
  max-width: 100%;
  min-height: 1.9rem;
  padding: 0.2rem 0.45rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
  font-family: ui-monospace, 'DejaVu Sans Mono', monospace;
  font-size: 0.82rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
}
/* static text is just text, as on the ticket */
.te-rows-chip.text {
  padding-inline: 0.1rem;
}
.te-rows-chip.bold {
  font-weight: 700;
}
.te-rows-chip.variable {
  background: color-mix(in srgb, var(--te-primary) 11%, transparent);
  color: var(--te-primary);
}
.te-rows-chip.qr,
.te-rows-chip.barcode,
.te-rows-chip.image,
.te-rows-chip.marker {
  background: var(--te-muted);
  color: var(--te-muted-fg);
  font-family: inherit;
  font-size: 0.75rem;
}
.te-rows-chip.warn {
  background: color-mix(in srgb, #f59e0b 16%, transparent);
  color: #b45309;
}
.te-rows-chip.selected {
  outline: 2px solid var(--te-ring);
  outline-offset: 1px;
}
.te-rows-append {
  min-height: 2.75rem;
  border: 0;
  background: transparent;
  color: var(--te-primary);
  font: inherit;
  font-size: 0.85rem;
  cursor: pointer;
}
.te-rows-append:active {
  background: var(--te-accent);
}
.te-sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
}
</style>
