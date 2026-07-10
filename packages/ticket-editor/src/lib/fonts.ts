// The editor's monospace font library. Each family's four faces are separate
// TTF assets, imported as URLs (Vite emits them as hashed files). The bytes are
// only fetched when a document actually uses the family — see `ensureFontsLoaded`
// in the renderer composable — so the wasm bundle and initial load stay small.
//
// The built-in "mono" family (DejaVu Sans Mono) is compiled into the wasm and is
// always available; it needs no entry here.
//
// To add a family: drop four subsetted .ttf faces under src/assets/fonts/<id>/
// and add an entry below. Only monospace fonts belong here (the layout is a
// fixed-width grid).

import jbRegular from '../assets/fonts/jetbrains-mono/regular.ttf?url'
import jbBold from '../assets/fonts/jetbrains-mono/bold.ttf?url'
import jbItalic from '../assets/fonts/jetbrains-mono/italic.ttf?url'
import jbBoldItalic from '../assets/fonts/jetbrains-mono/bold-italic.ttf?url'
import plexRegular from '../assets/fonts/ibm-plex-mono/regular.ttf?url'
import plexBold from '../assets/fonts/ibm-plex-mono/bold.ttf?url'
import plexItalic from '../assets/fonts/ibm-plex-mono/italic.ttf?url'
import plexBoldItalic from '../assets/fonts/ibm-plex-mono/bold-italic.ttf?url'

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

/** Families the editor can fetch on demand (the built-in is separate). */
export const FONT_LIBRARY: FontFamily[] = [
  {
    id: 'jetbrains-mono',
    label: 'JetBrains Mono',
    regular: jbRegular,
    bold: jbBold,
    italic: jbItalic,
    boldItalic: jbBoldItalic,
  },
  {
    id: 'ibm-plex-mono',
    label: 'IBM Plex Mono',
    regular: plexRegular,
    bold: plexBold,
    italic: plexItalic,
    boldItalic: plexBoldItalic,
  },
]

/** Options for a font picker: the built-in default first, then the library. */
export const FONT_OPTIONS: { id: string; label: string }[] = [
  { id: BUILTIN_FONT, label: 'DejaVu Sans Mono (default)' },
  ...FONT_LIBRARY.map((f) => ({ id: f.id, label: f.label })),
]
