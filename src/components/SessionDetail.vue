<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  Activity,
  Bot,
  CalendarClock,
  ChevronDown,
  Coins,
  Cpu,
  Gauge,
  GitBranch,
  MessagesSquare,
  ShieldCheck,
  Timer,
  TriangleAlert,
  Wrench,
} from 'lucide-vue-next'
import { fetchDetail, type Detail } from '../api'
import { date, duration, headline, modelName, PENDING_TITLE, time, tokens, usd, usePolling } from '../format'
import FolderLink from './FolderLink.vue'
import SessionClear from './SessionClear.vue'
import SessionOpen from './SessionOpen.vue'
import SessionMeta from './SessionMeta.vue'
import StatusBadge from './StatusBadge.vue'
import Timeline from './Timeline.vue'

const props = defineProps<{ sessionId: string }>()
// Nach /clear läuft im selben Terminal eine neue Session; diese hier ist dann beendet, also zurück zur Übersicht
const emit = defineEmits<{ back: [] }>()

const detail = ref<Detail | null>(null)
const error = ref('')

const { now } = usePolling(async () => {
  try {
    detail.value = await fetchDetail(props.sessionId)
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}, 2000)

const s = computed(() => detail.value?.stats)
const status = computed(() => detail.value?.status ?? 'ended')
const head = computed(() => (s.value && detail.value ? headline({ ...s.value, topic: detail.value.topic, titlePending: detail.value.titlePending }) : null))

const ctx = computed(() => {
  const c = s.value?.context
  return c ? { ...c, pct: Math.min(100, (c.tokens / c.window) * 100) } : null
})

const tools = computed(() => Object.entries(s.value?.tools ?? {}).sort((a, b) => b[1] - a[1]))
const toolMax = computed(() => tools.value[0]?.[1] ?? 1)
const TOOL_ROWS = 12

const CATEGORIES = [
  { key: 'input', label: 'Input', color: 'var(--color-series-1)' },
  { key: 'output', label: 'Output', color: 'var(--color-series-2)' },
  { key: 'cacheRead', label: 'Cache lesen', color: 'var(--color-series-3)' },
  { key: 'cacheWrite', label: 'Cache schreiben', color: 'var(--color-series-4)' },
] as const

const models = computed(() =>
  Object.entries(s.value?.models ?? {})
    .map(([id, u]) => ({ id, ...u, total: u.input + u.output + u.cacheRead + u.cacheWrite, cost: s.value?.cost?.byModel[id] }))
    .sort((a, b) => b.total - a.total),
)
const totals = computed(() => {
  const t = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }
  for (const m of models.value) for (const c of CATEGORIES) t[c.key] += m[c.key]
  return t
})
const totalTokens = computed(() => CATEGORIES.reduce((sum, c) => sum + totals.value[c.key], 0))

const PROMPTS_COLLAPSED = 4
const showAllPrompts = ref(false)
const openPrompts = ref(new Set<number>())
const prompts = computed(() => [...(s.value?.promptLog ?? [])].reverse())
const shownPrompts = computed(() => (showAllPrompts.value ? prompts.value : prompts.value.slice(0, PROMPTS_COLLAPSED)))
const togglePrompt = (t: number) => {
  const next = new Set(openPrompts.value)
  if (!next.delete(t)) next.add(t)
  openPrompts.value = next
}

const agentTypes = computed(() => {
  const counts = new Map<string, number>()
  for (const a of s.value?.agents ?? []) counts.set(a.agentType, (counts.get(a.agentType) ?? 0) + 1)
  return [...counts.entries()].sort((a, b) => b[1] - a[1])
})
const AGENT_STATUS: Record<string, { label: string; cls: string }> = {
  running: { label: 'läuft', cls: 'text-warning' },
  done: { label: 'fertig', cls: 'text-good' },
  failed: { label: 'fehlgeschlagen', cls: 'text-critical' },
  stopped: { label: 'gestoppt', cls: 'text-ink-3' },
}
const agentStatus = (st: string) => AGENT_STATUS[st] ?? { label: st, cls: 'text-ink-3' }

