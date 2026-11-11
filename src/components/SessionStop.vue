<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { Square } from 'lucide-vue-next'
import { stopHerdr } from '../api'

// Stoppen wie Esc im Terminal. Zwei Klicks statt Dialog: Die Karte ist überall klickbar, ein einzelner
// Fehlklick soll keine laufende Aufgabe abbrechen, ein Dialog wäre für eine Sekunde Entscheidung zu schwer.
const props = defineProps<{ paneId: string }>()
const CONFIRM_MS = 3000
const armed = ref(false)
const failed = ref('')
let timer: ReturnType<typeof setTimeout> | undefined

const disarm = () => {
  armed.value = false
  clearTimeout(timer)
}

const handleClick = async () => {
  if (!armed.value) {
    armed.value = true
    failed.value = ''
    timer = setTimeout(disarm, CONFIRM_MS)
    return
  }
  disarm()
  try {
    await stopHerdr(props.paneId)
  } catch (e) {
    failed.value = String(e)
  }
}

onBeforeUnmount(disarm)
</script>

<template>
  <button
    class="press relative z-10 flex shrink-0 items-center gap-1 rounded-md border px-1.5 text-[12px] leading-4 font-semibold"
    :class="
      armed || failed
        ? 'border-critical/60 bg-critical/12 text-critical'
        : 'border-line-strong text-ink-2 hover:border-critical/60 hover:text-critical'
    "
    :title="failed || (armed ? 'Noch einmal klicken, um Claude zu stoppen' : 'Claude stoppen (wie Esc im Terminal)')"
    @click.stop="handleClick"
  >
    <Square :size="10" :stroke-width="3" class="fill-current" aria-hidden="true" />
    {{ armed ? 'Wirklich?' : 'Stopp' }}
  </button>
</template>
