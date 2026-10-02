<script setup lang="ts">
// The structural grid editor, wired to the editor state, with its view controls
// (zoom, fit, fields/values) floating in the corner like a design tool's. Fills
// its container.
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import GridCanvas from '../components/GridCanvas.vue'
import { useTicketEditorContext } from '../core/useTicketEditor'

const {
  t,
  doc,
  selectedId,
  selectedBandId,
  zoom,
  showFields,
  sampleOf,
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

const MIN_ZOOM = 0.5
const MAX_ZOOM = 2.2
const clamp = (z: number) => Math.round(Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z)) * 10) / 10

// Room the canvas chrome takes besides the paper: the wrap's padding on both
// sides plus the row gutter and its gap (see GridCanvas styles).
const CHROME_PX = 2 * 20 + 96 + 4
const root = ref<HTMLElement | null>(null)
function fitZoom(): number {
  const avail = (root.value?.clientWidth ?? 0) - CHROME_PX
  const paperPx = (doc.value.paper.width_chars + 1) * (doc.value.paper.cell_width_px ?? 12)
  return avail > 0 ? clamp(Math.min(1.4, avail / paperPx)) : zoom.value
}
// Follow the container's width (a rail opening, the window resizing) until the
// user picks a zoom themselves; Fit hands control back.
const following = ref(true)
function fit() {
  following.value = true
  zoom.value = fitZoom()
}
function step(d: number) {
  following.value = false
  zoom.value = clamp(zoom.value + d)
}
let ro: ResizeObserver | undefined
onMounted(() => {
  fit()
  ro = new ResizeObserver(() => {
    if (following.value) zoom.value = fitZoom()
  })
  if (root.value) ro.observe(root.value)
})
onBeforeUnmount(() => ro?.disconnect())
// A different paper width changes what "fit" means.
watch(
  () => [doc.value.paper.width_chars, doc.value.paper.cell_width_px],
  () => {
    if (following.value) zoom.value = fitZoom()
  },
)
</script>

<template>
  <div ref="root" class="te-part te-canvas-part">
    <GridCanvas
      :doc="doc"
      :selected-id="selectedId"
      :selected-band-id="selectedBandId"
      :zoom="zoom"
      :variables="previewData"
      :sample-of="sampleOf"
      :show-fields="showFields"
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
    <div class="te-view-ctl">
      <button
        class="te-view-btn"
        :class="{ on: showFields }"
        type="button"
        :aria-pressed="showFields"
        :title="t('showFieldsTip')"
        @click="showFields = !showFields"
      >
        {{ t('showFields') }}
      </button>
      <span class="te-view-sep" />
      <button
        class="te-view-btn"
        type="button"
        :title="t('zoomOut')"
        :aria-label="t('zoomOut')"
        @click="step(-0.1)"
      >
        −
      </button>
      <span class="te-view-pct">{{ Math.round(zoom * 100) }}%</span>
      <button
        class="te-view-btn"
        type="button"
        :title="t('zoomIn')"
        :aria-label="t('zoomIn')"
        @click="step(0.1)"
      >
        +
      </button>
      <button
        class="te-view-btn"
        :class="{ on: following }"
        type="button"
        :title="t('zoomFitTip')"
        @click="fit"
      >
        {{ t('zoomFit') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.te-canvas-part {
  position: relative;
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
}
.te-view-ctl {
  position: absolute;
  right: 0.6rem;
  bottom: 0.6rem;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 0.15rem;
  padding: 0.2rem;
  border: 1px solid var(--te-border);
  border-radius: calc(var(--te-radius) + 2px);
  background: var(--te-card);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  font-size: 0.75rem;
}
.te-view-btn {
  min-width: 1.6rem;
  height: 1.6rem;
  padding: 0 0.45rem;
  border: 0;
  border-radius: calc(var(--te-radius) - 2px);
  background: transparent;
  color: var(--te-muted-fg);
  font: inherit;
  cursor: pointer;
}
.te-view-btn:hover {
  background: var(--te-accent);
  color: inherit;
}
.te-view-btn.on {
  background: color-mix(in srgb, var(--te-primary) 14%, transparent);
  color: var(--te-primary);
}
.te-view-pct {
  min-width: 2.6rem;
  text-align: center;
  font-variant-numeric: tabular-nums;
  color: var(--te-muted-fg);
}
.te-view-sep {
  width: 1px;
  height: 1rem;
  margin: 0 0.2rem;
  background: var(--te-border);
}
</style>
