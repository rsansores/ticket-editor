<script setup lang="ts">
// The 1:1 wasm preview with its missing-fields badge and sample reshuffle.
import PreviewPane from '../components/PreviewPane.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const { t, doc, previewData, missingPaths, reshuffle } = useTicketEditorContext()
</script>

<template>
  <PreviewPane class="te-part" :doc="doc" :variables="previewData">
    <template #actions>
      <span
        v-if="missingPaths.length"
        class="te-chip te-chip-warn"
        :title="t('missingFieldsTip') + '\n' + missingPaths.join('\n')"
      >
        ⚠ {{ t('missingFields', { n: missingPaths.length }) }}
      </span>
      <button class="te-chip" type="button" :title="t('reshuffleTip')" @click="reshuffle">
        {{ t('reshuffle') }}
      </button>
    </template>
  </PreviewPane>
</template>

<style scoped>
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
.te-chip-warn {
  color: #d97706;
  border-color: color-mix(in srgb, #d97706 55%, var(--te-input));
  background: color-mix(in srgb, #f59e0b 10%, transparent);
  cursor: help;
  white-space: pre-line;
}
</style>
