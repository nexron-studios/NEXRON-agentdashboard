<script setup lang="ts">
import { computed } from 'vue'
import { Circle, CircleAlert, CircleCheck, CircleStop, Hourglass, Timer } from 'lucide-vue-next'
import RingSpinner from './RingSpinner.vue'
import TerminalLoader from './TerminalLoader.vue'

/** `fresh`: Session ohne eigene Anfrage. „idle“ heißt dann „bereit“, nicht „wartet auf dich“.
 *  `note`: Claudes Meldung, warum die Session steht (Nutzungslimit), als Tooltip.
 *  `background`: how many shells, agents or skills Claude still waits for. */
const props = defineProps<{ status: string; fresh?: boolean; size?: 'sm' | 'md'; note?: string; background?: number }>()

const STATES = {
  blocked: { label: 'Braucht Freigabe', icon: CircleAlert, text: 'text-critical', bg: 'bg-critical/12 border-critical/40' },
  busy: { label: 'Arbeitet', icon: RingSpinner, text: 'text-warning', bg: 'bg-warning/10 border-warning/30' },
  shell: { label: 'Führt Befehl aus', icon: TerminalLoader, text: 'text-warning', bg: 'bg-warning/10 border-warning/30' },
  background: { label: 'Wartet auf Hintergrund', icon: Timer, text: 'text-warning', bg: 'bg-warning/10 border-warning/30' },
  waiting: { label: 'Aufgabe abgeschlossen', icon: CircleCheck, text: 'text-good', bg: 'bg-good/12 border-good/35' },
  limit: { label: 'Limit erreicht', icon: Hourglass, text: 'text-ink-2', bg: 'bg-surface-2 border-line-strong' },
  ready: { label: 'Bereit', icon: Circle, text: 'text-ink-2', bg: 'bg-surface-2 border-line-strong' },
  ended: { label: 'Beendet', icon: CircleStop, text: 'text-ink-3', bg: 'bg-surface-2 border-line' },
} as const

const state = computed(() => {
  const s = props.status
  if (s === 'blocked' || s === 'waiting') return STATES.blocked
  if (s === 'busy') return STATES.busy
  if (s === 'shell') return STATES.shell
  if (s === 'background') return STATES.background
  if (s === 'idle' || s === 'done') return props.fresh ? STATES.ready : STATES.waiting
  if (s === 'limit') return STATES.limit
  if (s === 'ended') return STATES.ended
  return { ...STATES.ready, label: s }
})

const shortLabel = computed(() => {
  if (state.value === STATES.waiting) return 'Fertig'
  if (state.value === STATES.background) return 'Wartet'
  return state.value.label
})

const tooltip = computed(() => {
  const n = props.background ?? 0
  if (state.value !== STATES.background || !n) return undefined
  return n === 1 ? 'Eine Aufgabe läuft noch im Hintergrund' : `${n} Aufgaben laufen noch im Hintergrund`
})
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center gap-1.5 rounded-full border font-semibold whitespace-nowrap"
    :class="[state.bg, state.text, size === 'sm' ? 'px-2 py-0.5 text-[12px]' : 'px-2.5 py-1 text-[13px]']"
    :title="note || tooltip"
  >
    <!-- Neuer Status: Icon ploppt einmal auf, damit der Wechsel auffällt -->
    <span :key="state.label" class="pop-in relative inline-flex" :class="{ attention: state === STATES.blocked }">
      <component :is="state.icon" :size="size === 'sm' ? 13 : 15" :stroke-width="2.25" />
    </span>
    {{ size === 'sm' ? shortLabel : state.label }}
  </span>
</template>
