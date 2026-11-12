<script setup lang="ts">
// Platzhalter, solange die Sessions eines Rechners laden: so viele Plätze und Karten, wie der Rechner
// laut Umschalter gerade hat, im selben Raster und etwa in Kartenhöhe. Dann bleibt die Seite beim
// Umschalten gleich hoch, statt erst zusammenzufallen und dann wieder aufzugehen.
defineProps<{ count: number }>()
</script>

<template>
  <div class="@container" aria-busy="true" aria-label="Sessions werden geladen">
    <div class="mb-4 grid gap-x-4 gap-y-2 @min-[640px]:grid-cols-2 @min-[980px]:grid-cols-3">
      <div v-for="i in count" :key="i" class="h-9 rounded-lg border border-dashed border-line" />
    </div>
    <div class="grid gap-4 @min-[640px]:grid-cols-2 @min-[980px]:grid-cols-3">
      <div v-for="i in count" :key="i" class="card h-[335px] animate-pulse" />
    </div>
  </div>
</template>
