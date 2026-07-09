import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

// Two modes:
//   `vite`        -> dev server serving the standalone demo (index.html)
//   `vite build`  -> library build for embedding into a host Vue app
export default defineConfig(({ command }) => ({
  plugins: [vue()],
  build:
    command === 'build'
      ? {
          lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'TicketEditor',
            fileName: 'ticket-editor',
          },
          rollupOptions: {
            // Vue is provided by the host app — never bundle it.
            external: ['vue'],
            output: { globals: { vue: 'Vue' } },
          },
        }
      : undefined,
  // The .wasm file must be served as an asset in dev.
  assetsInclude: ['**/*.wasm'],
}))
