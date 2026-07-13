// Drive the real editor, click Print, and inspect the document it hands the
// print dialog. Stubs window.print so no dialog opens.
import { chromium } from 'playwright'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } })
const errors = []
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()) })
page.on('pageerror', (e) => errors.push(String(e)))

await page.goto('http://localhost:5199/', { waitUntil: 'networkidle' })
await page.waitForSelector('.te-preview-img', { timeout: 20000 })

// Neuter the print dialog before it can block, on the page AND on any iframe.
await page.addInitScript(() => {})
await page.evaluate(() => {
  window.__printed = 0
  // Patch print on any iframe as it's created.
  const obs = new MutationObserver((muts) => {
    for (const m of muts) for (const n of m.addedNodes) {
      if (n.tagName === 'IFRAME' && n.contentWindow) {
        n.contentWindow.print = () => { window.__printed++ }
      }
    }
  })
  obs.observe(document.body, { childList: true, subtree: true })
})

// Set a 32-char doc so the dot width is the standard 384 (58 mm paper).
const widthInput = page.locator('.te-toolbar input[type=number]').first()
await widthInput.fill('32')
await page.waitForTimeout(800)

await page.getByRole('button', { name: /Print/i }).click()
await page.waitForTimeout(2500)

const result = await page.evaluate(() => {
  const f = document.querySelector('iframe[aria-hidden="true"]')
  if (!f || !f.contentDocument) return { found: false }
  const d = f.contentDocument
  const img = d.querySelector('img')
  return {
    found: true,
    printed: window.__printed,
    css: d.querySelector('style')?.textContent?.replace(/\s+/g, ' ').trim(),
    imgSrc: img?.src?.slice(0, 20),
    imgComplete: img?.complete,
    naturalWidth: img?.naturalWidth,
  }
})

console.log('print frame found :', result.found)
console.log('print() called    :', result.printed)
console.log('img is a blob     :', result.imgSrc)
console.log('img decoded       :', result.imgComplete, '| natural width:', result.naturalWidth, 'px (dots)')
console.log('page CSS          :', result.css)
if (errors.length) console.log('CONSOLE ERRORS    :', errors)
await browser.close()
