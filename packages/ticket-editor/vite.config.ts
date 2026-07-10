import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

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
    plugins: [vue()],
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
