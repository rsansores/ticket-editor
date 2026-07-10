// The editor's monospace font library. Each family's four faces are separate
// TTF assets under src/assets/fonts/<id>/, auto-imported here as URLs (Vite emits
// them as hashed files). The bytes are only fetched when a document uses the
// family — see `ensureFontsLoaded` in the renderer composable — so the wasm
// bundle and initial load stay small.
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

/** A lazily-loadable font family: its id, label, and the four faces' asset URLs. */
export interface FontFamily {
  id: string
  label: string
  regular: string
  bold: string
  italic: string
  boldItalic: string
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

// All face URLs, keyed by "<id>/<face>", resolved at build time by Vite.
const urls = import.meta.glob('../assets/fonts/*/*.ttf', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>

function faceUrl(id: string, face: string): string | undefined {
  return urls[`../assets/fonts/${id}/${face}.ttf`]
}

/** Families the editor can fetch on demand (the built-in is separate). Only those
 *  whose asset files are actually present are included. */
export const FONT_LIBRARY: FontFamily[] = FAMILIES.flatMap((f) => {
  const regular = faceUrl(f.id, 'regular')
  if (!regular) return []
  return [
    {
      id: f.id,
      label: f.label,
      regular,
      bold: faceUrl(f.id, 'bold') ?? regular,
      italic: faceUrl(f.id, 'italic') ?? regular,
      boldItalic: faceUrl(f.id, 'bold-italic') ?? regular,
    },
  ]
})

/** Options for a font picker: the built-in default first, then the library. */
export const FONT_OPTIONS: { id: string; label: string }[] = [
  { id: BUILTIN_FONT, label: 'DejaVu Sans Mono (default)' },
  ...FONT_LIBRARY.map((f) => ({ id: f.id, label: f.label })),
]
