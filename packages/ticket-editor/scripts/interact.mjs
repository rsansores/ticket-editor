// Click an element in the editor and screenshot, to verify interactive UI
// (selection, modifier panel) without a desktop.
import { chromium } from 'playwright'

const url = process.argv[2] ?? 'http://localhost:5199/'
const out = process.argv[3] ?? '/tmp/interact.png'
const hasText = process.argv[4] ?? 'total'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 2 })
const errors = []
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()) })
page.on('pageerror', (e) => errors.push(String(e)))

await page.goto(url, { waitUntil: 'networkidle' })
await page.waitForSelector('.te-el', { timeout: 15000 })
await page.locator('.te-el', { hasText }).first().click()
await page.waitForTimeout(400)
await page.screenshot({ path: out })
await browser.close()
console.log('shot ->', out, errors.length ? `ERRORS: ${errors.join(' | ')}` : '(no console errors)')
