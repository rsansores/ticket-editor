// The editor's headless core: the document, selection, derived variable data
// and every action, with no markup. `TicketEditor.vue` is one layout on top of
// it; a host with its own design system builds another by calling
// `useTicketEditor()` in its own component and placing the connected parts
// (`TicketEditorCanvas`, `TicketEditorInspector`, …) wherever its drawers, rails
// and sheets live. The parts find this state through provide/inject, so they
// keep working when a host teleports them into a sheet.
import {
  computed,
  getCurrentScope,
  inject,
  onScopeDispose,
  provide,
  ref,
  toRaw,
  toValue,
  watch,
  type InjectionKey,
  type MaybeRefOrGetter,
} from 'vue'
import { deriveTree, guessLength, pathTypeMap, randomizeSample } from '../lib/tree'
import { elementFootprint, resolvePath } from '../lib/layout'
import {
  previewComputed,
  previewRowComputed,
  renderPng,
  unresolvedPaths,
} from '../composables/useRenderer'
import { ticketPdf } from '../lib/pdf'
import { DOTS_PER_MM, PAPER_PRESETS, presetForDotWidth, STANDARD_DOT_WIDTHS } from '../lib/paper'
import { provideEditorI18n, type Messages } from '../i18n'
import { RESERVED_ROW_NAMES, SCHEMA_VERSION } from '../types'
import type {
  Computed,
  ComputedResult,
  Element,
  NumberFormat,
  Region,
  TicketDoc,
  VariableType,
  VarGroup,
  VarNode,
  VarOption,
} from '../types'
import '../styles/tokens.css'

export interface TicketEditorOptions {
  /** The document to edit. The editor works on a private copy; see `onUpdate`. */
  modelValue?: MaybeRefOrGetter<TicketDoc | undefined>
  /** Sample data. Drives the variable tree and the live preview. */
  variables?: MaybeRefOrGetter<Record<string, unknown> | undefined>
  /**
   * Authoritative variable types keyed by dotted path (e.g.
   * `{ 'sale.total': 'number', 'sale.items.0.date': 'date' }`).
   * The host declares these; anything not listed falls back to inference from
   * the sample data. Gates which Format options the editor offers.
   */
  variableTypes?: MaybeRefOrGetter<Record<string, VariableType> | undefined>
  /** Force a UI locale (e.g. 'es'). If omitted, follows the host's vue-i18n locale. */
  locale?: MaybeRefOrGetter<string | undefined>
  /** Override / extend built-in UI strings, keyed by locale. */
  messages?: MaybeRefOrGetter<Messages | undefined>
  /** Called with a fresh snapshot after every edit (the v-model write-back). */
  onUpdate?: (doc: TicketDoc) => void
  /** Called by `save()`. When absent, `canSave` is false. */
  onSave?: (doc: TicketDoc) => void | Promise<void>
  /** Overrides whether `save()` is offered. Defaults to `!!onSave`. */
  canSave?: MaybeRefOrGetter<boolean>
}

/**
 * What the user just asked to inspect: an element, a band, or the whole
 * ticket's settings. Fired for a real request, never for a plain clear.
 */
export type EditorSelection = { kind: 'element' | 'band'; id: string } | { kind: 'ticket' }

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

// Decimals and thousands grouping for the canvas, mirroring `format.rs` closely
// enough to read right (`24.5` → `24.50`). Rounding modes are not replicated:
// the 1:1 preview is the exact print, this is only the sketch.
function displayNumber(raw: string, fmt: NumberFormat): string {
  const n = Number(raw.trim().replace(/,/g, ''))
  if (!Number.isFinite(n)) return raw
  const [int, frac] = Math.abs(n).toFixed(Math.max(0, fmt.decimals)).split('.')
  const grouped = fmt.thousands ? int.replace(/\B(?=(\d{3})+(?!\d))/g, ',') : int
  return (n < 0 ? '-' : '') + grouped + (frac ? `.${frac}` : '')
}

