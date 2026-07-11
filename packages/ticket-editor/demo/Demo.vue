<script setup lang="ts">
// Standalone harness for developing the editor without the host app. Feeds it a
// sample variable tree shaped like the spec's example and logs saves.
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { TicketEditor } from '../src'
import type { TicketDoc, VariableType } from '../src'

// Toggling the host locale flips the editor's language automatically.
const { locale } = useI18n()

const variables = {
  sale: {
    subtotal: 46.24,
    discount: 4.62,
    total: 41.62,
    receipt_id: 'A-100294',
    legend:
      'This is not a fiscal receipt. Please keep your ticket for any inquiry about your purchase.',
    items: [
      { product: 'Dog Food', qty: 2, amount: 24.5 },
      { product: 'Cat Toy', qty: 1, amount: 8.99 },
      { product: 'Fish Food', qty: 3, amount: 12.75 },
    ],
    // Payment movements for the "cut" — drives the conditional-total examples.
    movements: [
      { payment: 'CASH', amount: 20.0 },
      { payment: 'CARD', amount: 15.0 },
      { payment: 'CASH', amount: 6.62 },
    ],
    store: { name: 'Pet Palace', address: '123 Main St', lat: '19.4326', lng: '-99.1332' },
  },
}

const doc = ref<TicketDoc>({
  version: 2,
  // Calculated values — small formulas over the variables above. The QR at the
  // bottom points at `calc.maps_link`, and the cut totals sum movements by type.
  computed: [
    {
      name: 'maps_link',
      formula: 'concat("https://maps.google.com/?q=", sale.store.lat, ",", sale.store.lng)',
    },
    { name: 'cash_total', formula: 'sumif(sale.movements, payment == "CASH", amount)' },
    { name: 'card_total', formula: 'sumif(sale.movements, payment == "CARD", amount)' },
    { name: 'sales_line', formula: 'concat(count(sale.movements), " payments in the cut")' },
  ],
  paper: {
    width_chars: 40,
    margin_left_chars: 1,
    margin_right_chars: 1,
    margin_top_lines: 1,
    margin_bottom_lines: 1,
    cell_width_px: 12,
    cell_height_px: 22,
    font_px: 20,
  },
  // rows 3..4 loop over items; rows 6..7 show only when there's a discount.
  regions: [
    { id: 'loop', start_row: 3, end_row: 4, source: 'sale.items' },
    {
      id: 'disc',
      start_row: 6,
      end_row: 7,
      condition: { var: 'sale.discount', op: 'gt', value: '0' },
    },
  ],
  elements: [
    { id: 'title', row: 0, col: 15, type: 'text', content: 'PET PALACE', style: { bold: true } },
    { id: 'h1', row: 2, col: 0, type: 'text', content: 'Item' },
    { id: 'h2', row: 2, col: 22, type: 'text', content: 'Qty' },
    { id: 'h3', row: 2, col: 31, type: 'text', content: 'Amount' },
    // loop band (row 3): item fields
    { id: 'm1', row: 3, col: 0, type: 'variable', path: 'sale.items.0.product', length: 18 },
    {
      id: 'm2',
      row: 3,
      col: 22,
      type: 'variable',
      path: 'sale.items.0.qty',
      length: 6,
      align: 'right',
    },
    {
      id: 'm3',
      row: 3,
      col: 30,
      type: 'variable',
      path: 'sale.items.0.amount',
      length: 9,
      align: 'right',
      number: { decimals: 2, rounding: 'half_up', thousands: true },
    },
    // after the loop — flows below all repetitions
    { id: 'sl', row: 5, col: 0, type: 'text', content: 'SUBTOTAL:' },
    {
      id: 'sv',
      row: 5,
      col: 28,
      type: 'variable',
      path: 'sale.subtotal',
      length: 11,
      align: 'right',
      number: { decimals: 2, rounding: 'half_up', thousands: true },
    },
    // conditional band (row 6): only if discount > 0
    { id: 'dl', row: 6, col: 0, type: 'text', content: 'DISCOUNT:' },
    {
      id: 'dv',
      row: 6,
      col: 28,
      type: 'variable',
      path: 'sale.discount',
      length: 11,
      align: 'right',
      number: { decimals: 2, rounding: 'half_up', thousands: true },
    },
    { id: 'tl', row: 7, col: 0, type: 'text', content: 'TOTAL:', style: { bold: true, scale: 2 } },
    {
      id: 'tv',
      row: 7,
      col: 20,
      type: 'variable',
      path: 'sale.total',
      length: 19,
      align: 'right',
      number: { decimals: 2, rounding: 'half_up', thousands: true },
      style: { bold: true, scale: 2 },
    },
    // QR built from the calculated maps link — scans to the store's location.
    {
      id: 'qr',
      row: 9,
      col: 13,
      type: 'qr',
      value: 'calc.maps_link',
      from_variable: true,
      size: 12,
    },
  ],
})

// The host declares authoritative variable types (a backend would derive these
// from its column types). Anything omitted falls back to inference from the sample.
const variableTypes: Record<string, VariableType> = {
  'sale.subtotal': 'number',
  'sale.discount': 'number',
  'sale.total': 'number',
  'sale.items.0.qty': 'number',
  'sale.items.0.amount': 'number',
}

function locBtn(active: boolean) {
  return {
    padding: '2px 10px',
    borderRadius: '6px',
    border: '1px solid #cbd5e1',
    cursor: 'pointer',
    background: active ? '#4f46e5' : '#fff',
    color: active ? '#fff' : '#334155',
  }
}

function onSave(d: TicketDoc) {
  console.log('SAVE', JSON.stringify(d, null, 2))
  alert('Saved! (see console for the JSON the host would persist)')
}
</script>

<template>
  <div style="height: 100%; padding: 12px; display: flex; flex-direction: column; gap: 8px">
    <div style="display: flex; gap: 6px; align-items: center; font: 13px sans-serif">
      <span style="color: #64748b">Host locale:</span>
      <button :style="locBtn(locale === 'en')" @click="locale = 'en'">EN</button>
      <button :style="locBtn(locale === 'es')" @click="locale = 'es'">ES</button>
    </div>
    <div style="flex: 1; min-height: 0">
      <TicketEditor
        v-model="doc"
        :variables="variables"
        :variable-types="variableTypes"
        :on-save="onSave"
      />
    </div>
  </div>
</template>
