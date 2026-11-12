<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, SquareArrowOutUpRight } from 'lucide-vue-next'
import { focusHerdr, openSession, type HerdrPlace } from '../api'
import { isRemote, machine } from '../store'

// „Öffnen“ für jede laufende Session: mit herdr springt es in den Tab, in VS Code in das Fenster
// mit dem Projekt, sonst holt es das Terminal-Fenster nach vorne, in dem Claude läuft.
// Auf einem anderen Rechner geht das nur über herdr; ein Fenster dort lässt sich von hier nicht holen.
// `compact`: nur das Icon, für schmale Spalten (Text steht dann im Tooltip und für Screenreader)
const props = defineProps<{ herdr: HerdrPlace | null; pid: number | null; cwd: string; entrypoint: string; compact?: boolean }>()
const opened = ref(false)
const failed = ref('')

const unreachable = computed(() => isRemote.value && !props.herdr)
const hint = computed(() =>
  props.herdr
    ? `Tab „${props.herdr.tab}“ in herdr${props.herdr.machine ? ` auf ${machine.value?.label}` : ''} in den Vordergrund holen`
    : unreachable.value
      ? `Läuft auf ${machine.value?.label} ohne herdr-Tab, von hier aus nicht erreichbar`
      : props.entrypoint === 'claude-vscode'
        ? 'VS-Code-Fenster mit diesem Projekt in den Vordergrund holen'
        : 'Terminal-Fenster dieser Session in den Vordergrund holen',
)

async function open() {
  try {
    if (props.herdr) await focusHerdr(props.herdr)
    else await openSession(props.pid, props.cwd, props.entrypoint)
    failed.value = ''
    opened.value = true
    window.setTimeout(() => (opened.value = false), 1600)
  } catch (e) {
    failed.value = String(e)
  }
}
</script>

<template>
  <button
    class="press relative z-10 flex shrink-0 items-center gap-1.5 rounded-lg bg-ink text-[13px] font-semibold text-app shadow-sm shadow-black/30 hover:bg-white"
    :class="[compact ? 'p-1.5' : 'px-3 py-1.5', { 'bg-critical! text-white!': failed, 'cursor-not-allowed opacity-40': unreachable }]"
    :title="failed || hint"
    :aria-disabled="unreachable"
    @click.stop="!unreachable && open()"
  >
    <span class="relative inline-flex h-3.5 w-3.5">
      <SquareArrowOutUpRight :size="14" :stroke-width="2" class="icon-swap absolute" :class="{ 'icon-out': opened }" />
      <Check :size="14" :stroke-width="2.5" class="icon-swap absolute" :class="{ 'icon-out': !opened }" />
    </span>
    <span aria-live="polite" :class="{ 'sr-only': compact }">{{ opened ? 'Geöffnet' : failed ? 'Fehlgeschlagen' : 'Zum Chat' }}</span>
  </button>
</template>
