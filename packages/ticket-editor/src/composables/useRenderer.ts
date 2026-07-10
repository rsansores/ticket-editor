// Bridge to the wasm renderer — the SAME `ticket-core` code the backend runs.
// Whatever this draws is byte-for-byte what the printer will produce.

import init, {
  render_png,
  schema_version,
  preview_computed,
  register_font,
  has_font,
} from '../wasm/ticket_wasm.js'
// Vite resolves this to a URL; the .wasm ships as an asset.
import wasmUrl from '../wasm/ticket_wasm_bg.wasm?url'
import { FONT_LIBRARY } from '../lib/fonts'
import type { Computed, ComputedResult, TicketDoc } from '../types'

let ready: Promise<void> | null = null

/** Initialize the wasm module once, lazily. Retries on failure. */
function ensureInit(): Promise<void> {
  if (!ready) {
    ready = init({ module_or_path: wasmUrl })
      .then(() => undefined)
      .catch((e: unknown) => {
        // Don't cache a rejected init (e.g. transient network/CSP failure) —
        // clear it so the next call can retry instead of failing forever.
        ready = null
        throw e
      })
  }
  return ready
}

// Families already fetched + registered with the wasm renderer this session.
const loadedFonts = new Set<string>()

/** Fetch a family's four faces and register them with the renderer (once). */
async function ensureFont(id: string): Promise<void> {
  if (id === 'mono' || loadedFonts.has(id) || has_font(id)) return
  const fam = FONT_LIBRARY.find((f) => f.id === id)
  if (!fam) return // unknown family → let the render surface `MissingFont`
  const [regular, bold, italic, boldItalic] = await Promise.all(
    [fam.regular, fam.bold, fam.italic, fam.boldItalic].map((url) =>
      fetch(url).then((r) => r.arrayBuffer()).then((b) => new Uint8Array(b)),
    ),
  )
  register_font(id, regular, bold, italic, boldItalic)
  loadedFonts.add(id)
}

/** The font families a document references (doc default + per-element). */
function fontsUsed(doc: TicketDoc): string[] {
  const s = new Set<string>()
  if (doc.font) s.add(doc.font)
  for (const el of doc.elements) if (el.style?.font) s.add(el.style.font)
  return [...s]
}

/**
 * Ensure every non-built-in font a document uses is fetched and registered
 * before it renders — so the preview matches the print and the renderer never
 * hits `MissingFont` for a family the editor knows how to load.
 */
export async function ensureFontsLoaded(doc: TicketDoc): Promise<void> {
  await ensureInit()
  await Promise.all(fontsUsed(doc).map(ensureFont))
}

/**
 * Render a document to PNG bytes. `variables` may be omitted to get a preview
 * filled with deterministic fake data. Lazily loads any fonts the document uses.
 * @throws the renderer's error message (bad doc, image too large, missing font, …)
 */
export async function renderPng(
  doc: TicketDoc,
  variables?: unknown,
): Promise<Uint8Array> {
  await ensureInit()
  await ensureFontsLoaded(doc)
  const varsJson = variables == null ? '' : JSON.stringify(variables)
  return render_png(JSON.stringify(doc), varsJson)
}

/**
 * Render straight to an object URL suitable for an `<img src>`.
 *
 * **The caller owns the returned URL** and must `URL.revokeObjectURL(url)` when
 * done (e.g. before replacing it), or it leaks one blob per render.
 */
export async function renderToUrl(
  doc: TicketDoc,
  variables?: unknown,
): Promise<string> {
  const bytes = await renderPng(doc, variables)
  const blob = new Blob([bytes], { type: 'image/png' })
  return URL.createObjectURL(blob)
}

/** The schema version the wasm build understands. */
export async function rendererSchemaVersion(): Promise<number> {
  await ensureInit()
  return schema_version()
}

/**
 * Evaluate calculated variables against sample data through the SAME engine the
 * renderer uses — so the editor's live formula preview matches the printed
 * result. Returns one result per input (in order), each with its value, kind and
 * any parse/evaluation error.
 * @throws only if the JSON boundary itself is malformed.
 */
export async function previewComputed(
  computed: Computed[],
  variables?: unknown,
): Promise<ComputedResult[]> {
  await ensureInit()
  const varsJson = variables == null ? '' : JSON.stringify(variables)
  return JSON.parse(preview_computed(JSON.stringify(computed), varsJson)) as ComputedResult[]
}
