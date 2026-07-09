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
import { deriveTree, guessLength, pathTypeMap, randomizeSample } from './lib/tree'
import { provideEditorI18n, type Messages } from './i18n'
import type { Element, Region, TicketDoc, VariableType, VarNode } from './types'
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
const t = provideEditorI18n(() => props.locale, () => props.messages)

function blankDoc(): TicketDoc {
  return {
    version: 1,
    paper: { width_chars: 40, margin_left_chars: 1, margin_right_chars: 1,
             margin_top_lines: 1, margin_bottom_lines: 1, cell_width_px: 12, cell_height_px: 22,
      font_px: 20, min_rows: 12 },
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
// path -> type: inferred from samples, overridden by explicit host declarations.
const types = computed<Record<string, VariableType>>(() => ({
  ...pathTypeMap(tree.value),
  ...(props.variableTypes ?? {}),
}))
function typeOf(path?: string): VariableType {
  return (path && types.value[path]) || 'text'
}

// Repeatable groups (loop sources) and all leaf vars (condition targets).
function collect(nodes: VarNode[], loops: { path: string; key: string }[], leaves: { path: string; key: string }[]) {
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
  return leaves
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
watch(() => props.variables, () => { shuffled.value = null })

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
  const el: Element = { id: newId(), row: nextRow(), col: 0, type: 'variable',
    path: node.path, length: guessLength(node.sample), align: 'left' }
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
  const el: Element = { id: newId(), row: nextRow(), col: 0, type: 'qr',
    value: 'https://example.com/r/', from_variable: false, size: 10 }
  doc.value.elements.push(el)
  selectElement(el.id)
}

// Image upload → base64 data URI embedded in the doc (self-contained template).
// Capped: the bytes live inside every persisted/rendered document.
const MAX_IMAGE_BYTES = 2 * 1024 * 1024
const uploadError = ref('')
const fileInput = ref<HTMLInputElement>()
function pickImage() {
  fileInput.value?.click()
}
function readImage(f: File, onData: (dataUrl: string) => void) {
  uploadError.value = ''
  if (!f.type.startsWith('image/')) {
    uploadError.value = 'Not an image file'
    return
  }
  if (f.size > MAX_IMAGE_BYTES) {
    uploadError.value = 'Image too large (max 2 MB)'
    return
  }
  const reader = new FileReader()
  reader.onerror = () => { uploadError.value = 'Could not read the file' }
  reader.onload = () => {
    if (typeof reader.result === 'string') onData(reader.result)
    else uploadError.value = 'Could not read the file'
  }
  reader.readAsDataURL(f)
}
function onImageFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  input.value = '' // allow re-picking the same file
  if (!f) return
  readImage(f, (data) => {
    const el: Element = { id: newId(), row: nextRow(), col: 0, type: 'image',
      data, w: 16, h: 6, mode: { kind: 'threshold', level: 128 } }
    doc.value.elements.push(el)
    selectElement(el.id)
  })
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
    if (row < s) { s -= 1; e -= 1 } // band below the removed row moves up
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

// Deliberate, one-click bulk cleanup: pull every off-paper element back inside
// the printable width. Never runs automatically.
function fitToWidth() {
  const p = doc.value.paper
  const contentCols = Math.max(1, p.width_chars - (p.margin_left_chars ?? 0) - (p.margin_right_chars ?? 0))
  doc.value.elements = doc.value.elements.map((el) => {
    const scale = el.style?.scale ?? 1
    const chars =
      el.type === 'variable'
        ? Math.min(el.length ?? 1, Math.floor(contentCols / scale))
        : ([...(el.content ?? '')].length || 1)
    const span = Math.max(1, chars) * scale
    return { ...el, col: Math.max(0, Math.min(el.col, contentCols - span)) }
  })
}

const saving = ref(false)
async function save() {
  if (!props.onSave) return
  saving.value = true
  try { await props.onSave(snapshot(doc.value)) } finally { saving.value = false }
}
</script>

<template>
  <div class="te-root te-editor">
    <header class="te-toolbar">
      <strong class="te-title">{{ t('title') }}</strong>
      <label class="te-inline">{{ t('width') }}
        <input class="te-num" type="number" min="16" max="120" :value="doc.paper.width_chars"
          @input="doc.paper.width_chars = Math.max(16, +($event.target as HTMLInputElement).value || 40)" />
      </label>
      <label class="te-inline">{{ t('zoom') }}
        <input type="range" min="0.8" max="2.2" step="0.1" v-model.number="zoom" />
        <span class="te-muted">{{ zoom.toFixed(1) }}×</span>
      </label>
      <button class="te-btn te-btn-ghost" type="button" @click="addText">{{ t('addText') }}</button>
      <button class="te-btn te-btn-ghost" type="button" @click="pickImage">{{ t('addImage') }}</button>
      <button class="te-btn te-btn-ghost" type="button" @click="addQr">{{ t('addQr') }}</button>
      <input ref="fileInput" type="file" accept="image/png,image/*" hidden @change="onImageFile" />
      <button class="te-btn te-btn-ghost" type="button" :title="t('fitToWidthTip')" @click="fitToWidth">{{ t('fitToWidth') }}</button>
      <span v-if="uploadError" class="te-upload-err" role="alert">{{ uploadError }}</span>
      <div class="te-spacer" />
      <button v-if="onSave" class="te-btn te-btn-primary" type="button" :disabled="saving" @click="save">
        {{ saving ? t('saving') : t('save') }}
      </button>
    </header>

    <div class="te-body">
      <aside class="te-rail" :class="{ collapsed: !leftOpen }">
        <button class="te-rail-toggle" type="button" @click="leftOpen = !leftOpen"
          :aria-label="leftOpen ? t('collapse') : t('railVariables')" :aria-expanded="leftOpen"
          :title="leftOpen ? t('collapse') : t('railVariables')">
          {{ leftOpen ? '‹' : '›' }}
        </button>
        <div v-if="leftOpen" class="te-rail-inner">
          <h3 class="te-rail-title">{{ t('railVariables') }}</h3>
          <VariableTree :nodes="tree" @add="addVariable" />
        </div>
      </aside>

      <main class="te-center">
        <GridCanvas :doc="doc" :selected-id="selectedId" :selected-band-id="selectedBandId"
          :zoom="zoom" :variables="previewData" :loop-sources="loopSources" :all-vars="allVars"
          @select="selectElement" @select-band="selectBand" @update:element="updateElement"
          @insert-row="insertRow" @delete-row="deleteRow"
          @create-region="createRegion" @remove-region="removeRegion" />
      </main>

      <section class="te-preview-col">
        <PreviewPane :doc="doc" :variables="previewData">
          <template #actions>
            <button class="te-chip" type="button" :title="t('reshuffleTip')" @click="reshuffle">{{ t('reshuffle') }}</button>
          </template>
        </PreviewPane>
      </section>

      <aside class="te-rail te-rail-right" :class="{ collapsed: !rightOpen }">
        <button class="te-rail-toggle right" type="button" @click="rightOpen = !rightOpen"
          :aria-label="rightOpen ? t('collapse') : t('railModifiers')" :aria-expanded="rightOpen"
          :title="rightOpen ? t('collapse') : t('railModifiers')">
          {{ rightOpen ? '›' : '‹' }}
        </button>
        <div v-if="rightOpen" class="te-rail-inner">
          <h3 class="te-rail-title">{{ selectedBand ? t('railBand') : t('railModifiers') }}</h3>
          <BandPanel v-if="selectedBand" :region="selectedBand" :loop-sources="loopSources"
            :all-vars="allVars" @update:region="updateRegion" @remove="removeRegion" />
          <ModifierPanel v-else :element="selected" :var-type="selectedType" :all-vars="allVars"
            @update:element="updateElement" @remove="removeElement" />
        </div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.te-editor { display: flex; flex-direction: column; height: 100%; min-height: 460px; }
.te-toolbar { display: flex; align-items: center; gap: 0.6rem; padding: 0.5rem 0.8rem; border-bottom: 1px solid var(--te-border); flex-wrap: wrap; }
.te-title { font-size: 0.95rem; }
.te-spacer { flex: 1; }
.te-inline { display: flex; align-items: center; gap: 0.35rem; font-size: 0.8rem; color: var(--te-muted-fg); }
.te-muted { color: var(--te-muted-fg); }
.te-num { width: 3.6rem; padding: 0.25rem 0.4rem; border: 1px solid var(--te-input); border-radius: calc(var(--te-radius) - 2px); background: var(--te-card); color: inherit; font: inherit; }
.te-body {
  flex: 1; min-height: 0; display: grid;
  /* editor gets the lion's share; preview is a narrower panel (its image scales) */
  grid-template-columns: auto minmax(0, 1.7fr) minmax(240px, 0.8fr) auto;
  gap: 0.6rem; padding: 0.6rem;
}
.te-rail { position: relative; display: flex; }
.te-rail-inner { width: 190px; overflow: auto; padding: 0.5rem; border: 1px solid var(--te-border); border-radius: var(--te-radius); background: var(--te-card); }
.te-rail.collapsed { width: 1.4rem; }
.te-rail-toggle {
  align-self: flex-start; width: 1.4rem; height: 1.8rem; border: 1px solid var(--te-border);
  background: var(--te-card); color: var(--te-muted-fg); border-radius: calc(var(--te-radius) - 2px);
  cursor: pointer; font-size: 0.9rem; line-height: 1; flex: none;
}
.te-rail-title { margin: 0 0 0.5rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--te-muted-fg); }
.te-center { display: flex; min-height: 0; }
.te-preview-col { min-height: 0; border: 1px solid var(--te-border); border-radius: var(--te-radius); background: var(--te-card); padding: 0.4rem; }
.te-btn { padding: 0.4rem 0.75rem; border-radius: calc(var(--te-radius) - 2px); border: 1px solid transparent; font: inherit; font-size: 0.82rem; cursor: pointer; }
.te-btn-ghost { background: transparent; border-color: var(--te-input); color: inherit; }
.te-btn-ghost:hover { background: var(--te-accent); }
.te-btn-primary { background: var(--te-primary); color: var(--te-primary-fg); }
.te-btn-primary:disabled { opacity: 0.6; cursor: default; }
.te-chip { border: 1px solid var(--te-input); background: var(--te-card); color: var(--te-muted-fg); border-radius: 999px; padding: 0.1rem 0.5rem; font-size: 0.72rem; cursor: pointer; }
.te-chip:hover { background: var(--te-accent); }
.te-upload-err { color: #dc2626; font-size: 0.78rem; }
</style>
