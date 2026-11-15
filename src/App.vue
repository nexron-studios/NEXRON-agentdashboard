<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ArrowLeft } from 'lucide-vue-next'
import { fetchOverview, followInHerdr, type Overview } from './api'
import { awaitsYou, isBusy, needsApproval, usePolling } from './format'
import { herdrNotice, machine, machineId, machines, profiles } from './store'
import ClawdLogo from './components/ClawdLogo.vue'
import HeaderClock from './components/HeaderClock.vue'
import HeaderStats from './components/HeaderStats.vue'
import NexronLogo from './components/NexronLogo.vue'
import PixelWord from './components/PixelWord.vue'
import QuotaMeter from './components/QuotaMeter.vue'
import SessionList from './components/SessionList.vue'
import SessionDetail from './components/SessionDetail.vue'
import TitleTooltips from './components/TitleTooltips.vue'

const RECENT = 40

const overview = ref<Overview | null>(null)
const error = ref('')
const selected = ref<string | null>(null)

const { now, refresh } = usePolling(async () => {
  try {
    const next = await fetchOverview(RECENT, machineId.value)
    machines.value = next.machines
    // Antwort zu einem Rechner, von dem inzwischen weggeschaltet wurde: verwerfen, der nächste Abruf läuft schon
    if (next.machine !== machineId.value && next.machines.some((m) => m.id === machineId.value)) return
    overview.value = next
    profiles.value = next.profiles
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}, 2000)

// Die Karten des vorigen Rechners verschwinden sofort; bis die neuen da sind, stehen Platzhalter
const selectMachine = (id: string) => {
  if (id === machineId.value) return
  machineId.value = id
  selected.value = null
  overview.value = null
  refresh()
  // herdr in Ghostty zieht mit; klappt das nicht, steht der Grund neben dem Umschalter
  herdrNotice.value = ''
  followInHerdr(id).catch((e) => (herdrNotice.value = String(e)))
}

const counts = computed(() => {
  const c = { approval: 0, busy: 0, waiting: 0 }
  for (const s of overview.value?.live ?? []) {
    if (needsApproval(s.status)) c.approval++
    else if (isBusy(s.status)) c.busy++
    else if (awaitsYou(s.status) && s.prompts > 0) c.waiting++
  }
  return c
})

const OS_NAMES: Record<string, string> = { windows: 'Windows', macos: 'macOS', linux: 'Linux' }

// Subtitle as on the homeserver: which computer, which system, which profiles
const subtitle = computed(() => {
  const m = machine.value
  if (!m) return error.value ? 'Nicht erreichbar' : 'Verbindet …'
  const parts = [m.label]
  if (m.host && m.host !== m.label) parts.push(m.host)
  if (OS_NAMES[m.os]) parts.push(OS_NAMES[m.os])
  // Right after a switch the profiles still belong to the previous machine until its overview arrives
  const own = overview.value?.machine === m.id ? overview.value.profiles : []
  if (own.length) parts.push(own.map((p) => p.label).join(' + '))
  return parts.join(' · ')
})

const liveTitle = computed(
  () => `Aktualisiert alle 2 Sekunden. Profile: ${profiles.value.map((p) => p.label).join(', ')}${overview.value?.herdrAvailable ? ' · herdr verbunden' : ''}`,
)

const onKey = (e: KeyboardEvent) => {
  // ⌘1, ⌘2 … (Windows: Strg) wählt den Rechner in der Reihenfolge des Umschalters
  const index = Number(e.key) - 1
  if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && machines.value.length > 1 && machines.value[index]) {
    e.preventDefault()
    selectMachine(machines.value[index].id)
    return
  }
  // Esc in einem offenen Dialog schließt nur den Dialog, nicht auch noch die Detailansicht
  if (e.key !== 'Escape' || document.querySelector('[role="alertdialog"]')) return
  selected.value = null
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- Detail view: a slim bar, so the history gets the room -->
    <header v-if="selected" class="@container flex h-16 shrink-0 items-center gap-4 border-b border-line bg-app px-8">
      <button
        class="press -ml-2 flex items-center gap-2 rounded-lg px-2 py-1.5 text-[14px] font-medium text-ink-2 hover:bg-surface-2 hover:text-ink"
        @click="selected = null"
      >
        <ArrowLeft :size="18" :stroke-width="2" /> Alle Sessions
      </button>
      <NexronLogo :height="24" class="ml-2 opacity-90" />
      <div class="ml-auto flex shrink-0 items-center gap-4">
        <QuotaMeter />
      </div>
      <HeaderStats :counts="counts" compact />
      <HeaderClock compact class="ml-2" />
      <span class="relative h-2.5 w-2.5 shrink-0 rounded-full" :class="error ? 'bg-critical' : 'bg-good live-ping'" :title="liveTitle" />
      <span class="sr-only">{{ error ? 'Fehler' : 'Live' }}</span>
    </header>

    <!-- Overview: large header in the style of the homeserver dashboard -->
    <header v-else class="@container starfield relative shrink-0 border-b border-line px-8 pt-5 pb-6">
      <div class="flex items-start justify-between gap-6">
        <NexronLogo :height="34" />
        <div class="flex items-center gap-4" :title="liveTitle">
          <HeaderClock />
          <span class="relative h-3 w-3 shrink-0 rounded-full" :class="error ? 'bg-critical' : 'bg-good live-ping'" />
          <span class="sr-only">{{ error ? 'Fehler' : 'Live' }}</span>
        </div>
      </div>

      <div class="mt-5 flex flex-wrap items-end justify-between gap-x-10 gap-y-5">
        <div>
          <h1 class="flex flex-col items-start gap-2.5">
            <span class="hero-title text-ink">Agent<span class="hero-dash" aria-hidden="true" /></span>
            <PixelWord text="Dashboard" :height="40" />
          </h1>
          <p class="mt-3 flex items-center gap-2 text-[15px] font-semibold text-ink-3">
            <ClawdLogo :size="18" class="clawd-hop" />
            {{ subtitle }}
          </p>
        </div>
        <div class="flex flex-wrap items-center justify-end gap-x-6 gap-y-3">
          <QuotaMeter />
          <HeaderStats :counts="counts" />
        </div>
      </div>
    </header>

    <main class="min-h-0 flex-1 overflow-y-auto">
      <Transition name="view" mode="out-in">
        <SessionDetail v-if="selected" :key="selected" :session-id="selected" @back="selected = null" />
        <SessionList
          v-else
          :overview="overview"
          :error="error"
          :now="now"
          @select="selected = $event"
          @machine="selectMachine"
        />
      </Transition>
    </main>
    <TitleTooltips />
  </div>
</template>
