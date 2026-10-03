<script setup lang="ts">
// The variable picker: the host's variable tree plus the calculated values.
// Clicking either places it on the ticket. Belongs in a left rail or drawer.
import VariableTree from '../components/VariableTree.vue'
import TypeTag from '../components/TypeTag.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  t,
  tree,
  types,
  addVariable,
  computedVars,
  calcReports,
  calcHasError,
  calcKind,
  addCalcElement,
  newCalc,
  editCalc,
  removeCalc,
} = useTicketEditorContext()
</script>

<template>
  <div class="te-part te-vars">
    <h3 class="te-rail-title">{{ t('railVariables') }}</h3>
    <VariableTree root :nodes="tree" :types="types" @add="addVariable" />

    <div class="te-calc">
      <h3 class="te-rail-title te-calc-title">{{ t('railCalculated') }}</h3>
      <ul v-if="computedVars.length" class="te-calc-list">
        <li v-for="c in computedVars" :key="c.name" class="te-calc-item">
          <button class="te-calc-add" type="button" :title="c.formula" @click="addCalcElement(c)">
            <span class="te-calc-eq" aria-hidden="true">=</span>
            <span class="te-calc-key">{{ c.name }}</span>
            <span
              v-if="calcHasError(c.name)"
              class="te-calc-warn"
              :title="calcReports[c.name]?.error ?? ''"
              >⚠</span
            >
            <TypeTag v-else class="te-calc-tag" :type="calcKind(c.name)" />
          </button>
          <button
            class="te-calc-icon"
            type="button"
            :aria-label="t('calcEdit')"
            :title="t('calcEdit')"
            @click="editCalc(c)"
          >
            ✎
          </button>
          <button
            class="te-calc-icon"
            type="button"
            :aria-label="t('calcDelete')"
            :title="t('calcDelete')"
            @click="removeCalc(c.name)"
          >
            🗑
          </button>
        </li>
      </ul>
      <p v-else class="te-calc-empty">{{ t('calcEmpty') }}</p>
      <button class="te-calc-new" type="button" @click="newCalc">
        {{ t('calcAddNew') }}
      </button>
    </div>
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
/* calculated variables section */
.te-calc {
  margin-top: 0.9rem;
  padding-top: 0.6rem;
  border-top: 1px solid var(--te-border);
}
.te-calc-title {
  margin-top: 0;
}
.te-calc-list {
  list-style: none;
  margin: 0 0 0.4rem;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}
.te-calc-item {
  display: flex;
  align-items: center;
  gap: 0.15rem;
}
.te-calc-add {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  flex: 1;
  min-width: 0;
  padding: 0.25rem 0.4rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}
.te-calc-add:hover {
  background: var(--te-accent);
}
/* calculated fields read as Tableau-style: a leading "=" and the accent colour. */
.te-calc-eq {
  color: var(--te-primary);
  font-family: ui-monospace, monospace;
  font-weight: 700;
  font-size: 0.8rem;
}
.te-calc-key {
  font-weight: 500;
  font-size: 0.85rem;
  color: var(--te-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.te-calc-tag {
  margin-left: auto;
}
.te-calc-warn {
  margin-left: auto;
  color: #dc2626;
  font-size: 0.8rem;
}
.te-calc-icon {
  border: 0;
  background: transparent;
  color: var(--te-muted-fg);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 0.15rem;
  flex: none;
}
.te-calc-icon:hover {
  color: inherit;
}
.te-calc-empty {
  margin: 0 0 0.4rem;
  color: var(--te-muted-fg);
  font-size: 0.74rem;
  line-height: 1.35;
}
.te-calc-new {
  width: 100%;
  padding: 0.3rem 0.5rem;
  border: 1px dashed var(--te-input);
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: var(--te-primary);
  cursor: pointer;
  font: inherit;
  font-size: 0.78rem;
}
.te-calc-new:hover {
  background: var(--te-accent);
}
</style>
