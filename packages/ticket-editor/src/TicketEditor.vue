<script setup lang="ts">
// The embeddable editor. Four zones: variable tree, structural grid editor, live
// 1:1 preview, modifier panel. It owns the TicketDoc and hands it back through
// `onSave`. The rails collapse (inline, not a drawer, so nothing clashes with a
// host's own drawers), the canvas has a zoom control, and placement is
// non-destructive — a manual "Fit to width" is the only thing that ever moves
// elements in bulk, and only when clicked.
//
// This is only the default layout: the state and every action live in
// `useTicketEditor()`, and each zone is a connected part a host can place in its
// own drawers instead (see `src/parts`).
import { ref } from 'vue'
import { useTicketEditor } from './core/useTicketEditor'
import TicketEditorToolbar from './parts/TicketEditorToolbar.vue'
import TicketEditorVariables from './parts/TicketEditorVariables.vue'
import TicketEditorCanvas from './parts/TicketEditorCanvas.vue'
import TicketEditorPreview from './parts/TicketEditorPreview.vue'
import TicketEditorInspector from './parts/TicketEditorInspector.vue'
import TicketEditorDialogs from './parts/TicketEditorDialogs.vue'
import type { Messages } from './i18n'
import type { TicketDoc, VariableType } from './types'

const props = defineProps<{
  modelValue?: TicketDoc
  variables?: Record<string, unknown>
  /**
   * Authoritative variable types keyed by dotted path (e.g.
   * `{ 'sale.total': 'number', 'sale.items.0.date': 'date' }`).
   * The host declares these; anything not listed falls back to inference from
   * the sample data. Gates which Format options the editor offers.
   */
  variableTypes?: Record<string, VariableType>
  /** Force a UI locale (e.g. 'es'). If omitted, follows the host's vue-i18n locale. */
  locale?: string
  /** Override / extend built-in UI strings, keyed by locale. */
  messages?: Messages
  onSave?: (doc: TicketDoc) => void | Promise<void>
}>()
const emit = defineEmits<{ 'update:modelValue': [doc: TicketDoc] }>()

const { t, selected, selectedBand, onSelect } = useTicketEditor({
  modelValue: () => props.modelValue,
  variables: () => props.variables,
  variableTypes: () => props.variableTypes,
  locale: () => props.locale,
  messages: () => props.messages,
  onUpdate: (doc) => emit('update:modelValue', doc),
  onSave: (doc) => props.onSave?.(doc),
  canSave: () => !!props.onSave,
})

const leftOpen = ref(true)
const rightOpen = ref(true)
// Selecting anything brings its properties into view.
onSelect(() => {
  rightOpen.value = true
})
</script>

<template>
  <div class="te-root te-editor">
    <TicketEditorToolbar />

    <div class="te-body">
      <aside class="te-rail" :class="{ collapsed: !leftOpen }">
        <button
          class="te-rail-toggle"
          type="button"
          @click="leftOpen = !leftOpen"
          :aria-label="leftOpen ? t('collapse') : t('railVariables')"
          :aria-expanded="leftOpen"
          :title="leftOpen ? t('collapse') : t('railVariables')"
        >
          {{ leftOpen ? '‹' : '›' }}
        </button>
        <div v-if="leftOpen" class="te-rail-inner">
          <TicketEditorVariables />
        </div>
      </aside>

      <main class="te-center">
        <TicketEditorCanvas />
      </main>

      <section class="te-preview-col">
        <TicketEditorPreview />
      </section>

      <aside class="te-rail te-rail-right" :class="{ collapsed: !rightOpen }">
        <button
          class="te-rail-toggle right"
          type="button"
          @click="rightOpen = !rightOpen"
          :aria-label="rightOpen ? t('collapse') : t('railModifiers')"
          :aria-expanded="rightOpen"
          :title="rightOpen ? t('collapse') : t('railModifiers')"
        >
          {{ rightOpen ? '›' : '‹' }}
        </button>
        <div v-if="rightOpen" class="te-rail-inner">
          <h3 class="te-rail-title">
            {{ selectedBand ? t('railBand') : selected ? t('railModifiers') : t('railTicket') }}
          </h3>
          <TicketEditorInspector />
        </div>
      </aside>
    </div>

    <TicketEditorDialogs />
  </div>
</template>

<style scoped>
.te-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 460px;
}
.te-body {
  flex: 1;
  min-height: 0;
  display: grid;
  /* editor gets the lion's share; preview is a narrower panel (its image scales) */
  grid-template-columns: auto minmax(0, 1.7fr) minmax(240px, 0.8fr) auto;
  gap: 0.6rem;
  padding: 0.6rem;
}
.te-rail {
  position: relative;
  display: flex;
}
.te-rail-inner {
  width: 190px;
  overflow: auto;
  padding: 0.5rem;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
}
.te-rail.collapsed {
  width: 1.4rem;
}
.te-rail-toggle {
  align-self: flex-start;
  width: 1.4rem;
  height: 1.8rem;
  border: 1px solid var(--te-border);
  background: var(--te-card);
  color: var(--te-muted-fg);
  border-radius: calc(var(--te-radius) - 2px);
  cursor: pointer;
  font-size: 0.9rem;
  line-height: 1;
  flex: none;
}
.te-rail-title {
  margin: 0 0 0.5rem;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--te-muted-fg);
}
.te-center {
  display: flex;
  min-height: 0;
}
.te-preview-col {
  min-height: 0;
  border: 1px solid var(--te-border);
  border-radius: var(--te-radius);
  background: var(--te-card);
  padding: 0.4rem;
}
</style>
