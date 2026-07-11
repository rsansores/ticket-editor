<script setup lang="ts">
// A plain-language condition builder: <variable> <operator> [value].
// Reused for conditional bands and per-element "show only if". Kept deliberately
// small so a non-programmer reads it at a glance.
import { computed } from 'vue'
import { useT } from '../i18n'
import type { CondOp, Condition } from '../types'

const t = useT()

const props = defineProps<{
  modelValue: Condition
  /** Selectable variable paths (leaves). */
  vars: { path: string; key: string }[]
}>()
const emit = defineEmits<{ 'update:modelValue': [c: Condition] }>()

const ops = computed<{ v: CondOp; label: string; needsValue: boolean }[]>(() => [
  { v: 'is_set', label: t('opIsSet'), needsValue: false },
  { v: 'is_empty', label: t('opIsEmpty'), needsValue: false },
  { v: 'eq', label: '=', needsValue: true },
  { v: 'ne', label: '≠', needsValue: true },
  { v: 'gt', label: '>', needsValue: true },
  { v: 'lt', label: '<', needsValue: true },
  { v: 'gte', label: '≥', needsValue: true },
  { v: 'lte', label: '≤', needsValue: true },
])
const needsValue = computed(
  () => ops.value.find((o) => o.v === props.modelValue.op)?.needsValue ?? false,
)

function patch(p: Partial<Condition>) {
  emit('update:modelValue', { ...props.modelValue, ...p })
}
function setOp(op: CondOp) {
  const needs = ops.value.find((o) => o.v === op)?.needsValue ?? false
  // Drop a now-meaningless operand when switching to is_set / is_empty.
  patch(needs ? { op } : { op, value: '' })
}
</script>

<template>
  <div class="te-cond">
    <select
      class="te-input"
      :value="modelValue.var"
      @change="patch({ var: ($event.target as HTMLSelectElement).value })"
    >
      <option v-for="v in vars" :key="v.path" :value="v.path">{{ v.path }}</option>
    </select>
    <div class="te-cond-row">
      <select
        class="te-input"
        :value="modelValue.op"
        @change="setOp(($event.target as HTMLSelectElement).value as CondOp)"
      >
        <option v-for="o in ops" :key="o.v" :value="o.v">{{ o.label }}</option>
      </select>
      <input
        v-if="needsValue"
        class="te-input"
        :value="modelValue.value ?? ''"
        :placeholder="t('condValue')"
        @input="patch({ value: ($event.target as HTMLInputElement).value })"
      />
    </div>
  </div>
</template>

<style scoped>
.te-cond {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.te-cond-row {
  display: flex;
  gap: 0.35rem;
}
.te-input {
  width: 100%;
  padding: 0.3rem 0.45rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
  font-size: 0.8rem;
}
.te-input:focus {
  outline: 2px solid var(--te-ring);
  outline-offset: -1px;
}
</style>
