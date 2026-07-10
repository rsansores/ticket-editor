// The editor's monospace font library. Fonts are loaded PER FAMILY, ON DEMAND:
// nothing here is statically imported, so a family's faces live in their own
// code-split chunk that the browser only downloads when a document uses that
// font. (The npm package can be large; what the end user downloads stays small.)
//
// The built-in "mono" family (DejaVu Sans Mono) is compiled into the wasm and is
// always available; it needs no entry here.
//
// To add a family: drop four subsetted .ttf faces (regular/bold/italic/
// bold-italic) under src/assets/fonts/<id>/ and add it to FAMILIES below. Only
// monospace fonts belong here — the layout is a fixed-width grid. See
// ../assets/fonts/README.md for provenance/licensing.

/** Family id of the built-in default (matches `DEFAULT_FAMILY` in ticket-core). */
export const BUILTIN_FONT = 'mono'

/** The four faces of a family, as raw TTF bytes ready for `register_font`. */
export interface FontFaceBytes {
  regular: Uint8Array
  bold: Uint8Array
  italic: Uint8Array
  boldItalic: Uint8Array
}

// Curated display order + human labels (grouped clean → typewriter → playful).
const FAMILIES: { id: string; label: string }[] = [
  { id: 'jetbrains-mono', label: 'JetBrains Mono' },
  { id: 'ibm-plex-mono', label: 'IBM Plex Mono' },
  { id: 'source-code-pro', label: 'Source Code Pro' },
  { id: 'fira-mono', label: 'Fira Mono' },
  { id: 'roboto-mono', label: 'Roboto Mono' },
  { id: 'inconsolata', label: 'Inconsolata' },
  { id: 'space-mono', label: 'Space Mono' },
  { id: 'b612-mono', label: 'B612 Mono' },
  { id: 'courier-prime', label: 'Courier Prime (typewriter)' },
  { id: 'cutive-mono', label: 'Cutive Mono (typewriter)' },
  { id: 'share-tech-mono', label: 'Share Tech Mono' },
  { id: 'nova-mono', label: 'Nova Mono' },
  { id: 'syne-mono', label: 'Syne Mono (quirky)' },
  { id: 'major-mono-display', label: 'Major Mono Display (quirky)' },
  { id: 'vt323', label: 'VT323 (retro terminal)' },
]

// LAZY glob: each entry is a `() => import('…')` that resolves to the face's URL.
// Because it is non-eager, Vite/the host bundler code-splits every face into its
// own chunk, fetched only when `loadFontBytes` runs for that family.
const faceLoaders = import.meta.glob('../assets/fonts/*/*.ttf', {
  query: '?url',
  import: 'default',
}) as Record<string, () => Promise<string>>

const FACES = ['regular', 'bold', 'italic', 'bold-italic'] as const

/**
 * Load a family's four faces as bytes, on demand. Triggers the per-face lazy
 * chunk imports, then fetches each URL. Returns null for an unknown family (the
 * caller lets the render surface `MissingFont`). A family that ships fewer
 * weights isn't handled here — all bundled families carry four faces.
 */
export async function loadFontBytes(id: string): Promise<FontFaceBytes | null> {
  const loaders = FACES.map((face) => faceLoaders[`../assets/fonts/${id}/${face}.ttf`])
  if (loaders.some((l) => !l)) return null
  const urls = await Promise.all(loaders.map((load) => load()))
  const [regular, bold, italic, boldItalic] = await Promise.all(
    urls.map((url) => fetch(url).then((r) => r.arrayBuffer()).then((b) => new Uint8Array(b))),
  )
  return { regular, bold, italic, boldItalic }
}

/** Options for a font picker: the built-in default first, then the library. */
export const FONT_OPTIONS: { id: string; label: string }[] = [
  { id: BUILTIN_FONT, label: 'DejaVu Sans Mono (default)' },
  ...FAMILIES.map((f) => ({ id: f.id, label: f.label })),
]
