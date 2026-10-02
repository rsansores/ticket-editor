<script setup lang="ts">
// The default toolbar: the ticket's settings, the paper warning when it
// applies, print and save. Objects are added from the Objects panel, paper
// setup lives in the inspector and view controls float on the canvas, so this
// stays one short row. A host with its own buttons skips this and calls the
// same actions on the context.
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  t,
  dotWidth,
  dotWidthOk,
  showTicketSettings,
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
    <button
      class="te-btn te-btn-ghost te-btn-icon"
      type="button"
      :title="t('ticketSettingsTip')"
      @click="showTicketSettings"
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
        <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12" />
        <circle cx="16" cy="6" r="2" />
        <circle cx="10" cy="12" r="2" />
        <circle cx="18" cy="18" r="2" />
      </svg>
      {{ t('ticketSettings') }}
    </button>
    <!-- The ticket settings are where this is fixed. -->
    <button
      v-if="!dotWidthOk"
      class="te-chip te-chip-warn"
      type="button"
      :title="t('dotWidthWarnTip', { px: dotWidth })"
      @click="showTicketSettings"
    >
      {{ t('dotWidthWarn', { px: dotWidth }) }}
    </button>
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
