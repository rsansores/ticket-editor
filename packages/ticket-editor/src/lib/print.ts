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
// So the page pins the image to its true PHYSICAL width and forbids margins.
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

/** How long to keep the print frame alive after handing off to the dialog. */
const CLEANUP_DELAY_MS = 60_000

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
 */
export async function printRaster(png: Uint8Array, dotWidth: number): Promise<void> {
  const widthMm = paperWidthMm(dotWidth)
  const blob = new Blob([png as BlobPart], { type: 'image/png' })
  const url = URL.createObjectURL(blob)

  // An offscreen iframe, rather than a popup or navigating the editor away: no
  // popup blocker, and the host application's own styles cannot leak into the
  // page we are about to print.
  const frame = document.createElement('iframe')
  frame.setAttribute('aria-hidden', 'true')
  frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0'
  document.body.appendChild(frame)

  const cleanup = () => {
    URL.revokeObjectURL(url)
    frame.remove()
  }

  try {
    const win = frame.contentWindow
    const docu = frame.contentDocument
    if (!win || !docu) throw new Error('could not open a print frame')

    docu.open()
    docu.write(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <style>
      /* Continuous roll: fix the width, let the length run. Zero margin — the
         document already carries whatever margins it wants, in character cells. */
      @page { size: ${widthMm}mm auto; margin: 0; }
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
</html>`)
    docu.close()

    // Print before the image has decoded and you print a blank page.
    const img = docu.querySelector('img')
    if (img && !img.complete) {
      await new Promise<void>((resolve) => {
        img.addEventListener('load', () => resolve(), { once: true })
        img.addEventListener('error', () => resolve(), { once: true })
      })
    }

    win.focus()
    win.print()
  } catch (e) {
    cleanup()
    throw e
  }

  // `print()` is synchronous in some browsers and not in others, and none of
  // them tell us when the dialog closes. Tearing the frame down immediately
  // cancels the job in Safari and Firefox, so leave it and sweep up later.
  setTimeout(cleanup, CLEANUP_DELAY_MS)
}