const errors = computed(() => [...(s.value?.errors ?? [])].reverse().slice(0, 40))
</script>

<template>
  <div class="mx-auto max-w-[1680px] px-8 pt-8 pb-12">
    <p v-if="error" class="flex items-center gap-2 rounded-xl border border-critical/40 bg-critical/10 px-4 py-3">
      <TriangleAlert :size="18" class="shrink-0 text-critical" /> {{ error }}
    </p>
    <div v-if="!s && !error" class="space-y-4">
      <div class="h-10 w-2/3 animate-pulse rounded-lg bg-surface" />
      <div class="grid gap-4 lg:grid-cols-2"><div class="card h-72 animate-pulse" /><div class="card h-72 animate-pulse" /></div>
    </div>

    <template v-if="s && detail">
      <header class="enter" style="--i: 0">
        <div class="flex flex-wrap items-center gap-3">
          <StatusBadge :status="status" :fresh="!s.prompts" />
          <SessionMeta size="lg" :herdr="detail.herdr" :account="detail.account" :entrypoint="detail.live?.entrypoint ?? ''" :name="detail.live?.name ?? ''" />
          <SessionOpen
            v-if="detail.live"
            :herdr="detail.herdr"
            :pid="detail.live.pid"
            :cwd="detail.live.cwd ?? s.cwd"
            :entrypoint="detail.live.entrypoint ?? ''"
          />
          <SessionClear v-if="detail.live && detail.herdr" :herdr="detail.herdr" :status="status" @cleared="emit('back')" />
        </div>
        <h1 class="mt-3 text-[28px] leading-tight font-semibold tracking-tight" :class="{ 'text-ink-3': head?.pending }">
          {{ head?.pending ? PENDING_TITLE : head?.text || sessionId.slice(0, 8) }}
        </h1>
        <p v-if="head?.startedAs" class="mt-1 text-[14px] text-ink-3">Gestartet als „{{ head.startedAs }}“</p>
        <div class="mt-3 flex flex-wrap items-center gap-x-5 gap-y-2 text-[14px] text-ink-2">
          <FolderLink :path="s.cwd" :size="16" />
          <span v-if="s.gitBranch && s.gitBranch !== 'HEAD'" class="flex items-center gap-1.5"><GitBranch :size="16" :stroke-width="1.75" class="text-ink-3" /> {{ s.gitBranch }}</span>
          <span class="flex items-center gap-1.5"><CalendarClock :size="16" :stroke-width="1.75" class="text-ink-3" /> {{ date(s.firstT) }}</span>
          <span class="num flex items-center gap-1.5"><Timer :size="16" :stroke-width="1.75" class="text-ink-3" /> {{ duration(s.lastT - s.firstT) }}</span>
          <span class="num flex items-center gap-1.5"><MessagesSquare :size="16" :stroke-width="1.75" class="text-ink-3" /> {{ s.prompts }} Anfragen · {{ s.turns }} Antworten</span>
          <span v-if="s.cost" class="num flex items-center gap-1.5"><Coins :size="16" :stroke-width="1.75" class="text-ink-3" /> ~{{ usd(s.cost.totalUsd) }}</span>
          <span v-if="s.lastModel" class="flex items-center gap-1.5"><Cpu :size="16" :stroke-width="1.75" class="text-ink-3" /> {{ modelName(s.lastModel) }}</span>
          <span v-if="s.permissionMode" class="flex items-center gap-1.5"><ShieldCheck :size="16" :stroke-width="1.75" class="text-ink-3" /> {{ s.permissionMode }}</span>
        </div>
        <p class="mt-2 font-mono text-[12px] text-ink-3">{{ s.cwd }} · {{ sessionId }}<template v-if="s.version"> · Claude Code {{ s.version }}</template></p>
      </header>

      <div class="mt-7 grid gap-4 lg:grid-cols-2">
        <section class="card enter p-6" style="--i: 1">
          <div class="flex items-center justify-between">
            <h2 class="flex items-center gap-2 text-[16px] font-semibold"><Gauge :size="18" :stroke-width="2" class="text-series-1" /> Kontextfenster</h2>
            <span v-if="ctx" class="rounded-md bg-surface-2 px-2 py-0.5 text-[12px] font-medium text-ink-2">{{ modelName(ctx.model) }}</span>
          </div>
          <template v-if="ctx">
            <div class="mt-5 flex items-baseline gap-2">
              <span class="num font-display text-[40px] leading-none font-semibold tracking-tight">{{ tokens(ctx.tokens) }}</span>
              <span class="num text-[15px] text-ink-3">von {{ tokens(ctx.window) }} · {{ ctx.pct.toFixed(0) }} %</span>
            </div>
            <div class="mt-4 h-3 overflow-hidden rounded-full bg-surface-3" :title="`${ctx.tokens.toLocaleString('de-DE')} Tokens im Kontext`">
              <div class="bar-grow h-full rounded-full" :class="ctx.pct > 85 ? 'bg-warning' : 'bg-series-1'" :style="{ width: ctx.pct + '%' }" />
            </div>
            <dl class="mt-6 grid grid-cols-3 gap-3">
              <div class="rounded-lg bg-surface-2 px-3 py-2.5">
                <dt class="text-[12px] text-ink-3">Frei</dt>
                <dd class="num mt-0.5 text-[17px] font-semibold">{{ tokens(Math.max(0, ctx.window - ctx.tokens)) }}</dd>
              </div>
              <div class="rounded-lg bg-surface-2 px-3 py-2.5">
                <dt class="text-[12px] text-ink-3">Komprimiert</dt>
                <dd class="num mt-0.5 text-[17px] font-semibold">{{ s.compactions }}×</dd>
              </div>
              <div class="rounded-lg bg-surface-2 px-3 py-2.5">
                <dt class="text-[12px] text-ink-3">Tool-Aufrufe</dt>
                <dd class="num mt-0.5 text-[17px] font-semibold">{{ s.toolCalls }}</dd>
              </div>
            </dl>
            <p class="mt-4 text-[12px] text-ink-3">Geschätzt aus der letzten Antwort (Input + Cache). Fenster 1M, sobald die Session je über 200K lag.</p>
          </template>
          <p v-else class="mt-4 text-ink-3">Noch keine Antwort im Transkript.</p>
        </section>

        <section class="card enter p-6" style="--i: 2">
          <h2 class="flex items-center gap-2 text-[16px] font-semibold"><Wrench :size="18" :stroke-width="2" class="text-series-1" /> Tool-Nutzung</h2>
          <p class="mt-1 text-[13px] text-ink-3">{{ tools.length }} Tools · {{ s.toolCalls }} Aufrufe</p>
          <div class="mt-4 space-y-1.5">
            <div
              v-for="[name, n] in tools.slice(0, TOOL_ROWS)"
              :key="name"
              class="grid grid-cols-[160px_1fr] items-center gap-3 rounded-md px-1.5 py-0.5 transition-colors duration-150 hover:bg-surface-2"
              :title="`${name}: ${n} Aufrufe`"
            >
              <span class="truncate font-mono text-[13px] text-ink-2">{{ name }}</span>
              <div class="flex items-center gap-2.5">
                <div class="bar-grow h-3 rounded-r-[4px] bg-series-1" :style="{ width: Math.max(1, (n / toolMax) * 82) + '%' }" />
                <span class="num text-[13px] text-ink-2">{{ n }}</span>
              </div>
            </div>
            <p v-if="tools.length > TOOL_ROWS" class="px-1.5 pt-1 text-[12px] text-ink-3">
              + {{ tools.length - TOOL_ROWS }} weitere mit zusammen {{ tools.slice(TOOL_ROWS).reduce((a, [, n]) => a + n, 0) }} Aufrufen
            </p>
            <p v-if="!tools.length" class="text-ink-3">Noch keine Tools benutzt.</p>
          </div>
        </section>
      </div>

      <section v-if="prompts.length" class="card enter mt-4 p-6" style="--i: 3">
        <h2 class="flex items-center gap-2 text-[16px] font-semibold">
          <MessagesSquare :size="18" :stroke-width="2" class="text-brand" /> Deine Anfragen
          <span class="num rounded-full bg-surface-2 px-2 text-[13px] font-medium text-ink-2">{{ prompts.length }}</span>
        </h2>
        <ol class="mt-4 space-y-2">
          <li v-for="p in shownPrompts" :key="p.t">
            <button
              class="flex w-full gap-4 rounded-lg border border-line bg-surface-2/50 px-4 py-3 text-left transition-colors duration-150 hover:border-line-strong hover:bg-surface-2"
              :aria-expanded="openPrompts.has(p.t)"
              @click="togglePrompt(p.t)"
            >
              <span class="num w-24 shrink-0 pt-0.5 text-[13px] text-ink-3">{{ date(p.t).replace(', ', ' ') }}</span>
              <span class="min-w-0 flex-1 text-[15px] whitespace-pre-line" :class="{ 'line-clamp-3': !openPrompts.has(p.t) }">{{ p.text }}</span>
              <ChevronDown :size="16" class="mt-1 shrink-0 text-ink-3 transition-transform duration-200" :class="{ 'rotate-180': openPrompts.has(p.t) }" />
            </button>
          </li>
        </ol>
        <button
          v-if="prompts.length > PROMPTS_COLLAPSED"
          class="mt-3 flex items-center gap-1.5 text-[14px] font-medium text-ink-2 hover:text-ink"
          :aria-expanded="showAllPrompts"
          @click="showAllPrompts = !showAllPrompts"
        >
          {{ showAllPrompts ? 'Weniger anzeigen' : `Alle ${prompts.length} Anfragen anzeigen` }}
          <ChevronDown :size="16" class="transition-transform duration-200" :class="{ 'rotate-180': showAllPrompts }" />
        </button>
      </section>

      <section class="card enter mt-4 p-6" style="--i: 4">
        <div class="flex items-center justify-between">
          <h2 class="flex items-center gap-2 text-[16px] font-semibold"><Coins :size="18" :stroke-width="2" class="text-series-4" /> Tokens &amp; Kosten</h2>
          <span class="num font-display text-[28px] leading-none font-semibold">{{ s.cost ? '~' + usd(s.cost.totalUsd) : tokens(totalTokens) + ' Tokens' }}</span>
        </div>
        <div class="mt-5 flex h-3.5 gap-[2px] overflow-hidden rounded-[4px]">
          <div
            v-for="c in CATEGORIES"
            v-show="totals[c.key] > 0"
            :key="c.key"
            class="h-full"
            :style="{ background: c.color, flexGrow: totals[c.key], minWidth: '3px' }"
            :title="`${c.label}: ${totals[c.key].toLocaleString('de-DE')} Tokens`"
          />
        </div>
        <div class="mt-3 flex flex-wrap gap-x-6 gap-y-1.5 text-[14px]">
          <span v-for="c in CATEGORIES" :key="c.key" class="flex items-center gap-2">
            <span class="h-3 w-3 rounded-[3px]" :style="{ background: c.color }" />
            <span class="text-ink-2">{{ c.label }}</span>
            <span class="num font-semibold">{{ tokens(totals[c.key]) }}</span>
          </span>
        </div>

        <table class="mt-6 w-full text-[14px]">
          <thead class="text-left text-[12px] text-ink-3">
            <tr>
              <th class="pb-2 font-medium">Modell</th>
              <th class="pb-2 text-right font-medium">Antworten</th>
              <th v-for="c in CATEGORIES" :key="c.key" class="pb-2 text-right font-medium">{{ c.label }}</th>
              <th class="pb-2 text-right font-medium">Kosten</th>
            </tr>
          </thead>
          <tbody class="num">
            <tr v-for="m in models" :key="m.id" class="border-t border-line">
              <td class="py-2 font-medium">{{ modelName(m.id) }}</td>
              <td class="py-2 text-right">{{ m.requests }}</td>
              <td v-for="c in CATEGORIES" :key="c.key" class="py-2 text-right">{{ tokens(m[c.key]) }}</td>
              <td class="py-2 text-right">{{ m.cost != null ? usd(m.cost) : '–' }}</td>
            </tr>
          </tbody>
        </table>
        <p class="mt-4 text-[12px] text-ink-3">
          <template v-if="s.cost">
            Kosten laut Claude Code zu API-Preisen, Stand der letzten Kostenmeldung im Transkript<template v-if="s.cost.linesAdded || s.cost.linesRemoved">
              · <span class="text-good">+{{ s.cost.linesAdded }}</span> / <span class="text-critical">−{{ s.cost.linesRemoved }}</span> Zeilen</template
            >.
          </template>
          <template v-else>Claude Code hat für diese Session noch keine Kosten ins Transkript geschrieben.</template>
          Im Abo nur rechnerisch.
        </p>
      </section>

      <section v-if="s.agents.length" class="card enter mt-4 p-6" style="--i: 5">
        <h2 class="flex items-center gap-2 text-[16px] font-semibold">
          <Bot :size="18" :stroke-width="2" class="text-series-3" /> Subagents
          <span class="num rounded-full bg-surface-2 px-2 text-[13px] font-medium text-ink-2">{{ s.agents.length }}</span>
        </h2>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <span v-for="[type, n] in agentTypes" :key="type" class="rounded-md bg-series-7/20 px-2 py-0.5 text-[12px] font-medium">
            {{ type }} <span class="text-ink-3">×{{ n }}</span>
          </span>
        </div>
        <div class="mt-4 space-y-1">
          <div
            v-for="a in [...s.agents].reverse()"
            :key="a.id"
            class="grid grid-cols-[auto_auto_1fr_auto_auto_auto] items-center gap-3 rounded-md px-1.5 py-1.5 text-[14px] transition-colors duration-150 hover:bg-surface-2"
          >
            <span class="rounded-md bg-series-7/20 px-2 py-0.5 text-[12px] font-medium">{{ a.agentType }}</span>
            <span class="rounded-md bg-surface-2 px-2 py-0.5 text-[12px] text-ink-3">{{ a.model ? modelName(a.model) : 'erbt Modell' }}</span>
            <span class="truncate" :title="a.description">{{ a.description }}</span>
            <span class="font-medium" :class="agentStatus(a.status).cls">{{ agentStatus(a.status).label }}</span>
            <span class="num w-20 text-right text-ink-3">{{ duration((a.finished ?? now) - a.started) }}</span>
            <span class="num w-12 text-right text-ink-3">{{ time(a.started) }}</span>
          </div>
        </div>
      </section>

      <section v-if="s.errorCount" class="card enter mt-4 border-critical/35! p-6" style="--i: 6">
        <h2 class="flex items-center gap-2 text-[16px] font-semibold">
          <TriangleAlert :size="18" :stroke-width="2" class="text-critical" /> Fehler
          <span class="num rounded-full bg-critical/15 px-2 text-[13px] font-medium">{{ s.errorCount }}</span>
        </h2>
        <div class="mt-4 max-h-96 space-y-1 overflow-y-auto">
          <div v-for="(e, i) in errors" :key="i" class="grid grid-cols-[130px_1fr_auto] gap-3 rounded-md px-1.5 py-1.5 text-[14px] transition-colors duration-150 hover:bg-critical/8">
            <span class="truncate font-mono text-[13px]">{{ e.source }}</span>
            <span class="truncate text-ink-2" :title="e.message">{{ e.message }}</span>
            <span class="num text-[13px] text-ink-3">{{ date(e.t) }}</span>
          </div>
        </div>
      </section>

      <section class="card enter mt-4 p-6" style="--i: 7">
        <h2 class="flex items-center gap-2 text-[16px] font-semibold"><Activity :size="18" :stroke-width="2" class="text-series-2" /> Zeitverlauf</h2>
        <Timeline class="mt-4" :stats="s" :now="now" :live="!!detail.live" />
      </section>
    </template>
  </div>
</template>
