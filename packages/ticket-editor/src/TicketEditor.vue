<script setup lang="ts">
// The embeddable editor. Four zones: variable tree, structural grid editor, live
// 1:1 preview, modifier panel. It owns the TicketDoc and hands it back through
// `onSave`. The rails collapse (inline, not a drawer, so nothing clashes with a
// host's own drawers), the canvas has a zoom control, and placement is
// non-destructive — a manual "Fit to width" is the only thing that ever moves
// elements in bulk, and only when clicked.
import { computed, ref, toRaw, watch } from 'vue'
import VariableTree from './components/VariableTree.vue'
import GridCanvas from './components/GridCanvas.vue'
import ModifierPanel from './components/ModifierPanel.vue'
import BandPanel from './components/BandPanel.vue'
import PreviewPane from './components/PreviewPane.vue'
import ComputedEditor from './components/ComputedEditor.vue'
import TypeTag from './components/TypeTag.vue'
import { deriveTree, guessLength, pathTypeMap, randomizeSample } from './lib/tree'
import { previewComputed } from './composables/useRenderer'
import { provideEditorI18n, type Messages } from './i18n'
import { SCHEMA_VERSION } from './types'
import type {
  Computed,
  ComputedResult,
  Element,
  Region,
  TicketDoc,
  VariableType,
  VarGroup,
  VarNode,
  VarOption,
} from './types'
import './styles/tokens.css'

const props = defineProps<{
  modelValue?: TicketDoc
  variables?: Record<string, unknown>
  /**
   * Authoritative variable types keyed by dotted path (e.g.
   * `{ 'sale.total': 'number', 'sale.items.0.date': 'date' }`).
   * The host declares these; anything not listed falls back to inference from
   * the sample data. Gates which Format options the editor offers.
   */
  variableTypes?: Record<string, VariableType>
  /** Force a UI locale (e.g. 'es'). If omitted, follows the host's vue-i18n locale. */
  locale?: string
  /** Override / extend built-in UI strings, keyed by locale. */
  messages?: Messages
  onSave?: (doc: TicketDoc) => void | Promise<void>
}>()
const emit = defineEmits<{ 'update:modelValue': [doc: TicketDoc] }>()

// Translation: built-in en/es, follows host vue-i18n locale, overridable.
const t = provideEditorI18n(
  () => props.locale,
  () => props.messages,
)

function blankDoc(): TicketDoc {
  return {
    version: SCHEMA_VERSION,
    paper: {
      width_chars: 40,
      margin_left_chars: 1,
      margin_right_chars: 1,
      margin_top_lines: 1,
      margin_bottom_lines: 1,
      cell_width_px: 12,
      cell_height_px: 22,
      font_px: 20,
      min_rows: 12,
    },
    elements: [],
  }
}

// The editor owns a PRIVATE deep copy of the document — it never mutates the
// host's object. Edits are emitted out as fresh snapshots (one-way data flow).
// A JSON round-trip is the right clone here: a TicketDoc is JSON-serializable by
// definition (it is persisted as JSON), and it strips Vue's reactive proxies.
function snapshot(d: TicketDoc): TicketDoc {
  return JSON.parse(JSON.stringify(d)) as TicketDoc
}
const doc = ref<TicketDoc>(props.modelValue ? snapshot(props.modelValue) : blankDoc())
// Track the snapshot we last emitted so the round-trip through v-model doesn't
// echo back into our state (which would loop); a genuinely new doc still loads.
let lastEmitted: TicketDoc | null = null
watch(
  () => props.modelValue,
  (v) => {
    // The host wraps our emitted snapshot in its own reactive ref, so compare
    // the raw target: if it's the snapshot we just sent, ignore the echo.
    if (v && toRaw(v) !== lastEmitted) doc.value = snapshot(v)
  },
)
watch(
  doc,
  (v) => {
    const snap = snapshot(v)
    lastEmitted = snap
    emit('update:modelValue', snap)
  },
  { deep: true },
)

