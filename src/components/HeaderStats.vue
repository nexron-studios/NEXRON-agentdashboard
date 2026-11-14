<script setup lang="ts">
import { CircleAlert, CircleCheck } from 'lucide-vue-next'
import RingSpinner from './RingSpinner.vue'

defineProps<{ counts: { approval: number; busy: number; waiting: number } }>()
</script>

<template>
  <div class="flex shrink-0 items-center gap-2 whitespace-nowrap text-[13px]">
    <span
      class="flex items-center gap-1.5 rounded-full border px-3 py-1"
      :class="counts.approval ? 'border-critical/40 bg-critical/12 text-ink' : 'border-line bg-app/60 text-ink-3'"
      title="Sessions, die auf eine Freigabe oder Auswahl warten"
    >
      <CircleAlert :size="15" :stroke-width="2.25" :class="counts.approval ? 'wiggle text-critical' : ''" />
      <b class="num">{{ counts.approval }}</b> <span class="@max-header:sr-only">Freigabe</span>
    </span>
    <span class="flex items-center gap-1.5 rounded-full border border-line bg-app/60 px-3 py-1 text-ink-2" title="Sessions, die gerade arbeiten">
      <RingSpinner :size="15" class="text-warning" :spinning="!!counts.busy" />
      <b class="num text-ink">{{ counts.busy }}</b> <span class="@max-header:sr-only">arbeiten</span>
    </span>
    <span
      class="flex items-center gap-1.5 rounded-full border px-3 py-1"
      :class="counts.waiting ? 'border-good/35 bg-good/10 text-ink' : 'border-line bg-app/60 text-ink-2'"
      title="Sessions, die ihre Aufgabe abgeschlossen haben"
    >
      <CircleCheck :size="15" :stroke-width="2.25" class="text-good" :class="{ beat: counts.waiting }" />
      <b class="num">{{ counts.waiting }}</b> <span class="@max-header:sr-only">abgeschlossen</span>
    </span>
  </div>
</template>
