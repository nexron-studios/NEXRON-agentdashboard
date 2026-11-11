<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChevronDown, ListTree, MessageSquareText } from 'lucide-vue-next'
import type { Step } from '../api'
import { time } from '../format'
import { SAID_KIND } from '../kinds'
import ToolCall from './ToolCall.vue'

// Ablauf in drei Ebenen nach prompt-kit: je Anfrage ein Block („Steps“), darin Claudes
// Zwischenschritte als Gedankenkette („Chain of Thought“), darunter ihre Tool-Aufrufe („Tool“).
// Offen sind die neueste Anfrage und ihr letzter Zwischenschritt; alles andere klappt auf Klick auf.
const props = defineProps<{ steps: Step[]; working: boolean }>()

interface Thread {
  key: string
  /** `null` für Tool-Aufrufe, vor denen Claude nichts geschrieben hat */
  thought: Step | null
  tools: Step[]
}

interface Group {
  key: string
  prompt: Step | null
  threads: Thread[]
  toolCount: number
}

// Schlüssel aus der Zeit statt aus dem Index: Ab 200 Schritten fallen vorne alte weg, und mit dem
// Index wanderten sonst alle Auf/Zu-Zustände um eins. Gleiche Zeiten werden durchgezählt.
const keyMaker = () => {
  const seen = new Map<number, number>()
  return (prefix: string, t: number) => {
    const n = seen.get(t) ?? 0
    seen.set(t, n + 1)
    return `${prefix}${t}-${n}`
  }
}

const groupSteps = (steps: Step[]): Group[] => {
  const groups: Group[] = []
  const keyOf = keyMaker()
  const lastGroup = () => {
    if (!groups.length) groups.push({ key: `g${steps[0]?.t ?? 0}`, prompt: null, threads: [], toolCount: 0 })
    return groups[groups.length - 1]
  }
  steps.forEach((step) => {
    if (step.kind === 'prompt') {
      groups.push({ key: keyOf('g', step.t), prompt: step, threads: [], toolCount: 0 })
      return
    }
    const group = lastGroup()
    if (step.kind === 'thought') {
      group.threads.push({ key: keyOf('t', step.t), thought: step, tools: [] })
      return
    }
    if (!group.threads.length) group.threads.push({ key: keyOf('t', step.t), thought: null, tools: [] })
    group.threads[group.threads.length - 1].tools.push(step)
    group.toolCount++
  })
  return groups.reverse()
}

const groups = computed(() => groupSteps(props.steps))
const lastStep = computed(() => props.steps[props.steps.length - 1])
const isCurrent = (step: Step) => props.working && step === lastStep.value
const groupTime = (g: Group) => g.prompt?.t ?? g.threads[0]?.thought?.t ?? g.threads[0]?.tools[0]?.t ?? 0

// Gemerkt wird nur, was vom Standard abweicht, damit neue Schritte den Standard bekommen
const toggled = ref(new Set<string>())
const toggle = (key: string) => {
  const next = new Set(toggled.value)
  if (!next.delete(key)) next.add(key)
  toggled.value = next
}
const isGroupOpen = (g: Group, gi: number) => (gi === 0) !== toggled.value.has(g.key)
const isThreadOpen = (g: Group, gi: number, ti: number) =>
  (gi === 0 && ti === g.threads.length - 1) !== toggled.value.has(g.threads[ti].key)
</script>

<template>
  <section class="card p-6">
    <h2 class="flex items-center gap-2 text-[16px] font-semibold">
      <ListTree :size="18" :stroke-width="2" class="text-ink-3" /> Ablauf
      <span class="num rounded-full bg-surface-2 px-2 text-[13px] font-medium text-ink-2">{{ steps.length }}</span>
    </h2>
    <p class="mt-1 text-[13px] text-ink-3">Neueste Anfrage oben, darunter Claudes Zwischenschritte mit ihren Tool-Aufrufen. Ein Klick auf einen Aufruf zeigt Eingaben und Ergebnis</p>

    <ol class="mt-4 space-y-2">
      <li v-for="(g, gi) in groups" :key="g.key">
        <button
          class="flex w-full items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left transition-colors duration-150 hover:bg-surface-2"
          :aria-expanded="isGroupOpen(g, gi)"
          @click="toggle(g.key)"
        >
          <MessageSquareText :size="16" :stroke-width="1.75" class="shrink-0 text-ink-3" />
          <span class="min-w-0 flex-1 truncate text-[14px]" :class="g.prompt ? 'text-ink' : 'text-ink-3'" :title="g.prompt?.label">
            {{ g.prompt?.label ?? 'Frühere Schritte' }}
          </span>
          <span class="num shrink-0 text-[12px] text-ink-3">{{ g.toolCount }} Tools · {{ time(groupTime(g)) }}</span>
          <ChevronDown :size="16" class="shrink-0 text-ink-3 transition-transform duration-200" :class="{ 'rotate-180': isGroupOpen(g, gi) }" />
        </button>

        <div v-if="isGroupOpen(g, gi) && g.threads.length" class="mt-1 ml-3.5 flex gap-3">
          <span class="w-[2px] shrink-0 rounded-full bg-line-strong" aria-hidden="true" />
          <ol class="min-w-0 flex-1 space-y-1.5 py-1">
            <li v-for="(th, ti) in g.threads" :key="th.key">
              <button
                v-if="th.thought"
                class="-mx-1.5 flex w-[calc(100%+0.75rem)] items-start gap-2 rounded-md px-1.5 py-1 text-left transition-colors duration-150 hover:bg-surface-2"
                :aria-expanded="isThreadOpen(g, gi, ti)"
                @click="toggle(th.key)"
              >
                <component :is="SAID_KIND.icon" :size="14" :stroke-width="2" class="mt-0.5 shrink-0" :class="SAID_KIND.text" aria-hidden="true" />
                <span
                  class="min-w-0 flex-1 text-[13.5px] leading-snug"
                  :class="[isCurrent(th.thought) ? 'shimmer' : 'text-ink', { 'line-clamp-2': !isThreadOpen(g, gi, ti) }]"
                >{{ th.thought.label }}</span>
                <span v-if="th.tools.length" class="num mt-0.5 shrink-0 text-[12px] text-ink-3">{{ th.tools.length }} Tools</span>
                <ChevronDown :size="14" class="mt-0.5 shrink-0 text-ink-3 transition-transform duration-200" :class="{ 'rotate-180': isThreadOpen(g, gi, ti) }" />
              </button>
              <ul
                v-if="th.tools.length && (!th.thought || isThreadOpen(g, gi, ti))"
                class="space-y-1"
                :class="th.thought ? 'mt-1 ml-[6px] border-l-2 border-line py-0.5 pl-4' : ''"
              >
                <ToolCall v-for="(st, si) in th.tools" :key="`${st.t}-${si}`" :step="st" :current="isCurrent(st)" />
              </ul>
            </li>
          </ol>
        </div>
      </li>
    </ol>
  </section>
</template>
