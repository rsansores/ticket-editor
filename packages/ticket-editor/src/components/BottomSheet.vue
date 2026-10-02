<script setup lang="ts">
// A panel that slides up from the bottom edge — the phone layout's drawer.
// Backdrop tap and Escape close it.
import { onBeforeUnmount, onMounted } from 'vue'
import { useT } from '../i18n'

const t = useT()

defineProps<{ title?: string }>()
const emit = defineEmits<{ close: [] }>()

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close')
}
onMounted(() => document.addEventListener('keydown', onKey))
onBeforeUnmount(() => document.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="te-sheet-backdrop" @click.self="emit('close')">
    <section class="te-sheet" role="dialog" aria-modal="true" :aria-label="title">
      <header class="te-sheet-head">
        <h3 class="te-sheet-title">{{ title ?? '' }}</h3>
        <button class="te-sheet-x" type="button" :aria-label="t('close')" @click="emit('close')">
          ×
        </button>
      </header>
      <div class="te-sheet-body">
        <slot />
      </div>
    </section>
  </div>
</template>

<style scoped>
.te-sheet-backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: flex;
  align-items: flex-end;
  background: rgba(0, 0, 0, 0.35);
}
.te-sheet {
  width: 100%;
  max-height: 78vh;
  display: flex;
  flex-direction: column;
  background: var(--te-card);
  border-radius: calc(var(--te-radius) + 6px) calc(var(--te-radius) + 6px) 0 0;
  box-shadow: 0 -6px 24px rgba(0, 0, 0, 0.15);
  animation: te-sheet-in 0.18s ease-out;
}
@keyframes te-sheet-in {
  from {
    transform: translateY(24px);
    opacity: 0;
  }
}
.te-sheet-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.7rem 1rem 0.4rem;
}
.te-sheet-title {
  margin: 0;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--te-muted-fg);
}
.te-sheet-x {
  width: 2.2rem;
  height: 2.2rem;
  border: 0;
  border-radius: 999px;
  background: var(--te-muted);
  color: inherit;
  font-size: 1.2rem;
  line-height: 1;
  cursor: pointer;
}
.te-sheet-body {
  overflow: auto;
  padding: 0.4rem 1rem calc(1rem + env(safe-area-inset-bottom));
}
</style>
