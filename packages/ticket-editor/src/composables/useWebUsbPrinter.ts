// Print a ticket to a real USB thermal printer, from the browser, over WebUSB.
//
// EXPERIMENTAL. The point of this is trust: a preview can be perfect and still
// not be believed, and the person designing a ticket is usually nowhere near the
// device that will print it. This lets them put a real ticket on real paper from
// the editor, using the same encoder the backend links natively — so what comes
// out of the printer is not "close to" the preview, it IS the preview.
//
// Two things are worth knowing before you rely on it.
//
// 1. WebUSB is Chrome/Edge only. Firefox and Safari have declined to implement
//    it and there is no polyfill — this is not a version to wait for. Gate the
//    UI on `isSupported` and say so plainly.
//
// 2. The browser can only talk to the printer if the OPERATING SYSTEM isn't
//    already holding it. Printers that expose a vendor-specific interface are
//    free for the taking. Printers that expose the USB printer class (0x07) get
//    claimed by the kernel — `usblp` on Linux, `usbprint.sys` on Windows — and
//    `claimInterface` then fails no matter what the browser supports. That is a
//    one-time setup per machine (a udev rule, or WinUSB via Zadig), not a bug in
//    this code, and `PrinterBusyError` exists to say exactly that instead of
//    surfacing a generic failure that sends someone hunting in the wrong place.

/** The OS (or another program) is holding the printer — see note 2 above. */
export class PrinterBusyError extends Error {
  constructor(cause?: unknown) {
    super('the operating system is holding this printer')
    this.name = 'PrinterBusyError'
    this.cause = cause
  }
}

/** The printer offers no bulk OUT endpoint we can write ESC/POS bytes to. */
export class NoBulkEndpointError extends Error {
  constructor() {
    super('no bulk OUT endpoint on this device')
    this.name = 'NoBulkEndpointError'
  }
}

/** USB base class 7 — "Printer". The other candidates are vendor-specific (0xFF). */
const USB_CLASS_PRINTER = 0x07
const USB_CLASS_VENDOR = 0xff

/** WebUSB is a Chrome/Edge affair. No polyfill exists; don't pretend otherwise. */
export function isSupported(): boolean {
  return typeof navigator !== 'undefined' && 'usb' in navigator
}

/**
 * A claimed interface and the endpoint to write bytes at.
 */
interface Claimed {
  device: USBDevice
  interfaceNumber: number
  endpointNumber: number
}

/**
 * Find an interface we can push bytes into: printer-class first, then
 * vendor-specific (which is what a lot of cheap ESC/POS units actually expose,
 * and — happily — the ones the OS does NOT claim).
 */
function findBulkOut(device: USBDevice): Omit<Claimed, 'device'> {
  const configuration = device.configuration ?? device.configurations[0]
  const candidates = [USB_CLASS_PRINTER, USB_CLASS_VENDOR]

  for (const wanted of candidates) {
    for (const iface of configuration?.interfaces ?? []) {
      for (const alt of iface.alternates) {
        if (alt.interfaceClass !== wanted) continue
        const out = alt.endpoints.find((e) => e.direction === 'out' && e.type === 'bulk')
        if (out) {
          return { interfaceNumber: iface.interfaceNumber, endpointNumber: out.endpointNumber }
        }
      }
    }
  }
  throw new NoBulkEndpointError()
}

/**
 * Ask the user to pick a USB device, then open and claim it.
 *
 * `requestDevice` MUST be called from a user gesture (a click) — Chrome refuses
 * otherwise. We accept all devices rather than filtering on the printer class:
 * filtering would hide exactly the vendor-class printers that work best here,
 * because they're the ones the OS doesn't claim.
 */
export async function connect(): Promise<Claimed> {
  if (!isSupported()) throw new Error('WebUSB is not available in this browser')

  const device = await navigator.usb.requestDevice({ filters: [] })
  await device.open()
  if (!device.configuration) await device.selectConfiguration(1)

  const { interfaceNumber, endpointNumber } = findBulkOut(device)

  try {
    await device.claimInterface(interfaceNumber)
  } catch (e) {
    // Almost always the kernel driver holding the printer class. Say which.
    await device.close().catch(() => {})
    throw new PrinterBusyError(e)
  }

  return { device, interfaceNumber, endpointNumber }
}

/**
 * Write the ESC/POS stream to the printer, in chunks.
 *
 * A ticket raster runs to tens of kilobytes and printers have small input
 * buffers; `transferOut` of one huge buffer is where cheap devices stall. Chunk
 * it and let each transfer be acknowledged.
 */
const CHUNK_BYTES = 4096

export async function write(claimed: Claimed, bytes: Uint8Array): Promise<void> {
  for (let offset = 0; offset < bytes.length; offset += CHUNK_BYTES) {
    const chunk = bytes.subarray(offset, offset + CHUNK_BYTES)
    const result = await claimed.device.transferOut(claimed.endpointNumber, chunk)
    if (result.status !== 'ok') {
      throw new Error(`printer rejected the data (${result.status})`)
    }
  }
}

/** Release the device so the OS (or the next print) can have it back. */
export async function disconnect(claimed: Claimed): Promise<void> {
  await claimed.device.releaseInterface(claimed.interfaceNumber).catch(() => {})
  await claimed.device.close().catch(() => {})
}

/**
 * The whole job: pick a printer, send the bytes, let it go.
 *
 * Deliberately does not cache the device between calls. A test print is rare and
 * a held-open handle is how you end up with a printer nothing else can use.
 */
export async function printBytes(bytes: Uint8Array): Promise<void> {
  const claimed = await connect()
  try {
    await write(claimed, bytes)
  } finally {
    await disconnect(claimed)
  }
}