const tree = computed<VarNode[]>(() => deriveTree(props.variables ?? {}))
// Calculated variables live on the doc; they surface everywhere host vars do
// (element source, QR "from variable", conditions) at path `calc.<name>`.
const computedVars = computed<Computed[]>(() => doc.value.computed ?? [])
const calcLeaves = computed<{ path: string; key: string }[]>(() =>
  computedVars.value.map((c) => ({ path: `calc.${c.name}`, key: c.name })),
)
// Live results for the calc vars, evaluated by the wasm engine (same as print).
// Keyed by name → { value, kind, error }. Refreshed whenever the formulas or the
// sample data change; drives the rail preview and the placed-element type.
const calcReports = ref<Record<string, ComputedResult>>({})
watch(
  [computedVars, () => props.variables],
  async () => {
    try {
      const rep = await previewComputed(computedVars.value, props.variables ?? {})
      calcReports.value = Object.fromEntries(rep.map((r) => [r.name, r]))
    } catch {
      calcReports.value = {}
    }
  },
  { immediate: true, deep: true },
)
function calcKind(name: string): VariableType {
  const k = calcReports.value[name]?.kind
  return k === 'number' ? 'number' : 'text'
}
// path -> type: inferred from samples, then calc-var kinds, then host overrides.
const types = computed<Record<string, VariableType>>(() => ({
  ...pathTypeMap(tree.value),
  ...Object.fromEntries(computedVars.value.map((c) => [`calc.${c.name}`, calcKind(c.name)])),
  ...(props.variableTypes ?? {}),
}))
function typeOf(path?: string): VariableType {
  return (path && types.value[path]) || 'text'
}

// Repeatable groups (loop sources) and all leaf vars (condition targets).
function collect(
  nodes: VarNode[],
  loops: { path: string; key: string }[],
  leaves: { path: string; key: string }[],
) {
  for (const n of nodes) {
    if (n.repeatable) loops.push({ path: n.path, key: n.key })
    if (n.children) collect(n.children, loops, leaves)
    else leaves.push({ path: n.path, key: n.key })
  }
}
const loopSources = computed(() => {
  const loops: { path: string; key: string }[] = []
  collect(tree.value, loops, [])
  return loops
})
const allVars = computed(() => {
  const leaves: { path: string; key: string }[] = []
  collect(tree.value, [], leaves)
  // Calculated variables are selectable anywhere a variable is (QR, conditions).
  return [...leaves, ...calcLeaves.value]
})

const selectedId = ref<string | null>(null)
const selectedBandId = ref<string | null>(null)
const selected = computed(() => doc.value.elements.find((e) => e.id === selectedId.value) ?? null)
const selectedType = computed<VariableType>(() =>
  selected.value?.type === 'variable' ? typeOf(selected.value.path) : 'text',
)
const selectedBand = computed(
  () => (doc.value.regions ?? []).find((r) => r.id === selectedBandId.value) ?? null,
)
// Element and band selection are mutually exclusive (one right-drawer at a time).
function selectElement(id: string | null) {
  selectedId.value = id
  if (id) {
    selectedBandId.value = null
    rightOpen.value = true
  }
}
function selectBand(id: string | null) {
  selectedBandId.value = id
  if (id) {
    selectedId.value = null
    rightOpen.value = true
  }
}

// preview data: real variables, or a reshuffled clone when the user asks.
const shuffled = ref<Record<string, unknown> | null>(null)
const previewData = computed(() => shuffled.value ?? props.variables)
function reshuffle() {
  shuffled.value = randomizeSample(props.variables ?? {})
}
// Drop stale reshuffled data when the host swaps the variable set.
watch(
  () => props.variables,
  () => {
    shuffled.value = null
  },
)

// view state
const zoom = ref(1.0)
const leftOpen = ref(true)
const rightOpen = ref(true)

let seq = 0
function newId() {
  seq += 1
  return `el_${seq}_${Math.floor(Math.random() * 1e6)}`
}
function nextRow(): number {
  return doc.value.elements.reduce((m, e) => Math.max(m, e.row + 1), 0)
}

