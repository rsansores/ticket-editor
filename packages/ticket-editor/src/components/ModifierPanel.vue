<script setup lang="ts">
// The modifier panel for the selected element: content, reserved length, wrap,
// alignment, size magnification, bold/italic, and — for variables — type-aware
// number (decimals + rounding + thousands) or date-format modifiers.
//
// Exported on its own so a host app can drop it into their own
// Sheet/drawer instead of the inline rail if they prefer.
import { computed, ref } from 'vue'
import ConditionEditor from './ConditionEditor.vue'
import { useT } from '../i18n'
import { FONT_OPTIONS } from '../lib/fonts'
import type {
  Align,
  Condition,
  Element,
  NumberFormat,
  Rounding,
  Symbology,
  VAlign,
  VariableType,
} from '../types'

const t = useT()

const props = defineProps<{
  element: Element | null
  varType?: VariableType
  /** All leaf variables, for the QR "from variable" picker. */
  allVars?: { path: string; key: string }[]
  /** Repeatable list paths, so loop-relative fields aren't flagged unavailable. */
  loopSources?: { path: string; key: string }[]
  /** Printable content width in characters, for the "Fit to width" action. */
  contentCols?: number
  /** Selectable targets for the element's "show only if" condition. Includes
   *  the band's row.* values when the element sits inside a loop band. */
  condVars?: { path: string; key: string }[]
  /** Paths valid for THIS element beyond the global catalog (its band's
   *  row.* values), so they aren't flagged UNAVAILABLE. */
  extraKnownPaths?: string[]
  /** Whether the element already sits inside a band (the collapse-row sugar
   *  would create a second, overlapping one — hidden in that case). */
  inBand?: boolean
}>()
const emit = defineEmits<{
  'update:element': [el: Element]
  remove: [id: string]
  /** Convert this element's condition into a one-row collapsible band. */
  'collapse-row': [id: string]
}>()

// Image replace. Capture the target element at pick time so a selection change
// during the async read can't apply the image to the wrong element. Validated
// and size-capped like the toolbar upload.
const MAX_IMAGE_BYTES = 2 * 1024 * 1024
const replaceInput = ref<HTMLInputElement>()
function onReplaceFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  const target = props.element // captured now, not at callback time
  input.value = ''
  if (!f || !target) return
  if (!f.type.startsWith('image/') || f.size > MAX_IMAGE_BYTES) return
  const reader = new FileReader()
  reader.onload = () => {
    if (typeof reader.result === 'string') {
      // Providing a file makes the image static (embedded bytes).
      emit('update:element', { ...target, data: reader.result, from_variable: false })
    }
  }
  reader.readAsDataURL(f)
}

const el = computed(() => props.element)

// A reference to a variable that isn't in the current catalog (e.g. an imported
// design). Surfaced so the user can remove the element or point it elsewhere.
const knownPaths = computed(() => new Set((props.allVars ?? []).map((v) => v.path)))
const loopPrefixes = computed(() => (props.loopSources ?? []).map((l) => `${l.path}.`))
// A loop-relative field (`sale.items.0.qty`) is "known" whenever its list is —
// even if the sample array is empty, so it isn't derivable from `allVars`.
function pathKnown(path: string): boolean {
  return (
    knownPaths.value.has(path) ||
    loopPrefixes.value.some((p) => path.startsWith(p)) ||
    (props.extraKnownPaths ?? []).includes(path)
  )
}
const unavailable = computed(() => {
  const e = el.value
  if (!e) return false
  if (e.type === 'variable') return !!e.path && !pathKnown(e.path)
  if ((e.type === 'qr' || e.type === 'barcode') && e.from_variable)
    return !!e.value && !pathKnown(e.value)
  if (e.type === 'image' && e.from_variable) return !!e.data && !pathKnown(e.data)
  return false
})

