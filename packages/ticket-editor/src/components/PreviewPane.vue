<script setup lang="ts">
// The truth pane. Renders the document through the wasm build of `ticket-core`
// — the exact renderer the backend uses — so what shows here is what prints.
// Debounced so dragging/typing doesn't thrash the renderer (the same backoff
// idea the spec wants for the server path, applied locally since we render in
// the browser).
import { onScopeDispose, ref, watch } from 'vue'
import { renderToUrl } from '../composables/useRenderer'
import { useT } from '../i18n'
import type { TicketDoc } from '../types'

const t = useT()

const props = defineProps<{ doc: TicketDoc; variables?: unknown; debounceMs?: number }>()

const url = ref<string>('')
const error = ref<string>('')
let timer: ReturnType<typeof setTimeout> | undefined
let lastUrl = ''
let generation = 0 // guards against out-of-order async completions
let disposed = false

function schedule() {
  clearTimeout(timer)
  timer = setTimeout(() => void run(), props.debounceMs ?? 120)
}

async function run() {
  const my = ++generation
  try {
    const next = await renderToUrl(props.doc, props.variables)
    // A newer render started (or we were unmounted) while this awaited — drop it.
    if (disposed || my !== generation) {
      URL.revokeObjectURL(next)
      return
    }
    if (lastUrl) URL.revokeObjectURL(lastUrl)
    lastUrl = next
    url.value = next
    error.value = ''
  } catch (e) {
    if (!disposed && my === generation) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }
}

watch(() => [props.doc, props.variables], schedule, { deep: true, immediate: true })
onScopeDispose(() => {
  disposed = true
  clearTimeout(timer)
  if (lastUrl) URL.revokeObjectURL(lastUrl)
})
</script>

<template>
  <div class="te-preview">
    <div class="te-preview-head">
      <span>{{ t('preview') }}</span>
      <div class="te-preview-actions">
        <slot name="actions" />
        <span class="te-preview-tag">{{ t('previewTag') }}</span>
      </div>
    </div>
    <div class="te-preview-body">
      <p v-if="error" class="te-preview-error">{{ error }}</p>
      <img v-else-if="url" :src="url" alt="ticket preview" class="te-preview-img" />
      <p v-else class="te-preview-loading">{{ t('rendering') }}</p>
    </div>
  </div>
</template>

<style scoped>
.te-preview {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.te-preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 0.25rem;
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--te-muted-fg);
}
.te-preview-actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.te-preview-tag {
  font-weight: 500;
  font-size: 0.7rem;
  padding: 0.1rem 0.4rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--te-primary) 14%, transparent);
  color: var(--te-primary);
}
.te-preview-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 1rem;
  background: var(--te-muted);
  border-radius: var(--te-radius);
}
.te-preview-img {
  /* The preview scales to fit its panel; use smooth resampling so a downscaled
     ticket doesn't alias. (Crisp 1:1 pixels aren't meaningful once scaled.) */
  image-rendering: auto;
  background: #fff;
  box-shadow: 0 1px 6px rgba(0, 0, 0, 0.18);
  max-width: 100%;
}
.te-preview-error {
  color: #b91c1c;
  font-family: ui-monospace, monospace;
  font-size: 0.8rem;
}
.te-preview-loading {
  color: var(--te-muted-fg);
}
</style>
