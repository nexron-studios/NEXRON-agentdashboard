<script setup lang="ts">
import { computed, ref } from 'vue'
import { Bot, ChevronDown } from 'lucide-vue-next'
import type { Step } from '../api'
import { time } from '../format'
import { KIND_CHIP, toolKind } from '../kinds'
import SourceChip from './SourceChip.vue'

// Ein Tool-Aufruf nach prompt-kit „Tool“: zugeklappt eine Zeile wie bisher, aufgeklappt Status,
// alle Eingaben und der Anfang des Ergebnisses. Quellen stehen immer darunter, auch zugeklappt.
const props = defineProps<{ step: Step; current: boolean }>()

/** So viele Quellen zeigt die zugeklappte Zeile, der Rest steht aufgeklappt */
const SOURCES_COLLAPSED = 4

const isOpen = ref(false)
const kind = computed(() => toolKind(props.step.label))
const state = computed(() => {
  if (props.step.failed) return { text: 'Fehlgeschlagen', class: 'text-critical' }
  if (props.step.done) return { text: 'Fertig', class: 'text-good' }
  return props.current ? { text: 'Läuft', class: 'text-warning' } : { text: 'Kein Ergebnis', class: 'text-ink-3' }
})
const visibleSources = computed(() => (isOpen.value ? props.step.sources : props.step.sources.slice(0, SOURCES_COLLAPSED)))
const hiddenSources = computed(() => props.step.sources.length - visibleSources.value.length)
</script>

<template>
  <li :class="{ 'pl-4': step.sidechain }">
    <button
      class="-mx-1.5 flex w-[calc(100%+0.75rem)] items-center gap-2.5 rounded-md px-1.5 py-0.5 text-left text-[13px] transition-colors duration-150 hover:bg-surface-2"
      :aria-expanded="isOpen"
      @click="isOpen = !isOpen"
    >
      <Bot v-if="step.sidechain" :size="13" :stroke-width="2" class="shrink-0 text-ink-3" aria-label="Subagent" />
      <span class="w-40 shrink-0">
        <span
          class="inline-flex max-w-full items-center gap-1 rounded px-1.5 align-middle font-mono text-[12px]"
          :class="step.failed ? 'bg-critical/15 text-critical' : KIND_CHIP"
          :aria-label="step.failed ? `${step.label} ist fehlgeschlagen` : undefined"
        ><component :is="kind.icon" :size="12" :stroke-width="2.25" class="shrink-0" :class="{ [kind.text]: !step.failed }" /><span class="truncate" :title="step.label">{{ step.label }}</span></span>
      </span>
      <span class="min-w-0 flex-1 truncate" :class="current ? 'shimmer' : 'text-ink-2'" :title="step.detail">{{ step.detail }}</span>
      <span class="num shrink-0 text-[12px] text-ink-3">{{ time(step.t) }}</span>
      <ChevronDown :size="14" class="shrink-0 text-ink-3 transition-transform duration-200" :class="{ 'rotate-180': isOpen }" />
    </button>

    <div v-if="step.sources.length" class="mt-1 mb-1.5 flex flex-wrap items-center gap-1.5" :class="step.sidechain ? 'ml-[calc(11.25rem+13px)]' : 'ml-[10.625rem]'">
      <SourceChip v-for="source in visibleSources" :key="source.url" :source="source" />
      <span v-if="hiddenSources > 0" class="num text-[12px] text-ink-3">+{{ hiddenSources }}</span>
    </div>

    <div v-if="isOpen" class="mt-1 mb-2 space-y-3 rounded-lg border border-line p-3 text-[12.5px]">
      <p class="font-medium" :class="state.class">{{ state.text }}</p>
      <dl v-if="step.input.length" class="grid grid-cols-[minmax(0,8rem)_1fr] gap-x-3 gap-y-1">
        <template v-for="field in step.input" :key="field.key">
          <dt class="truncate font-mono text-ink-3" :title="field.key">{{ field.key }}</dt>
          <dd class="font-mono break-all whitespace-pre-wrap text-ink-2">{{ field.value }}</dd>
        </template>
      </dl>
      <div v-if="step.output">
        <p class="mb-1 text-ink-3">Ergebnis</p>
        <pre
          class="max-h-48 overflow-auto rounded-md bg-app p-2.5 font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap"
          :class="step.failed ? 'text-critical' : 'text-ink-2'"
        >{{ step.output }}</pre>
      </div>
    </div>
  </li>
</template>
