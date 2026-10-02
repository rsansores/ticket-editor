<script setup lang="ts">
// The structural grid editor, wired to the editor state. Fills its container.
import GridCanvas from '../components/GridCanvas.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  doc,
  selectedId,
  selectedBandId,
  zoom,
  previewData,
  loopSources,
  allVars,
  selectElement,
  selectBand,
  updateElement,
  insertRow,
  deleteRow,
  createRegion,
  removeRegion,
} = useTicketEditorContext()
</script>

<template>
  <GridCanvas
    class="te-part"
    :doc="doc"
    :selected-id="selectedId"
    :selected-band-id="selectedBandId"
    :zoom="zoom"
    :variables="previewData"
    :loop-sources="loopSources"
    :all-vars="allVars"
    @select="selectElement"
    @select-band="selectBand"
    @update:element="updateElement"
    @insert-row="insertRow"
    @delete-row="deleteRow"
    @create-region="createRegion"
    @remove-region="removeRegion"
  />
</template>
