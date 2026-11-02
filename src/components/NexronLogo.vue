<script setup lang="ts">
// NEXRON-Logo aus nexron/brand/logo/nexron_logo.svg, unverändert bis auf die viewBox:
// die Original-Seite ist 1066×800 mit viel leerem Rand, zugeschnitten auf das Logo selbst.
import logoUrl from '../assets/nexron_logo.svg'

defineProps<{ height?: number }>()
</script>

<template>
  <img :src="logoUrl" alt="NEXRON Studios" :style="{ height: `${height ?? 28}px` }" class="w-auto select-none" draggable="false" />
</template>
