// Print a ticket through the browser's ordinary print dialog.
//
// The printer is whatever the operating system already knows about — a POS
// printer installed as a normal printer, which is what an OS printer driver is
// for. We hand the print dialog a page and get out of the way, so this works in
// every browser, needs no permissions, no drivers and no setup, and "Save as
// PDF" comes free because the print dialog already offers it.
//
// # Why this is not just `window.print()`
//
// The whole point of this renderer is that ONE IMAGE PIXEL IS ONE PRINTER DOT.
// Hand the raster to the page and let it lay out, and the browser scales it to
// fit — and a resampled 1-bit raster comes back blurred and speckled. That is
// worse than not printing at all: it makes people distrust a preview that was
// correct.
//
// So the page pins the image to its true PHYSICAL size and forbids margins.
// Thermal printers are 203 dpi, i.e. exactly 8 dots per millimetre, so a raster
// `dots` wide is `dots / 8` millimetres wide on paper. Give the browser that and
// there is nothing left for it to scale.
//
// # What this does and does not prove
//
// It goes through the OS driver, not through ESC/POS. So it shows the LAYOUT on
// real paper — spacing, fonts, sizes, the logo, where the total lands — which is
// the thing people actually doubt. It does not exercise cut markers, the drawer
// kick, or raster banding, because the driver rasterizes its own page. Nobody
// looks at a receipt and doubts the cut command; they doubt the layout.

/** Thermal printers are 203 dpi. That is 8 dots per millimetre, exactly. */
const DOTS_PER_MM = 8

/**
 * The raster's size in dots, straight out of the PNG header.
 *
 * A PNG always opens with an 8-byte signature and then the IHDR chunk, whose
 * first two fields are width and height as big-endian u32 — at offsets 16 and
 * 20. No decoding needed, and we need the numbers *before* the image loads,
 * because they go into the stylesheet.
 */
function pngSize(png: Uint8Array): { width: number; height: number } {
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength)
  return { width: view.getUint32(16), height: view.getUint32(20) }
}

/**
 * Fallback teardown, if `afterprint` never fires. It is well supported, but a
 * browser that skips it must not leak an iframe and a blob URL forever.
 */
const CLEANUP_FALLBACK_MS = 60_000

/** Teardowns for print frames still on the page, so a caller can dispose them. */
const liveFrames = new Set<() => void>()

/**
 * Tear down any print frame still open.
 *
 * A print frame lives on `document.body`, outside the Vue tree, so nothing
 * unmounts it for us. Call this from `onScopeDispose` in whatever component
 * triggered the print.
 */
export function cleanupPrintFrames(): void {
  for (const dispose of [...liveFrames]) dispose()
}

/**
 * The physical width of a raster on paper, in millimetres.
 *
 * `dotWidth` is `width_chars × cell_width_px` — the document's own dot width,
 * which is why it must match the printer's (384 for 58 mm paper, 576 for 80 mm).
 * The editor already warns when it doesn't.
 */
export function paperWidthMm(dotWidth: number): number {
  return dotWidth / DOTS_PER_MM
}

/**
 * Open the browser's print dialog for a rendered ticket.
 *
 * `png` is the raster; `dotWidth` is what it was rendered at. Resolves once the
 * dialog has been handed the page — not when anything has been printed, which
 * the browser will not tell us.
 *
 * # The page is sized in both dimensions, deliberately
 *
 * `size: <width> auto` looks like the obvious way to say "a roll: fix the width,
 * let the length run" — and it is **invalid CSS**. The property takes one or two
 * lengths, *or* the keyword `auto`, never a mixture. Browsers drop the whole
 * declaration and fall back to Letter/A4, which is how a receipt ends up centred
 * on a sheet of office paper.
 *
 * So we give it both, and take the height from the raster itself rather than
 * asking anyone for it: the ticket's length is whatever the renderer just drew.
 */
export async function printRaster(png: Uint8Array, dotWidth: number): Promise<void> {
  const widthMm = paperWidthMm(dotWidth)
  // The page must be exactly the ticket — see the `@page` note below.
  const heightMm = pngSize(png).height / DOTS_PER_MM
  const blob = new Blob([png as BlobPart], { type: 'image/png' })
  const url = URL.createObjectURL(blob)

  // An offscreen iframe, rather than a popup or navigating the editor away: no
  // popup blocker, and the host application's own styles cannot leak into the
  // page we are about to print.
  const frame = document.createElement('iframe')
  frame.setAttribute('aria-hidden', 'true')
  frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0'

  const dispose = () => {
    if (!liveFrames.has(dispose)) return // already torn down
    liveFrames.delete(dispose)
    URL.revokeObjectURL(url)
    frame.remove()
  }
  liveFrames.add(dispose)

  try {
    // `srcdoc` rather than document.open/write/close: same result, without a
    // legacy API, and the iframe's own `load` event then tells us when the
    // image inside has decoded — print before that and you print a blank page.
    frame.srcdoc = `<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <style>
      /* The page IS the ticket: both dimensions, in millimetres. See the note on
         printRaster for why a width with an 'auto' height is not an option.
         Zero margin — the document already carries whatever margins it wants,
         in character cells. */
      @page { size: ${widthMm}mm ${heightMm}mm; margin: 0; }
      html, body { margin: 0; padding: 0; background: #fff; }
      img {
        display: block;
        width: ${widthMm}mm;   /* true size: no scaling, so 1 pixel stays 1 dot */
        height: auto;
        /* If a driver does resample, snap to the nearest dot rather than
           smoothing — a blurred 1-bit raster reads as a broken ticket. */
        image-rendering: pixelated;
      }
    </style>
  </head>
  <body><img src="${url}" alt=""></body>
</html>`

    await new Promise<void>((resolve, reject) => {
      frame.addEventListener('load', () => resolve(), { once: true })
      frame.addEventListener('error', () => reject(new Error('print frame failed to load')), {
        once: true,
      })
      document.body.appendChild(frame)
    })

    const win = frame.contentWindow
    if (!win) throw new Error('could not open a print frame')

    // The browser tells us when the dialog is done. Tearing the frame down any
    // earlier cancels the job in Safari and Firefox.
    win.addEventListener('afterprint', dispose, { once: true })
    setTimeout(dispose, CLEANUP_FALLBACK_MS)

    win.focus()
    win.print()
  } catch (e) {
    dispose()
    throw e
  }
}
