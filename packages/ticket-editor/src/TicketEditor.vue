<script setup lang="ts">
// The embeddable editor. Four zones: variable tree, structural grid editor, live
// 1:1 preview, properties. It owns the TicketDoc and hands it back through
// `onSave`. Placement is non-destructive — a manual "Fit to width" is the only
// thing that ever moves elements in bulk, and only when clicked.
//
// The layout follows the editor's own width (not the window's: a host sidebar
// eats space), in three tiers:
//   wide    — four zones side by side; the rails collapse inline.
//   medium  — canvas and preview; variables and properties become drawers over
//             them, the properties one opening on selection.
//   narrow  — a phone: the ticket as a list of lines (TicketEditorRows) with an
//             Edit / Preview switch, and bottom sheets instead of drawers.
//
// This is only the default layout: the state and every action live in
// `useTicketEditor()`, and each zone is a connected part a host can place in its
// own drawers instead (see `src/parts`).
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useTicketEditor } from './core/useTicketEditor'
import TicketEditorToolbar from './parts/TicketEditorToolbar.vue'
import TicketEditorObjects from './parts/TicketEditorObjects.vue'
import TicketEditorCanvas from './parts/TicketEditorCanvas.vue'
import TicketEditorRows from './parts/TicketEditorRows.vue'
import TicketEditorPreview from './parts/TicketEditorPreview.vue'
import TicketEditorInspector from './parts/TicketEditorInspector.vue'
import TicketEditorDialogs from './parts/TicketEditorDialogs.vue'
import BottomSheet from './components/BottomSheet.vue'
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

const { t, selected, selectedBand, selectElement, selectBand, onSelect, addTarget } =
  useTicketEditor({
    modelValue: () => props.modelValue,
    variables: () => props.variables,
    variableTypes: () => props.variableTypes,
    locale: () => props.locale,
    messages: () => props.messages,
    onUpdate: (doc) => emit('update:modelValue', doc),
    onSave: (doc) => props.onSave?.(doc),
    canSave: () => !!props.onSave,
  })

// Below WIDE the four zones don't fit beside each other; below MEDIUM the grid
// canvas itself stops being usable.
const WIDE_PX = 1180
const MEDIUM_PX = 720
type Tier = 'wide' | 'medium' | 'narrow'
const tier = ref<Tier>('wide')
const root = ref<HTMLElement | null>(null)
let ro: ResizeObserver | undefined
onMounted(() => {
  const measure = () => {
    const w = root.value?.clientWidth ?? 0
    tier.value = w >= WIDE_PX ? 'wide' : w >= MEDIUM_PX ? 'medium' : 'narrow'
  }
  measure()
  ro = new ResizeObserver(measure)
  if (root.value) ro.observe(root.value)
})
onBeforeUnmount(() => ro?.disconnect())

const leftOpen = ref(true)
const rightOpen = ref(true)
// Phone: which bottom sheet is up, and which half of the screen shows.
const sheet = ref<'add' | 'inspect' | null>(null)
const tab = ref<'edit' | 'preview'>('edit')

// Rails are columns when wide and drawers over the canvas otherwise, so they
// start open only when they don't cover anything.
watch(tier, (next) => {
  leftOpen.value = rightOpen.value = next === 'wide'
  sheet.value = null
  addTarget.value = null
})
// Phone: a line's "+ Object" sets the add target; the Objects picker opens for
// it. Picking something consumes the target and selects the new element, which
// swaps this sheet for its properties.
watch(addTarget, (row) => {
  if (row !== null && tier.value === 'narrow') sheet.value = 'add'
})
function closeAdd() {
  sheet.value = null
  addTarget.value = null
}

// Tablet drawers sit over the canvas, so a press anywhere else closes them —
// except inside a dialog they opened. Pressing an element still opens
// the properties drawer: the selection reopens it in the same tick.
const leftRail = ref<HTMLElement | null>(null)
const rightRail = ref<HTMLElement | null>(null)
function onOutsidePress(e: PointerEvent) {
  const target = e.target as Element | null
  if (!target || target.closest('.te-modal-backdrop')) return
  if (!leftRail.value?.contains(target)) leftOpen.value = false
  if (!rightRail.value?.contains(target)) rightOpen.value = false
}
watch(
  () => tier.value === 'medium' && (leftOpen.value || rightOpen.value),
  (listen) => {
    if (listen) document.addEventListener('pointerdown', onOutsidePress, true)
    else document.removeEventListener('pointerdown', onOutsidePress, true)
  },
)
onBeforeUnmount(() => document.removeEventListener('pointerdown', onOutsidePress, true))

