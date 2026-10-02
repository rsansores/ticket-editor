<script setup lang="ts">
// The default toolbar: one Add menu, the paper warning when it applies, print
// and save. Paper setup lives in the inspector (with nothing selected) and view
// controls float on the canvas, so this stays one short row. A host with its
// own buttons skips this and calls the same actions on the context.
import { onBeforeUnmount, ref, watch } from 'vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  t,
  dotWidth,
  dotWidthOk,
  addText,
  addImage,
  addQr,
  addBarcode,
  addMarker,
  selectElement,
  selectBand,
  printing,
  printError,
  print,
  canSave,
  saving,
  save,
} = useTicketEditorContext()

const addItems = [
  { key: 'addMenuText', icon: 'T', run: addText },
  { key: 'addMenuImage', icon: '▣', run: addImage },
  { key: 'addMenuQr', icon: '▦', run: addQr },
  { key: 'addMenuBarcode', icon: '▥', run: addBarcode },
  { key: 'addMenuMarker', icon: '✂', run: addMarker },
]

const menuOpen = ref(false)
const menuEl = ref<HTMLElement | null>(null)
function pick(run: () => void) {
  menuOpen.value = false
  run()
}
// Close on a press anywhere else, or Escape — a plain disclosure menu.
function onDocPointer(e: PointerEvent) {
  if (menuEl.value && !menuEl.value.contains(e.target as Node)) menuOpen.value = false
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') menuOpen.value = false
}
function unlisten() {
  document.removeEventListener('pointerdown', onDocPointer, true)
  document.removeEventListener('keydown', onKey)
}
watch(menuOpen, (open) => {
  if (!open) return unlisten()
  document.addEventListener('pointerdown', onDocPointer, true)
  document.addEventListener('keydown', onKey)
})
onBeforeUnmount(unlisten)
</script>

<template>
  <header class="te-part te-toolbar">
    <strong class="te-title">{{ t('title') }}</strong>
    <div ref="menuEl" class="te-add">
      <button
        class="te-btn te-btn-ghost"
        type="button"
        aria-haspopup="menu"
        :aria-expanded="menuOpen"
        @click="menuOpen = !menuOpen"
      >
        {{ t('add') }} <span class="te-caret" aria-hidden="true">▾</span>
      </button>
      <div v-if="menuOpen" class="te-menu" role="menu">
        <button
          v-for="it in addItems"
          :key="it.key"
          class="te-menu-item"
          type="button"
          role="menuitem"
          @click="pick(it.run)"
        >
          <span class="te-menu-ico" aria-hidden="true">{{ it.icon }}</span>
          {{ t(it.key) }}
        </button>
        <p class="te-menu-hint">{{ t('addMenuHint') }}</p>
      </div>
    </div>
    <!-- Clearing the selection shows the ticket settings, where this is fixed. -->
    <button
      v-if="!dotWidthOk"
      class="te-chip te-chip-warn"
      type="button"
      :title="t('dotWidthWarnTip', { px: dotWidth })"
      @click="(selectElement(null), selectBand(null))"
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
.te-add {
  position: relative;
}
.te-caret {
  margin-left: 0.15rem;
  opacity: 0.7;
}
.te-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 20;
  min-width: 12rem;
  padding: 0.25rem;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.12);
}
.te-menu-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 0.85rem;
  text-align: left;
  cursor: pointer;
}
.te-menu-item:hover,
.te-menu-item:focus-visible {
  background: var(--te-accent);
  outline: none;
}
.te-menu-ico {
  width: 1rem;
  text-align: center;
  color: var(--te-muted-fg);
}
.te-menu-hint {
  margin: 0.25rem 0 0;
  padding: 0.4rem 0.5rem 0.2rem;
  border-top: 1px solid var(--te-border);
  font-size: 0.72rem;
  line-height: 1.35;
  color: var(--te-muted-fg);
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
