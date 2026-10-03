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
const clamp = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z))

const root = ref<HTMLElement | null>(null)
const wrapEl = () => root.value?.querySelector<HTMLElement>('.te-canvas-wrap') ?? null

// The largest zoom at which the paper fits the canvas without a horizontal
// scrollbar. Measured, not assumed: the room inside the wrap's padding and
// scrollbar, minus the row gutter. Rounded DOWN — rounding up is what made Fit
// scroll by a few pixels.
function fitZoom(): number {
  const wrap = wrapEl()
  const gutter = root.value?.querySelector<HTMLElement>('.te-gutter')
  const stage = gutter?.parentElement
  if (!wrap || !gutter || !stage) return zoom.value
  const cs = getComputedStyle(wrap)
  const stageGap = parseFloat(getComputedStyle(stage).columnGap) || 0
  const avail =
    wrap.clientWidth -
    parseFloat(cs.paddingLeft) -
    parseFloat(cs.paddingRight) -
    gutter.offsetWidth -
    stageGap
  const paperPx = (doc.value.paper.width_chars + 1) * (doc.value.paper.cell_width_px ?? 12)
  if (avail <= 0) return zoom.value
  return clamp(Math.floor(Math.min(1.4, avail / paperPx) * 100) / 100)
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
  zoom.value = clamp(Math.round((zoom.value + d) * 10) / 10)
}

// The view controls float over the canvas's corner; keep them clear of its
// scrollbars, which come and go with the zoom and the ticket's length.
const scrollbar = ref({ right: 0, bottom: 0 })
function measureScrollbars() {
  const wrap = wrapEl()
  if (!wrap) return
  scrollbar.value = {
    right: wrap.offsetWidth - wrap.clientWidth,
    bottom: wrap.offsetHeight - wrap.clientHeight,
  }
}

let ro: ResizeObserver | undefined
onMounted(() => {
  fit()
  ro = new ResizeObserver(() => {
    if (following.value) zoom.value = fitZoom()
    measureScrollbars()
  })
  if (root.value) ro.observe(root.value)
  // The stage grows with rows and zoom; that is what makes scrollbars appear.
  const stage = root.value?.querySelector('.te-stage')
  if (stage) ro.observe(stage)
  measureScrollbars()
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
    <div
      class="te-view-ctl"
      :style="{
        right: `calc(0.6rem + ${scrollbar.right}px)`,
        bottom: `calc(0.6rem + ${scrollbar.bottom}px)`,
      }"
    >
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
