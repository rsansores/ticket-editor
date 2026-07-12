<script setup lang="ts">
// Right-drawer configuration for a flow band (loop / condition) — same place and
// pattern as configuring an element, so there's no new mental model. Selected by
// clicking the band's bar in the left lane. Loop bands also host their
// "calculated columns" (row-scoped formulas like importe = volume × price),
// managed here and edited in the same dialog as doc-level calculated values.
import { computed } from 'vue'
import ConditionEditor from './ConditionEditor.vue'
import TypeTag from './TypeTag.vue'
import { useT } from '../i18n'
import type { Computed, ComputedResult, Condition, Region } from '../types'

const t = useT()

const props = defineProps<{
  region: Region
  loopSources: { path: string; key: string }[]
  allVars: { path: string; key: string }[]
  /** Live first-row results for this band's calculated columns, keyed by name. */
  calcReports?: Record<string, ComputedResult>
}>()
const emit = defineEmits<{
  'update:region': [r: Region]
  remove: [id: string]
  /** Open the calculated-column dialog (null calc = new). */
  'edit-calc': [regionId: string, calc: Computed | null]
  'remove-calc': [regionId: string, name: string]
  /** Place `row.<name>` on the ticket as a variable element inside this band. */
  'place-calc': [regionId: string, calc: Computed]
}>()

const rowCalcs = computed<Computed[]>(() => props.region.computed ?? [])
function calcError(name: string): string | undefined {
  return props.calcReports?.[name]?.error ?? undefined
}
function calcValue(name: string): string {
  return props.calcReports?.[name]?.value ?? ''
}

const span = computed(() => Math.max(1, props.region.end_row - props.region.start_row))

