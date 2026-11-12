<script setup lang="ts">
import { computed, ref } from 'vue'
import { Trash2 } from 'lucide-vue-next'
import { AlertDialogAction, AlertDialogCancel, AlertDialogDescription, AlertDialogRoot, AlertDialogTitle } from 'reka-ui'
import { clearHerdr, type HerdrPlace } from '../api'
import { awaitsYou } from '../format'
import AlertDialogContent from './ui/alert-dialog/AlertDialogContent.vue'

// „Chat löschen“ tippt `/clear` in die Session. Nur mit herdr-Tab: in ein beliebiges Terminal oder in
// VS Code lässt sich nicht zuverlässig schreiben. Und nur, solange Claude auf dich wartet: während einer
// Aufgabe würde der Verlauf mittendrin gelöscht. Der Button bleibt dann sichtbar, aber gesperrt
// (`aria-disabled` statt `disabled`, damit der Tooltip erklärt, warum).
const props = defineProps<{ herdr: HerdrPlace; status: string }>()
const emit = defineEmits<{ cleared: [] }>()
const open = ref(false)
const failed = ref('')
const ready = computed(() => awaitsYou(props.status))

const hint = computed(() =>
  failed.value
    ? failed.value
    : ready.value
      ? `Chat löschen: /clear in Tab „${props.herdr.tab}“`
      : 'Chat löschen geht erst, wenn Claude fertig ist',
)

function setOpen(value: boolean) {
  open.value = value && ready.value
}

async function clear() {
  try {
    await clearHerdr(props.herdr)
    failed.value = ''
    emit('cleared')
  } catch (e) {
    failed.value = String(e)
  }
}
</script>

<template>
  <button
    class="press relative z-10 grid shrink-0 place-items-center self-stretch rounded-lg border border-line-strong px-2.5 text-ink-2 hover:border-critical/60 hover:text-critical"
    :class="{ 'cursor-not-allowed opacity-40 hover:border-line-strong! hover:text-ink-2!': !ready, 'border-critical! text-critical!': failed }"
    :aria-disabled="!ready"
    :title="hint"
    @click.stop="setOpen(true)"
  >
    <Trash2 :size="16" :stroke-width="2" aria-hidden="true" />
    <span class="sr-only">Chat löschen</span>
  </button>

  <AlertDialogRoot :open="open" @update:open="setOpen">
    <AlertDialogContent>
      <div class="space-y-2">
        <AlertDialogTitle class="text-[17px] font-semibold tracking-tight">Chat löschen?</AlertDialogTitle>
        <AlertDialogDescription class="text-[14px] leading-relaxed text-ink-2">
          Schickt <code class="rounded bg-surface-3 px-1 py-px text-[13px] text-ink">/clear</code> an Tab „{{ herdr.tab }}“.
          Claude vergisst den bisherigen Verlauf und beginnt dort mit leerem Kontext. Das alte Transkript bleibt unter „Zuletzt“.
        </AlertDialogDescription>
      </div>
      <div class="flex justify-end gap-2">
        <AlertDialogCancel class="press rounded-lg border border-line-strong px-3 py-1.5 text-[13px] font-semibold text-ink-2 hover:bg-surface-2 hover:text-ink">
          Abbrechen
        </AlertDialogCancel>
        <AlertDialogAction class="press rounded-lg bg-critical px-3 py-1.5 text-[13px] font-semibold text-white hover:bg-critical/85" @click="clear">
          Chat löschen
        </AlertDialogAction>
      </div>
    </AlertDialogContent>
  </AlertDialogRoot>
</template>
