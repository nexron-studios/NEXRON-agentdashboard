<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChevronDown, History, Radio, Search, TriangleAlert, UserRound } from 'lucide-vue-next'
import type { Overview, SessionRow } from '../api'
import { byTab, dayLabel, headline, projectName, time } from '../format'
import { accountInfo } from '../store'
import SessionCard from './SessionCard.vue'

const props = defineProps<{ overview: Overview | null; error: string; now: number }>()
const emit = defineEmits<{ select: [id: string] }>()

const COLLAPSED = 6

const live = computed(() =>
  // Feste Reihenfolge wie in herdr (claude-1, claude-2, …), damit Karten beim Statuswechsel nicht springen
  [...(props.overview?.live ?? [])].sort((a, b) => byTab(a.herdr?.tab, b.herdr?.tab) || b.startedAt - a.startedAt),
)

const query = ref('')
const expanded = ref(false)
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  const all = props.overview?.recent ?? []
  if (!q) return all
  return all.filter((s) => [s.title, s.cwd, s.firstPrompt, s.currentTask, s.lastPrompt].some((f) => f.toLowerCase().includes(q)))
})
const shown = computed(() => (expanded.value || query.value ? filtered.value : filtered.value.slice(0, COLLAPSED)))
const groups = computed(() => {
  const out: { day: string; items: SessionRow[] }[] = []
  for (const s of shown.value) {
    const day = dayLabel(s.lastT, props.now)
    if (out.at(-1)?.day !== day) out.push({ day, items: [] })
    out.at(-1)!.items.push(s)
  }
  return out
})
</script>

<template>
  <div class="mx-auto max-w-[1680px] px-8 pt-5 pb-12">
    <p v-if="error" class="mb-6 flex items-center gap-2 rounded-xl border border-critical/40 bg-critical/10 px-4 py-3">
      <TriangleAlert :size="18" class="shrink-0 text-critical" /> {{ error }}
    </p>

    <!-- Füllt das Fenster unter dem Header (4rem) und dem Abstand oben (1.25rem): die ersten
         sechs Karten teilen sich die Fläche, „Zuletzt“ kommt erst beim Scrollen. -->
    <section class="flex min-h-[calc(100dvh-5.25rem)] flex-col pb-6">
      <h2 class="mb-4 flex items-center gap-2.5 text-[20px] font-semibold tracking-tight">
        <Radio :size="20" :stroke-width="2" class="radio-pulse text-brand" /> Laufende Sessions
        <span class="num rounded-full bg-surface-2 px-2.5 text-[14px] font-medium text-ink-2">{{ live.length }}</span>
      </h2>

      <div v-if="!overview && !error" class="grid gap-4 min-[760px]:grid-cols-2 min-[1150px]:grid-cols-3">
        <div v-for="i in 2" :key="i" class="card h-64 animate-pulse" />
      </div>
      <div v-else-if="!live.length" class="card flex flex-col items-center gap-2 px-6 py-12 text-center text-ink-2">
        <Radio :size="28" class="text-ink-3" />
        Gerade läuft keine Claude-Code-Session.
        <span class="text-[13px] text-ink-3">Starte <code class="font-mono">claude</code> in einem Terminal, dann taucht sie hier auf.</span>
      </div>
      <div
        v-else
        class="grid flex-1 gap-4 min-[760px]:grid-cols-2 min-[760px]:grid-rows-[repeat(3,1fr)] min-[1150px]:grid-cols-3 min-[1150px]:grid-rows-[repeat(2,1fr)]"
      >
        <SessionCard
          v-for="(s, i) in live"
          :key="s.sessionId"
          class="enter"
          :style="{ '--i': i }"
          :session="s"
          :now="now"
          @open="emit('select', s.sessionId)"
        />
      </div>
    </section>

    <section v-if="overview" class="mt-6">
      <div class="mb-4 flex flex-wrap items-center gap-3">
        <h2 class="flex items-center gap-2.5 text-[20px] font-semibold tracking-tight">
          <History :size="20" :stroke-width="2" class="history-rewind text-ink-2" /> Zuletzt
        </h2>
        <label class="relative ml-auto w-80 max-w-full">
          <Search :size="16" class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-ink-3" />
          <input
            v-model="query"
            type="search"
            placeholder="Titel, Ordner oder Prompt suchen"
            aria-label="Sessions durchsuchen"
            class="w-full rounded-lg border border-line bg-surface py-2 pr-3 pl-9 text-[14px] placeholder:text-ink-3 focus:border-series-1 focus:outline-none"
          />
        </label>
      </div>

      <div class="card overflow-hidden">
        <p v-if="!filtered.length" class="px-5 py-8 text-center text-ink-3">Keine Session passt zu „{{ query }}“.</p>
        <template v-for="g in groups" :key="g.day">
          <div class="border-b border-line bg-surface-2/60 px-5 py-2 text-[12px] font-semibold tracking-wide text-ink-3 uppercase">
            {{ g.day }}
          </div>
          <button
            v-for="s in g.items"
            :key="s.sessionId"
            class="flex w-full items-center gap-4 border-b border-line px-5 py-3 text-left transition-colors duration-150 last:border-b-0 hover:bg-surface-2"
            @click="emit('select', s.sessionId)"
          >
            <span class="num w-12 shrink-0 text-[13px] text-ink-3">{{ time(s.lastT) }}</span>
            <span class="min-w-0 flex-1">
              <span class="block truncate font-medium">{{ headline(s).text || 'Ohne Titel' }}</span>
              <span class="block truncate text-[13px] text-ink-3" :title="s.firstPrompt">{{ s.firstPrompt }}</span>
            </span>
            <span class="max-w-[30%] shrink-0 truncate rounded-md bg-surface-2 px-2 py-0.5 font-mono text-[12px] text-ink-2" :title="s.cwd">
              {{ projectName(s.cwd) }}
            </span>
            <span class="flex w-28 shrink-0 items-center gap-1.5 truncate text-[13px] text-ink-3" :title="accountInfo(s.account).email ?? ''">
              <UserRound :size="14" :stroke-width="1.75" :style="{ color: accountInfo(s.account).color }" /> {{ s.account }}
            </span>
          </button>
        </template>
        <button
          v-if="!query && filtered.length > COLLAPSED"
          class="flex w-full items-center justify-center gap-1.5 border-t border-line py-3 text-[14px] font-medium text-ink-2 transition-colors duration-150 hover:bg-surface-2 hover:text-ink"
          :aria-expanded="expanded"
          @click="expanded = !expanded"
        >
          {{ expanded ? 'Weniger anzeigen' : `Alle ${filtered.length} anzeigen` }}
          <ChevronDown :size="16" class="transition-transform duration-200" :class="{ 'rotate-180': expanded }" />
        </button>
      </div>
    </section>
  </div>
</template>
