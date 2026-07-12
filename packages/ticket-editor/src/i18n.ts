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
  addText: '+ Text',
  addImage: '+ Image',
  addQr: '+ QR',
  addBarcode: '+ Barcode',
  fitToWidth: 'Fit to width',
  fitToWidthTip: "Stretch this field's reserved width to the paper's right edge",
  save: 'Save',
  saving: 'Saving…',
  railVariables: 'Variables',
  searchVars: 'Search variables…',
  noVarMatches: 'No matches',
  railModifiers: 'Modifiers',
  railBand: 'Loop / condition',
  repeatableTip: 'Repeatable — usable in loops',
  // modifier panel
  selectPrompt: 'Select an element to edit its modifiers.',
  unavailable: 'UNAVAILABLE',
  unavailableTip: 'This variable is not in the current data. Remove it or pick another.',
  typeText: 'text',
  typeVariable: 'variable',
  typeImage: 'image',
  typeQr: 'qr',
  typeBarcode: 'barcode',
  barcodeValue: 'Value',
  symbology: 'Barcode type',
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
  font: 'Font',
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
  imageVariable: 'Image variable (dynamic)',
  imagePickVar: 'Pick a variable…',
  imageUseFile: 'Use a file instead…',
  imageUseVariable: '↺ Use a variable instead',
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
  // calculated values
  railCalculated: 'Calculated',
  calcAddNew: '+ New calculated value',
  calcEmpty:
    'None yet. Build a value from your variables — e.g. a maps link, a total, or a per-payment sum.',
  calcEdit: 'Edit',
  calcDelete: 'Delete',
  calcNewTitle: 'New calculated value',
  calcEditTitle: 'Edit calculated value',
  calcLead: 'Write a formula from your variables. Use it anywhere — as a field or in a QR.',
  calcName: 'Name',
  calcNamePlaceholder: 'e.g. cash_total',
  calcNameRequired: 'Give it a name.',
  calcNameInvalid: 'Use letters, numbers and _ only (must start with a letter).',
  calcNameTaken: 'That name is already used.',
  calcFormula: 'Formula',
  calcFormulaPlaceholder: 'e.g. sumif(sale.movements, payment == "CASH", qty)',
  calcInsertVar: '+ Insert variable…',
  calcInsertFn: '+ Insert function…',
  calcGroupValues: 'Values',
  calcGroupLists: 'Lists (for count/sum/…)',
  calcGroupRow: 'in each {list} row',
  calcSyntaxHint:
    'Text in "quotes". Compare with == != > <, combine with and / or. Inside an aggregate, use a row\'s short field name (e.g. payment) — see the picker.',
  calcError: 'Error',
  calcEngineError: 'Could not evaluate',
  calcPreview: 'Preview',
  calcPreviewEmpty: '(empty)',
  calcSave: 'Save',
  cancel: 'Cancel',
  // function docs (shown in the Insert-function picker)
  fnConcat: 'join text and values together',
  fnRound: 'round a number to N decimals',
  fnMin: 'the smallest of the values',
  fnMax: 'the largest of the values',
  fnAbs: 'drop the sign (absolute value)',
  fnCoalesce: 'the first value that isn’t empty',
  fnCount: 'how many rows are in a list',
  fnCountif: 'how many rows match a condition',
  fnSum: 'add a value across every row',
  fnSumif: 'add a value across rows that match',
  fnAvg: 'average a value across rows',
  fnAvgif: 'average across rows that match',
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
  // calculated columns (row-scoped, in the band drawer)
  bandCalcTitle: 'Calculated columns',
  bandCalcEmpty:
    'Compute a value per line — e.g. amount = volume × price — and place it as a column.',
  bandCalcAdd: '+ New calculated column',
  bandCalcPlace: 'Place on the ticket',
  rowCalcNewTitle: 'New calculated column',
  rowCalcEditTitle: 'Edit calculated column',
  rowCalcLead:
    'Write a formula from this row\u2019s fields. It is computed for every line of the loop.',
  rowCalcPreviewNote: 'Preview uses the first row of your sample data.',
  calcNameReserved: 'This name is built in (row.{name}) — pick another.',
  calcGroupThisRow: 'This row',
  calcGroupRowCalcs: 'Calculated columns (earlier ones)',
  calcGroupLineInfo: 'Line info',
  rowLineNumber: 'line number (1, 2, …)',
  rowLineIndex: 'line index (0, 1, …)',
  rowLineCount: 'how many lines',
  rowLineFirst: 'is the first line',
  rowLineLast: 'is the last line',
  // wrap bound
  maxLines: 'Max lines',
  maxLinesTip: 'Cut a very long value after N lines (shows … at the cut). 0 = no limit.',
  // element condition + collapse-row sugar
  showOnlyIfEl: '? Show only if (condition)',
  collapseRow: 'Also collapse the row when hidden',
  collapseRowTip:
    'Turns this condition into a one-row band: when hidden, the line disappears instead of leaving a gap.',
  // missing-fields badge
  missingFields: '{n} missing',
  missingFieldsTip: 'Fields not in your sample data — they will print EMPTY on a real ticket:',
}