function addVariable(node: VarNode) {
  const t = typeOf(node.path)
  const el: Element = {
    id: newId(),
    row: nextRow(),
    col: 0,
    type: 'variable',
    path: node.path,
    length: guessLength(node.sample),
    align: 'left',
  }
  // Sensible default formatting for the variable's type.
  if (t === 'number') {
    const hasFraction = typeof node.sample === 'number' && !Number.isInteger(node.sample)
    el.number = { decimals: hasFraction ? 2 : 0, rounding: 'half_up', thousands: true }
    el.align = 'right'
  } else if (t === 'date') {
    el.date_format = 'DD/MM/YYYY HH:mm'
  }
  doc.value.elements.push(el)
  selectElement(el.id)
}
function addText() {
  const el: Element = { id: newId(), row: nextRow(), col: 0, type: 'text', content: 'Text' }
  doc.value.elements.push(el)
  selectElement(el.id)
}
function addQr() {
  const el: Element = {
    id: newId(),
    row: nextRow(),
    col: 0,
    type: 'qr',
    value: 'https://example.com/r/',
    from_variable: false,
    size: 10,
  }
  doc.value.elements.push(el)
  selectElement(el.id)
}
function addBarcode() {
  const el: Element = {
    id: newId(),
    row: nextRow(),
    col: 0,
    type: 'barcode',
    value: '012345678905',
    from_variable: false,
    symbology: 'code128',
    width: 24,
    height: 4,
  }
  doc.value.elements.push(el)
  selectElement(el.id)
}

// --- calculated variables ---
// The one being edited, or null when the dialog is closed. Its `name` doubles as
// the "original name" so a rename still updates in place.
const editingCalc = ref<Computed | null>(null)
// Options for the formula editor's "Insert variable" picker, grouped so the row
// fields of a list are visible as bare names — that's what tells the user what
// lives inside `this` when they write an aggregate like sumif(movements, …).
function collectRowFields(nodes: VarNode[], prefix: string, out: VarOption[]) {
  for (const n of nodes) {
    // Don't descend into a nested list — its fields aren't bare fields of THIS
    // row (a per-row aggregate over them would need its own aggregate call).
    if (n.repeatable) continue
    if (n.children) collectRowFields(n.children, prefix, out)
    else {
      const rel = n.path.startsWith(prefix) ? n.path.slice(prefix.length) : n.key
      out.push({ label: rel, insert: rel })
    }
  }
}
function collectGroups(
  nodes: VarNode[],
  scalars: VarOption[],
  lists: VarOption[],
  rows: VarGroup[],
) {
  for (const n of nodes) {
    if (n.repeatable) {
      lists.push({ label: n.path, insert: n.path })
      const fields: VarOption[] = []
      collectRowFields(n.children ?? [], `${n.path}.0.`, fields)
      if (fields.length) rows.push({ label: t('calcGroupRow', { list: n.path }), options: fields })
    } else if (n.children) {
      collectGroups(n.children, scalars, lists, rows)
    } else {
      scalars.push({ label: n.path, insert: n.path })
    }
  }
}
const varGroups = computed<VarGroup[]>(() => {
  const scalars: VarOption[] = []
  const lists: VarOption[] = []
  const rows: VarGroup[] = []
  collectGroups(tree.value, scalars, lists, rows)
  // Other calc vars are usable too (but not the one being edited — no self-ref).
  const calcs: VarOption[] = calcLeaves.value
    .filter((v) => v.path !== `calc.${editingCalc.value?.name}`)
    .map((v) => ({ label: v.path, insert: v.path }))
  const groups: VarGroup[] = []
  const values = [...scalars, ...calcs]
  if (values.length) groups.push({ label: t('calcGroupValues'), options: values })
  if (lists.length) groups.push({ label: t('calcGroupLists'), options: lists })
  groups.push(...rows)
  return groups
})
function calcHasError(name: string): boolean {
  return !!calcReports.value[name]?.error
}
// Evaluate a DRAFT formula through the wasm engine. Preview must match print, so
// evaluate the draft IN THE SAME POSITION the real doc will: for an edit, replace
// the var in place (so forward references resolve exactly as they will at render
// — earlier-only); for a new var, append it. The sentinel key can't collide with
// a real name because the editor forbids non-`[A-Za-z_]` names.
const DRAFT_KEY = '--draft--'
async function previewFormula(formula: string): Promise<ComputedResult> {
  const all = computedVars.value
  const orig = editingCalc.value?.name
  const idx = orig ? all.findIndex((c) => c.name === orig) : -1
  const key = idx >= 0 ? all[idx].name : DRAFT_KEY
  const list =
    idx >= 0
      ? all.map((c, i) => (i === idx ? { name: key, formula } : c))
      : [...all, { name: key, formula }]
  const rep = await previewComputed(list, props.variables ?? {})
  return rep.find((r) => r.name === key) ?? { name: key, value: '', kind: 'empty', error: null }
}
function newCalc() {
  editingCalc.value = { name: '', formula: '' }
}
function editCalc(c: Computed) {
  editingCalc.value = JSON.parse(JSON.stringify(c)) as Computed
}
function saveCalc(next: Computed) {
  const list = [...(doc.value.computed ?? [])]
  // Match by the original name captured when the dialog opened (renames keep place).
  const orig = editingCalc.value?.name
  const i = orig ? list.findIndex((c) => c.name === orig) : -1
  if (i >= 0) list[i] = next
  else list.push(next)
  doc.value.computed = list
  editingCalc.value = null
}
function removeCalc(name: string) {
  doc.value.computed = (doc.value.computed ?? []).filter((c) => c.name !== name)
}
// Place a calculated variable on the ticket as a Variable element. The value
// crosses the wasm boundary as a string; coerce a numeric result back to a
// number so `addVariable` picks sensible decimals (e.g. 123.45 → 2 decimals).
function addCalcElement(c: Computed) {
  const r = calcReports.value[c.name]
  let sample: string | number | undefined
  if (r && r.value !== '') sample = r.kind === 'number' ? Number(r.value) : r.value
  addVariable({ key: c.name, path: `calc.${c.name}`, sample, type: calcKind(c.name) })
}

