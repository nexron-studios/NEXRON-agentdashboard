<script setup lang="ts">
import { computed, ref } from 'vue'
import { Bot, ChevronDown, ListTree, MessageSquareText } from 'lucide-vue-next'
import type { Step } from '../api'
import { time } from '../format'
import { toolKind } from '../kinds'

// Ablauf nach prompt-kit „Steps“: je Anfrage ein aufklappbarer Block, darunter die Tool-Aufrufe an
// einer senkrechten Linie. Die neueste Anfrage ist offen, ältere klappen auf Klick auf.
const props = defineProps<{ steps: Step[]; working: boolean }>()

interface Group {
  key: number
  prompt: Step | null
  tools: Step[]
}

const groups = computed(() => {
  const out: Group[] = []
  for (const st of props.steps) {
    if (st.kind === 'prompt') {
      out.push({ key: st.t, prompt: st, tools: [] })
      continue
    }
    if (!out.length) out.push({ key: st.t, prompt: null, tools: [] })
    out[out.length - 1].tools.push(st)
  }
  return out.reverse()
})

const toggled = ref(new Set<number>())
const isOpen = (g: Group, i: number) => (i === 0) !== toggled.value.has(g.key)
const toggle = (g: Group) => {
  const next = new Set(toggled.value)
  if (!next.delete(g.key)) next.add(g.key)
  toggled.value = next
}
const isCurrent = (gi: number, si: number, g: Group) => props.working && gi === 0 && si === g.tools.length - 1
</script>

<template>
  <section class="card p-6">
    <h2 class="flex items-center gap-2 text-[16px] font-semibold">
      <ListTree :size="18" :stroke-width="2" class="text-series-1" /> Ablauf
      <span class="num rounded-full bg-surface-2 px-2 text-[13px] font-medium text-ink-2">{{ steps.length }}</span>
    </h2>
    <p class="mt-1 text-[13px] text-ink-3">Neueste Anfrage oben, darunter ihre Tool-Aufrufe in der Reihenfolge, in der Claude sie gemacht hat</p>

    <ol class="mt-4 space-y-2">
      <li v-for="(g, gi) in groups" :key="g.key">
        <button
          class="flex w-full items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left transition-colors duration-150 hover:bg-surface-2"
          :aria-expanded="isOpen(g, gi)"
          @click="toggle(g)"
        >
          <MessageSquareText :size="16" :stroke-width="1.75" class="shrink-0 text-ink-3" />
          <span class="min-w-0 flex-1 truncate text-[14px]" :class="g.prompt ? 'text-ink' : 'text-ink-3'" :title="g.prompt?.label">
            {{ g.prompt?.label ?? 'Frühere Schritte' }}
          </span>
          <span class="num shrink-0 text-[12px] text-ink-3">{{ g.tools.length }} Tools · {{ time(g.key) }}</span>
          <ChevronDown :size="16" class="shrink-0 text-ink-3 transition-transform duration-200" :class="{ 'rotate-180': isOpen(g, gi) }" />
        </button>
        <div v-if="isOpen(g, gi) && g.tools.length" class="mt-1 ml-3.5 flex gap-3">
          <span class="w-[2px] shrink-0 rounded-full bg-line-strong" aria-hidden="true" />
          <ul class="min-w-0 flex-1 space-y-1 py-1">
            <li v-for="(st, si) in g.tools" :key="si" class="flex items-center gap-2.5 text-[13px]" :class="{ 'pl-4': st.sidechain }">
              <Bot v-if="st.sidechain" :size="13" :stroke-width="2" class="shrink-0 text-series-7" aria-label="Subagent" />
              <span class="w-40 shrink-0">
                <span
                  class="inline-flex max-w-full items-center gap-1 rounded px-1.5 align-middle font-mono text-[12px]"
                  :class="st.failed ? 'bg-critical/15 text-critical' : toolKind(st.label).chip"
                  :aria-label="st.failed ? `${st.label} ist fehlgeschlagen` : undefined"
                ><component :is="toolKind(st.label).icon" :size="12" :stroke-width="2.25" class="shrink-0" /><span class="truncate" :title="st.label">{{ st.label }}</span></span>
              </span>
              <span class="min-w-0 flex-1 truncate" :class="isCurrent(gi, si, g) ? 'shimmer' : 'text-ink-3'" :title="st.detail">{{ st.detail }}</span>
              <span class="num shrink-0 text-[12px] text-ink-3">{{ time(st.t) }}</span>
            </li>
          </ul>
        </div>
      </li>
    </ol>
  </section>
</template>
