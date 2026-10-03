<script setup lang="ts">
// The properties of whatever is selected: a band's config, the selected
// element's properties, or — with nothing selected — the whole ticket's paper
// settings. Belongs in a right rail or drawer; open it from `onSelect`.
import BandPanel from '../components/BandPanel.vue'
import DocPanel from '../components/DocPanel.vue'
import ModifierPanel from '../components/ModifierPanel.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  doc,
  paperId,
  dotWidth,
  dotWidthOk,
  selectPaper,
  setWidthChars,
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
    v-else-if="selected"
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
  <DocPanel
    v-else
    class="te-part"
    :paper-id="paperId"
    :width-chars="doc.paper.width_chars"
    :dot-width="dotWidth"
    :dot-width-ok="dotWidthOk"
    @select-paper="selectPaper"
    @set-width="setWidthChars"
  />
</template>
