<script setup lang="ts">
// Right-drawer configuration for a flow band (loop / condition) — same place and
// pattern as configuring an element, so there's no new mental model. Selected by
// clicking the band's bar in the left lane.
import { computed } from 'vue'
import ConditionEditor from './ConditionEditor.vue'
import { useT } from '../i18n'
import type { Condition, Region } from '../types'

const t = useT()

const props = defineProps<{
  region: Region
  loopSources: { path: string; key: string }[]
  allVars: { path: string; key: string }[]
}>()
const emit = defineEmits<{ 'update:region': [r: Region]; remove: [id: string] }>()

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
</style>
