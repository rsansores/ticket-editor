# Bundled monospace fonts

Lazily-loaded font families the editor offers (the built-in default, DejaVu Sans
Mono, lives in `ticket-core`). Each family ships four TTF faces
(regular / bold / italic / bold-italic) — single-weight families reuse the
regular face for the others — already subset to Latin so each face is ~20–75 KB
and only fetched when a document uses the family.

Registered in `src/lib/fonts.ts` (grouped clean → typewriter → playful):

- **Clean:** JetBrains Mono, IBM Plex Mono, Source Code Pro, Fira Mono,
  Roboto Mono, Inconsolata, Space Mono, B612 Mono
- **Typewriter:** Courier Prime, Cutive Mono
- **Playful / display:** Share Tech Mono, Nova Mono, Syne Mono,
  Major Mono Display, VT323 (retro pixel terminal)

## Licensing

All families are **SIL OFL 1.1** except **Roboto Mono** (**Apache-2.0**) — all
permissive and redistributable. Sourced from the corresponding `@fontsource/*`
packages (Latin subsets); the `.ttf` were decompressed from fontsource `.woff2`
via `wawoff2`.

To add a family: drop its four subsetted `.ttf` faces under `<id>/` and add an
entry to `FAMILIES` in `src/lib/fonts.ts`. Only monospace fonts belong here — the
layout is a fixed-width grid.
