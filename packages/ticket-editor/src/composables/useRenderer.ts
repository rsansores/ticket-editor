// Bridge to the wasm renderer — the SAME `ticket-core` code the backend runs.
// Whatever this draws is byte-for-byte what the printer will produce.

import init, {
  render_png,
  schema_version,
  preview_computed,
  register_font,
  has_font,
} from '../wasm/ticket_wasm.js'
// In the built library Vite inlines this as a `data:` URL; in dev it's a real
// URL. See `wasmSource()` below for why we don't just hand it to `init`.
import wasmUrl from '../wasm/ticket_wasm_bg.wasm?url'
import { loadFontBytes } from '../lib/fonts'
import type { Computed, ComputedResult, TicketDoc } from '../types'

let ready: Promise<void> | null = null

/**
 * The wasm module source to hand `init`.
 *
 * Critically, when the wasm is inlined (the built library), we decode the
 * `data:` URL to bytes and instantiate *those* — we do NOT pass the URL and let
 * wasm-bindgen `fetch()` it. Passing a URL/fetch makes initialization depend on
 * the *host* app's bundler resolving and serving that wasm reference, and Vite's
 * dep pipeline mishandles a wasm URL inside a consumed dependency — the value
 * reaching `WebAssembly.instantiateStreaming` ends up `undefined`
 * ("compile … must be a Response"). Raw bytes go straight to
 * `WebAssembly.instantiate`, with no URL, fetch, or asset handling for any
 * bundler to break. In dev, `wasmUrl` is a normal URL and passes through.
 */
function wasmSource(): Uint8Array | string {
  if (typeof wasmUrl === 'string' && wasmUrl.startsWith('data:')) {
    const base64 = wasmUrl.slice(wasmUrl.indexOf(',') + 1)
    const binary = atob(base64)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
    return bytes
  }
  return wasmUrl
}

/** Initialize the wasm module once, lazily. Retries on failure. */
function ensureInit(): Promise<void> {
  if (!ready) {
    ready = init({ module_or_path: wasmSource() })
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
// In-flight loads, so concurrent renders of the same new font share one fetch.
const inflightFonts = new Map<string, Promise<void>>()

/** Fetch a family's four faces and register them with the renderer (once). */
function ensureFont(id: string): Promise<void> {
  if (id === 'mono' || loadedFonts.has(id) || has_font(id)) return Promise.resolve()
  const existing = inflightFonts.get(id)
  if (existing) return existing
  const load = loadFontBytes(id)
    .then((faces) => {
      if (!faces) return // unknown family → let the render surface `MissingFont`
      register_font(id, faces.regular, faces.bold, faces.italic, faces.boldItalic)
      loadedFonts.add(id)
    })
    .finally(() => inflightFonts.delete(id))
  inflightFonts.set(id, load)
  return load
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
export async function renderPng(doc: TicketDoc, variables?: unknown): Promise<Uint8Array> {
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
export async function renderToUrl(doc: TicketDoc, variables?: unknown): Promise<string> {
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
