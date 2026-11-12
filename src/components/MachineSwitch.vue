<script setup lang="ts">
import { CircleAlert, CircleCheck, Laptop, Monitor, Server, Unplug } from 'lucide-vue-next'
import type { MachineSummary } from '../api'
import { machine, machines } from '../store'
import RingSpinner from './RingSpinner.vue'

// Umschalter zwischen diesem Rechner und den SSH-Machines aus herdr. Die Kopfzeile zählt nur den
// gezeigten Rechner; damit auf dem verdeckten nichts untergeht, trägt sein Segment dessen
// wichtigsten Zustand mit demselben Icon wie die Kopfzeile: Freigabe vor fertig vor arbeitet.
const emit = defineEmits<{ select: [id: string] }>()

const OS_ICONS: Record<string, typeof Laptop> = { macos: Laptop, windows: Monitor, linux: Server }
const SHORTCUT = navigator.platform.startsWith('Mac') ? '⌘' : 'Strg+'

const icon = (m: MachineSummary) => (m.connected || m.connecting ? (OS_ICONS[m.os] ?? Monitor) : Unplug)

const hint = (m: MachineSummary, i: number) => {
  const head = m.host && m.host !== m.label ? `${m.label} (${m.host})` : m.label
  const body = m.connected
    ? `${m.live} Sessions · ${m.approval} Freigabe · ${m.busy} arbeiten · ${m.waiting} abgeschlossen`
    : m.connecting
      ? 'Verbindet …'
      : `Nicht erreichbar: ${m.error ?? 'unbekannter Fehler'}`
  return `${head}\n${body}\n${SHORTCUT}${i + 1}`
}
</script>

<template>
  <div role="tablist" aria-label="Rechner" class="flex items-center gap-0.5 rounded-xl border border-line bg-surface p-1">
    <button
      v-for="(m, i) in machines"
      :key="m.id"
      role="tab"
      :aria-selected="m.id === machine?.id"
      class="press flex items-center gap-2 rounded-lg px-3 py-1 text-[14px] font-medium whitespace-nowrap transition-colors duration-150"
      :class="m.id === machine?.id ? 'bg-surface-3 text-ink shadow-sm shadow-black/30' : 'text-ink-3 hover:bg-surface-2 hover:text-ink-2'"
      :title="hint(m, i)"
      @click="emit('select', m.id)"
    >
      <component :is="icon(m)" :size="16" :stroke-width="1.75" class="shrink-0" />
      {{ m.label }}
      <RingSpinner v-if="m.connecting" :size="13" class="text-ink-3" spinning />
      <span v-else-if="!m.connected" class="text-[12px] font-normal text-ink-3">offline</span>
      <span v-else class="num rounded-full bg-surface-2 px-2 text-[13px] text-ink-2">{{ m.live }}</span>

      <!-- Nur auf dem verdeckten Rechner; den gezeigten zählt die Kopfzeile -->
      <template v-if="m.id !== machine?.id && m.connected">
        <span v-if="m.approval" class="num flex items-center gap-1 text-[13px] text-ink">
          <CircleAlert :size="14" :stroke-width="2.25" class="wiggle text-critical" />{{ m.approval }}
        </span>
        <span v-else-if="m.waiting" class="num flex items-center gap-1 text-[13px] text-ink">
          <CircleCheck :size="14" :stroke-width="2.25" class="text-good" />{{ m.waiting }}
        </span>
        <RingSpinner v-else-if="m.busy" :size="13" class="text-warning" spinning />
      </template>
    </button>
  </div>
</template>
