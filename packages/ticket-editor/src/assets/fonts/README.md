# Bundled monospace fonts

Lazily-loaded font families offered by the editor (the built-in default, DejaVu
Sans Mono, lives in `ticket-core`). Each family ships four TTF faces
(regular / bold / italic / bold-italic), already subset to Latin so each face is
~40–60 KB and only fetched when a document uses the family.

| Family | License | Source |
|--------|---------|--------|
| JetBrains Mono | SIL OFL 1.1 | fontsource `@fontsource/jetbrains-mono` (Latin subset) |
| IBM Plex Mono  | SIL OFL 1.1 | fontsource `@fontsource/ibm-plex-mono` (Latin subset) |

The `.ttf` files were derived from fontsource's `.woff2` (Latin, weights 400/700,
normal/italic) via `wawoff2` decompression. To add a family, drop its four
subsetted `.ttf` faces here under `<id>/` and register it in `src/lib/fonts.ts`.
Only monospace fonts belong here — the layout is a fixed-width grid.