// Add a DYNAMIC image: its bytes come from a variable at print time (a signature,
// a plot, …). This is the default because it's the common case; providing a file
// in the modifier panel downgrades it to a static, embedded image. No upload
// dialog on add — an image with no source just shows a placeholder.
function addImage() {
  const el: Element = {
    id: newId(),
    row: nextRow(),
    col: 0,
    type: 'image',
    data: '',
    from_variable: true,
    w: 16,
    h: 6,
    mode: { kind: 'threshold', level: 128 },
  }
  doc.value.elements.push(el)
  selectElement(el.id)
}
function updateElement(next: Element) {
  const i = doc.value.elements.findIndex((e) => e.id === next.id)
  if (i >= 0) doc.value.elements[i] = next
}
function removeElement(id: string) {
  doc.value.elements = doc.value.elements.filter((e) => e.id !== id)
  if (selectedId.value === id) selectedId.value = null
}

// Insert a blank line: everything at or below `row` shifts down one, and the
// ticket grows by exactly one line. Lets you open space in the middle of a
// finished ticket — or at the very end (a signature) — without hand-moving each
// field. `eff` is the current content height reported by the canvas.
function insertRow(row: number, eff: number) {
  doc.value.elements = doc.value.elements.map((e) => (e.row >= row ? { ...e, row: e.row + 1 } : e))
  // Bands shift with their rows: a band at/below the insert moves down; a band
  // that straddles the insert grows by one row.
  doc.value.regions = (doc.value.regions ?? []).map((r) => {
    if (row <= r.start_row) return { ...r, start_row: r.start_row + 1, end_row: r.end_row + 1 }
    if (row < r.end_row) return { ...r, end_row: r.end_row + 1 }
    return r
  })
  doc.value.paper.min_rows = eff + 1
}
// Remove an (empty) line: everything below shifts up one and the ticket shrinks
// by one. The canvas only offers this on rows nothing occupies, so it can't
// destroy work.
function deleteRow(row: number, eff: number) {
  doc.value.elements = doc.value.elements.map((e) => (e.row > row ? { ...e, row: e.row - 1 } : e))
  doc.value.regions = (doc.value.regions ?? []).flatMap((r) => {
    let s = r.start_row
    let e = r.end_row
    if (row < s) {
      s -= 1
      e -= 1
    } // band below the removed row moves up
    else if (row < e) e -= 1 // removed row was inside the band → shrink
    if (e <= s) return [] // band collapsed to nothing → drop it
    return [{ ...r, start_row: s, end_row: e }]
  })
  doc.value.paper.min_rows = Math.max(0, eff - 1)
}

