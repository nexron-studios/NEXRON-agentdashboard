<script setup lang="ts">
import { ref } from 'vue'
import { Check, SquareArrowOutUpRight } from 'lucide-vue-next'
import { focusHerdr, type HerdrPlace } from '../api'

const props = defineProps<{ herdr: HerdrPlace }>()
const opened = ref(false)
const failed = ref('')

async function open() {
  try {
    await focusHerdr(props.herdr.tabId)
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
    class="press relative z-10 flex shrink-0 items-center gap-1.5 rounded-lg border border-line-strong px-2.5 py-1 text-[13px] font-medium text-ink-2 hover:border-ink-3 hover:bg-surface-3 hover:text-ink"
    :title="failed || `Tab „${herdr.tab}“ in herdr in den Vordergrund holen`"
    @click.stop="open"
  >
    <span class="relative inline-flex h-3.5 w-3.5">
      <SquareArrowOutUpRight :size="14" :stroke-width="2" class="icon-swap absolute" :class="{ 'icon-out': opened }" />
      <Check :size="14" :stroke-width="2.25" class="icon-swap absolute text-good" :class="{ 'icon-out': !opened }" />
    </span>
    <span aria-live="polite">{{ opened ? 'Geöffnet' : failed ? 'Fehlgeschlagen' : 'Öffnen' }}</span>
  </button>
</template>
