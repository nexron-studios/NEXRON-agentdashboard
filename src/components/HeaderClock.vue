<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

const TICK_MS = 1000
const clockFmt = new Intl.DateTimeFormat('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: true })
const dateFmt = new Intl.DateTimeFormat('de-DE', { weekday: 'long', day: 'numeric', month: 'long' })

/** compact: the time only, for the slim bar of the detail view */
defineProps<{ compact?: boolean }>()

const now = ref(new Date())
let timer: number | undefined

// Own one-second tick: the dashboard polls every 2 s only, so the seconds would jump.
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