function patch(p: Partial<Region>) {
  emit('update:region', { ...props.region, ...p })
}
function setStart(v: number) {
  const start = Math.max(0, v)
  patch({ start_row: start, end_row: start + span.value })
}
function setSpan(v: number) {
  patch({ end_row: props.region.start_row + Math.max(1, v) })
}
function setLoop(on: boolean) {
  if (on) patch({ source: props.region.source ?? props.loopSources[0]?.path ?? '' })
  else patch({ source: undefined })
}
function setCond(on: boolean) {
  if (on) {
    const c: Condition = props.region.condition ?? {
      var: props.allVars[0]?.path ?? '',
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
  <div class="te-band-cfg">
    <header class="te-bc-head">
      <span class="te-bc-type">{{ t('band') }}</span>
      <button
        class="te-bc-del"
        type="button"
        :title="t('removeBand')"
        :aria-label="t('removeBand')"
        @click="emit('remove', region.id)"
      >
        🗑
      </button>
    </header>

    <div class="te-field-row">
      <label class="te-half">
        <span>{{ t('startsAtRow') }}</span>
        <input
          class="te-input"
          type="number"
          min="0"
          :value="region.start_row"
          @input="setStart(+($event.target as HTMLInputElement).value || 0)"
        />
      </label>
      <label class="te-half">
        <span>{{ t('spansRows') }}</span>
        <input
          class="te-input"
          type="number"
          min="1"
          :value="span"
          @input="setSpan(+($event.target as HTMLInputElement).value || 1)"
        />
      </label>
    </div>

    <label class="te-toggle">
      <input
        type="checkbox"
        :checked="!!region.source"
        :disabled="!loopSources.length"
        @change="setLoop(($event.target as HTMLInputElement).checked)"
      />
      <span>{{ t('repeatLoop') }}</span>
    </label>
    <label v-if="region.source" class="te-field te-indent">
      <span>{{ t('forEach') }}</span>
      <select
        class="te-input"
        :value="region.source"
        @change="patch({ source: ($event.target as HTMLSelectElement).value })"
      >
        <option v-for="s in loopSources" :key="s.path" :value="s.path">{{ s.key }}</option>
      </select>
    </label>

    <label class="te-toggle">
      <input
        type="checkbox"
        :checked="!!region.condition"
        @change="setCond(($event.target as HTMLInputElement).checked)"
      />
      <span>{{ t('showOnlyIf') }}</span>
    </label>
    <div v-if="region.condition" class="te-indent">
      <ConditionEditor
        :model-value="region.condition"
        :vars="allVars"
        @update:model-value="patch({ condition: $event })"
      />
    </div>

    <p v-if="!region.source && !region.condition" class="te-bc-hint">
      {{ t('bandHint') }}
    </p>

    <!-- calculated columns: per-line formulas exposed as row.<name> -->
    <div v-if="region.source" class="te-bc-calc">
      <h4 class="te-bc-calc-title">{{ t('bandCalcTitle') }}</h4>
      <ul v-if="rowCalcs.length" class="te-bc-calc-list">
        <li v-for="c in rowCalcs" :key="c.name" class="te-bc-calc-item">
          <button
            class="te-bc-calc-add"
            type="button"
            :title="t('bandCalcPlace') + ' — ' + c.formula"
            @click="emit('place-calc', region.id, c)"
          >
            <span class="te-bc-calc-eq" aria-hidden="true">=</span>
            <span class="te-bc-calc-key">{{ c.name }}</span>
            <span v-if="calcError(c.name)" class="te-bc-calc-warn" :title="calcError(c.name)"
              >⚠</span
            >
            <template v-else>
              <code v-if="calcValue(c.name) !== ''" class="te-bc-calc-val">{{
                calcValue(c.name)
              }}</code>
              <TypeTag
                v-else
                class="te-bc-calc-tag"
                :type="calcReports?.[c.name]?.kind === 'number' ? 'number' : 'text'"
              />
            </template>
          </button>
          <button
            class="te-bc-calc-icon"
            type="button"
            :aria-label="t('calcEdit')"
            :title="t('calcEdit')"
            @click="emit('edit-calc', region.id, c)"
          >
            ✎
          </button>
          <button
            class="te-bc-calc-icon"
            type="button"
            :aria-label="t('calcDelete')"
            :title="t('calcDelete')"
            @click="emit('remove-calc', region.id, c.name)"
          >
            🗑
          </button>
        </li>
      </ul>
      <p v-else class="te-bc-calc-empty">{{ t('bandCalcEmpty') }}</p>
      <button class="te-bc-calc-new" type="button" @click="emit('edit-calc', region.id, null)">
        {{ t('bandCalcAdd') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.te-band-cfg {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  font-size: 0.85rem;
}
.te-bc-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.te-bc-type {
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-size: 0.7rem;
  color: var(--te-muted-fg);
}
.te-bc-del {
  border: 0;
  background: transparent;
  cursor: pointer;
  font-size: 0.9rem;
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
.te-field {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
.te-field > span {
  color: var(--te-muted-fg);
  font-size: 0.75rem;
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
.te-bc-hint {
  color: var(--te-muted-fg);
  font-size: 0.78rem;
  margin: 0;
}
/* calculated columns — visually rhymes with the left rail's Calculated section */
.te-bc-calc {
  margin-top: 0.4rem;
  padding-top: 0.6rem;
  border-top: 1px solid var(--te-border);
}
.te-bc-calc-title {
  margin: 0 0 0.45rem;
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--te-muted-fg);
  font-weight: 600;
}
.te-bc-calc-list {
  list-style: none;
  margin: 0 0 0.4rem;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}
.te-bc-calc-item {
  display: flex;
  align-items: center;
  gap: 0.15rem;
}
.te-bc-calc-add {
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
.te-bc-calc-add:hover {
  background: var(--te-accent);
}
.te-bc-calc-eq {
  color: var(--te-primary);
  font-family: ui-monospace, monospace;
  font-weight: 700;
  font-size: 0.8rem;
}
.te-bc-calc-key {
  font-weight: 500;
  font-size: 0.85rem;
  color: var(--te-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.te-bc-calc-val {
  margin-left: auto;
  font-family: ui-monospace, monospace;
  font-size: 0.72rem;
  color: var(--te-muted-fg);
  max-width: 6.5rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.te-bc-calc-tag {
  margin-left: auto;
}
.te-bc-calc-warn {
  margin-left: auto;
  color: #dc2626;
  font-size: 0.8rem;
}
.te-bc-calc-icon {
  border: 0;
  background: transparent;
  color: var(--te-muted-fg);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 0.15rem;
  flex: none;
}
.te-bc-calc-icon:hover {
  color: inherit;
}
.te-bc-calc-empty {
  margin: 0 0 0.4rem;
  color: var(--te-muted-fg);
  font-size: 0.74rem;
  line-height: 1.35;
}
.te-bc-calc-new {
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
.te-bc-calc-new:hover {
  background: var(--te-accent);
}
</style>
