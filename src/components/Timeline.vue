<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { Stats } from '../api'
import { time } from '../format'

const props = defineProps<{ stats: Stats; now: number; live: boolean }>()

const LABEL_W = 160
const LANE_H = 30
const AXIS_H = 26
const MIN = 60_000
const RANGES = [
  { label: 'Alles', ms: 0 },
  { label: '6 h', ms: 360 * MIN },
  { label: '1 h', ms: 60 * MIN },
  { label: '15 min', ms: 15 * MIN },
]
const TICK_STEPS = [1, 5, 10, 15, 30, 60, 120, 180, 360, 720, 1440].map((m) => m * MIN)

const root = ref<HTMLElement | null>(null)
const width = ref(800)
const range = ref(0)
const hover = ref('')
let observer: ResizeObserver | undefined
onMounted(() => {
  observer = new ResizeObserver(([e]) => (width.value = Math.max(300, e.contentRect.width)))
  if (root.value) observer.observe(root.value)
})
onBeforeUnmount(() => observer?.disconnect())

const end = computed(() => (props.live ? props.now : props.stats.lastT))
const start = computed(() => {
  const r = RANGES[range.value].ms
  return r ? Math.max(props.stats.firstT, end.value - r) : props.stats.firstT
})
const plotW = computed(() => width.value - LABEL_W - 12)
const x = (t: number) => LABEL_W + ((t - start.value) / Math.max(1, end.value - start.value)) * plotW.value
const inRange = (t: number) => t >= start.value && t <= end.value

const agents = computed(() => props.stats.agents.filter((a) => (a.finished ?? end.value) >= start.value && a.started <= end.value))
const lanes = computed(() => [
  { key: 'prompt', label: 'Du' },
  { key: 'main', label: 'Hauptagent' },
  ...agents.value.map((a) => ({ key: a.id, label: a.description || a.agentType })),
  { key: 'error', label: 'Fehler' },
])
const laneY = computed(() => new Map(lanes.value.map((l, i) => [l.key, AXIS_H + i * LANE_H + LANE_H / 2])))
const height = computed(() => AXIS_H + lanes.value.length * LANE_H + 4)

const marks = computed(() =>
  props.stats.events
    .filter((e) => inRange(e.t))
    .map((e) => ({
      ...e,
      cx: x(e.t),
      cy: laneY.value.get(e.kind === 'prompt' ? 'prompt' : e.kind === 'error' ? 'error' : 'main')!,
      tip: `${time(e.t)} · ${e.kind === 'prompt' ? 'Anfrage: ' : e.kind === 'error' ? 'Fehler in ' : ''}${e.label}${e.sidechain ? ' (Subagent)' : ''}`,
    })),
)
const bars = computed(() =>
  agents.value.map((a) => {
    const x1 = x(Math.max(a.started, start.value))
    const x2 = x(Math.min(a.finished ?? end.value, end.value))
    return { ...a, x1, w: Math.max(6, x2 - x1), cy: laneY.value.get(a.id)! }
  }),
)
const ticks = computed(() => {
  const span = end.value - start.value
  const step = TICK_STEPS.find((s) => span / s <= Math.max(3, plotW.value / 90)) ?? TICK_STEPS.at(-1)!
  const offset = new Date().getTimezoneOffset() * MIN // Ticks auf volle lokale Zeiten legen
  const out: number[] = []
  for (let t = Math.ceil((start.value - offset) / step) * step + offset; t <= end.value; t += step) out.push(t)
  return out
})
</script>

<template>
  <div ref="root">
    <div class="mb-3 flex flex-wrap items-center gap-x-5 gap-y-2 text-[13px]">
      <div class="flex overflow-hidden rounded-md border border-line">
        <button
          v-for="(r, i) in RANGES"
          :key="r.label"
          class="px-3 py-1.5 transition-colors duration-150"
          :class="range === i ? 'bg-surface-2 text-ink' : 'text-ink-3 hover:text-ink'"
          @click="range = i"
        >
          {{ r.label }}
        </button>
      </div>
      <span class="flex items-center gap-1.5 text-ink-2"><span class="h-2.5 w-2.5 rounded-full bg-series-2" /> Anfrage</span>
      <span class="flex items-center gap-1.5 text-ink-2"><span class="h-2.5 w-2.5 rounded-full bg-series-1" /> Tool-Aufruf</span>
      <span class="flex items-center gap-1.5 text-ink-2"><span class="h-2.5 w-4 rounded-sm bg-series-3" /> Subagent</span>
      <span class="flex items-center gap-1.5 text-ink-2"><span class="font-bold text-critical">✕</span> Fehler</span>
      <span class="num ml-auto min-h-[1.2em] truncate text-ink">{{ hover }}</span>
    </div>

    <svg :width="width" :height="height" class="block" @mouseleave="hover = ''">
      <g class="text-[12px]" fill="var(--color-ink-3)">
        <g v-for="t in ticks" :key="t">
          <line :x1="x(t)" :x2="x(t)" :y1="AXIS_H - 4" :y2="height" stroke="var(--color-line)" />
          <text :x="x(t)" :y="AXIS_H - 8" text-anchor="middle" class="num">{{ time(t) }}</text>
        </g>
      </g>

      <g v-for="(l, i) in lanes" :key="l.key">
        <rect v-if="i % 2 === 0" :x="0" :y="AXIS_H + i * LANE_H" :width="width" :height="LANE_H" fill="#ffffff05" />
        <text
          :x="4"
          :y="AXIS_H + i * LANE_H + LANE_H / 2 + 4"
          class="text-[13px]"
          :fill="l.key === 'error' ? 'var(--color-critical)' : 'var(--color-ink-2)'"
        >
          {{ l.label.length > 20 ? l.label.slice(0, 19) + '…' : l.label }}
          <title>{{ l.label }}</title>
        </text>
      </g>

      <g v-for="b in bars" :key="b.id" @mouseenter="hover = `${time(b.started)}–${b.finished ? time(b.finished) : 'läuft'} · ${b.agentType}: ${b.description}`">
        <rect :x="b.x1" :y="b.cy - 6" :width="b.w" height="12" rx="4" fill="var(--color-series-3)" :opacity="b.status === 'running' ? 0.65 : 1" />
        <rect :x="b.x1" :y="b.cy - LANE_H / 2" :width="b.w" :height="LANE_H" fill="transparent" />
      </g>

      <g v-for="(m, i) in marks" :key="i" @mouseenter="hover = m.tip">
        <template v-if="m.kind === 'error'">
          <path
            :d="`M${m.cx - 4},${m.cy - 4}L${m.cx + 4},${m.cy + 4}M${m.cx + 4},${m.cy - 4}L${m.cx - 4},${m.cy + 4}`"
            stroke="var(--color-critical)"
            stroke-width="2"
            stroke-linecap="round"
          />
        </template>
        <circle
          v-else
          :cx="m.cx"
          :cy="m.cy"
          r="5"
          :fill="m.kind === 'prompt' ? 'var(--color-series-2)' : 'var(--color-series-1)'"
          stroke="var(--color-surface)"
          stroke-width="2"
          :opacity="m.sidechain ? 0.5 : 1"
        />
        <circle :cx="m.cx" :cy="m.cy" r="8" fill="transparent" />
      </g>
    </svg>
    <p class="mt-2 text-[12px] text-ink-3">
      Zeigt die letzten 6000 Ereignisse der Session. Fahr mit der Maus über einen Punkt für Details.
    </p>
  </div>
</template>