function createTicketEditor(opts: TicketEditorOptions) {
  const variables = () => toValue(opts.variables)

  // Translation: built-in en/es, follows host vue-i18n locale, overridable.
  const t = provideEditorI18n(
    () => toValue(opts.locale),
    () => toValue(opts.messages),
  )

  const initial = toValue(opts.modelValue)
  const doc = ref<TicketDoc>(initial ? snapshot(initial) : blankDoc())
  // Track the snapshot we last emitted so the round-trip through v-model doesn't
  // echo back into our state (which would loop); a genuinely new doc still loads.
  let lastEmitted: TicketDoc | null = null
  watch(
    () => toValue(opts.modelValue),
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
      opts.onUpdate?.(snap)
    },
    { deep: true },
  )

  const tree = computed<VarNode[]>(() => deriveTree(variables() ?? {}))
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
    [computedVars, variables],
    async () => {
      try {
        const rep = await previewComputed(computedVars.value, variables() ?? {})
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
    ...(toValue(opts.variableTypes) ?? {}),
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
  // A layout reacts to a selection (opens its inspector drawer) through this,
  // not a watch on the ids: re-selecting the already-selected element must still
  // reopen a drawer the user collapsed, and a watch would not fire for it.
  const selectListeners = new Set<(sel: EditorSelection) => void>()
  function onSelect(fn: (sel: EditorSelection) => void): () => void {
    selectListeners.add(fn)
    const off = () => selectListeners.delete(fn)
    if (getCurrentScope()) onScopeDispose(off)
    return off
  }
  // Element and band selection are mutually exclusive (one right-drawer at a time).
  function selectElement(id: string | null) {
    selectedId.value = id
    if (id) {
      selectedBandId.value = null
      for (const fn of selectListeners) fn({ kind: 'element', id })
    }
  }
  function selectBand(id: string | null) {
    selectedBandId.value = id
    if (id) {
      selectedId.value = null
      for (const fn of selectListeners) fn({ kind: 'band', id })
    }
  }

  // The inspector shows the ticket's settings when nothing is selected; this
  // clears the selection AND tells the layout to bring the inspector into view
  // (a plain clear, like clicking empty canvas, doesn't).
  function showTicketSettings() {
    selectedId.value = null
    selectedBandId.value = null
    for (const fn of selectListeners) fn({ kind: 'ticket' })
  }

  // preview data: real variables, or a reshuffled clone when the user asks.
  const shuffled = ref<Record<string, unknown> | null>(null)
  const previewData = computed(() => shuffled.value ?? variables())
  function reshuffle() {
    shuffled.value = randomizeSample(variables() ?? {})
  }
  // Drop stale reshuffled data when the host swaps the variable set.
  watch(variables, () => {
    shuffled.value = null
  })

  // view state
  const zoom = ref(1.0)
  // The canvas shows sample values by default (it reads like the ticket); this
  // flips it to the variable names, for wiring work.
  const showFields = ref(false)

  let seq = 0
  function newId() {
    seq += 1
    return `el_${seq}_${Math.floor(Math.random() * 1e6)}`
  }
  function nextRow(): number {
    return doc.value.elements.reduce((m, e) => Math.max(m, e.row + 1), 0)
  }

  // Where the next added element goes. Null (the default) appends it on a new
  // line at the bottom; a row number puts it on THAT line, after whatever is
  // already there — the phone layout's per-line "+" sets it before opening the
  // picker. Consumed (reset to null) by the add.
  const addTarget = ref<number | null>(null)
  // First free column on a line: one past the right edge of everything that
  // occupies it (a gap so values don't run together), or 0 on an empty line.
  // Never moves existing elements; a new one that doesn't fit still lands past
  // the edge, where the canvas flags it like any other.
  function nextFreeCol(row: number): number {
    let end = -1
    for (const e of doc.value.elements) {
      if (e.type === 'marker') continue
      const f = elementFootprint(e, doc.value.paper, sampleOf(e))
      if (row >= e.row && row < e.row + f.rows) end = Math.max(end, e.col + f.cols)
    }
    return end < 0 ? 0 : end + 1
  }
  function pushNew(el: Element) {
    const row = addTarget.value
    if (row !== null) {
      addTarget.value = null
      el.row = row
      el.col = el.type === 'marker' ? 0 : nextFreeCol(row)
    }
    doc.value.elements.push(el)
    selectElement(el.id)
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
    pushNew(el)
  }
  function addText() {
    const el: Element = { id: newId(), row: nextRow(), col: 0, type: 'text', content: 'Text' }
    pushNew(el)
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
    pushNew(el)
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
    pushNew(el)
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
        if (fields.length)
          rows.push({ label: t('calcGroupRow', { list: n.path }), options: fields })
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
  // Splice a draft formula into a computed list at its real position (an edit
  // replaces in place so forward references resolve exactly as they will save;
  // a new entry is appended). Shared by the doc-level and row-level dialogs so
  // the two preview paths can't drift.
  function draftList(
    all: Computed[],
    origName: string | undefined,
    formula: string,
  ): { list: Computed[]; key: string } {
    const idx = origName ? all.findIndex((c) => c.name === origName) : -1
    const key = idx >= 0 ? all[idx].name : DRAFT_KEY
    const list =
      idx >= 0
        ? all.map((c, i) => (i === idx ? { name: key, formula } : c))
        : [...all, { name: key, formula }]
    return { list, key }
  }
  // Upsert by the original name captured when the dialog opened — renames keep
  // their position. Shared by doc-level and band-level saves.
  function upsertComputed(
    list: Computed[],
    origName: string | undefined,
    next: Computed,
  ): Computed[] {
    const i = origName ? list.findIndex((c) => c.name === origName) : -1
    const out = [...list]
    if (i >= 0) out[i] = next
    else out.push(next)
    return out
  }
  async function previewFormula(formula: string): Promise<ComputedResult> {
    const { list, key } = draftList(computedVars.value, editingCalc.value?.name, formula)
    const rep = await previewComputed(list, variables() ?? {})
    return rep.find((r) => r.name === key) ?? { name: key, value: '', kind: 'empty', error: null }
  }
  function newCalc() {
    editingCalc.value = { name: '', formula: '' }
  }
  function editCalc(c: Computed) {
    editingCalc.value = JSON.parse(JSON.stringify(c)) as Computed
  }
  function saveCalc(next: Computed) {
    doc.value.computed = upsertComputed(doc.value.computed ?? [], editingCalc.value?.name, next)
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

  // --- calculated columns (row-scoped, live on a band) -----------------------
  // The column being edited: which band it belongs to plus the draft value. The
  // same ComputedEditor dialog is reused with variant="row" — no new mental model.
  const editingRowCalc = ref<{ regionId: string; calc: Computed } | null>(null)
  function regionById(id: string): Region | undefined {
    return (doc.value.regions ?? []).find((r) => r.id === id)
  }
  function editRowCalc(regionId: string, calc: Computed | null) {
    editingRowCalc.value = {
      regionId,
      calc: calc ? (JSON.parse(JSON.stringify(calc)) as Computed) : { name: '', formula: '' },
    }
  }
  function saveRowCalc(next: Computed) {
    const ctx = editingRowCalc.value
    editingRowCalc.value = null
    if (!ctx) return
    const region = regionById(ctx.regionId)
    if (!region) return
    updateRegion({
      ...region,
      computed: upsertComputed(region.computed ?? [], ctx.calc.name, next),
    })
  }
  function removeRowCalc(regionId: string, name: string) {
    const region = regionById(regionId)
    if (!region) return
    const list = (region.computed ?? []).filter((c) => c.name !== name)
    updateRegion({ ...region, computed: list.length ? list : undefined })
  }
  // Live first-row results per band, keyed regionId → name → result. Drives the
  // value chips in the band drawer and the default formatting when placed.
  // Populated by the shared integrity watcher below.
  const rowCalcReports = ref<Record<string, Record<string, ComputedResult>>>({})
  // Evaluate a DRAFT row formula in its real position within the band's list
  // (same replace-in-place trick as previewFormula) against the first data row.
  async function previewRowFormula(formula: string): Promise<ComputedResult> {
    const ctx = editingRowCalc.value
    if (!ctx) return { name: '', value: '', kind: 'empty', error: null }
    const { list, key } = draftList(
      regionById(ctx.regionId)?.computed ?? [],
      ctx.calc.name,
      formula,
    )
    const rep = await previewRowComputed(
      snapshot(doc.value),
      ctx.regionId,
      list,
      previewData.value ?? {},
    )
    return rep.find((r) => r.name === key) ?? { name: key, value: '', kind: 'empty', error: null }
  }
  function findNode(nodes: VarNode[], path: string): VarNode | undefined {
    for (const n of nodes) {
      if (n.path === path) return n
      if (n.children) {
        const f = findNode(n.children, path)
        if (f) return f
      }
    }
    return undefined
  }
  // Variable groups for the row-formula picker: this row's fields first, then
  // earlier calculated columns, the implicit line info, then everything a
  // doc-level formula can see. Ordering is the teaching device: the top group is
  // what a non-technical user almost always wants.
  const rowVarGroups = computed<VarGroup[]>(() => {
    const ctx = editingRowCalc.value
    if (!ctx) return varGroups.value
    const region = regionById(ctx.regionId)
    const groups: VarGroup[] = []
    if (region?.source) {
      const fields: VarOption[] = []
      collectRowFields(
        findNode(tree.value, region.source)?.children ?? [],
        `${region.source}.0.`,
        fields,
      )
      if (fields.length) groups.push({ label: t('calcGroupThisRow'), options: fields })
    }
    const earlier: VarOption[] = []
    for (const c of region?.computed ?? []) {
      if (ctx.calc.name && c.name === ctx.calc.name) break // only EARLIER columns resolve
      earlier.push({ label: `row.${c.name}`, insert: `row.${c.name}` })
    }
    if (earlier.length) groups.push({ label: t('calcGroupRowCalcs'), options: earlier })
    if (region?.source) {
      groups.push({
        label: t('calcGroupLineInfo'),
        options: [
          { label: `row.number — ${t('rowLineNumber')}`, insert: 'row.number' },
          { label: `row.count — ${t('rowLineCount')}`, insert: 'row.count' },
          { label: `row.first — ${t('rowLineFirst')}`, insert: 'row.first' },
          { label: `row.last — ${t('rowLineLast')}`, insert: 'row.last' },
          { label: `row.index — ${t('rowLineIndex')}`, insert: 'row.index' },
        ],
      })
    }
    groups.push(...varGroups.value)
    return groups
  })
  // Place a calculated column on the ticket: a variable element bound to
  // row.<name>, dropped on the band's first row with type-appropriate defaults.
  function placeRowCalc(regionId: string, c: Computed) {
    const region = regionById(regionId)
    if (!region) return
    const rep = rowCalcReports.value[regionId]?.[c.name]
    const el: Element = {
      id: newId(),
      row: region.start_row,
      col: 0,
      type: 'variable',
      path: `row.${c.name}`,
      length: 10,
      align: 'left',
    }
    if (rep?.kind === 'number') {
      const hasFraction = rep.value.includes('.')
      el.number = { decimals: hasFraction ? 2 : 0, rounding: 'half_up', thousands: true }
      el.align = 'right'
      el.length = Math.max(8, rep.value.length + 2)
    } else if (rep?.value) {
      el.length = Math.min(40, Math.max(6, rep.value.length + 2))
    }
    pushNew(el)
  }

  // The sample text a variable element shows on the canvas: host data, a
  // calculated value, or a band's calculated column (first data row). Raw, not
  // formatted — the 1:1 preview is the exact print; this only has to read right.
  function sampleOf(el: Element): string | undefined {
    const raw = rawSampleOf(el)
    return raw !== undefined && el.type === 'variable' && el.number
      ? displayNumber(raw, el.number)
      : raw
  }
  function rawSampleOf(el: Element): string | undefined {
    if (el.type !== 'variable' || !el.path) return undefined
    const path = el.path
    if (path.startsWith('calc.')) {
      const r = calcReports.value[path.slice('calc.'.length)]
      return r && !r.error && r.value !== '' ? r.value : undefined
    }
    if (path.startsWith('row.')) {
      const band = (doc.value.regions ?? []).find(
        (r) => el.row >= r.start_row && el.row < r.end_row,
      )
      const r = band && rowCalcReports.value[band.id]?.[path.slice('row.'.length)]
      return r && !r.error && r.value !== '' ? r.value : undefined
    }
    return resolvePath(previewData.value, path)
  }

  // --- missing-fields badge ---------------------------------------------------
  // Paths the document references that don't exist in the sample data. On a REAL
  // print these render empty, so surface them while designing (the preview shows
  // fakes for liveliness — this badge is the honesty layer on top).
  const missingPaths = ref<string[]>([])

  // One debounced integrity pass per doc/data change: a single snapshot feeds
  // the missing-paths check and every band's row-calc preview, in parallel. The
  // sequence token drops stale async results (rapid edits can resolve out of
  // order and would otherwise clobber a newer state with an older answer).
  let integritySeq = 0
  let integrityTimer: ReturnType<typeof setTimeout> | undefined
  async function runIntegrityPass() {
    const seq = ++integritySeq
    const snap = snapshot(doc.value)
    const data = previewData.value ?? {}
    try {
      const bands = (snap.regions ?? []).filter((r) => r.computed?.length)
      const [missing, reports] = await Promise.all([
        unresolvedPaths(snap, data),
        Promise.all(bands.map((r) => previewRowComputed(snap, r.id, r.computed ?? [], data))),
      ])
      if (seq !== integritySeq) return // superseded by a newer edit
      missingPaths.value = missing
      rowCalcReports.value = Object.fromEntries(
        bands.map((r, i) => [r.id, Object.fromEntries(reports[i].map((x) => [x.name, x]))]),
      )
    } catch {
      if (seq !== integritySeq) return
      missingPaths.value = []
      rowCalcReports.value = {}
    }
  }
  watch(
    [doc, previewData],
    () => {
      clearTimeout(integrityTimer)
      integrityTimer = setTimeout(runIntegrityPass, 180)
    },
    { immediate: true, deep: true },
  )

  // --- element condition context ----------------------------------------------
  // The band containing the selected element (if any): its row.* names are valid
  // condition targets and must not be flagged UNAVAILABLE.
  const selectedElBand = computed<Region | null>(() => {
    const e = selected.value
    if (!e) return null
    return (doc.value.regions ?? []).find((r) => e.row >= r.start_row && e.row < r.end_row) ?? null
  })
  const selectedRowPaths = computed<string[]>(() => {
    const r = selectedElBand.value
    if (!r) return []
    const names = (r.computed ?? []).map((c) => `row.${c.name}`)
    if (r.source) names.push(...RESERVED_ROW_NAMES.map((n) => `row.${n}`))
    return names
  })
  const selectedCondVars = computed(() => [
    ...selectedRowPaths.value.map((p) => ({ path: p, key: p })),
    ...allVars.value,
  ])
  // "Collapse the row when hidden": turn the element's condition into a one-row
  // conditional band (regions collapse; elements only hide). The band is a real,
  // visible object in the lane afterwards — no hidden magic to keep in sync.
  function collapseRow(id: string) {
    const el = doc.value.elements.find((e) => e.id === id)
    if (!el?.condition) return
    const condition = el.condition
    updateElement({ ...el, condition: undefined })
    createRegion({ start_row: el.row, end_row: el.row + 1, condition })
  }

  // Add a DYNAMIC image: its bytes come from a variable at print time (a signature,
  // a plot, …). This is the default because it's the common case; providing a file
  // in the modifier panel downgrades it to a static, embedded image. No upload
  // dialog on add — an image with no source just shows a placeholder.
  // A finishing marker: zero ink, tells the backend where to cut / kick the
  // drawer. Defaults to `cut` at the end of the ticket — the overwhelmingly
  // common case (one receipt = cut at the end).
  function addMarker() {
    const el: Element = { id: newId(), row: nextRow(), col: 0, type: 'marker', name: 'cut' }
    pushNew(el)
  }
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
    pushNew(el)
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
    doc.value.elements = doc.value.elements.map((e) =>
      e.row >= row ? { ...e, row: e.row + 1 } : e,
    )
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

  // Delete a whole line WITH what starts on it (the phone's swipe-to-delete,
  // which asks for a deliberate tap). Unlike deleteRow it doesn't need the line
  // to be empty; the elements go, then the line closes up like deleteRow.
  function removeLine(row: number, eff: number) {
    const gone = new Set(doc.value.elements.filter((e) => e.row === row).map((e) => e.id))
    doc.value.elements = doc.value.elements.filter((e) => !gone.has(e.id))
    if (selectedId.value && gone.has(selectedId.value)) selectedId.value = null
    deleteRow(row, eff)
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

  // Raster width in printer dots. Thermal heads have a fixed native dot width
  // (203 dpi: 384 = 58 mm paper, 576 = 80 mm); anything else gets scaled by the
  // driver and prints fuzzy. Worth far more than a contrast slider.
  const dotWidth = computed(
    () => doc.value.paper.width_chars * (doc.value.paper.cell_width_px ?? 12),
  )
  const dotWidthOk = computed(() => STANDARD_DOT_WIDTHS.includes(dotWidth.value))

  // Pick the paper, get the columns — not the other way round. Landing the grid on
  // the printer's dot width is a constraint the user cannot be expected to solve in
  // their head, and getting it wrong is invisible until the paper comes out narrow.
  // Columns stay editable: a denser font (a smaller cell) is a legitimate thing to
  // want, and 'custom' covers the rare printer neither preset fits.
  const paperId = computed(() => presetForDotWidth(dotWidth.value)?.id ?? 'custom')

  function selectPaper(id: string) {
    const preset = PAPER_PRESETS.find((p) => p.id === id)
    if (!preset) return // 'custom' — leave the document alone, the user drives.
    doc.value.paper.width_chars = preset.cols
    doc.value.paper.cell_width_px = preset.cellPx
  }
  // Columns per line, clamped to the narrowest grid the renderer handles. A
  // cleared or non-numeric input falls back to 40.
  function setWidthChars(n: number) {
    doc.value.paper.width_chars = Math.max(16, n || 40)
  }

  // Printable content width in characters — used by a field's per-element
  // "Fit to width" action in the modifier panel.
  const contentCols = computed(() => {
    const p = doc.value.paper
    return Math.max(1, p.width_chars - (p.margin_left_chars ?? 0) - (p.margin_right_chars ?? 0))
  })

  // Hand the ticket to the OS as a PDF, sized to the paper.
  //
  // Not the browser's print dialog: it draws its own headers and footers (URL,
  // timestamp, page title) from a checkbox we cannot reach, and its "Margins:
  // Default" overrides the CSS that would reserve no room for them. A PDF has an
  // authoritative page size and prints exactly what it says.
  //
  // Renders in PRINT mode (placeholders off): a ticket someone holds must show
  // what the customer would get, never a plausible fake for a missing value.
  const printing = ref(false)
  const printError = ref('')

  async function print() {
    printing.value = true
    printError.value = ''
    try {
      const png = await renderPng(snapshot(doc.value), variables(), false)
      const pdf = await ticketPdf(png, DOTS_PER_MM)
      const url = URL.createObjectURL(pdf)
      // A new tab, opened from the click, so no popup blocker: the PDF viewer
      // shows it at true size and its own Print prints it clean.
      const opened = window.open(url, '_blank')
      if (!opened) {
        // Popups blocked after all — fall back to a download.
        const a = document.createElement('a')
        a.href = url
        a.download = 'ticket.pdf'
        a.click()
      }
      // The tab has the blob; give it time to load before dropping our handle.
      setTimeout(() => URL.revokeObjectURL(url), 60_000)
    } catch (e) {
      printError.value = e instanceof Error ? e.message : String(e)
    } finally {
      printing.value = false
    }
  }

  const canSave = computed(() => toValue(opts.canSave) ?? !!opts.onSave)
  const saving = ref(false)
  async function save() {
    if (!opts.onSave) return
    saving.value = true
    try {
      await opts.onSave(snapshot(doc.value))
    } finally {
      saving.value = false
    }
  }

  return {
    t,
    doc,
    // variables
    tree,
    types,
    typeOf,
    loopSources,
    allVars,
    computedVars,
    calcReports,
    calcKind,
    calcHasError,
    // selection
    selectedId,
    selectedBandId,
    selected,
    selectedType,
    selectedBand,
    selectedElBand,
    selectedRowPaths,
    selectedCondVars,
    selectElement,
    selectBand,
    showTicketSettings,
    onSelect,
    // preview
    previewData,
    reshuffle,
    missingPaths,
    zoom,
    showFields,
    addTarget,
    sampleOf,
    // elements
    addVariable,
    addText,
    addImage,
    addQr,
    addBarcode,
    addMarker,
    addCalcElement,
    updateElement,
    removeElement,
    collapseRow,
    insertRow,
    deleteRow,
    removeLine,
    // bands
    createRegion,
    updateRegion,
    removeRegion,
    regionById,
    // calculated variables
    editingCalc,
    varGroups,
    previewFormula,
    newCalc,
    editCalc,
    saveCalc,
    removeCalc,
    // calculated columns
    editingRowCalc,
    rowVarGroups,
    rowCalcReports,
    previewRowFormula,
    editRowCalc,
    saveRowCalc,
    removeRowCalc,
    placeRowCalc,
    // paper
    dotWidth,
    dotWidthOk,
    paperId,
    selectPaper,
    setWidthChars,
    contentCols,
    // output
    printing,
    printError,
    print,
    canSave,
    saving,
    save,
  }
}

export type TicketEditorContext = ReturnType<typeof createTicketEditor>

const EDITOR_KEY: InjectionKey<TicketEditorContext> = Symbol('ticket-editor')

/**
 * Create the editor state and provide it to every connected part below the
 * calling component. Call once, from the setup of the component that owns the
 * layout. Returns the same context the parts inject.
 */
export function useTicketEditor(opts: TicketEditorOptions = {}): TicketEditorContext {
  const ctx = createTicketEditor(opts)
  provide(EDITOR_KEY, ctx)
  return ctx
}

/** The context provided by the nearest `useTicketEditor()` above this component. */
export function useTicketEditorContext(): TicketEditorContext {
  const ctx = inject(EDITOR_KEY, null)
  if (!ctx) {
    throw new Error(
      'ticket-editor: a connected part was mounted outside useTicketEditor(). ' +
        'Call useTicketEditor() in an ancestor component.',
    )
  }
  return ctx
}
