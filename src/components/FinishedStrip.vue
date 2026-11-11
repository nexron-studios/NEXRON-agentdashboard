<script setup lang="ts">
import { computed } from 'vue'
import type { SessionRow } from '../api'
import { ago, clock, date, headline, projectName } from '../format'
import SessionOpen from './SessionOpen.vue'

/**
 * Fertige Sessions über den Karten, die zuletzt fertig gewordene vorn (①), in Lesereihenfolge.
 * Es gibt genau so viele Plätze wie laufende Sessions (mehr können nicht fertig sein), im selben
 * Spaltenraster wie die Karten. Die Höhe hängt also nur an der Zahl der Sessions: Wird eine fertig,
 * füllt sich ein Platz, und nichts darunter verschiebt sich. Die Karten tragen dieselbe Nummer.
 */
const props = defineProps<{ live: SessionRow[]; done: SessionRow[]; now: number }>()
const emit = defineEmits<{ select: [id: string] }>()

const free = computed(() => Math.max(0, props.live.length - props.done.length))
// Erster freier Platz sagt, warum noch nichts dasteht
const reason = computed(() => {
  const busy = props.live.filter((s) => s.status === 'busy' || s.status === 'shell').length
  return busy ? `Noch nichts fertig · ${busy} ${busy === 1 ? 'arbeitet' : 'arbeiten'}` : 'Noch keine Session fertig'
})
const name = (s: SessionRow) => s.herdr?.tab || s.name || projectName(s.cwd)
</script>

<template>
  <TransitionGroup tag="ol" name="slot" class="grid gap-x-4 gap-y-2 @min-[640px]:grid-cols-2 @min-[980px]:grid-cols-3" aria-label="Fertige Sessions, neueste zuerst">
    <li
      v-for="(s, i) in done"
      :key="s.sessionId"
      class="relative flex h-9 min-w-0 items-center gap-2 rounded-lg border pr-1 pl-2 text-[13px] transition-colors duration-150"
      :class="i === 0 ? 'border-good/40 bg-good/10 hover:bg-good/15' : 'border-line bg-surface hover:bg-surface-2'"
      :title="`${headline(s).text}\nFertig geworden ${date(s.finishedAt)} (${ago(s.finishedAt, now)})`"
    >
      <span class="num grid h-5 w-5 shrink-0 place-items-center rounded-full text-[11px] font-bold" :class="i === 0 ? 'bg-good text-app' : 'bg-surface-3 text-ink-2'">
        {{ i + 1 }}
      </span>
      <span class="max-w-[12ch] shrink-0 truncate font-semibold" :class="i === 0 ? 'text-ink' : 'text-ink-2'">{{ name(s) }}</span>
      <!-- Klick auf die Zeile öffnet die Detailansicht, der Pfeil springt direkt in den Chat -->
      <button class="card-link min-w-0 flex-1 truncate text-left" :class="i === 0 ? 'text-ink' : 'text-ink-2'" @click="emit('select', s.sessionId)">
        {{ headline(s).text }}
      </button>
      <span class="num shrink-0 whitespace-nowrap" :class="i === 0 ? 'font-semibold text-good' : 'text-ink-3'">{{ clock(s.finishedAt, now) }}</span>
      <SessionOpen compact :herdr="s.herdr" :pid="s.pid" :cwd="s.cwd" :entrypoint="s.entrypoint" />
    </li>
    <li
      v-for="j in free"
      :key="`free-${done.length + j}`"
      class="flex h-9 min-w-0 items-center rounded-lg border border-dashed border-line px-3 text-[13px] text-ink-3"
    >
      <span v-if="j === 1 && !done.length" class="truncate">{{ reason }}</span>
    </li>
  </TransitionGroup>
</template>

<style scoped>
/* Neu fertig: rutscht von links auf ① rein, die anderen rücken einen Platz weiter. Was die Leiste
   verlässt (neue Aufgabe, freier Platz), ist sofort aus dem Raster: Ohne `display: none` hält Vue es
   für die Dauer von `transition-colors` (150 ms) fest, dann steht ein Platz zu viel im Raster, es
   bricht in eine neue Zeile um und alles darunter springt runter und wieder hoch. */
.slot-leave-active {
  display: none;
}
.slot-move,
.slot-enter-active {
  transition:
    transform 320ms cubic-bezier(0.2, 0.8, 0.2, 1),
    opacity 320ms ease;
}
.slot-enter-from {
  opacity: 0;
  transform: translateX(-16px);
}
@media (prefers-reduced-motion: reduce) {
  .slot-move,
  .slot-enter-active {
    transition: none;
  }
}
</style>
