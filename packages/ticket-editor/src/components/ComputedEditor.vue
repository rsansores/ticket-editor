<script setup lang="ts">
// The calculated-variable editor: a small formula box made comfortable for
// non-technical users. You never have to remember a variable's full name or a
// function's syntax — the two pickers insert them for you, and the function
// dropdown shows each signature so the editor is self-documenting. A live
// preview (evaluated by the same wasm engine that prints) and an inline error
// line make it forgiving to type.
import { computed, nextTick, ref, watch } from 'vue'
import { useT } from '../i18n'
import type { Computed, ComputedResult, VarGroup } from '../types'
import { FUNCTIONS, isValidName, type FnDoc } from '../lib/computed'

const t = useT()

const props = defineProps<{
  /** The value being edited, or null when the dialog is closed. Empty name = new. */
  modelValue: Computed | null
  /** Insertable variables, grouped (Values / Lists / each list's row fields). */
  varGroups: VarGroup[]
  /** Evaluate a draft formula through the wasm engine (provided by the parent). */
  preview: (formula: string) => Promise<ComputedResult>
  /** Names already taken (to reject duplicates). */
  existingNames: string[]
  /** Doc-level calculated value (default) or a band's calculated column. Only
   *  swaps the dialog copy — the form and engine are identical. */
  variant?: 'doc' | 'row'
  /** Names the engine reserves (the implicit row.* values); rejected like dupes. */
  reservedNames?: readonly string[]
}>()
const emit = defineEmits<{ save: [c: Computed]; cancel: [] }>()

const name = ref('')
const formula = ref('')
const originalName = ref('')
const formulaEl = ref<HTMLTextAreaElement>()
const varPick = ref('')
const fnPick = ref('')
const pickedFn = ref<FnDoc | null>(null)

// --- live preview (debounced) --------------------------------------------
const result = ref<ComputedResult | null>(null)
let timer: ReturnType<typeof setTimeout> | undefined
function runPreview() {
  clearTimeout(timer)
  timer = setTimeout(async () => {
    const f = formula.value
    try {
      const r = await props.preview(f)
      if (formula.value === f) result.value = r // ignore stale
    } catch {
      result.value = { name: '', value: '', kind: 'empty', error: t('calcEngineError') }
    }
  }, 180)
}
watch(formula, runPreview)

// Load an incoming value into the form whenever the dialog opens.
watch(
  () => props.modelValue,
  (c) => {
    if (!c) return
    originalName.value = c.name
    name.value = c.name
    formula.value = c.formula
    pickedFn.value = null
    runPreview()
  },
  { immediate: true },
)

// --- insertion helpers ----------------------------------------------------
function insert(text: string, caretBack = 0) {
  const el = formulaEl.value
  const s = el?.selectionStart ?? formula.value.length
  const e = el?.selectionEnd ?? formula.value.length
  const before = formula.value.slice(0, s)
  // Add a space before a token if we're butting up against a word/number.
  const glue = before && /[\w.)"]$/.test(before) && /^[\w"]/.test(text) ? ' ' : ''
  formula.value = before + glue + text + formula.value.slice(e)
  const caret = s + glue.length + text.length - caretBack
  nextTick(() => {
    el?.focus()
    el?.setSelectionRange(caret, caret)
  })
}
function onPickVar(e: Event) {
  const v = (e.target as HTMLSelectElement).value
  if (v) insert(v)
  varPick.value = ''
}
function onPickFn(e: Event) {
  const f = FUNCTIONS.find((x) => x.name === (e.target as HTMLSelectElement).value)
  if (f) {
    insert(f.insert, f.caretBack)
    pickedFn.value = f
  }
  fnPick.value = ''
}

// --- validation -----------------------------------------------------------
const nameError = computed<string>(() => {
  const n = name.value.trim()
  if (n === '') return t('calcNameRequired')
  if (!isValidName(n)) return t('calcNameInvalid')
  if (props.reservedNames?.includes(n)) return t('calcNameReserved', { name: n })
  if (n !== originalName.value && props.existingNames.includes(n)) return t('calcNameTaken')
  return ''
})
const formulaError = computed<string>(() => result.value?.error ?? '')
const canSave = computed(
  () => nameError.value === '' && formula.value.trim() !== '' && formulaError.value === '',
)

function save() {
  if (!canSave.value) return
  emit('save', { name: name.value.trim(), formula: formula.value.trim() })
}
</script>

