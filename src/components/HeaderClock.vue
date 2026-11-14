<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

const TICK_MS = 1000
const clockFmt = new Intl.DateTimeFormat('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: true })
const dateFmt = new Intl.DateTimeFormat('de-DE', { weekday: 'long', day: 'numeric', month: 'long' })

/** compact: nur die Uhrzeit, für die schmale Leiste der Detailansicht */
defineProps<{ compact?: boolean }>()

const now = ref(new Date())
let timer: number | undefined

// Eigener Sekundentakt: das Dashboard pollt nur alle 2 s, die Sekunden würden sonst springen.
onMounted(() => (timer = window.setInterval(() => (now.value = new Date()), TICK_MS)))
onBeforeUnmount(() => window.clearInterval(timer))
</script>

<template>
  <div class="flex flex-col items-end leading-none">
    <time class="num font-display font-bold tracking-tight text-ink" :class="compact ? 'text-[18px]' : 'text-[24px]'" :datetime="now.toISOString()">
      {{ clockFmt.format(now) }}
    </time>
    <span v-if="!compact" class="mt-1.5 text-[12px] text-ink-3">{{ dateFmt.format(now) }}</span>
  </div>
</template>