const es: MessageTable = {
  title: 'Editor de tickets',
  width: 'Ancho',
  zoom: 'Zoom',
  addText: '+ Texto',
  addImage: '+ Imagen',
  addQr: '+ QR',
  addBarcode: '+ Código de barras',
  fitToWidth: 'Ajustar al ancho',
  fitToWidthTip: 'Extiende el ancho reservado de este campo hasta el borde derecho',
  save: 'Guardar',
  saving: 'Guardando…',
  railVariables: 'Variables',
  searchVars: 'Buscar variables…',
  noVarMatches: 'Sin coincidencias',
  railModifiers: 'Modificadores',
  railBand: 'Bucle / condición',
  repeatableTip: 'Repetible — usable en bucles',
  selectPrompt: 'Selecciona un elemento para editar sus modificadores.',
  unavailable: 'NO DISPONIBLE',
  unavailableTip: 'Esta variable no está en los datos actuales. Quítala o elige otra.',
  typeText: 'texto',
  typeVariable: 'variable',
  typeImage: 'imagen',
  typeQr: 'qr',
  typeBarcode: 'código de barras',
  barcodeValue: 'Valor',
  symbology: 'Tipo de código',
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
  font: 'Fuente',
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
  imageVariable: 'Variable de imagen (dinámica)',
  imagePickVar: 'Elige una variable…',
  imageUseFile: 'Usar un archivo…',
  imageUseVariable: '↺ Usar una variable',
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
  railCalculated: 'Calculadas',
  calcAddNew: '+ Nueva variable calculada',
  calcEmpty:
    'Ninguna aún. Crea un valor a partir de tus variables — p. ej. un enlace de mapa, un total o una suma por pago.',
  calcEdit: 'Editar',
  calcDelete: 'Eliminar',
  calcNewTitle: 'Nueva variable calculada',
  calcEditTitle: 'Editar variable calculada',
  calcLead: 'Escribe una fórmula con tus variables. Úsala donde quieras — como campo o en un QR.',
  calcName: 'Nombre',
  calcNamePlaceholder: 'p. ej. total_efectivo',
  calcNameRequired: 'Ponle un nombre.',
  calcNameInvalid: 'Usa solo letras, números y _ (debe empezar con letra).',
  calcNameTaken: 'Ese nombre ya está en uso.',
  calcFormula: 'Fórmula',
  calcFormulaPlaceholder: 'p. ej. sumif(sale.movements, payment == "CASH", qty)',
  calcInsertVar: '+ Insertar variable…',
  calcInsertFn: '+ Insertar función…',
  calcGroupValues: 'Valores',
  calcGroupLists: 'Listas (para count/sum/…)',
  calcGroupRow: 'en cada fila de {list}',
  calcSyntaxHint:
    'Texto entre "comillas". Compara con == != > <, combina con and / or. Dentro de un agregado, usa el nombre corto del campo de la fila (p. ej. payment) — mira el selector.',
  calcError: 'Error',
  calcEngineError: 'No se pudo evaluar',
  calcPreview: 'Vista previa',
  calcPreviewEmpty: '(vacío)',
  calcSave: 'Guardar',
  cancel: 'Cancelar',
  fnConcat: 'unir texto y valores',
  fnRound: 'redondear un número a N decimales',
  fnMin: 'el menor de los valores',
  fnMax: 'el mayor de los valores',
  fnAbs: 'quitar el signo (valor absoluto)',
  fnCoalesce: 'el primer valor que no esté vacío',
  fnCount: 'cuántas filas hay en una lista',
  fnCountif: 'cuántas filas cumplen una condición',
  fnSum: 'sumar un valor en todas las filas',
  fnSumif: 'sumar un valor en las filas que cumplen',
  fnAvg: 'promediar un valor en las filas',
  fnAvgif: 'promediar en las filas que cumplen',
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
  bandCalcTitle: 'Columnas calculadas',
  bandCalcEmpty:
    'Calcula un valor por línea — p. ej. importe = volumen × precio — y colócalo como columna.',
  bandCalcAdd: '+ Nueva columna calculada',
  bandCalcPlace: 'Colocar en el ticket',
  rowCalcNewTitle: 'Nueva columna calculada',
  rowCalcEditTitle: 'Editar columna calculada',
  rowCalcLead:
    'Escribe una fórmula con los campos de la fila. Se calcula para cada línea del bucle.',
  rowCalcPreviewNote: 'La vista previa usa la primera fila de tus datos de muestra.',
  calcNameReserved: 'Ese nombre es interno (row.{name}) — elige otro.',
  calcGroupThisRow: 'Esta fila',
  calcGroupRowCalcs: 'Columnas calculadas (anteriores)',
  calcGroupLineInfo: 'Info de línea',
  rowLineNumber: 'número de línea (1, 2, …)',
  rowLineIndex: 'índice de línea (0, 1, …)',
  rowLineCount: 'cuántas líneas hay',
  rowLineFirst: 'es la primera línea',
  rowLineLast: 'es la última línea',
  maxLines: 'Máx. líneas',
  maxLinesTip: 'Corta un valor muy largo tras N líneas (muestra … al corte). 0 = sin límite.',
  showOnlyIfEl: '? Mostrar solo si (condición)',
  collapseRow: 'Además, contraer la fila al ocultarse',
  collapseRowTip:
    'Convierte esta condición en una banda de una fila: al ocultarse, la línea desaparece en vez de dejar un hueco.',
  missingFields: '{n} faltan',
  missingFieldsTip:
    'Campos que no están en tus datos de muestra — se imprimirán VACÍOS en un ticket real:',
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
export function provideEditorI18n(
  getLocale: () => string | undefined,
  getMessages: () => Messages | undefined,
): TFn {
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
  return inject(I18N_KEY, (k: string) => k)
}
