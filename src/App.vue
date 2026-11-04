<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ArrowLeft, CircleAlert, CircleCheck, LoaderCircle } from 'lucide-vue-next'
import { fetchOverview, type Overview } from './api'
import { awaitsYou, isBusy, needsApproval, usePolling } from './format'
import { profiles } from './store'
import ClawdLogo from './components/ClawdLogo.vue'
import NexronLogo from './components/NexronLogo.vue'
import SessionList from './components/SessionList.vue'
import SessionDetail from './components/SessionDetail.vue'
import TitleTooltips from './components/TitleTooltips.vue'

const RECENT = 40

const overview = ref<Overview | null>(null)
const error = ref('')
const selected = ref<string | null>(null)

const { now } = usePolling(async () => {
  try {
    overview.value = await fetchOverview(RECENT)
    profiles.value = overview.value.profiles
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}, 2000)

const counts = computed(() => {
  const c = { approval: 0, busy: 0, waiting: 0 }
  for (const s of overview.value?.live ?? []) {
    if (needsApproval(s.status)) c.approval++
    else if (isBusy(s.status)) c.busy++
    else if (awaitsYou(s.status) && s.prompts > 0) c.waiting++
  }
  return c
})

const onKey = (e: KeyboardEvent) => {
  // Esc in einem offenen Dialog schließt nur den Dialog, nicht auch noch die Detailansicht
  if (e.key !== 'Escape' || document.querySelector('[role="alertdialog"]')) return
  selected.value = null
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="flex h-full flex-col">
    <header class="sticky top-0 z-10 flex h-16 shrink-0 items-center gap-4 border-b border-line bg-app/85 px-8 backdrop-blur">
      <button
        v-if="selected"
        class="press -ml-2 flex items-center gap-2 rounded-lg px-2 py-1.5 text-[14px] font-medium text-ink-2 hover:bg-surface-2 hover:text-ink"
        @click="selected = null"
      >
        <ArrowLeft :size="18" :stroke-width="2" /> Alle Sessions
      </button>
      <div v-else class="flex items-center gap-3">
        <ClawdLogo :size="20" class="clawd-hop" />
        <span class="font-display text-[17px] font-semibold tracking-tight"><span class="text-brand">Agent</span> Dashboard</span>
        <span class="h-5 w-px bg-line-strong" aria-hidden="true" />
        <NexronLogo :height="30" />
      </div>

      <div class="ml-auto flex items-center gap-2 text-[13px]">
        <span
          class="flex items-center gap-1.5 rounded-full border px-3 py-1"
          :class="counts.approval ? 'border-critical/40 bg-critical/12 text-ink' : 'border-line text-ink-3'"
          title="Sessions, die auf eine Freigabe oder Auswahl warten"
        >
          <CircleAlert :size="15" :stroke-width="2.25" :class="counts.approval ? 'wiggle text-critical' : ''" />
          <b class="num">{{ counts.approval }}</b> Freigabe
        </span>
        <span class="flex items-center gap-1.5 rounded-full border border-line px-3 py-1 text-ink-2">
          <LoaderCircle :size="15" :stroke-width="2.25" class="text-warning" :class="{ spin: counts.busy }" />
          <b class="num text-ink">{{ counts.busy }}</b> arbeiten
        </span>
        <span
          class="flex items-center gap-1.5 rounded-full border px-3 py-1"
          :class="counts.waiting ? 'border-good/35 bg-good/10 text-ink' : 'border-line text-ink-2'"
          title="Sessions, die ihre Aufgabe abgeschlossen haben"
        >
          <CircleCheck :size="15" :stroke-width="2.25" class="text-good" :class="{ beat: counts.waiting }" />
          <b class="num">{{ counts.waiting }}</b> abgeschlossen
        </span>
        <span
          class="ml-2 flex items-center gap-2 text-[12px] text-ink-3"
          :title="`Aktualisiert alle 2 Sekunden. Profile: ${profiles.map((p) => p.label).join(', ')}${overview?.herdrAvailable ? ' · herdr verbunden' : ''}`"
        >
          <span class="relative h-2 w-2 rounded-full" :class="error ? 'bg-critical' : 'bg-good live-ping'" />
          {{ error ? 'Fehler' : 'Live' }}
        </span>
      </div>
    </header>

    <main class="min-h-0 flex-1 overflow-y-auto">
      <Transition name="view" mode="out-in">
        <SessionDetail v-if="selected" :key="selected" :session-id="selected" @back="selected = null" />
        <SessionList v-else :overview="overview" :error="error" :now="now" @select="selected = $event" />
      </Transition>
    </main>
    <TitleTooltips />
  </div>
</template>
