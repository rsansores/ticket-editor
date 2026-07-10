<script setup lang="ts">
// A small Tableau-style type chip: Abc = text, # = number, date = date. Colour
// carries the meaning at a glance; the title spells it out for a11y.
import type { VariableType } from '../types'

defineProps<{ type: VariableType }>()
const labels: Record<VariableType, string> = { text: 'Abc', number: '#', date: 'date' }
</script>

<template>
  <span class="te-type" :class="`te-type-${type}`" :title="type">{{ labels[type] }}</span>
</template>

<style scoped>
.te-type {
  flex: none;
  font-size: 0.62rem;
  font-weight: 700;
  line-height: 1;
  padding: 0.14rem 0.3rem;
  border-radius: 3px;
  font-family: ui-monospace, monospace;
  letter-spacing: 0.02em;
}
.te-type-text {
  color: #64748b;
  background: color-mix(in srgb, #64748b 15%, transparent);
}
.te-type-number {
  color: #2563eb;
  background: color-mix(in srgb, #2563eb 15%, transparent);
}
.te-type-date {
  color: #0d9488;
  background: color-mix(in srgb, #0d9488 16%, transparent);
}
</style>
