<script setup lang="ts">
// The properties of whatever is selected: a band's config, or the selected
// element's modifiers (an empty prompt when nothing is). Belongs in a right
// rail or drawer; open it from the context's `onSelect`.
import BandPanel from '../components/BandPanel.vue'
import ModifierPanel from '../components/ModifierPanel.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  selected,
  selectedType,
  selectedBand,
  selectedElBand,
  selectedCondVars,
  selectedRowPaths,
  loopSources,
  allVars,
  contentCols,
  rowCalcReports,
  updateElement,
  removeElement,
  collapseRow,
  updateRegion,
  removeRegion,
  editRowCalc,
  removeRowCalc,
  placeRowCalc,
} = useTicketEditorContext()
</script>

<template>
  <BandPanel
    v-if="selectedBand"
    class="te-part"
    :region="selectedBand"
    :loop-sources="loopSources"
    :all-vars="allVars"
    :calc-reports="rowCalcReports[selectedBand.id]"
    @update:region="updateRegion"
    @remove="removeRegion"
    @edit-calc="editRowCalc"
    @remove-calc="removeRowCalc"
    @place-calc="placeRowCalc"
  />
  <ModifierPanel
    v-else
    class="te-part"
    :element="selected"
    :var-type="selectedType"
    :all-vars="allVars"
    :loop-sources="loopSources"
    :content-cols="contentCols"
    :cond-vars="selectedCondVars"
    :extra-known-paths="selectedRowPaths"
    :in-band="!!selectedElBand"
    @update:element="updateElement"
    @remove="removeElement"
    @collapse-row="collapseRow"
  />
</template>