// Selecting anything, or asking for the ticket settings, brings the inspector
// into view.
onSelect(() => {
  if (tier.value === 'narrow') sheet.value = 'inspect'
  else rightOpen.value = true
})

const inspectorTitle = computed(() =>
  selectedBand.value ? t('railBand') : selected.value ? t('railModifiers') : t('railTicket'),
)
function closeInspector() {
  sheet.value = null
  selectElement(null)
  selectBand(null)
}
</script>

<template>
  <div ref="root" class="te-root te-editor" :class="tier">
    <TicketEditorToolbar />

    <div v-if="tier === 'narrow'" class="te-narrow">
      <div class="te-narrow-bar">
        <div class="te-tabs" role="tablist">
          <button
            class="te-tab"
            type="button"
            role="tab"
            :aria-selected="tab === 'edit'"
            @click="tab = 'edit'"
          >
            {{ t('tabEdit') }}
          </button>
          <button
            class="te-tab"
            type="button"
            role="tab"
            :aria-selected="tab === 'preview'"
            @click="tab = 'preview'"
          >
            {{ t('tabPreview') }}
          </button>
        </div>
      </div>
      <div class="te-narrow-main">
        <TicketEditorRows v-if="tab === 'edit'" />
        <TicketEditorPreview v-else />
      </div>

      <BottomSheet
        v-if="sheet === 'add' && addTarget !== null"
        :title="t('addToLine')"
        @close="closeAdd"
      >
        <TicketEditorObjects />
      </BottomSheet>
      <BottomSheet v-if="sheet === 'inspect'" :title="inspectorTitle" @close="closeInspector">
        <TicketEditorInspector />
      </BottomSheet>
    </div>

    <div v-else class="te-body">
      <aside ref="leftRail" class="te-rail" :class="{ collapsed: !leftOpen }">
        <button
          class="te-rail-toggle"
          type="button"
          @click="leftOpen = !leftOpen"
          :aria-label="leftOpen ? t('collapse') : t('railObjects')"
          :aria-expanded="leftOpen"
          :title="leftOpen ? t('collapse') : t('railObjects')"
        >
          {{ leftOpen ? '‹' : '›' }}
        </button>
        <div v-if="leftOpen" class="te-rail-inner">
          <TicketEditorObjects />
        </div>
      </aside>

      <main class="te-center">
        <TicketEditorCanvas />
      </main>

      <section class="te-preview-col">
        <TicketEditorPreview />
      </section>

      <aside ref="rightRail" class="te-rail te-rail-right" :class="{ collapsed: !rightOpen }">
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
          <h3 class="te-rail-title">{{ inspectorTitle }}</h3>
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
  /* the parts size themselves against the editor, not the window */
  container: te-editor / inline-size;
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

/* medium: canvas + preview; the rails become drawers over them */
.medium .te-body {
  position: relative;
  grid-template-columns: minmax(0, 1fr) minmax(200px, 0.55fr);
}
.medium .te-rail {
  position: absolute;
  top: 0.6rem;
  bottom: 0.6rem;
  left: 0.6rem;
  z-index: 15;
}
.medium .te-rail-right {
  left: auto;
  right: 0.6rem;
}
.medium .te-rail-inner {
  width: 270px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.18);
}

/* narrow: the phone layout */
.te-narrow {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.te-narrow-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.5rem 0.6rem;
  border-bottom: 1px solid var(--te-border);
}
.te-tabs {
  flex: 1;
  display: flex;
  padding: 0.15rem;
  border-radius: calc(var(--te-radius) + 2px);
  background: var(--te-muted);
}
.te-tab {
  flex: 1;
  min-height: 2.4rem;
  padding: 0 0.85rem;
  border: 0;
  border-radius: var(--te-radius);
  background: transparent;
  color: var(--te-muted-fg);
  font: inherit;
  font-size: 0.85rem;
  cursor: pointer;
}
.te-tab[aria-selected='true'] {
  background: var(--te-card);
  color: inherit;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}
.te-narrow-main {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0.6rem;
}
</style>
