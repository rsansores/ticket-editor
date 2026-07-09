// Public API of @ticket-editor/vue.
export { default as TicketEditor } from './TicketEditor.vue'
// Sub-components, exported so a host can compose its own layout (e.g. drop the
// modifier panel into its own drawer).
export { default as ModifierPanel } from './components/ModifierPanel.vue'
export { default as PreviewPane } from './components/PreviewPane.vue'
export { default as VariableTree } from './components/VariableTree.vue'

// Renderer helpers — handy if a host wants to render a saved doc without the UI
// (e.g. a thumbnail), still through the exact same wasm engine.
export { renderPng, renderToUrl, rendererSchemaVersion } from './composables/useRenderer'
export { deriveTree, guessLength, randomizeSample, inferType, pathTypeMap } from './lib/tree'
export { footprint, overlappingIds } from './lib/layout'
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
} from './types'
export { SCHEMA_VERSION } from './types'
