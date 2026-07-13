import { chromium } from 'playwright'
const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } })
const errors = []
page.on('pageerror', (e) => errors.push(String(e)))
await page.goto('http://localhost:5199/', { waitUntil: 'networkidle' })
await page.waitForSelector('.te-preview-img', { timeout: 20000 })

await page.evaluate(() => {
  window.__printed = 0
  new MutationObserver((muts) => {
    for (const m of muts) for (const n of m.addedNodes)
      if (n.tagName === 'IFRAME' && n.contentWindow) n.contentWindow.print = () => window.__printed++
  }).observe(document.body, { childList: true, subtree: true })
})

const sel = page.locator('.te-toolbar select').first()
const cols = page.locator('.te-toolbar input[type=number]').first()
const warn = page.locator('.te-chip-warn')

async function probe(label) {
  await page.waitForTimeout(700)
  await page.getByRole('button', { name: /Print/i }).click()
  await page.waitForTimeout(1800)
  const r = await page.evaluate(() => {
    const f = document.querySelector('iframe[aria-hidden="true"]')
    const d = f?.contentDocument
    const css = d?.querySelector('style')?.textContent ?? ''
    return {
      page: css.match(/@page \{ size: ([\d.]+mm) auto/)?.[1],
      imgW: css.match(/width: ([\d.]+mm);/)?.[1],
      dots: d?.querySelector('img')?.naturalWidth,
    }
  })
  const w = (await warn.count()) ? await warn.first().innerText() : 'none'
  console.log(`${label.padEnd(26)} preset=${await sel.inputValue()} cols=${await cols.inputValue()} dots=${r.dots} @page=${r.page} img=${r.imgW} warn=${w}`)
  await page.evaluate(() => document.querySelector('iframe[aria-hidden="true"]')?.remove())
}

await probe('default (on launch)')
await sel.selectOption('58'); await probe('after selecting 58 mm')
await sel.selectOption('80'); await probe('after selecting 80 mm')
await cols.fill('40');        await probe('manual 40 cols (custom)')
if (errors.length) console.log('PAGE ERRORS:', errors)
await browser.close()
