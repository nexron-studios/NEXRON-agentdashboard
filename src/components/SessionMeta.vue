<script setup lang="ts">
import { computed } from 'vue'
import { Code, LayoutPanelTop, SquareTerminal, UserRound } from 'lucide-vue-next'
import type { HerdrPlace } from '../api'
import { accountInfo } from '../store'

const props = defineProps<{ herdr: HerdrPlace | null; account: string; entrypoint?: string; name?: string; size?: 'md' | 'lg' }>()

const acc = computed(() => accountInfo(props.account))
const lg = computed(() => props.size === 'lg')
</script>

<template>
  <div class="flex flex-wrap items-center gap-2" :class="lg ? 'text-[14px]' : 'text-[13px]'">
    <span
      v-if="herdr"
      class="flex items-center gap-1.5 rounded-md border border-line-strong bg-surface-2 px-2 py-0.5 font-semibold text-ink"
      :title="`herdr: Workspace „${herdr.workspace}“, Tab „${herdr.tab}“`"
    >
      <LayoutPanelTop :size="lg ? 16 : 14" :stroke-width="2" class="text-ink-3" />
      <span v-if="herdr.workspace" class="font-medium text-ink-2">{{ herdr.workspace }} ›</span>
      {{ herdr.tab }}
    </span>
    <span v-else-if="name" class="flex items-center gap-1.5 rounded-md px-1 py-0.5 font-mono text-[12px] text-ink-3" title="Name aus der Claude-Session (nicht in herdr gefunden)">
      <SquareTerminal :size="14" :stroke-width="1.75" /> {{ name }}
    </span>

    <span v-if="account" class="flex items-center gap-1.5 rounded-md bg-surface-2 px-2 py-0.5 font-medium text-ink-2" :title="acc.email ? `Claude-Account: ${acc.email}` : 'Claude-Profil'">
      <UserRound :size="lg ? 16 : 14" :stroke-width="1.75" :style="{ color: acc.color }" />
      {{ account }}
    </span>

    <span v-if="entrypoint === 'claude-vscode'" class="flex items-center gap-1.5 rounded-md bg-surface-2 px-2 py-0.5 text-ink-2" title="Läuft in VS Code">
      <Code :size="14" :stroke-width="1.75" /> VS Code
    </span>

  </div>
</template>
