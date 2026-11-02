<script setup lang="ts">
// Clawd, nachgebaut aus den Blockzeichen des Claude-Code-Startbildschirms (▐▛███▜▌ / ▝▜█████▛▘ / ▘▘ ▝▝).
// Ein Terminal-Viertelblock ist doppelt so hoch wie breit, deshalb sind die Pixel 1×2.
defineProps<{ size?: number }>()
</script>

<template>
  <svg :width="(size ?? 18) * 1.125" :height="size ?? 18" viewBox="1 0 16 10" shape-rendering="crispEdges" aria-label="Clawd" role="img">
    <g fill="#d77757"><rect x="3" y="0" width="12" height="2"/><rect x="3" y="2" width="2" height="2"/><rect x="6" y="2" width="6" height="2"/><rect x="13" y="2" width="2" height="2"/><rect x="1" y="4" width="16" height="2"/><rect x="3" y="6" width="12" height="2"/><rect x="4" y="8" width="1" height="2"/><rect x="6" y="8" width="1" height="2"/><rect x="11" y="8" width="1" height="2"/><rect x="13" y="8" width="1" height="2"/></g>
  </svg>
</template>