// --- flow bands ---
function createRegion(r: Omit<Region, 'id'>) {
  const region: Region = { ...r, id: `rg_${(seq += 1)}_${Math.floor(Math.random() * 1e6)}` }
  doc.value.regions = [...(doc.value.regions ?? []), region]
  selectBand(region.id) // open the new band's config in the drawer
}
function updateRegion(next: Region) {
  doc.value.regions = (doc.value.regions ?? []).map((r) => (r.id === next.id ? next : r))
}
function removeRegion(id: string) {
  doc.value.regions = (doc.value.regions ?? []).filter((r) => r.id !== id)
  if (selectedBandId.value === id) selectedBandId.value = null
}

// Printable content width in characters — used by a field's per-element
// "Fit to width" action in the modifier panel.
const contentCols = computed(() => {
  const p = doc.value.paper
  return Math.max(1, p.width_chars - (p.margin_left_chars ?? 0) - (p.margin_right_chars ?? 0))
})

const saving = ref(false)
async function save() {
  if (!props.onSave) return
  saving.value = true
  try {
    await props.onSave(snapshot(doc.value))
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="te-root te-editor">
    <header class="te-toolbar">
      <strong class="te-title">{{ t('title') }}</strong>
      <label class="te-inline"
        >{{ t('width') }}
        <input
          class="te-num"
          type="number"
          min="16"
          max="120"
          :value="doc.paper.width_chars"
          @input="
            doc.paper.width_chars = Math.max(16, +($event.target as HTMLInputElement).value || 40)
          "
        />
      </label>
      <label class="te-inline"
        >{{ t('zoom') }}
        <input type="range" min="0.8" max="2.2" step="0.1" v-model.number="zoom" />
        <span class="te-muted">{{ zoom.toFixed(1) }}×</span>
      </label>
      <button class="te-btn te-btn-ghost" type="button" @click="addText">{{ t('addText') }}</button>
      <button class="te-btn te-btn-ghost" type="button" @click="addImage">
        {{ t('addImage') }}
      </button>
      <button class="te-btn te-btn-ghost" type="button" @click="addQr">{{ t('addQr') }}</button>
      <button class="te-btn te-btn-ghost" type="button" @click="addBarcode">
        {{ t('addBarcode') }}
      </button>
      <div class="te-spacer" />
      <button
        v-if="onSave"
        class="te-btn te-btn-primary"
        type="button"
        :disabled="saving"
        @click="save"
      >
        {{ saving ? t('saving') : t('save') }}
      </button>
    </header>

    <div class="te-body">
      <aside class="te-rail" :class="{ collapsed: !leftOpen }">
        <button
          class="te-rail-toggle"
          type="button"
          @click="leftOpen = !leftOpen"
          :aria-label="leftOpen ? t('collapse') : t('railVariables')"
          :aria-expanded="leftOpen"
          :title="leftOpen ? t('collapse') : t('railVariables')"
        >
          {{ leftOpen ? '‹' : '›' }}
        </button>
        <div v-if="leftOpen" class="te-rail-inner">
          <h3 class="te-rail-title">{{ t('railVariables') }}</h3>
          <VariableTree root :nodes="tree" :types="types" @add="addVariable" />

          <div class="te-calc">
            <h3 class="te-rail-title te-calc-title">{{ t('railCalculated') }}</h3>
            <ul v-if="computedVars.length" class="te-calc-list">
              <li v-for="c in computedVars" :key="c.name" class="te-calc-item">
                <button
                  class="te-calc-add"
                  type="button"
                  :title="c.formula"
                  @click="addCalcElement(c)"
                >
                  <span class="te-calc-eq" aria-hidden="true">=</span>
                  <span class="te-calc-key">{{ c.name }}</span>
                  <span
                    v-if="calcHasError(c.name)"
                    class="te-calc-warn"
                    :title="calcReports[c.name]?.error ?? ''"
                    >⚠</span
                  >
                  <TypeTag v-else class="te-calc-tag" :type="calcKind(c.name)" />
                </button>
                <button
                  class="te-calc-icon"
                  type="button"
                  :aria-label="t('calcEdit')"
                  :title="t('calcEdit')"
                  @click="editCalc(c)"
                >
                  ✎
                </button>
                <button
                  class="te-calc-icon"
                  type="button"
                  :aria-label="t('calcDelete')"
                  :title="t('calcDelete')"
                  @click="removeCalc(c.name)"
                >
                  🗑
                </button>
              </li>
            </ul>
            <p v-else class="te-calc-empty">{{ t('calcEmpty') }}</p>
            <button class="te-calc-new" type="button" @click="newCalc">
              {{ t('calcAddNew') }}
            </button>
          </div>
        </div>
      </aside>

      <main class="te-center">
        <GridCanvas
          :doc="doc"
          :selected-id="selectedId"
          :selected-band-id="selectedBandId"
          :zoom="zoom"
          :variables="previewData"
          :loop-sources="loopSources"
          :all-vars="allVars"
          @select="selectElement"
          @select-band="selectBand"
          @update:element="updateElement"
          @insert-row="insertRow"
          @delete-row="deleteRow"
          @create-region="createRegion"
          @remove-region="removeRegion"
        />
      </main>

      <section class="te-preview-col">
        <PreviewPane :doc="doc" :variables="previewData">
          <template #actions>
            <button class="te-chip" type="button" :title="t('reshuffleTip')" @click="reshuffle">
              {{ t('reshuffle') }}
            </button>
          </template>
        </PreviewPane>
      </section>

      <aside class="te-rail te-rail-right" :class="{ collapsed: !rightOpen }">
        <button
          class="te-rail-toggle right"
          type="button"
          @click="rightOpen = !rightOpen"
          :aria-label="rightOpen ? t('collapse') : t('railModifiers')"
          :aria-expanded="rightOpen"
          :title="rightOpen ? t('collapse') : t('railModifiers')"
        >
          {{ rightOpen ? '›' : '‹' }}
        </button>
        <div v-if="rightOpen" class="te-rail-inner">
          <h3 class="te-rail-title">{{ selectedBand ? t('railBand') : t('railModifiers') }}</h3>
          <BandPanel
            v-if="selectedBand"
            :region="selectedBand"
            :loop-sources="loopSources"
            :all-vars="allVars"
            @update:region="updateRegion"
            @remove="removeRegion"
          />
          <ModifierPanel
            v-else
            :element="selected"
            :var-type="selectedType"
            :all-vars="allVars"
            :loop-sources="loopSources"
            :content-cols="contentCols"
            @update:element="updateElement"
            @remove="removeElement"
          />
        </div>
      </aside>
    </div>

    <ComputedEditor
      v-if="editingCalc"
      :model-value="editingCalc"
      :var-groups="varGroups"
      :preview="previewFormula"
      :existing-names="computedVars.map((c) => c.name)"
      @save="saveCalc"
      @cancel="editingCalc = null"
    />
  </div>
</template>

<style scoped>
.te-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 460px;
}
.te-toolbar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.5rem 0.8rem;
  border-bottom: 1px solid var(--te-border);
  flex-wrap: wrap;
}
.te-title {
  font-size: 0.95rem;
}
.te-spacer {
  flex: 1;
}
.te-inline {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.8rem;
  color: var(--te-muted-fg);
}
.te-muted {
  color: var(--te-muted-fg);
}
.te-num {
  width: 3.6rem;
  padding: 0.25rem 0.4rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
}
.te-body {
  flex: 1;
  min-height: 0;
  display: grid;
  /* editor gets the lion's share; preview is a narrower panel (its image scales) */
  grid-template-columns: auto minmax(0, 1.7fr) minmax(240px, 0.8fr) auto;
  gap: 0.6rem;
  padding: 0.6rem;
}
.te-rail {
  position: relative;
  display: flex;
}
.te-rail-inner {
  width: 190px;
  overflow: auto;
  padding: 0.5rem;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
}
.te-rail.collapsed {
  width: 1.4rem;
}
.te-rail-toggle {
  align-self: flex-start;
  width: 1.4rem;
  height: 1.8rem;
  border: 1px solid var(--te-border);
  background: var(--te-card);
  color: var(--te-muted-fg);
  border-radius: calc(var(--te-radius) - 2px);
  cursor: pointer;
  font-size: 0.9rem;
  line-height: 1;
  flex: none;
}
.te-rail-title {
  margin: 0 0 0.5rem;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--te-muted-fg);
}
.te-center {
  display: flex;
  min-height: 0;
}
.te-preview-col {
  min-height: 0;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
  padding: 0.4rem;
}
.te-btn {
  padding: 0.4rem 0.75rem;
  border-radius: calc(var(--te-radius) - 2px);
  border: 1px solid transparent;
  font: inherit;
  font-size: 0.82rem;
  cursor: pointer;
}
.te-btn-ghost {
  background: transparent;
  border-color: var(--te-input);
  color: inherit;
}
.te-btn-ghost:hover {
  background: var(--te-accent);
}
.te-btn-primary {
  background: var(--te-primary);
  color: var(--te-primary-fg);
}
.te-btn-primary:disabled {
  opacity: 0.6;
  cursor: default;
}
.te-chip {
  border: 1px solid var(--te-input);
  background: var(--te-card);
  color: var(--te-muted-fg);
  border-radius: 999px;
  padding: 0.1rem 0.5rem;
  font-size: 0.72rem;
  cursor: pointer;
}
.te-chip:hover {
  background: var(--te-accent);
}
/* calculated variables section (left rail) */
.te-calc {
  margin-top: 0.9rem;
  padding-top: 0.6rem;
  border-top: 1px solid var(--te-border);
}
.te-calc-title {
  margin-top: 0;
}
.te-calc-list {
  list-style: none;
  margin: 0 0 0.4rem;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}