<template>
  <div class="te-modal-backdrop" @click.self="emit('cancel')">
    <div class="te-modal" role="dialog" aria-modal="true">
      <header class="te-modal-head">
        <strong>{{
          variant === 'row'
            ? modelValue && modelValue.name
              ? t('rowCalcEditTitle')
              : t('rowCalcNewTitle')
            : modelValue && modelValue.name
              ? t('calcEditTitle')
              : t('calcNewTitle')
        }}</strong>
        <button class="te-modal-x" type="button" :aria-label="t('cancel')" @click="emit('cancel')">
          ✕
        </button>
      </header>
      <p class="te-modal-lead">{{ variant === 'row' ? t('rowCalcLead') : t('calcLead') }}</p>

      <label class="te-field">
        <span>{{ t('calcName') }}</span>
        <input
          class="te-input"
          :value="name"
          :placeholder="t('calcNamePlaceholder')"
          @input="name = ($event.target as HTMLInputElement).value"
        />
        <small v-if="nameError" class="te-err">{{ nameError }}</small>
      </label>

      <div class="te-field">
        <span>{{ t('calcFormula') }}</span>
        <textarea
          ref="formulaEl"
          class="te-input te-formula"
          rows="3"
          spellcheck="false"
          :value="formula"
          :placeholder="t('calcFormulaPlaceholder')"
          @input="formula = ($event.target as HTMLTextAreaElement).value"
        ></textarea>
        <div class="te-inserters">
          <select class="te-mini" :value="varPick" @change="onPickVar">
            <option value="">{{ t('calcInsertVar') }}</option>
            <optgroup v-for="g in varGroups" :key="g.label" :label="g.label">
              <option v-for="(o, i) in g.options" :key="g.label + i" :value="o.insert">
                {{ o.label }}
              </option>
            </optgroup>
          </select>
          <select class="te-mini" :value="fnPick" @change="onPickFn">
            <option value="">{{ t('calcInsertFn') }}</option>
            <option v-for="f in FUNCTIONS" :key="f.name" :value="f.name">{{ f.sig }}</option>
          </select>
        </div>
        <p v-if="pickedFn" class="te-fn-doc">
          <code>{{ pickedFn.sig }}</code> — {{ t(pickedFn.descKey) }}
        </p>
        <p class="te-hint">{{ t('calcSyntaxHint') }}</p>
      </div>

      <div class="te-preview-line" :class="{ 'has-error': formulaError }">
        <span class="te-preview-label">{{ formulaError ? t('calcError') : t('calcPreview') }}</span>
        <code v-if="formulaError" class="te-preview-val err">{{ formulaError }}</code>
        <code v-else class="te-preview-val">{{
          result && result.value !== '' ? result.value : t('calcPreviewEmpty')
        }}</code>
      </div>
      <p v-if="variant === 'row'" class="te-hint">{{ t('rowCalcPreviewNote') }}</p>

      <footer class="te-modal-foot">
        <button class="te-btn te-btn-ghost" type="button" @click="emit('cancel')">
          {{ t('cancel') }}
        </button>
        <button class="te-btn te-btn-primary" type="button" :disabled="!canSave" @click="save">
          {{ t('calcSave') }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.te-modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
  padding: 1rem;
}
.te-modal {
  width: min(480px, 100%);
  max-height: 90vh;
  overflow: auto;
  background: var(--te-card);
  color: inherit;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  font-size: 0.85rem;
}
.te-modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.te-modal-x {
  border: 0;
  background: transparent;
  color: var(--te-muted-fg);
  cursor: pointer;
  font-size: 0.9rem;
}
.te-modal-lead {
  margin: 0;
  color: var(--te-muted-fg);
  font-size: 0.78rem;
}
.te-field {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
.te-field > span {
  color: var(--te-muted-fg);
  font-size: 0.75rem;
}
.te-input {
  width: 100%;
  padding: 0.35rem 0.5rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
  font-size: 0.85rem;
}
.te-input:focus {
  outline: 2px solid var(--te-ring);
  outline-offset: -1px;
}
.te-formula {
  font-family: ui-monospace, monospace;
  font-size: 0.82rem;
  resize: vertical;
  line-height: 1.4;
}
.te-err {
  color: #dc2626;
  font-size: 0.72rem;
}
.te-inserters {
  display: flex;
  gap: 0.4rem;
}
.te-mini {
  flex: 1;
  min-width: 0;
  padding: 0.25rem 0.4rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: var(--te-primary);
  font: inherit;
  font-size: 0.76rem;
  cursor: pointer;
}
.te-fn-doc {
  margin: 0;
  font-size: 0.75rem;
  color: var(--te-muted-fg);
}
.te-fn-doc code {
  font-family: ui-monospace, monospace;
  color: inherit;
}
.te-hint {
  margin: 0;
  font-size: 0.72rem;
  color: var(--te-muted-fg);
  line-height: 1.35;
}
.te-preview-line {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  padding: 0.5rem 0.6rem;
  background: var(--te-accent);
  border-radius: calc(var(--te-radius) - 2px);
  flex-wrap: wrap;
}
.te-preview-line.has-error {
  background: color-mix(in srgb, #dc2626 12%, transparent);
}
.te-preview-label {
  color: var(--te-muted-fg);
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.te-preview-val {
  font-family: ui-monospace, monospace;
  font-size: 0.8rem;
  word-break: break-all;
}
.te-preview-val.err {
  color: #dc2626;
}
.te-modal-foot {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 0.2rem;
}
.te-btn {
  padding: 0.4rem 0.8rem;
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
  opacity: 0.55;
  cursor: default;
}
</style>
