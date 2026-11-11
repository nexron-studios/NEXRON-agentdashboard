<script setup lang="ts">
// shadcn-vue HoverCard (Stil „new-york“) auf den Dashboard-Farben. Anders als der Tooltip eine
// dunkle Karte, weil sie mehrere Zeilen mit Titel, Text und Adresse trägt.
import { HoverCardContent, HoverCardPortal, useForwardProps, type HoverCardContentProps } from 'reka-ui'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<HoverCardContentProps>(), { sideOffset: 6, side: 'top' })
// Nur gesetzte Props weiterreichen, sonst schaltet Vue `avoidCollisions` per `false` aus (siehe Tooltip)
const forwarded = useForwardProps(props)
</script>

<template>
  <HoverCardPortal>
    <HoverCardContent
      data-slot="hover-card-content"
      v-bind="{ ...forwarded, ...$attrs }"
      class="z-50 w-80 origin-(--reka-hover-card-content-transform-origin) rounded-lg border border-line-strong bg-surface-2 p-3 text-[13px] shadow-lg shadow-black/40 outline-none animate-in fade-in-0 zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
    >
      <slot />
    </HoverCardContent>
  </HoverCardPortal>
</template>
