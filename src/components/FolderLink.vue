<script setup lang="ts">
import { ref } from 'vue'
import { Folder, FolderOpen } from 'lucide-vue-next'
import { openFolder } from '../api'
import { projectName } from '../format'

// Ordner-Chip der Session: Klick öffnet den Arbeitsordner im Explorer. Beim Hover klappt der Ordner auf.
const props = defineProps<{ path: string; size?: number }>()
const failed = ref('')

async function open() {
  try {
    await openFolder(props.path)
    failed.value = ''
  } catch (e) {
    failed.value = String(e)
  }
}
</script>

<template>
  <button
    class="press folder-link relative z-10 flex items-center gap-1.5 rounded-lg border border-line-strong bg-surface-2 px-2.5 py-[5px] text-[13px] font-medium text-ink-2 hover:border-ink-3 hover:bg-surface-3 hover:text-ink"
    :class="{ 'text-critical!': failed }"
    :title="failed || `${path}\nIm Explorer öffnen`"
    @click.stop="open"
  >
    <span class="relative inline-flex shrink-0" :style="{ width: `${size ?? 14}px`, height: `${size ?? 14}px` }">
      <Folder :size="size ?? 14" :stroke-width="1.75" class="folder-closed absolute text-ink-3" />
      <FolderOpen :size="size ?? 14" :stroke-width="1.75" class="folder-open absolute text-ink-3" />
    </span>
    <span class="truncate">{{ projectName(path) }}</span>
  </button>
</template>
