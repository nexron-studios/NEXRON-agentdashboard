<script setup lang="ts">
// shadcn-vue AlertDialog (Stil „new-york“), auf die Dashboard-Farben umgestellt. Wie der Tooltip nur der
// Inhalt mit Portal und Abdunklung; Root, Trigger, Titel und Buttons kommen direkt aus reka-ui.
import {
  AlertDialogContent,
  AlertDialogOverlay,
  AlertDialogPortal,
  useForwardPropsEmits,
  type AlertDialogContentEmits,
  type AlertDialogContentProps,
} from 'reka-ui'

defineOptions({ inheritAttrs: false })
const props = defineProps<AlertDialogContentProps>()
const emits = defineEmits<AlertDialogContentEmits>()
const forwarded = useForwardPropsEmits(props, emits)
</script>

<template>
  <AlertDialogPortal>
    <AlertDialogOverlay
      class="fixed inset-0 z-50 bg-black/60 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0"
    />
    <AlertDialogContent
      data-slot="alert-dialog-content"
      v-bind="{ ...forwarded, ...$attrs }"
      class="fixed top-1/2 left-1/2 z-50 grid w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 gap-4 rounded-xl border border-line-strong bg-surface p-6 shadow-[0_1px_2px_rgb(0_0_0/0.4),0_16px_40px_rgb(0_0_0/0.5)] duration-200 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95"
    >
      <slot />
    </AlertDialogContent>
  </AlertDialogPortal>
</template>
