// Wrap a rendered ticket in a PDF whose page IS the ticket.
//
// # Why a PDF and not the browser's print dialog
//
// Printing the raster as an HTML page seems simpler, and it does not work. Two
// things in the print dialog are not ours to control:
//
//   * **Headers and footers.** The page URL, the timestamp and the document
//     title are drawn by the browser, from a checkbox in its own dialog. No CSS
//     turns them off.
//   * **Margins.** Chromium's "Margins: Default" *overrides* `@page { margin: 0 }`,
//     which is what reserves the space those headers print into.
//
// So the best an HTML print can do is ask the user to set Margins → None and
// untick Headers and footers, every time. A PDF has neither problem: its page
// size is authoritative (`MediaBox`), and printing a PDF injects nothing.
//
// # Why it is written by hand
//
// A PDF containing one image is a small, boring file, and a library to produce
// it would outweigh it. The raster is already 1-bit — that is the whole point of
// this renderer — so it embeds as a `DeviceGray` image at 1 bit per component:
// no colour conversion, no compression needed, and an 80 mm ticket lands in a
// few tens of kilobytes.

/** PostScript points per millimetre. A point is 1/72 inch. */
const PT_PER_MM = 72 / 25.4

/** Below this luminance a pixel is ink. The raster is already black-on-white. */
const BLACK_THRESHOLD = 128

/** A rendered ticket, reduced to the 1-bit rows a PDF image wants. */
interface Bitmap {
  width: number
  height: number
  /** Packed rows, MSB first, each row padded to a byte. 1 = white, 0 = ink. */
  rows: Uint8Array
}

/**
 * Decode the PNG and pack it to 1 bit per pixel.
 *
 * `DeviceGray` at 1 bpc means 0 is black and 1 is white — the same convention
 * the renderer's own 1-bit encoder uses, so this is a repack, not a conversion.
 */
async function toBitmap(png: Uint8Array): Promise<Bitmap> {
  const blob = new Blob([png as BlobPart], { type: 'image/png' })
  const bitmap = await createImageBitmap(blob)
  const { width, height } = bitmap

  const canvas = new OffscreenCanvas(width, height)
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('could not read the ticket raster')
  // The ticket is opaque black-on-white; paint white first so any transparency
  // lands on paper rather than on black.
  ctx.fillStyle = '#fff'
  ctx.fillRect(0, 0, width, height)
  ctx.drawImage(bitmap, 0, 0)
  bitmap.close()

  const { data } = ctx.getImageData(0, 0, width, height)
  const rowBytes = Math.ceil(width / 8)
  const rows = new Uint8Array(rowBytes * height)

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4
      // Rec. 601 luma, matching the renderer.
      const luma = 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2]
      if (luma >= BLACK_THRESHOLD) {
        rows[y * rowBytes + (x >> 3)] |= 0x80 >> (x & 7) // set = white
      }
    }
  }
  return { width, height, rows }
}

/** Assemble the PDF. Offsets are byte-exact, so everything is built as bytes. */
function assemble(bmp: Bitmap, widthPt: number, heightPt: number): Blob {
  const enc = new TextEncoder()
  const parts: Uint8Array[] = []
  const offsets: number[] = []
  let length = 0

  const push = (chunk: Uint8Array | string) => {
    const bytes = typeof chunk === 'string' ? enc.encode(chunk) : chunk
    parts.push(bytes)
    length += bytes.length
  }
  /** Start object `n`, recording where it begins — the xref table needs that. */
  const obj = (body: string) => {
    offsets.push(length)
    push(`${offsets.length} 0 obj\n${body}\nendobj\n`)
  }

  const content = `q ${widthPt.toFixed(4)} 0 0 ${heightPt.toFixed(4)} 0 0 cm /Im0 Do Q\n`

  push('%PDF-1.4\n')
  obj('<< /Type /Catalog /Pages 2 0 R >>')
  obj('<< /Type /Pages /Kids [3 0 R] /Count 1 >>')
  obj(
    `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${widthPt.toFixed(4)} ${heightPt.toFixed(4)}] ` +
      `/Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>`,
  )

  // The image, object 4. Written in pieces because the data is binary.
  offsets.push(length)
  push(
    `4 0 obj\n<< /Type /XObject /Subtype /Image /Width ${bmp.width} /Height ${bmp.height} ` +
      `/ColorSpace /DeviceGray /BitsPerComponent 1 /Length ${bmp.rows.length} >>\nstream\n`,
  )
  push(bmp.rows)
  push('\nendstream\nendobj\n')

  obj(`<< /Length ${content.length} >>\nstream\n${content}endstream`)

  const xref = length
  const rows = offsets.map((o) => `${String(o).padStart(10, '0')} 00000 n \n`).join('')
  push(
    `xref\n0 ${offsets.length + 1}\n0000000000 65535 f \n${rows}` +
      `trailer\n<< /Size ${offsets.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`,
  )

  return new Blob(parts as BlobPart[], { type: 'application/pdf' })
}

/**
 * A PDF of the ticket, one page, exactly the ticket's physical size.
 *
 * `dotsPerMm` ties the raster to paper: thermal printers are 203 dpi, i.e. 8
 * dots per millimetre, so a 576-dot-wide raster is 72 mm across.
 */
export async function ticketPdf(png: Uint8Array, dotsPerMm: number): Promise<Blob> {
  const bmp = await toBitmap(png)
  return assemble(bmp, (bmp.width / dotsPerMm) * PT_PER_MM, (bmp.height / dotsPerMm) * PT_PER_MM)
}
