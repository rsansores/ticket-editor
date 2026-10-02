<script setup lang="ts">
// A host-owned layout, the way an app with its own design system embeds the
// editor: its own toolbar buttons, and the objects and inspector in its own
// drawers, teleported to <body> outside any `.te-root`. Served at ?layout=custom.
import { ref } from 'vue'
import {
  TicketEditorCanvas,
  TicketEditorDialogs,
  TicketEditorInspector,
  TicketEditorPreview,
  TicketEditorObjects,
  useTicketEditor,
} from '../src'
import type { TicketDoc, VariableType } from '../src'

const props = defineProps<{
  variables: Record<string, unknown>
  variableTypes: Record<string, VariableType>
}>()
const doc = defineModel<TicketDoc>({ required: true })

const editor = useTicketEditor({
  modelValue: doc,
  variables: () => props.variables,
  variableTypes: () => props.variableTypes,
  onUpdate: (d) => (doc.value = d),
})

const leftOpen = ref(false)
const rightOpen = ref(false)
// The host decides what a selection does: here, open the right drawer.
editor.onSelect(() => {
  rightOpen.value = true
})
</script>

<template>
  <div class="host">
    <header class="host-bar">
      <button @click="leftOpen = !leftOpen">Objects</button>
      <button @click="editor.showTicketSettings">Ticket</button>
      <span class="host-grow" />
      <button @click="editor.print">Print</button>
    </header>
    <div class="host-body">
      <TicketEditorCanvas />
      <div class="host-preview"><TicketEditorPreview /></div>
    </div>
    <TicketEditorDialogs />

    <Teleport to="body">
      <aside v-if="leftOpen" class="host-drawer left">
        <button class="host-x" @click="leftOpen = false">×</button>
        <TicketEditorObjects />
      </aside>
      <aside v-if="rightOpen" class="host-drawer right">
        <button class="host-x" @click="rightOpen = false">×</button>
        <TicketEditorInspector />
      </aside>
    </Teleport>
  </div>
</template>

<style scoped>
.host {
  height: 100%;
  display: flex;
  flex-direction: column;
  font: 14px sans-serif;
}
.host-bar {
  display: flex;
  gap: 8px;
  padding: 8px;
  border-bottom: 1px solid #cbd5e1;
}
.host-bar button {
  padding: 6px 12px;
  border: 0;
  border-radius: 999px;
  background: #0f766e;
  color: #fff;
  cursor: pointer;
}
.host-grow {
  flex: 1;
}
.host-body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: 8px;
  padding: 8px;
}
.host-preview {
  min-height: 0;
}
.host-drawer {
  position: fixed;
  top: 0;
  bottom: 0;
  width: 320px;
  overflow: auto;
  padding: 40px 16px 16px;
  background: #fff;
  box-shadow: 0 0 24px rgb(0 0 0 / 0.2);
  z-index: 10;
}
.host-drawer.left {
  left: 0;
}
.host-drawer.right {
  right: 0;
}
.host-x {
  position: absolute;
  top: 8px;
  right: 8px;
  border: 0;
  background: none;
  font-size: 20px;
  cursor: pointer;
}
</style>
