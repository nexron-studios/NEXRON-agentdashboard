<script setup lang="ts">
// prompt-kit „terminal“-Loader als Icon: Eingabezeile mit Cursor, der blinkt, solange ein Befehl läuft.
withDefaults(defineProps<{ size?: number; running?: boolean }>(), { size: 16, running: true })
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class="shrink-0">
    <rect x="2.5" y="3.5" width="19" height="17" rx="3" />
    <path d="m7 9.5 3 2.5-3 2.5" :class="{ 'prompt-nudge': running }" />
    <path d="M12.5 15h4.5" :class="{ 'caret-blink': running }" />
  </svg>
</template>
