import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

// Vite's library mode inlines assets referenced via `new URL(..., import.meta.url)`
// as `data:` URLs regardless of `assetsInlineLimit` — and a 1.9 MB inlined wasm
// breaks in a host app's bundler ("WebAssembly.compile must be a Response") and
// bloats the entry. This plugin undoes that for the ES build: it extracts the
// inlined wasm back to a real `ticket_wasm_bg.wasm` file and rewrites the
// reference to a normal `new URL('./ticket_wasm_bg.wasm', import.meta.url)`, so a
// host serves + fetches it the same way the dev demo does.
function emitWasmAsFile(): Plugin {
  const WASM = 'ticket_wasm_bg.wasm'
  // Match the inlined wasm data-URL string literal in either form the bundle
  // produces it: a bare `"data:..."` (the `?url` import) or inside a
  // `new URL("data:...")` (the wasm-bindgen default). Replacing the string with a
  // `new URL('./file', import.meta.url)` works for both — `new URL(anotherUrl)`
  // just copies it — and yields a normal fetchable file reference.
  const dataUrl = /"data:application\/wasm;base64,([A-Za-z0-9+/=]+)"/g
  return {
    name: 'emit-wasm-as-file',
    apply: 'build',
    generateBundle(_opts, bundle) {
      let bytes: Buffer | null = null
      for (const chunk of Object.values(bundle)) {
        // ESM only: the CJS build cannot use import.meta.url anyway.
        if (chunk.type !== 'chunk' || !chunk.fileName.endsWith('.js')) continue
        if (!chunk.code.includes('data:application/wasm')) continue
        chunk.code = chunk.code.replace(dataUrl, (_m, b64) => {
          bytes ??= Buffer.from(b64, 'base64')
          return `new URL(${JSON.stringify('./' + WASM)}, import.meta.url)`
        })
      }
      if (bytes) this.emitFile({ type: 'asset', fileName: WASM, source: bytes })
    },
  }
}

// Three modes:
//   `vite`                 -> dev server serving the standalone demo (index.html)
//   `vite build`           -> library build for embedding into a host Vue app
//   `BUILD_DEMO=1 vite build` -> static demo SPA for GitHub Pages (dist-demo/)
//
// The demo is 100% client-side (the renderer runs in the browser via wasm), so
// it deploys as plain static files.
const isDemo = process.env.BUILD_DEMO === '1'

export default defineConfig(({ command }) => {
  if (isDemo) {
    return {
      plugins: [vue()],
      // Served from https://<user>.github.io/ticket-editor/ — set the base path.
      base: process.env.DEMO_BASE ?? '/ticket-editor/',
      build: { outDir: 'dist-demo', emptyOutDir: true },
      assetsInclude: ['**/*.wasm'],
    }
  }

  return {
    plugins: [vue(), emitWasmAsFile()],
    build:
      command === 'build'
        ? {
            lib: {
              entry: resolve(__dirname, 'src/index.ts'),
              // ES + CJS (NOT UMD): UMD forces a single inlined file, which would
              // bake every font into the entry. ES/CJS preserve code-split chunks
              // so each font (and the wasm) is a separate, lazily-fetched file.
              formats: ['es', 'cjs'],
              fileName: 'ticket-editor',
            },
            rollupOptions: {
              // Vue is provided by the host app — never bundle it.
              external: ['vue', 'vue-i18n'],
            },
          }
        : undefined,
    // The .wasm file must be served as an asset in dev.
    assetsInclude: ['**/*.wasm'],
  }
})