function patch(p: Partial<Element>) {
  if (!el.value) return
  emit('update:element', { ...el.value, ...p })
}
function patchStyle(p: Partial<NonNullable<Element['style']>>) {
  if (!el.value) return
  emit('update:element', { ...el.value, style: { ...el.value.style, ...p } })
}
const fontOptions = FONT_OPTIONS
// Store `undefined` for the built-in default so a plain document stays clean.
function setFont(v: string) {
  patchStyle({ font: v === 'mono' ? undefined : v })
}
// Stretch a variable's reserved width to fill from its column to the paper's
// right edge (respecting size magnification). With an alignment set, this is how
// you make a value span the line — e.g. a right-aligned total.
function fitWidth() {
  const e = el.value
  if (!e || e.type !== 'variable') return
  const scale = Math.max(1, e.style?.scale ?? 1) // guard a hand-authored scale of 0
  const len = Math.max(1, Math.floor(((props.contentCols ?? 1) - e.col) / scale))
  patch({ length: len })
}
// Toggle a QR/barcode between a literal and a variable source. Turning ON resets
// the value to a real variable, so a leftover literal isn't left dangling (and
// flagged UNAVAILABLE) with the picker showing nothing.
function toggleFromVariable(on: boolean) {
  if (!el.value) return
  const value = on ? (props.allVars?.[0]?.path ?? '') : el.value.value
  emit('update:element', { ...el.value, from_variable: on, value })
}
// Turn a static image back into a dynamic one (bytes from a variable). The
// reverse — file upload — happens in onReplaceFile.
function makeDynamic() {
  if (!el.value) return
  emit('update:element', { ...el.value, from_variable: true, data: props.allVars?.[0]?.path ?? '' })
}

const aligns: { v: Align; key: string }[] = [
  { v: 'left', key: 'alignLeft' },
  { v: 'center', key: 'alignCenter' },
  { v: 'right', key: 'alignRight' },
]
const valigns: { v: VAlign; key: string }[] = [
  { v: 'top', key: 'vTop' },
  { v: 'middle', key: 'vMid' },
  { v: 'bottom', key: 'vBottom' },
]
const sizes = [1, 2, 3, 4]

// Format mode is derived from which formatting field is present.
type FormatMode = 'text' | 'number' | 'date'
const formatMode = computed<FormatMode>(() => {
  if (el.value?.number) return 'number'
  if (el.value?.date_format) return 'date'
  return 'text'
})
function setFormat(mode: FormatMode) {
  if (!el.value) return
  if (mode === 'number') {
    const def: NumberFormat = { decimals: 2, rounding: 'half_up', thousands: true }
    emit('update:element', { ...el.value, number: def, date_format: undefined })
  } else if (mode === 'date') {
    emit('update:element', { ...el.value, date_format: 'DD/MM/YYYY', number: undefined })
  } else {
    emit('update:element', { ...el.value, number: undefined, date_format: undefined })
  }
}
function patchNumber(p: Partial<NumberFormat>) {
  if (!el.value?.number) return
  emit('update:element', { ...el.value, number: { ...el.value.number, ...p } })
}
// Only offer the formatting that matches the variable's declared type: numbers
// as numbers, dates as dates. Text variables get no format control at all.
const formatOptions = computed<{ v: FormatMode; key: string }[]>(() => {
  if (props.varType === 'number')
    return [
      { v: 'text', key: 'formatRaw' },
      { v: 'number', key: 'formatNumber' },
    ]
  if (props.varType === 'date')
    return [
      { v: 'text', key: 'formatRaw' },
      { v: 'date', key: 'formatDate' },
    ]
  return []
})
const roundings: { v: Rounding; key: string }[] = [
  { v: 'half_up', key: 'roundHalfUp' },
  { v: 'half_even', key: 'roundHalfEven' },
  { v: 'down', key: 'roundDown' },
  { v: 'up', key: 'roundUp' },
]
const typeLabels: Record<string, string> = {
  text: 'typeText',
  variable: 'typeVariable',
  image: 'typeImage',
  qr: 'typeQr',
  barcode: 'typeBarcode',
}
const datePresets = ['DD/MM/YYYY', 'YYYY-MM-DD', 'DD/MM/YYYY HH:mm', 'HH:mm:ss']

// --- "show only if" (element condition) -------------------------------------
const condTargets = computed(() => props.condVars ?? props.allVars ?? [])
function toggleCondition(on: boolean) {
  if (!el.value) return
  if (on) {
    const c: Condition = el.value.condition ?? {
      var: condTargets.value[0]?.path ?? '',
      op: 'is_set',
      value: '',
    }
    patch({ condition: c })
  } else {
    patch({ condition: undefined })
  }
}
</script>