.te-calc-item {
  display: flex;
  align-items: center;
  gap: 0.15rem;
}
.te-calc-add {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  flex: 1;
  min-width: 0;
  padding: 0.25rem 0.4rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}
.te-calc-add:hover {
  background: var(--te-accent);
}
/* calculated fields read as Tableau-style: a leading "=" and the accent colour. */
.te-calc-eq {
  color: var(--te-primary);
  font-family: ui-monospace, monospace;
  font-weight: 700;
  font-size: 0.8rem;
}
.te-calc-key {
  font-weight: 500;
  font-size: 0.85rem;
  color: var(--te-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.te-calc-tag {
  margin-left: auto;
}
.te-calc-warn {
  margin-left: auto;
  color: #dc2626;
  font-size: 0.8rem;
}
.te-calc-icon {
  border: 0;
  background: transparent;
  color: var(--te-muted-fg);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 0.15rem;
  flex: none;
}
.te-calc-icon:hover {
  color: inherit;
}
.te-calc-empty {
  margin: 0 0 0.4rem;
  color: var(--te-muted-fg);
  font-size: 0.74rem;
  line-height: 1.35;
}
.te-calc-new {
  width: 100%;
  padding: 0.3rem 0.5rem;
  border: 1px dashed var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: var(--te-primary);
  cursor: pointer;
  font: inherit;
  font-size: 0.78rem;
}
.te-calc-new:hover {
  background: var(--te-accent);
}
</style>
