// Public API of @ticket-editor/vue.
export { default as TicketEditor } from './TicketEditor.vue'
// Headless core + connected parts: a host with its own design system calls
// `useTicketEditor()` in its own layout component and places these parts in its
// own rails, drawers and sheets. `TicketEditor` above is one such layout.
export { useTicketEditor, useTicketEditorContext } from './core/useTicketEditor'
export type {
  TicketEditorOptions,
  TicketEditorContext,
  EditorSelection,
} from './core/useTicketEditor'
export { default as TicketEditorToolbar } from './parts/TicketEditorToolbar.vue'
export { default as TicketEditorObjects } from './parts/TicketEditorObjects.vue'
export { default as TicketEditorVariables } from './parts/TicketEditorVariables.vue'
export { default as TicketEditorCanvas } from './parts/TicketEditorCanvas.vue'
export { default as TicketEditorPreview } from './parts/TicketEditorPreview.vue'
export { default as TicketEditorInspector } from './parts/TicketEditorInspector.vue'
export { default as TicketEditorDialogs } from './parts/TicketEditorDialogs.vue'
// Presentational sub-components (props in, events out), for a host that wants
// to wire them itself.
export { default as ModifierPanel } from './components/ModifierPanel.vue'
export { default as PreviewPane } from './components/PreviewPane.vue'
export { default as VariableTree } from './components/VariableTree.vue'
export { default as ComputedEditor } from './components/ComputedEditor.vue'

// Renderer helpers — handy if a host wants to render a saved doc without the UI
// (e.g. a thumbnail), still through the exact same wasm engine.
export {
  renderPng,
  renderToUrl,
  rendererSchemaVersion,
  previewComputed,
} from './composables/useRenderer'
export { deriveTree, guessLength, randomizeSample, inferType, pathTypeMap } from './lib/tree'
export { footprint, overlappingIds } from './lib/layout'
// Thermal paper formats. A host creating a new TicketDoc needs these to land the
// grid on a real printer's dot width — see `lib/paper.ts` for why that matters.
export { PAPER_PRESETS, DEFAULT_PRESET, STANDARD_DOT_WIDTHS, presetForDotWidth } from './lib/paper'
export type { PaperPreset } from './lib/paper'
export type { Footprint } from './lib/layout'
export { builtinMessages } from './i18n'
export type { Messages, Locale } from './i18n'

export type {
  TicketDoc,
  Paper,
  Element,
  ElementKind,
  TextKind,
  VariableKind,
  Style,
  Align,
  VAlign,
  Rounding,
  NumberFormat,
  VariableType,
  CondOp,
  Condition,
  Region,
  VarNode,
  Computed,
  ComputedResult,
  Symbology,
} from './types'
export { SCHEMA_VERSION } from './types'
