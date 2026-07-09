// Lightweight i18n for the editor.
//
// The editor ships its own English + Spanish and, when the host app has a global
// vue-i18n instance, follows the host's locale automatically (no wiring). A host
// can override any string via the `messages` prop, or pin a `locale` prop. If no
// vue-i18n instance is present it falls back to the `locale` prop (default 'en').
//
// vue-i18n is a peer dependency — only its runtime `locale` ref is borrowed; the
// editor keeps its own message tables so hosts never have to translate its strings.

import { computed, inject, provide, type InjectionKey, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'

export type Locale = 'en' | 'es' | string
export type MessageTable = Record<string, string>
export type Messages = Record<Locale, MessageTable>

const en: MessageTable = {
  title: 'Ticket editor',
  width: 'Width',
  zoom: 'Zoom',
  addText: '＋ Text',
  addImage: '＋ Image',
  addQr: '＋ QR',
  fitToWidth: 'Fit to width',
  fitToWidthTip: 'Pull off-paper elements back inside the width',
  save: 'Save',
  saving: 'Saving…',
  railVariables: 'Variables',
  railModifiers: 'Modifiers',
  railBand: 'Loop / condition',
  repeatableTip: 'Repeatable — usable in loops',
  // modifier panel
  selectPrompt: 'Select an element to edit its modifiers.',
  typeText: 'text',
  typeVariable: 'variable',
  typeImage: 'image',
  typeQr: 'qr',
  remove: 'Remove',
  bold: 'Bold',
  italic: 'Italic',
  collapse: 'Collapse',
  fieldText: 'Text',
  fieldVariable: 'Variable',
  widthChars: 'Width (chars)',
  wrap: 'Wrap',
  align: 'Align',
  alignLeft: 'left',
  alignCenter: 'center',
  alignRight: 'right',
  format: 'Format',
  formatRaw: 'Raw',
  formatNumber: 'Number',
  formatDate: 'Date',
  decimals: 'Decimals',
  thousands: '1,000s',
  rounding: 'Rounding',
  roundHalfUp: 'Half up',
  roundHalfEven: 'Banker',
  roundDown: 'Truncate',
  roundUp: 'Ceil',
  datePattern: 'Date pattern',
  size: 'Size',
  vAlign: 'Vertical align',
  vTop: 'Top',
  vMid: 'Mid',
  vBottom: 'Bottom',
  style: 'Style',
  nudge: 'Nudge (rows) — fine vertical offset',
  row: 'Row',
  col: 'Col',
  widthCells: 'Width (cells)',
  heightCells: 'Height (cells)',
  blackWhite: 'Black & white',
  threshold: 'Threshold',
  dither: 'Dither',
  thresholdLevel: 'Threshold level',
  replaceImage: 'Replace image…',
  fromVariable: 'From a variable',
  textUrl: 'Text / URL',
  sizeCells: 'Size (cells)',
  // band panel
  band: 'Band',
  startsAtRow: 'Starts at row',
  spansRows: 'Spans (rows)',
  repeatLoop: '↻ Repeat these rows (loop)',
  forEach: 'for each',
  showOnlyIf: '? Show only if (condition)',
  bandHint: 'Turn on Repeat and/or Show-only-if above to give this band an effect.',
  removeBand: 'Remove band',
  // condition editor
  opIsSet: 'is set',
  opIsEmpty: 'is empty',
  condValue: 'value',
  // preview
  preview: 'Preview',
  previewTag: '1:1 with print',
  rendering: 'Rendering…',
  reshuffle: '⟳ sample',
  reshuffleTip: 'Randomize sample data',
  // gutter / bands
  addLine: 'Add a blank line above row {n}',
  removeLine: 'Remove empty row {n}',
  rowNotEmpty: 'Row not empty — clear it first',
  addLineEnd: 'Add a blank line at the end (e.g. space for a signature)',
  bandCreate: 'Add a loop / condition band here',
  bandConfigure: 'click to configure',
  bandForEach: '↻ for each {name}',
  bandIf: '? if {cond}',
}

const es: MessageTable = {
  title: 'Editor de tickets',
  width: 'Ancho',
  zoom: 'Zoom',
  addText: '＋ Texto',
  addImage: '＋ Imagen',
  addQr: '＋ QR',
  fitToWidth: 'Ajustar al ancho',
  fitToWidthTip: 'Traer los elementos fuera del papel de vuelta al ancho',
  save: 'Guardar',
  saving: 'Guardando…',
  railVariables: 'Variables',
  railModifiers: 'Modificadores',
  railBand: 'Bucle / condición',
  repeatableTip: 'Repetible — usable en bucles',
  selectPrompt: 'Selecciona un elemento para editar sus modificadores.',
  typeText: 'texto',
  typeVariable: 'variable',
  typeImage: 'imagen',
  typeQr: 'qr',
  remove: 'Eliminar',
  bold: 'Negrita',
  italic: 'Cursiva',
  collapse: 'Contraer',
  fieldText: 'Texto',
  fieldVariable: 'Variable',
  widthChars: 'Ancho (caracteres)',
  wrap: 'Ajustar',
  align: 'Alineación',
  alignLeft: 'izquierda',
  alignCenter: 'centro',
  alignRight: 'derecha',
  format: 'Formato',
  formatRaw: 'Sin formato',
  formatNumber: 'Número',
  formatDate: 'Fecha',
  decimals: 'Decimales',
  thousands: 'Miles',
  rounding: 'Redondeo',
  roundHalfUp: 'Mitad arriba',
  roundHalfEven: 'Bancario',
  roundDown: 'Truncar',
  roundUp: 'Techo',
  datePattern: 'Formato de fecha',
  size: 'Tamaño',
  vAlign: 'Alineación vertical',
  vTop: 'Arriba',
  vMid: 'Medio',
  vBottom: 'Abajo',
  style: 'Estilo',
  nudge: 'Desplazar (filas) — ajuste vertical fino',
  row: 'Fila',
  col: 'Col',
  widthCells: 'Ancho (celdas)',
  heightCells: 'Alto (celdas)',
  blackWhite: 'Blanco y negro',
  threshold: 'Umbral',
  dither: 'Difuminado',
  thresholdLevel: 'Nivel de umbral',
  replaceImage: 'Reemplazar imagen…',
  fromVariable: 'Desde una variable',
  textUrl: 'Texto / URL',
  sizeCells: 'Tamaño (celdas)',
  band: 'Banda',
  startsAtRow: 'Empieza en la fila',
  spansRows: 'Abarca (filas)',
  repeatLoop: '↻ Repetir estas filas (bucle)',
  forEach: 'por cada',
  showOnlyIf: '? Mostrar solo si (condición)',
  bandHint: 'Activa Repetir o Mostrar-solo-si arriba para que esta banda tenga efecto.',
  removeBand: 'Eliminar banda',
  opIsSet: 'existe',
  opIsEmpty: 'está vacío',
  condValue: 'valor',
  preview: 'Vista previa',
  previewTag: '1:1 con impresión',
  rendering: 'Renderizando…',
  reshuffle: '⟳ muestra',
  reshuffleTip: 'Aleatorizar datos de muestra',
  addLine: 'Agregar una línea en blanco sobre la fila {n}',
  removeLine: 'Eliminar la fila vacía {n}',
  rowNotEmpty: 'La fila no está vacía — vacíala primero',
  addLineEnd: 'Agregar una línea al final (p. ej. espacio para una firma)',
  bandCreate: 'Agregar una banda de bucle / condición aquí',
  bandConfigure: 'clic para configurar',
  bandForEach: '↻ por cada {name}',
  bandIf: '? si {cond}',
}

export const builtinMessages: Messages = { en, es }

export type TFn = (key: string, params?: Record<string, unknown>) => string
const I18N_KEY: InjectionKey<TFn> = Symbol('ticket-editor-i18n')

function interpolate(s: string, params?: Record<string, unknown>): string {
  if (!params) return s
  return s.replace(/\{(\w+)\}/g, (_, k) => (k in params ? String(params[k]) : `{${k}}`))
}

function mergeMessages(over?: Messages): Messages {
  if (!over) return builtinMessages
  const merged: Messages = { ...builtinMessages }
  for (const loc of Object.keys(over)) {
    merged[loc] = { ...(builtinMessages[loc] ?? {}), ...over[loc] }
  }
  return merged
}

/**
 * Set up translation for the editor tree. Call once from the root component's
 * setup. Returns the `t` function (also provided to descendants via `useT`).
 */
export function provideEditorI18n(getLocale: () => string | undefined, getMessages: () => Messages | undefined): TFn {
  // Borrow the host's vue-i18n locale ref if a global instance exists.
  let hostLocale: Ref<unknown> | null = null
  try {
    hostLocale = useI18n().locale as unknown as Ref<unknown>
  } catch {
    hostLocale = null
  }
  const active = computed(() => getLocale() ?? (hostLocale ? String(hostLocale.value) : 'en'))
  const messages = computed(() => mergeMessages(getMessages()))
  const t: TFn = (key, params) => {
    const table = messages.value
    const s = table[active.value]?.[key] ?? table.en?.[key] ?? key
    return interpolate(s, params)
  }
  provide(I18N_KEY, t)
  return t
}

/** Inject the editor's `t`. Identity fallback so components render even in isolation. */
export function useT(): TFn {
  return inject(
    I18N_KEY,
    (k: string) => k,
  )
}
