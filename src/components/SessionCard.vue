<script setup lang="ts">
import { computed } from 'vue'
import { Bot, CircleCheck, CircleX, Clock, Cpu, Gauge, LoaderCircle, MessageSquareText, Sparkles, Wrench } from 'lucide-vue-next'
import type { SessionRow } from '../api'
import { ago, awaitsYou, duration, headline, modelName, needsApproval, plainPrompt, tokens } from '../format'
import EmptySession from './EmptySession.vue'
import FolderLink from './FolderLink.vue'
import SessionOpen from './SessionOpen.vue'
import SessionMeta from './SessionMeta.vue'
import StatusBadge from './StatusBadge.vue'

const props = defineProps<{ session: SessionRow; now: number }>()
const emit = defineEmits<{ open: [] }>()
const s = computed(() => props.session)
const waiting = computed(() => needsApproval(s.value.status))
const fresh = computed(() => !s.value.title && !s.value.lastPrompt)
const yourTurn = computed(() => awaitsYou(s.value.status) && !fresh.value)
const AGENT_ICON = { running: LoaderCircle, done: CircleCheck, failed: CircleX } as const
const agentIcon = (st: string) => AGENT_ICON[st as keyof typeof AGENT_ICON] ?? CircleCheck
const ctxPct = computed(() => (s.value.context ? Math.min(100, (s.value.context.tokens / s.value.context.window) * 100) : 0))
const head = computed(() => headline(s.value))
// „Gestartet mit“ nur, solange die Session beim ersten Thema ist und die erste Anfrage etwas anderes sagt als die letzte
const showFirst = computed(() => !head.value.switched && s.value.firstPrompt && s.value.firstPrompt !== s.value.lastPrompt)
// Stammt die Überschrift aus der letzten Anfrage, nicht noch einmal darunter wiederholen (voller Text in der Detailansicht)
const showLast = computed(() => {
  const last = plainPrompt(s.value.lastPrompt).toLowerCase()
  const start = head.value.text.replace(/…$/, '').toLowerCase()
  return !!last && !(head.value.switched && last.startsWith(start))
})
// Der letzte Schritt ist entweder ein Tool-Aufruf oder Text von Claude (vom Backend mit 💬 markiert).
const action = computed(() => {
  const a = s.value.lastAction
  const said = a.startsWith('💬 ')
  return { said, text: (said ? a.slice(3) : a).replace(/\*\*|`/g, '') }
})
</script>

<template>
  <article
    class="card press group relative flex flex-col p-5 hover:border-line-strong hover:bg-surface-2/60"
    :class="{ 'border-critical/50!': waiting, 'border-good/35!': yourTurn }"
  >
    <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
      <StatusBadge :status="s.status" :fresh="fresh" />
      <span v-if="s.lastT" class="ml-auto flex shrink-0 items-center gap-1.5 text-[13px] whitespace-nowrap text-ink-3">
        <Clock :size="14" :stroke-width="1.75" /> {{ ago(s.lastT, now) }}
      </span>
    </div>

    <!-- Öffnen sitzt in der Zeile mit herdr-Tab und Account, damit die Statuszeile nicht umbricht -->
    <div class="mt-3.5 flex items-start gap-2">
      <SessionMeta class="min-w-0 flex-1" :herdr="s.herdr" :account="s.account" :entrypoint="s.entrypoint" :name="s.name" />
      <SessionOpen v-if="s.live" :herdr="s.herdr" :pid="s.pid" :cwd="s.cwd" :entrypoint="s.entrypoint" />
    </div>

    <h3 class="mt-3 line-clamp-2 text-[18px] leading-snug font-semibold tracking-tight" :class="{ 'text-ink-3': fresh }">
      <button class="card-link text-left" @click="emit('open')">
        {{ fresh ? 'Neue Session' : head.text }}
      </button>
    </h3>
    <p v-if="!fresh && head.startedAs" class="mt-1 truncate text-[13px] text-ink-3" :title="`Titel von Claude Code: ${head.startedAs}`">
      Gestartet als „{{ head.startedAs }}“
    </p>

    <div class="mt-2 flex flex-wrap items-center gap-2 text-[13px]">
      <FolderLink :path="s.cwd" class="group-hover:bg-surface-3" />
      <span v-if="s.prompts" class="flex items-center gap-1.5 px-1 py-0.5 text-ink-3">
        <MessageSquareText :size="14" :stroke-width="1.75" /> {{ s.prompts }} {{ s.prompts === 1 ? 'Anfrage' : 'Anfragen' }}
      </span>
    </div>

    <EmptySession v-if="fresh" :vscode="s.entrypoint === 'claude-vscode'" />

    <div v-else class="mt-4 space-y-3 border-t border-line pt-4">
      <div v-if="showFirst" class="flex gap-2.5 text-[14px]">
        <Sparkles :size="16" :stroke-width="1.75" class="mt-0.5 shrink-0 text-ink-3" />
        <p class="line-clamp-2 text-ink-3" :title="s.firstPrompt">
          <span class="font-medium text-ink-2">Gestartet mit:</span> {{ s.firstPrompt }}
        </p>
      </div>
      <div v-if="showLast" class="flex gap-2.5 text-[14.5px]">
        <MessageSquareText :size="16" :stroke-width="1.75" class="mt-1 shrink-0 text-brand" />
        <p class="line-clamp-4 whitespace-pre-line text-ink" :title="s.lastPrompt">{{ s.lastPrompt }}</p>
      </div>
      <div v-if="s.lastAction" class="flex gap-2.5 text-[13px]" :class="waiting ? 'text-ink' : 'text-ink-3'">
        <component :is="action.said ? Bot : Wrench" :size="15" :stroke-width="1.75" class="mt-0.5 shrink-0" :class="{ 'text-critical': waiting }" />
        <p class="line-clamp-2 [overflow-wrap:anywhere]" :title="action.text">
          <span v-if="waiting" class="font-semibold text-critical">Fragt nach: </span>
          <span v-else-if="action.said" class="font-medium text-ink-2">Claude: </span>{{ action.text }}
        </p>
      </div>
    </div>

    <div v-if="s.agentsTotal" class="mt-4 border-t border-line pt-4">
      <p class="flex items-center gap-2 text-[13px] font-semibold text-ink-2">
        <Bot :size="15" :stroke-width="2" class="text-series-3" />
        {{ s.agentsTotal }} {{ s.agentsTotal === 1 ? 'Subagent' : 'Subagents' }}
        <span v-if="s.agentsRunning" class="font-medium text-warning">· {{ s.agentsRunning }} {{ s.agentsRunning === 1 ? 'läuft' : 'laufen' }}</span>
      </p>
      <ul class="mt-2 space-y-1.5">
        <li v-for="a in s.agents" :key="a.id" class="flex items-center gap-2 text-[13px]">
          <component
            :is="agentIcon(a.status)"
            :size="14"
            :stroke-width="2.25"
            class="shrink-0"
            :class="{ 'spin text-warning': a.status === 'running', 'text-good': a.status === 'done', 'text-critical': a.status === 'failed', 'text-ink-3': a.status === 'stopped' }"
          />
          <span class="shrink-0 rounded bg-series-7/20 px-1.5 text-[12px] font-medium">{{ a.agentType }}</span>
          <span class="min-w-0 flex-1 truncate" :class="a.status === 'running' ? 'text-ink' : 'text-ink-3'">{{ a.description }}</span>
          <span class="num shrink-0 text-[12px] text-ink-3">{{ duration((a.finished ?? now) - a.started) }}</span>
        </li>
      </ul>
    </div>

    <div v-if="s.context" class="mt-auto flex items-center gap-4 pt-5 text-[13px] text-ink-3">
      <span class="flex items-center gap-1.5" title="Modell der letzten Antwort"><Cpu :size="14" :stroke-width="1.75" /> {{ modelName(s.lastModel) }}</span>
      <span class="flex flex-1 items-center gap-2.5" :title="`${s.context.tokens.toLocaleString('de-DE')} von ${s.context.window.toLocaleString('de-DE')} Tokens im Kontext`">
        <Gauge :size="14" :stroke-width="1.75" />
        <span class="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-3">
          <span class="bar-grow block h-full rounded-full" :class="ctxPct > 85 ? 'bg-warning' : 'bg-series-1'" :style="{ width: ctxPct + '%' }" />
        </span>
        <span class="num">{{ tokens(s.context.tokens) }} / {{ tokens(s.context.window) }}</span>
      </span>
    </div>
  </article>
</template>
