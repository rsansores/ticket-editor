<script setup lang="ts">
// The formula dialogs (calculated values and a band's calculated columns).
// They open from actions elsewhere, so mount this once anywhere in the layout.
import ComputedEditor from '../components/ComputedEditor.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'
import { RESERVED_ROW_NAMES } from '../types'

const {
  editingCalc,
  varGroups,
  previewFormula,
  computedVars,
  saveCalc,
  editingRowCalc,
  rowVarGroups,
  previewRowFormula,
  regionById,
  saveRowCalc,
} = useTicketEditorContext()
</script>

<template>
  <div class="te-part">
    <ComputedEditor
      v-if="editingCalc"
      :model-value="editingCalc"
      :var-groups="varGroups"
      :preview="previewFormula"
      :existing-names="computedVars.map((c) => c.name)"
      @save="saveCalc"
      @cancel="editingCalc = null"
    />

    <ComputedEditor
      v-if="editingRowCalc"
      :model-value="editingRowCalc.calc"
      :var-groups="rowVarGroups"
      :preview="previewRowFormula"
      :existing-names="(regionById(editingRowCalc.regionId)?.computed ?? []).map((c) => c.name)"
      variant="row"
      :reserved-names="RESERVED_ROW_NAMES"
      @save="saveRowCalc"
      @cancel="editingRowCalc = null"
    />
  </div>
</template>