<template>
  <div class="te-mod">
    <p v-if="!el" class="te-mod-empty">{{ t('selectPrompt') }}</p>
    <template v-else>
      <header class="te-mod-head">
        <span class="te-mod-type">{{ t(typeLabels[el.type] ?? el.type) }}</span>
        <button
          class="te-mod-del"
          type="button"
          :title="t('remove')"
          :aria-label="t('remove')"
          @click="emit('remove', el.id)"
        >
          🗑
        </button>
      </header>

      <p v-if="unavailable" class="te-mod-warn" role="alert">⚠ {{ t('unavailableTip') }}</p>

      <label v-if="el.type === 'text'" class="te-field">
        <span>{{ t('fieldText') }}</span>
        <input
          class="te-input"
          :value="el.content"
          @input="patch({ content: ($event.target as HTMLInputElement).value })"
        />
      </label>

      <!-- image -->
      <template v-else-if="el.type === 'image'">
        <!-- Source: dynamic (a variable) by default; uploading a file makes it a
             static embedded image. One concept, opposite default. -->
        <template v-if="el.from_variable">
          <label class="te-field">
            <span>{{ t('imageVariable') }}</span>
            <select
              class="te-input"
              :value="el.data"
              @change="patch({ data: ($event.target as HTMLSelectElement).value })"
            >
              <option value="" disabled>{{ t('imagePickVar') }}</option>
              <option v-for="v in allVars ?? []" :key="v.path" :value="v.path">{{ v.path }}</option>
            </select>
          </label>
          <button class="te-btn-replace" type="button" @click="replaceInput?.click()">
            {{ t('imageUseFile') }}
          </button>
        </template>
        <template v-else>
          <button class="te-btn-replace" type="button" @click="replaceInput?.click()">
            {{ t('replaceImage') }}
          </button>
          <button class="te-btn-replace" type="button" @click="makeDynamic()">
            {{ t('imageUseVariable') }}
          </button>
        </template>
        <input
          ref="replaceInput"
          type="file"
          accept="image/png,image/*"
          hidden
          @change="onReplaceFile"
        />

        <div class="te-field-row">
          <label class="te-half">
            <span>{{ t('widthCells') }}</span>
            <input
              class="te-input"
              type="number"
              min="1"
              max="200"
              :value="el.w"
              @input="patch({ w: Math.max(1, +($event.target as HTMLInputElement).value || 1) })"
            />
          </label>
          <label class="te-half">
            <span>{{ t('heightCells') }}</span>
            <input
              class="te-input"
              type="number"
              min="1"
              max="200"
              :value="el.h"
              @input="patch({ h: Math.max(1, +($event.target as HTMLInputElement).value || 1) })"
            />
          </label>
        </div>
        <div class="te-field">
          <span>{{ t('blackWhite') }}</span>
          <div class="te-seg">
            <button
              type="button"
              class="te-seg-btn"
              :class="{ active: el.mode?.kind !== 'dither' }"
              @click="
                patch({
                  mode: {
                    kind: 'threshold',
                    level: el.mode?.kind === 'threshold' ? el.mode.level : 128,
                  },
                })
              "
            >
              {{ t('threshold') }}
            </button>
            <button
              type="button"
              class="te-seg-btn"
              :class="{ active: el.mode?.kind === 'dither' }"
              @click="patch({ mode: { kind: 'dither' } })"
            >
              {{ t('dither') }}
            </button>
          </div>
        </div>
        <label v-if="el.mode?.kind !== 'dither'" class="te-field">
          <span
            >{{ t('thresholdLevel') }} ({{
              el.mode?.kind === 'threshold' ? el.mode.level : 128
            }})</span
          >
          <input
            type="range"
            min="0"
            max="255"
            :value="el.mode?.kind === 'threshold' ? el.mode.level : 128"
            @input="
              patch({
                mode: { kind: 'threshold', level: +($event.target as HTMLInputElement).value },
              })
            "
          />
        </label>
      </template>

      <!-- QR -->
      <template v-else-if="el.type === 'qr'">
        <label class="te-check">
          <input
            type="checkbox"
            :checked="el.from_variable"
            @change="toggleFromVariable(($event.target as HTMLInputElement).checked)"
          />
          <span>{{ t('fromVariable') }}</span>
        </label>
        <label v-if="el.from_variable" class="te-field">
          <span>{{ t('fieldVariable') }}</span>
          <select
            class="te-input"
            :value="el.value"
            @change="patch({ value: ($event.target as HTMLSelectElement).value })"
          >
            <option v-for="v in allVars ?? []" :key="v.path" :value="v.path">{{ v.path }}</option>
          </select>
        </label>
        <label v-else class="te-field">
          <span>{{ t('textUrl') }}</span>
          <input
            class="te-input"
            :value="el.value"
            @input="patch({ value: ($event.target as HTMLInputElement).value })"
          />
        </label>
        <label class="te-field">
          <span>{{ t('sizeCells') }}</span>
          <input
            class="te-input"
            type="number"
            min="4"
            max="80"
            :value="el.size"
            @input="patch({ size: Math.max(4, +($event.target as HTMLInputElement).value || 4) })"
          />
        </label>
      </template>

      <!-- barcode -->
      <template v-else-if="el.type === 'barcode'">
        <label class="te-check">
          <input
            type="checkbox"
            :checked="el.from_variable"
            @change="toggleFromVariable(($event.target as HTMLInputElement).checked)"
          />
          <span>{{ t('fromVariable') }}</span>
        </label>
        <label v-if="el.from_variable" class="te-field">
          <span>{{ t('fieldVariable') }}</span>
          <select
            class="te-input"
            :value="el.value"
            @change="patch({ value: ($event.target as HTMLSelectElement).value })"
          >
            <option v-for="v in allVars ?? []" :key="v.path" :value="v.path">{{ v.path }}</option>
          </select>
        </label>
        <label v-else class="te-field">
          <span>{{ t('barcodeValue') }}</span>
          <input
            class="te-input"
            :value="el.value"
            @input="patch({ value: ($event.target as HTMLInputElement).value })"
          />
        </label>
        <label class="te-field">
          <span>{{ t('symbology') }}</span>
          <select
            class="te-input"
            :value="el.symbology ?? 'code128'"
            @change="patch({ symbology: ($event.target as HTMLSelectElement).value as Symbology })"
          >
            <option value="code128">Code 128</option>
            <option value="code39">Code 39</option>
            <option value="ean13">EAN-13</option>
          </select>
        </label>
        <div class="te-field-row">
          <label class="te-half">
            <span>{{ t('widthCells') }}</span>
            <input
              class="te-input"
              type="number"
              min="6"
              max="200"
              :value="el.width"
              @input="
                patch({ width: Math.max(6, +($event.target as HTMLInputElement).value || 6) })
              "
            />
          </label>
          <label class="te-half">
            <span>{{ t('heightCells') }}</span>
            <input
              class="te-input"
              type="number"
              min="1"
              max="40"
              :value="el.height"
              @input="
                patch({ height: Math.max(1, +($event.target as HTMLInputElement).value || 1) })
              "
            />
          </label>
        </div>
      </template>

      <!-- variable -->
      <template v-else-if="el.type === 'variable'">
        <label class="te-field">
          <span>{{ t('fieldVariable') }}</span>
          <input class="te-input" :value="el.path" readonly />
        </label>
        <div class="te-field-row">
          <label class="te-half">
            <span>{{ t('widthChars') }}</span>
            <input
              class="te-input"
              type="number"
              min="1"
              max="200"
              :value="el.length"
              @input="
                patch({ length: Math.max(1, +($event.target as HTMLInputElement).value || 1) })
              "
            />
          </label>
          <label class="te-half te-check">
            <input
              type="checkbox"
              :checked="el.wrap"
              @change="patch({ wrap: ($event.target as HTMLInputElement).checked })"
            />
            <span>{{ t('wrap') }}</span>
          </label>
        </div>
        <label v-if="el.wrap" class="te-field">
          <span :title="t('maxLinesTip')">{{ t('maxLines') }}</span>
          <input
            class="te-input"
            type="number"
            min="0"
            max="50"
            :title="t('maxLinesTip')"
            :value="el.max_lines ?? 0"
            @input="
              patch({
                max_lines: +($event.target as HTMLInputElement).value || undefined,
              })
            "
          />
        </label>
        <button class="te-btn-replace" type="button" :title="t('fitToWidthTip')" @click="fitWidth">
          ⇥ {{ t('fitToWidth') }}
        </button>
        <div class="te-field">
          <span>{{ t('align') }}</span>
          <div class="te-seg">
            <button
              v-for="a in aligns"
              :key="a.v"
              type="button"
              class="te-seg-btn"
              :class="{ active: (el.align ?? 'left') === a.v }"
              @click="patch({ align: a.v })"
            >
              {{ t(a.key) }}
            </button>
          </div>
        </div>

        <!-- type-aware formatting: only what the variable's type allows -->
        <div v-if="formatOptions.length" class="te-field">
          <span>{{ t('format') }}</span>
          <div class="te-seg">
            <button
              v-for="o in formatOptions"
              :key="o.v"
              type="button"
              class="te-seg-btn"
              :class="{ active: formatMode === o.v }"
              @click="setFormat(o.v)"
            >
              {{ t(o.key) }}
            </button>
          </div>
        </div>

        <template v-if="formatMode === 'number' && el.number">
          <div class="te-field-row">
            <label class="te-half">
              <span>{{ t('decimals') }}</span>
              <input
                class="te-input"
                type="number"
                min="0"
                max="6"
                :value="el.number.decimals"
                @input="
                  patchNumber({
                    decimals: Math.max(
                      0,
                      Math.min(6, +($event.target as HTMLInputElement).value || 0),
                    ),
                  })
                "
              />
            </label>
            <label class="te-half te-check">
              <input
                type="checkbox"
                :checked="el.number.thousands"
                @change="patchNumber({ thousands: ($event.target as HTMLInputElement).checked })"
              />
              <span>{{ t('thousands') }}</span>
            </label>
          </div>
          <label class="te-field">
            <span>{{ t('rounding') }}</span>
            <select
              class="te-input"
              :value="el.number.rounding"
              @change="
                patchNumber({ rounding: ($event.target as HTMLSelectElement).value as Rounding })
              "
            >
              <option v-for="r in roundings" :key="r.v" :value="r.v">{{ t(r.key) }}</option>
            </select>
          </label>
        </template>

        <label v-else-if="formatMode === 'date'" class="te-field">
          <span>{{ t('datePattern') }}</span>
          <input
            class="te-input"
            list="te-date-presets"
            :value="el.date_format"
            @input="patch({ date_format: ($event.target as HTMLInputElement).value })"
          />
          <datalist id="te-date-presets">
            <option v-for="p in datePresets" :key="p" :value="p" />
          </datalist>
        </label>
      </template>

      <!-- size / vertical align / style apply to text and variables only -->
      <template v-if="el.type === 'text' || el.type === 'variable'">
        <label class="te-field">
          <span>{{ t('font') }}</span>
          <select
            class="te-input"
            :value="el.style?.font ?? 'mono'"
            @change="setFont(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="f in fontOptions" :key="f.id" :value="f.id">{{ f.label }}</option>
          </select>
        </label>
        <div class="te-field">
          <span>{{ t('size') }}</span>
          <div class="te-seg">
            <button
              v-for="s in sizes"
              :key="s"
              type="button"
              class="te-seg-btn"
              :class="{ active: (el.style?.scale ?? 1) === s }"
              @click="patchStyle({ scale: s })"
            >
              {{ s }}×
            </button>
          </div>
        </div>
        <div v-if="(el.style?.scale ?? 1) > 1" class="te-field">
          <span>{{ t('vAlign') }}</span>
          <div class="te-seg">
            <button
              v-for="va in valigns"
              :key="va.v"
              type="button"
              class="te-seg-btn"
              :class="{ active: (el.style?.valign ?? 'middle') === va.v }"
              @click="patchStyle({ valign: va.v })"
            >
              {{ t(va.key) }}
            </button>
          </div>
        </div>
        <div class="te-field">
          <span>{{ t('style') }}</span>
          <div class="te-seg">
            <button
              type="button"
              class="te-seg-btn"
              :class="{ active: el.style?.bold }"
              :aria-label="t('bold')"
              :aria-pressed="!!el.style?.bold"
              @click="patchStyle({ bold: !el.style?.bold })"
            >
              <b>B</b>
            </button>
            <button
              type="button"
              class="te-seg-btn"
              :class="{ active: el.style?.italic }"
              :aria-label="t('italic')"
              :aria-pressed="!!el.style?.italic"
              @click="patchStyle({ italic: !el.style?.italic })"
            >
              <i>I</i>
            </button>
          </div>
        </div>
      </template>

      <!-- position applies to everything -->
      <label class="te-field">
        <span>{{ t('nudge') }}</span>
        <input
          class="te-input"
          type="number"
          step="0.25"
          min="-4"
          max="4"
          :value="el.y_offset ?? 0"
          @input="patch({ y_offset: +($event.target as HTMLInputElement).value || 0 })"
        />
      </label>

      <div class="te-field-row">
        <label class="te-half">
          <span>{{ t('row') }}</span>
          <input
            class="te-input"
            type="number"
            min="0"
            :value="el.row"
            @input="patch({ row: Math.max(0, +($event.target as HTMLInputElement).value || 0) })"
          />
        </label>
        <label class="te-half">
          <span>{{ t('col') }}</span>
          <input
            class="te-input"
            type="number"
            min="0"
            :value="el.col"
            @input="patch({ col: Math.max(0, +($event.target as HTMLInputElement).value || 0) })"
          />
        </label>
      </div>

      <!-- show only if: hides the element in place. The "collapse row" action
           converts it into a one-row band so the whole line disappears. -->
      <label class="te-toggle">
        <input
          type="checkbox"
          :checked="!!el.condition"
          @change="toggleCondition(($event.target as HTMLInputElement).checked)"
        />
        <span>{{ t('showOnlyIfEl') }}</span>
      </label>
      <div v-if="el.condition" class="te-indent">
        <ConditionEditor
          :model-value="el.condition"
          :vars="condTargets"
          @update:model-value="patch({ condition: $event })"
        />
        <button
          v-if="!inBand"
          class="te-btn-replace te-collapse-row"
          type="button"
          :title="t('collapseRowTip')"
          @click="emit('collapse-row', el.id)"
        >
          {{ t('collapseRow') }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.te-mod {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  font-size: 0.85rem;
}
.te-mod-empty {
  color: var(--te-muted-fg);
  margin: 0;
}
.te-mod-warn {
  margin: 0;
  padding: 0.4rem 0.5rem;
  border-radius: calc(var(--te-radius) - 2px);
  font-size: 0.78rem;
  color: #dc2626;
  background: color-mix(in srgb, #dc2626 12%, transparent);
}
.te-mod-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.te-mod-type {
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-size: 0.7rem;
  color: var(--te-muted-fg);
}
.te-mod-del {
  border: 0;
  background: transparent;
  cursor: pointer;
  font-size: 0.9rem;
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
.te-field-row {
  display: flex;
  gap: 0.5rem;
}
.te-half {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  flex: 1;
}
.te-half > span {
  color: var(--te-muted-fg);
  font-size: 0.75rem;
}
.te-check {
  flex-direction: row;
  align-items: center;
  gap: 0.35rem;
  align-self: flex-end;
  padding-bottom: 0.4rem;
}
.te-check > span {
  color: inherit;
  font-size: 0.8rem;
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
.te-seg {
  display: flex;
  gap: 0.25rem;
}
.te-seg-btn {
  flex: 1;
  padding: 0.35rem 0.4rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  cursor: pointer;
  text-transform: capitalize;
  font-size: 0.8rem;
  white-space: nowrap;
}
.te-seg-btn.active {
  background: var(--te-primary);
  color: var(--te-primary-fg);
  border-color: var(--te-primary);
}
.te-btn-replace {
  padding: 0.35rem 0.6rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
  font-size: 0.8rem;
  cursor: pointer;
}
.te-btn-replace:hover {
  background: var(--te-accent);
}
.te-toggle {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.82rem;
  cursor: pointer;
}
.te-indent {
  padding-left: 1.3rem;
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}
.te-collapse-row {
  align-self: flex-start;
}
</style>
