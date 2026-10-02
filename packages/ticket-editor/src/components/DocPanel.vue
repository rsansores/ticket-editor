<script setup lang="ts">
// Whole-ticket settings, shown in the inspector when nothing is selected — so
// the inspector is never an empty "select something" box, and paper setup is
// out of the toolbar.
import { PAPER_PRESETS } from '../lib/paper'
import { useT } from '../i18n'

const t = useT()

defineProps<{
  /** Preset id matching the current dot width, or 'custom'. */
  paperId: string
  widthChars: number
  dotWidth: number
  dotWidthOk: boolean
}>()
const emit = defineEmits<{
  'select-paper': [id: string]
  'set-width': [chars: number]
}>()
</script>

<template>
  <div class="te-doc">
    <p class="te-doc-hint">{{ t('ticketHint') }}</p>
    <label class="te-field">
      <span>{{ t('paper') }}</span>
      <select
        class="te-input"
        :value="paperId"
        :title="t('paperTip')"
        @change="emit('select-paper', ($event.target as HTMLSelectElement).value)"
      >
        <option v-for="p in PAPER_PRESETS" :key="p.id" :value="p.id">
          {{ t('paperOption', { mm: p.paperMm, dots: p.dots }) }}
        </option>
        <option value="custom">{{ t('paperCustom') }}</option>
      </select>
    </label>
    <label class="te-field">
      <span>{{ t('width') }}</span>
      <input
        class="te-input"
        type="number"
        min="16"
        max="120"
        :value="widthChars"
        @input="emit('set-width', +($event.target as HTMLInputElement).value)"
      />
    </label>
    <p v-if="!dotWidthOk" class="te-doc-warn" role="alert">
      {{ t('dotWidthWarn', { px: dotWidth }) }} — {{ t('dotWidthWarnTip', { px: dotWidth }) }}
    </p>
  </div>
</template>

<style scoped>
.te-doc {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  font-size: 0.85rem;
}
.te-doc-hint {
  margin: 0;
  color: var(--te-muted-fg);
  font-size: 0.78rem;
  line-height: 1.4;
}
.te-doc-warn {
  margin: 0;
  padding: 0.4rem 0.5rem;
  border-radius: calc(var(--te-radius) - 2px);
  font-size: 0.75rem;
  line-height: 1.4;
  color: #b45309;
  background: color-mix(in srgb, #f59e0b 12%, transparent);
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
</style>
