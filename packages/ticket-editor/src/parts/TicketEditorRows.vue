<script setup lang="ts">
// The ticket as a list of lines — the phone's editor. A receipt is mostly
// one-dimensional (lines, with a few things side by side), so on a narrow
// screen it edits as a list instead of a 2D grid: tap an element to edit it
// (row, column and alignment live in its properties), and each line has its
// own actions. Same document and actions as the canvas.
import { computed, ref } from 'vue'
import { contentRows, elementFootprint, type Footprint } from '../lib/layout'
import { bandDescription, leaf } from '../lib/bands'
import { useTicketEditorContext } from '../core/useTicketEditor'
import type { Element, Region } from '../types'

const {
  t,
  doc,
  selectedId,
  selectedBandId,
  sampleOf,
  loopSources,
  selectElement,
  selectBand,
  insertRow,
  deleteRow,
  createRegion,
} = useTicketEditorContext()

const fps = computed(() => {
  const m = new Map<string, Footprint>()
  for (const el of doc.value.elements)
    m.set(el.id, elementFootprint(el, doc.value.paper, sampleOf(el)))
  return m
})
const fp = (el: Element) => fps.value.get(el.id) ?? elementFootprint(el, doc.value.paper)
const rowCount = computed(() => contentRows(doc.value, fp))

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
      return sampleOf(el) || el.path || ''
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

// One line's actions are open at a time.
const openRow = ref<number | null>(null)
function toggleRow(r: number) {
  openRow.value = openRow.value === r ? null : r
}
function addAbove(r: number) {
  insertRow(r, rowCount.value)
  // Stay on the new blank line: its Remove undoes the insert in one tap.
  openRow.value = r
}
function remove(r: number) {
  deleteRow(r, rowCount.value)
  openRow.value = null
}
function makeBand(r: number) {
  openRow.value = null
  createRegion({ start_row: r, end_row: r + 1, source: loopSources.value[0]?.path })
}
</script>

<template>
  <div class="te-part te-rows">
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
      <div
        class="te-rows-line"
        :class="{
          empty: isEmpty(r),
          'in-band': !!bandOf(r),
          loop: !!bandOf(r)?.source,
          open: openRow === r,
        }"
      >
        <span class="te-rows-num">{{ r }}</span>
        <div class="te-rows-chips">
          <button
            v-for="el in byRow.get(r) ?? []"
            :key="el.id"
            class="te-rows-chip"
            :class="[el.type, { selected: el.id === selectedId, bold: el.style?.bold }]"
            type="button"
            @click="selectElement(el.id)"
          >
            {{ chipText(el) }}
          </button>
          <span v-if="isEmpty(r)" class="te-rows-blank">{{ t('rowBlank') }}</span>
        </div>
        <button
          class="te-rows-more"
          type="button"
          :aria-expanded="openRow === r"
          :aria-label="t('rowActions', { n: r })"
          @click="toggleRow(r)"
        >
          ⋯
        </button>
        <div v-if="openRow === r" class="te-rows-actions">
          <button class="te-rows-act" type="button" @click="addAbove(r)">
            {{ t('rowAddAbove') }}
          </button>
          <button
            class="te-rows-act danger"
            type="button"
            :disabled="!isEmpty(r)"
            :title="isEmpty(r) ? undefined : t('rowRemoveBlocked')"
            @click="remove(r)"
          >
            {{ t('rowRemove') }}
          </button>
          <button v-if="!bandOf(r)" class="te-rows-act" type="button" @click="makeBand(r)">
            ↻ {{ t('rowMakeBand') }}
          </button>
        </div>
      </div>
    </template>
    <button class="te-rows-append" type="button" @click="insertRow(rowCount, rowCount)">
      {{ t('rowsAppend') }}
    </button>
  </div>
</template>

<style scoped>
.te-rows {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
.te-rows-band {
  align-self: flex-start;
  display: inline-flex;
  gap: 0.35rem;
  margin-top: 0.4rem;
  padding: 0.25rem 0.6rem;
  border: 0;
  border-radius: 999px;
  background: color-mix(in srgb, #f59e0b 16%, transparent);
  color: #b45309;
  font: inherit;
  font-size: 0.75rem;
  cursor: pointer;
}
.te-rows-band.loop {
  background: color-mix(in srgb, var(--te-primary) 14%, transparent);
  color: var(--te-primary);
}
.te-rows-band.selected {
  outline: 2px solid var(--te-ring);
}
.te-rows-line {
  display: grid;
  grid-template-columns: 1.6rem minmax(0, 1fr) 2.4rem;
  align-items: center;
  gap: 0.4rem;
  min-height: 2.75rem;
  padding: 0.25rem 0.25rem 0.25rem 0.4rem;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
}
.te-rows-line.in-band {
  border-left: 3px solid #f59e0b;
}
.te-rows-line.in-band.loop {
  border-left-color: var(--te-primary);
}
.te-rows-line.empty {
  min-height: 2.2rem;
  border-style: dashed;
  background: transparent;
}
.te-rows-num {
  font-family: ui-monospace, monospace;
  font-size: 0.7rem;
  color: var(--te-muted-fg);
  text-align: right;
}
.te-rows-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
  min-width: 0;
}
.te-rows-chip {
  max-width: 100%;
  min-height: 2rem;
  padding: 0.2rem 0.55rem;
  border: 1px solid var(--te-border);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font-family: ui-monospace, 'DejaVu Sans Mono', monospace;
  font-size: 0.8rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
}
.te-rows-chip.bold {
  font-weight: 700;
}
.te-rows-chip.variable {
  background: color-mix(in srgb, var(--te-primary) 12%, transparent);
  border-color: color-mix(in srgb, var(--te-primary) 45%, transparent);
  color: var(--te-primary);
}
.te-rows-chip.qr,
.te-rows-chip.barcode,
.te-rows-chip.image,
.te-rows-chip.marker {
  border-style: dashed;
  color: var(--te-muted-fg);
}
.te-rows-chip.selected {
  outline: 2px solid var(--te-ring);
  outline-offset: 1px;
}
.te-rows-blank {
  font-size: 0.75rem;
  color: var(--te-muted-fg);
  font-style: italic;
}
.te-rows-more {
  width: 2.4rem;
  height: 2.2rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: var(--te-muted-fg);
  font-size: 1.1rem;
  cursor: pointer;
}
.te-rows-line.open .te-rows-more {
  background: var(--te-muted);
}
.te-rows-actions {
  grid-column: 1 / -1;
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  padding: 0.3rem 0 0.1rem;
}
.te-rows-act {
  min-height: 2.2rem;
  padding: 0.3rem 0.7rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
  font-size: 0.8rem;
  cursor: pointer;
}
.te-rows-act.danger {
  color: #dc2626;
}
.te-rows-act:disabled {
  opacity: 0.45;
  cursor: default;
}
.te-rows-append {
  min-height: 2.6rem;
  margin-top: 0.3rem;
  border: 1px dashed var(--te-input);
  border-radius: var(--te-radius);
  background: transparent;
  color: var(--te-primary);
  font: inherit;
  font-size: 0.85rem;
  cursor: pointer;
}
</style>
