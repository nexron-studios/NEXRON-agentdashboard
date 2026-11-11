<script setup lang="ts">
import { computed } from 'vue'
import { Bot, CircleCheck, CircleX, Code, LayoutPanelTop, LoaderCircle, MessageSquareText, SquareTerminal, UserRound, Wrench } from 'lucide-vue-next'
import type { SessionRow } from '../api'
import { ago, awaitsYou, clock, date, duration, headline, modelName, needsApproval, PENDING_TITLE, plainPrompt, tokens } from '../format'
import EmptySession from './EmptySession.vue'
import FolderLink from './FolderLink.vue'
import SessionClear from './SessionClear.vue'
import SessionOpen from './SessionOpen.vue'
import StatusBadge from './StatusBadge.vue'
import { accountInfo } from '../store'

// `rank`: Platz in der Fertig-Reihenfolge (1 = zuletzt fertig geworden), wie die Chips über den Karten
const props = defineProps<{ session: SessionRow; now: number; rank?: number }>()
const emit = defineEmits<{ open: [] }>()
const s = computed(() => props.session)
const waiting = computed(() => needsApproval(s.value.status))
const fresh = computed(() => !s.value.title && !s.value.lastPrompt)
const yourTurn = computed(() => awaitsYou(s.value.status) && !fresh.value)
const working = computed(() => s.value.status === 'busy' || s.value.status === 'shell')
const AGENT_ICON = { running: LoaderCircle, done: CircleCheck, failed: CircleX } as const
const agentIcon = (st: string) => AGENT_ICON[st as keyof typeof AGENT_ICON] ?? CircleCheck
const ctxPct = computed(() => (s.value.context ? Math.min(100, (s.value.context.tokens / s.value.context.window) * 100) : 0))
const head = computed(() => headline(s.value))
const acc = computed(() => accountInfo(s.value.account))
// „Gestartet mit“ nur, solange die Session beim ersten Thema ist und die erste Anfrage etwas anderes sagt als die letzte
const showFirst = computed(() => !head.value.switched && s.value.firstPrompt && s.value.firstPrompt !== s.value.lastPrompt)
// Eine Zeile für den Ursprung: „Gestartet als“ (Titel nach Themenwechsel) oder „Gestartet mit“ (erste Anfrage)
const origin = computed(() => {
  if (head.value.startedAs) return { label: 'Gestartet als', text: `„${head.value.startedAs}“`, tip: `Titel von Claude Code: ${head.value.startedAs}` }
  if (showFirst.value) return { label: 'Gestartet mit:', text: plainPrompt(s.value.firstPrompt), tip: s.value.firstPrompt }
  return null
})
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
  <!-- Jede Karte gleich aufgebaut, damit Reihen ruhig wirken: Kopf (wer + Status), Titel mit einer
       Infozeile, Inhalt, und unten immer an derselben Stelle die beiden Buttons (Ordner, Zum Chat). -->
  <article
    class="card press group relative flex flex-col p-5 hover:border-line-strong hover:bg-surface-2/60"
    :class="{ 'border-critical/50!': waiting, 'border-good/35!': yourTurn }"
  >
    <div class="flex items-center gap-3">
      <div class="flex min-w-0 flex-1 items-center gap-2.5 text-[14px]">
        <span
          class="flex min-w-0 shrink-0 items-center gap-1.5"
          :title="s.herdr ? `herdr: Workspace „${s.herdr.workspace}“, Tab „${s.herdr.tab}“` : 'Name aus der Claude-Session (nicht in herdr gefunden)'"
        >
          <component :is="s.herdr ? LayoutPanelTop : SquareTerminal" :size="15" :stroke-width="2" class="shrink-0 text-ink-3" />
          <span class="truncate">
            <span v-if="s.herdr?.workspace" class="text-ink-3">{{ s.herdr.workspace }} › </span>
            <span class="font-semibold text-ink">{{ s.herdr?.tab || s.name || 'Session' }}</span>
          </span>
        </span>
        <span v-if="s.account" class="flex min-w-0 items-center gap-1 text-[13px] text-ink-3" :title="acc.email ? `Claude-Account: ${acc.email}` : 'Claude-Profil'">
          <UserRound :size="13" :stroke-width="2" class="shrink-0" :style="{ color: acc.color }" /> <span class="truncate">{{ s.account }}</span>
        </span>
        <span v-if="s.entrypoint === 'claude-vscode'" class="flex min-w-0 items-center gap-1 text-[13px] text-ink-3" title="Läuft in VS Code">
          <Code :size="13" :stroke-width="2" class="shrink-0" /> <span class="truncate">VS Code</span>
        </span>
      </div>
      <StatusBadge :status="s.status" :fresh="fresh" size="sm" />
    </div>

    <!-- Jeder Textbereich hat eine feste Zeilenzahl: Ändert sich der Text, bleibt die Karte gleich hoch. -->
    <h3 class="mt-4 truncate text-[17px] leading-snug font-semibold tracking-tight" :class="{ 'text-ink-3': fresh || head.pending }">
      <button class="card-link text-left" :title="fresh || head.pending ? undefined : head.text" @click="emit('open')">
        {{ fresh ? 'Neue Session' : head.pending ? PENDING_TITLE : head.text }}
      </button>
    </h3>

    <!-- Eine ruhige Infozeile statt einzelner Chips. Sie bricht nie um: Sobald „vor 55 s“ zu „vor 1 min“
         wird, würde sie sonst bei jedem Tick anders umbrechen und die ganze Karte springen lassen. -->
    <p class="mt-1 flex min-w-0 items-center gap-x-2 overflow-hidden text-[13px] whitespace-nowrap text-ink-3">
      <template v-if="yourTurn && s.finishedAt">
        <span class="flex shrink-0 items-center gap-1.5 font-semibold text-good" :title="`Fertig geworden ${date(s.finishedAt)}`">
          <span
            v-if="rank"
            class="num grid h-4.5 min-w-4.5 place-items-center rounded-full px-1 text-[11px] font-bold"
            :class="rank === 1 ? 'bg-good text-app' : 'bg-surface-3 text-ink-2'"
            :title="`Platz ${rank} der fertigen Sessions`"
          >{{ rank }}</span>
          fertig {{ clock(s.finishedAt, now) }}
        </span>
        <span>{{ ago(s.finishedAt, now) }}</span>
      </template>
      <span v-else-if="s.lastT">{{ fresh ? 'gestartet' : 'aktiv' }} {{ ago(s.lastT, now) }}</span>
      <template v-if="s.prompts">
        <span aria-hidden="true">·</span>
        <span class="truncate">{{ s.prompts }} {{ s.prompts === 1 ? 'Anfrage' : 'Anfragen' }}</span>
      </template>
    </p>
    <!-- Kontext als eigene Zeile über die volle Kartenbreite, damit die Leiste in jeder Karte gleich lang ist -->
    <div
      v-if="s.context"
      class="mt-2 flex items-center gap-2.5 text-[13px] text-ink-3"
      :title="`${s.context.tokens.toLocaleString('de-DE')} von ${s.context.window.toLocaleString('de-DE')} Tokens im Kontext`"
    >
      <span class="shrink-0" title="Modell der letzten Antwort">{{ modelName(s.lastModel) }}</span>
      <span class="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-surface-3">
        <span class="bar-grow block h-full rounded-full" :class="ctxPct > 85 ? 'bg-warning' : 'bg-series-1'" :style="{ width: ctxPct + '%' }" />
      </span>
      <span class="num shrink-0">{{ tokens(s.context.tokens) }}</span>
    </div>
    <p v-if="!fresh" class="mt-1 truncate text-[13px] text-ink-3" :title="origin?.tip">
      <template v-if="origin"><span class="font-medium text-ink-2">{{ origin.label }}</span> {{ origin.text }}</template>
      <template v-else>&nbsp;</template>
    </p>

    <EmptySession v-if="fresh" :vscode="s.entrypoint === 'claude-vscode'" />

    <div v-else class="mt-4 space-y-2.5 border-t border-line pt-4">
      <div class="flex gap-2.5 text-[14px]">
        <MessageSquareText :size="16" :stroke-width="1.75" class="mt-0.5 shrink-0 text-ink-3" :class="{ invisible: !showLast }" />
        <p class="line-clamp-2 h-[2lh] [overflow-wrap:anywhere] text-ink" :title="showLast ? s.lastPrompt : undefined">
          {{ showLast ? plainPrompt(s.lastPrompt) : '' }}
        </p>
      </div>
      <div class="flex h-[1lh] gap-2.5 text-[13px]" :class="waiting ? 'text-ink' : 'text-ink-3'">
        <template v-if="s.lastAction">
          <component :is="action.said ? Bot : Wrench" :size="16" :stroke-width="1.75" class="mt-px shrink-0" :class="{ 'text-critical': waiting }" />
          <p class="min-w-0 truncate" :class="{ shimmer: working && !action.said }" :title="action.text">
            <span v-if="waiting" class="font-semibold text-critical">Fragt nach: </span>
            <span v-else-if="action.said" class="font-medium text-ink-2">Claude: </span>{{ action.text }}
          </p>
        </template>
      </div>
    </div>

    <div v-if="s.agentsTotal" class="mt-4 border-t border-line pt-4">
      <p class="flex items-center gap-2 text-[13px] font-semibold text-ink-2">
        <Bot :size="15" :stroke-width="2" class="text-series-3" />
        {{ s.agentsTotal }} {{ s.agentsTotal === 1 ? 'Subagent' : 'Subagents' }}
        <span v-if="s.agentsRunning" class="font-medium text-warning">· {{ s.agentsRunning }} {{ s.agentsRunning === 1 ? 'läuft' : 'laufen' }}</span>
      </p>
      <!-- Höchstens vier Zeilen hoch, der Rest scrollt; über dem Klickfeld der Karte, damit das Scrollrad ankommt -->
      <ul class="relative z-10 mt-2 max-h-[calc(4lh+3*--spacing(1.5))] space-y-1.5 scroll-thin overflow-y-auto overscroll-contain pr-1.5 text-[13px]">
        <li v-for="a in s.agents" :key="a.id" class="flex items-center gap-2 text-[13px]">
          <component
            :is="agentIcon(a.status)"
            :size="14"
            :stroke-width="2.25"
            class="shrink-0"
            :class="{ 'spin text-warning': a.status === 'running', 'text-good': a.status === 'done', 'text-critical': a.status === 'failed', 'text-ink-3': a.status === 'stopped' }"
          />
          <span class="shrink-0 rounded bg-series-7/20 px-1.5 text-[12px] font-medium">{{ a.agentType }}</span>
          <span class="min-w-0 flex-1 truncate" :class="a.status === 'running' ? 'shimmer text-ink' : 'text-ink-3'">{{ a.description }}</span>
          <span class="num shrink-0 text-[12px] text-ink-3">{{ duration((a.finished ?? now) - a.started) }}</span>
        </li>
      </ul>
    </div>

    <div class="mt-auto flex items-center gap-2 pt-5">
      <FolderLink :path="s.cwd" class="min-w-0" />
      <div v-if="s.live" class="ml-auto flex shrink-0 items-center gap-2">
        <SessionClear v-if="s.herdr" :herdr="s.herdr" :status="s.status" />
        <SessionOpen :herdr="s.herdr" :pid="s.pid" :cwd="s.cwd" :entrypoint="s.entrypoint" />
      </div>
    </div>
  </article>
</template>
