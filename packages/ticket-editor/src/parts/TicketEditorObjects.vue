<script setup lang="ts">
// Everything that can go on the ticket, in one place: static objects (text,
// image, QR, barcode, a printer action), the host's variables and the
// calculated values. Clicking one places it on a new line at the bottom.
import TicketEditorVariables from './TicketEditorVariables.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const { t, addText, addImage, addQr, addBarcode, addMarker } = useTicketEditorContext()

const statics = [
  { key: 'addMenuText', icon: 'T', run: addText },
  { key: 'addMenuImage', icon: '▣', run: addImage },
  { key: 'addMenuQr', icon: '▦', run: addQr },
  { key: 'addMenuBarcode', icon: '▥', run: addBarcode },
  { key: 'addMenuMarker', icon: '✂', run: addMarker },
]
</script>

<template>
  <div class="te-part te-objects">
    <h3 class="te-rail-title">{{ t('railStatic') }}</h3>
    <ul class="te-static">
      <li v-for="s in statics" :key="s.key">
        <button class="te-static-add" type="button" @click="s.run()">
          <span class="te-static-ico" aria-hidden="true">{{ s.icon }}</span>
          {{ t(s.key) }}
        </button>
      </li>
    </ul>
    <TicketEditorVariables class="te-objects-vars" />
  </div>
</template>

<style scoped>
.te-rail-title {
  margin: 0 0 0.5rem;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--te-muted-fg);
}
.te-static {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}
.te-static-add {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.3rem 0.4rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 0.85rem;
  text-align: left;
  cursor: pointer;
}
.te-static-add:hover {
  background: var(--te-accent);
}
.te-static-ico {
  width: 1rem;
  text-align: center;
  color: var(--te-muted-fg);
}
/* the same section rule the variables part draws above its calculated values */
.te-objects-vars {
  margin-top: 0.9rem;
  padding-top: 0.6rem;
  border-top: 1px solid var(--te-border);
}
</style>
