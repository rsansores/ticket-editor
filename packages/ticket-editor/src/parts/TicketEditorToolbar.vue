<script setup lang="ts">
// The default toolbar: paper, zoom, add-element buttons, print and save. A host
// with its own buttons skips this and calls the same actions on the context.
import { PAPER_PRESETS } from '../lib/paper'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  t,
  doc,
  zoom,
  paperId,
  selectPaper,
  setWidthChars,
  dotWidth,
  dotWidthOk,
  addText,
  addImage,
  addQr,
  addBarcode,
  addMarker,
  printing,
  printError,
  print,
  canSave,
  saving,
  save,
} = useTicketEditorContext()
</script>

<template>
  <header class="te-part te-toolbar">
    <strong class="te-title">{{ t('title') }}</strong>
    <label class="te-inline"
      >{{ t('paper') }}
      <select
        class="te-select"
        :value="paperId"
        :title="t('paperTip')"
        @change="selectPaper(($event.target as HTMLSelectElement).value)"
      >
        <option v-for="p in PAPER_PRESETS" :key="p.id" :value="p.id">
          {{ t('paperOption', { mm: p.paperMm, dots: p.dots }) }}
        </option>
        <option value="custom">{{ t('paperCustom') }}</option>
      </select>
    </label>
    <label class="te-inline"
      >{{ t('width') }}
      <input
        class="te-num"
        type="number"
        min="16"
        max="120"
        :value="doc.paper.width_chars"
        @input="setWidthChars(+($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="te-inline"
      >{{ t('zoom') }}
      <input type="range" min="0.8" max="2.2" step="0.1" v-model.number="zoom" />
      <span class="te-muted">{{ zoom.toFixed(1) }}×</span>
    </label>
    <button class="te-btn te-btn-ghost" type="button" @click="addText">{{ t('addText') }}</button>
    <button class="te-btn te-btn-ghost" type="button" @click="addImage">
      {{ t('addImage') }}
    </button>
    <button class="te-btn te-btn-ghost" type="button" @click="addQr">{{ t('addQr') }}</button>
    <button class="te-btn te-btn-ghost" type="button" @click="addBarcode">
      {{ t('addBarcode') }}
    </button>
    <button class="te-btn te-btn-ghost" type="button" @click="addMarker">
      {{ t('addMarker') }}
    </button>
    <span
      v-if="!dotWidthOk"
      class="te-chip te-chip-warn"
      :title="t('dotWidthWarnTip', { px: dotWidth })"
    >
      {{ t('dotWidthWarn', { px: dotWidth }) }}
    </span>
    <div class="te-spacer" />
    <span v-if="printError" class="te-chip te-chip-warn" :title="printError">{{ printError }}</span>
    <button
      class="te-btn te-btn-ghost te-btn-icon"
      type="button"
      :disabled="printing"
      :title="t('printHint')"
      @click="print"
    >
      <svg
        class="te-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M6 9V3h12v6" />
        <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2" />
        <rect x="6" y="14" width="12" height="7" rx="1" />
      </svg>
      {{ printing ? t('printing') : t('print') }}
    </button>
    <button
      v-if="canSave"
      class="te-btn te-btn-primary"
      type="button"
      :disabled="saving"
      @click="save"
    >
      {{ saving ? t('saving') : t('save') }}
    </button>
  </header>
</template>

<style scoped>
.te-toolbar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.5rem 0.8rem;
  border-bottom: 1px solid var(--te-border);
  flex-wrap: wrap;
}
.te-title {
  font-size: 0.95rem;
}
.te-spacer {
  flex: 1;
}
.te-inline {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.8rem;
  color: var(--te-muted-fg);
}
.te-muted {
  color: var(--te-muted-fg);
}
.te-select {
  /* Wide enough for "80 mm (576 dots)" plus room for the native chevron, which
     sat on top of the text when this reused .te-num (3.6rem, sized for a
     two-digit number box). */
  padding: 0.25rem 0.4rem;
  padding-right: 1.6rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
}

.te-btn-icon {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.te-icon {
  width: 1.05em;
  height: 1.05em;
  flex: none;
}

.te-num {
  width: 3.6rem;
  padding: 0.25rem 0.4rem;
  border: 1px solid var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: var(--te-card);
  color: inherit;
  font: inherit;
}
.te-btn {
  padding: 0.4rem 0.75rem;
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
  opacity: 0.6;
  cursor: default;
}
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
