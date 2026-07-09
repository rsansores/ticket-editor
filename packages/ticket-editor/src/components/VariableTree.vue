<script setup lang="ts">
// The left rail: the variable tree the host app supplies. Clicking a leaf adds
// it to the ticket. Groups collapse/expand. Repeatable arrays (loopable, e.g.
// `items`) are badged so the user can tell them apart — loops land in a
// later pass but the affordance is visible now.
import { ref } from 'vue'
import { useT } from '../i18n'
import type { VarNode } from '../types'

const t = useT()
defineProps<{ nodes: VarNode[] }>()
const emit = defineEmits<{ add: [node: VarNode] }>()

const collapsed = ref<Set<string>>(new Set())
function toggle(path: string) {
  const s = new Set(collapsed.value)
  if (s.has(path)) s.delete(path)
  else s.add(path)
  collapsed.value = s
}
</script>

<template>
  <ul class="te-tree">
    <li v-for="node in nodes" :key="node.path">
      <template v-if="node.children">
        <button class="te-tree-group" type="button" @click="toggle(node.path)">
          <span class="te-tree-caret" :class="{ open: !collapsed.has(node.path) }">▸</span>
          <span class="te-tree-key">{{ node.key }}</span>
          <span v-if="node.repeatable" class="te-badge" :title="t('repeatableTip')">↻</span>
        </button>
        <VariableTree
          v-if="!collapsed.has(node.path)"
          class="te-tree-nested"
          :nodes="node.children"
          @add="emit('add', $event)"
        />
      </template>
      <button v-else class="te-tree-leaf" type="button" @click="emit('add', node)">
        <span class="te-tree-key">{{ node.key }}</span>
        <span v-if="node.sample !== undefined" class="te-tree-sample">{{ node.sample }}</span>
        <span class="te-tree-plus">＋</span>
      </button>
    </li>
  </ul>
</template>

<style scoped>
.te-tree {
  list-style: none;
  margin: 0;
  padding: 0;
  font-size: 0.85rem;
}
.te-tree-nested {
  margin-left: 0.75rem;
  border-left: 1px solid var(--te-border);
  padding-left: 0.25rem;
}
.te-tree-group,
.te-tree-leaf {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  width: 100%;
  padding: 0.25rem 0.4rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}
.te-tree-group:hover,
.te-tree-leaf:hover {
  background: var(--te-accent);
}
.te-tree-caret {
  display: inline-block;
  transition: transform 0.12s ease;
  color: var(--te-muted-fg);
  font-size: 0.7rem;
}
.te-tree-caret.open {
  transform: rotate(90deg);
}
.te-tree-key {
  font-weight: 500;
}
.te-tree-sample {
  margin-left: auto;
  color: var(--te-muted-fg);
  font-family: ui-monospace, monospace;
  font-size: 0.75rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 8ch;
}
.te-tree-plus {
  margin-left: 0.35rem;
  color: var(--te-primary);
  opacity: 0;
}
.te-tree-leaf:hover .te-tree-plus {
  opacity: 1;
}
.te-badge {
  margin-left: auto;
  color: var(--te-primary);
  font-size: 0.8rem;
}
</style>
