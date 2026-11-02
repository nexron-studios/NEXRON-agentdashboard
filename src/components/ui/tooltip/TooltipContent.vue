<script setup lang="ts">
// shadcn-vue Tooltip (Stil „new-york“), auf die Dashboard-Farben umgestellt: helle Blase auf dunklem
// Grund, wie shadcn mit `bg-foreground text-background`. Baut wie shadcn-vue auf reka-ui auf.
import { TooltipArrow, TooltipContent, TooltipPortal, useForwardProps, type TooltipContentProps } from 'reka-ui'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<TooltipContentProps>(), { sideOffset: 6, side: 'top' })
// Wie in shadcn-vue: nur gesetzte Props weiterreichen. Sonst macht Vue aus jedem fehlenden Boolean
// ein `false`, und `avoidCollisions` wäre aus (Tooltip ragt am oberen Rand aus dem Fenster).
const forwarded = useForwardProps(props)
</script>

<template>
  <TooltipPortal>
    <TooltipContent
      data-slot="tooltip-content"
      v-bind="{ ...forwarded, ...$attrs }"
      class="z-50 w-fit max-w-80 origin-(--reka-tooltip-content-transform-origin) rounded-md bg-ink px-3 py-1.5 text-[12.5px] leading-snug font-medium whitespace-pre-line text-app shadow-lg shadow-black/40 animate-in fade-in-0 zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
    >
      <slot />
      <TooltipArrow class="fill-ink" :width="10" :height="5" />
    </TooltipContent>
  </TooltipPortal>
</template>
