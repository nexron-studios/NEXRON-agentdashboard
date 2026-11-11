<script setup lang="ts">
// Ring statt Lucides `LoaderCircle`: Der offene Dreiviertelbogen wirkt beim Drehen je nach Winkel
// schief. Eine blasse volle Spur mit hellem Bogen ist in jedem Winkel symmetrisch.
withDefaults(defineProps<{ size?: number; spinning?: boolean }>(), { size: 14, spinning: true })
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" aria-hidden="true" class="shrink-0" :class="{ spin: spinning }">
    <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-opacity="0.25" stroke-width="3" />
    <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-dasharray="14 100" />
  </svg>
</template>
