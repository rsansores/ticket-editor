// Screenshot the running demo so visuals can be verified without a desktop.
// Usage: node scripts/shoot.mjs <url> <outfile>
import { chromium } from 'playwright'

const url = process.argv[2] ?? 'http://localhost:5199/'
const out = process.argv[3] ?? '/tmp/editor.png'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 2 })
const errors = []
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()) })
page.on('pageerror', (e) => errors.push(String(e)))

await page.goto(url, { waitUntil: 'networkidle' })
// Wait for the wasm preview image to actually render.
await page.waitForSelector('.te-preview-img', { timeout: 15000 }).catch(() => {})
await page.waitForTimeout(600)
await page.screenshot({ path: out })
await browser.close()

console.log('shot ->', out)
if (errors.length) {
  console.log('PAGE ERRORS:')
  for (const e of errors) console.log('  ', e)
} else {
  console.log('no console errors')
}
