/// <reference types="vite/client" />

// Vite resolves `?url` imports to a string URL at build time. vue-tsc needs to
// be told the shape explicitly for the wasm asset.
declare module '*.wasm?url' {
  const src: string
  export default src
}
