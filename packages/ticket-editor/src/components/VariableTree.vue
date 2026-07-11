<script setup lang="ts">
// The left rail: the variable tree the host app supplies. Clicking a leaf adds
// it to the ticket. The root instance renders a search box; groups collapse by
// default and show a match count, so a large model (hundreds of fields) stays
// navigable. Repeatable arrays (loopable, e.g. `items`) are badged.
import { computed, inject, provide, ref, type Ref } from 'vue'
import { useT } from '../i18n'
import TypeTag from './TypeTag.vue'
import type { VariableType, VarNode } from '../types'

const t = useT()
const props = defineProps<{
  nodes: VarNode[]
  /** Resolved path -> type (host declarations win over inference). Drives the tag. */
  types?: Record<string, VariableType>
  /** True on the top-level instance: renders the search box + empty hint. */
  root?: boolean
}>()
const emit = defineEmits<{ add: [node: VarNode] }>()

// The search query is shared with every nested instance so all levels filter
// against the same text. The root owns it; descendants re-provide what they got.
const QUERY_KEY = 'te-var-query'
const injected = inject<Ref<string> | null>(QUERY_KEY, null)
const query = injected ?? ref('')
provide(QUERY_KEY, query)

const q = computed(() => query.value.trim().toLowerCase())

function typeOf(node: VarNode): VariableType {
  return (node.path && props.types?.[node.path]) || node.type || 'text'
}

/** Number of leaf descendants — shown as a count so a group's size is legible. */
function leafCount(node: VarNode): number {
  return node.children ? node.children.reduce((n, c) => n + leafCount(c), 0) : 1
}

/** A node survives the filter if it — or, for a group, any descendant — matches. */
function matches(node: VarNode): boolean {
  if (!q.value) return true
  if (node.key.toLowerCase().includes(q.value) || node.path.toLowerCase().includes(q.value)) {
    return true
  }
  return node.children ? node.children.some(matches) : false
}

const visible = computed(() => props.nodes.filter(matches))

// Groups are collapsed by default; an active search forces every surviving
// branch open so matches are always in view.
const expanded = ref<Set<string>>(new Set())
function isOpen(node: VarNode): boolean {
  return !!q.value || expanded.value.has(node.path)
}
function toggle(path: string) {
  const next = new Set(expanded.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  expanded.value = next
}
</script>

<template>
  <div v-if="root" class="te-tree-search">
    <input
      class="te-tree-search-input"
      type="search"
      :value="query"
      :placeholder="t('searchVars')"
      @input="query = ($event.target as HTMLInputElement).value"
    />
  </div>

  <p v-if="root && q && !visible.length" class="te-tree-empty">{{ t('noVarMatches') }}</p>

  <ul class="te-tree">
    <li v-for="node in visible" :key="node.path">
      <template v-if="node.children">
        <button class="te-tree-group" type="button" @click="toggle(node.path)">
          <span class="te-tree-caret" :class="{ open: isOpen(node) }">▸</span>
          <span class="te-tree-key">{{ node.key }}</span>
          <span v-if="node.repeatable" class="te-badge" :title="t('repeatableTip')">↻</span>
          <span class="te-tree-count">{{ leafCount(node) }}</span>
        </button>
        <VariableTree
          v-if="isOpen(node)"
          class="te-tree-nested"
          :nodes="node.children"
          :types="types"
          @add="emit('add', $event)"
        />
      </template>
      <button v-else class="te-tree-leaf" type="button" @click="emit('add', node)">
        <span class="te-tree-key">{{ node.key }}</span>
        <TypeTag class="te-tree-tag" :type="typeOf(node)" />
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
.te-tree-search {
  padding: 0 0 0.4rem;
}
.te-tree-search-input {
  width: 100%;
  box-sizing: border-box;
  padding: 0.3rem 0.5rem;
  font-size: 0.8rem;
  border: 1px solid var(--te-border);
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
}
.te-tree-search-input:focus {
  outline: none;
  border-color: var(--te-primary);
}
.te-tree-empty {
  padding: 0.4rem;
  margin: 0;
  color: var(--te-muted-fg);
  font-size: 0.8rem;
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
.te-tree-count {
  margin-left: auto;
  color: var(--te-muted-fg);
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
}
.te-tree-tag {
  margin-left: auto;
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
