<script setup lang="ts">
import { ref } from 'vue'
import { fetchQuota, type AccountQuota, type QuotaWindow } from '../api'
import { usePolling } from '../format'

// The backend caches for 90 s, so polling faster only refreshes the countdown.
const POLL_MS = 30_000
const CRITICAL_FREE = 10
const LOW_FREE = 25
const WINDOW_LABELS: Record<string, string> = { five_hour: '5 Std', seven_day: '7 Tage' }

const accounts = ref<AccountQuota[]>([])

const { now } = usePolling(async () => {
  accounts.value = await fetchQuota()
}, POLL_MS)

const freePercent = (w: QuotaWindow) => Math.max(0, Math.round(100 - w.usedPercent))

const barColor = (w: QuotaWindow) => {
  const free = freePercent(w)
  if (free <= CRITICAL_FREE) return 'bg-critical'
  if (free <= LOW_FREE) return 'bg-warning'
  return 'bg-series-1'
}

const countdown = (w: QuotaWindow) => {
  if (!w.resetsAt) return ''
  const minutes = Math.max(0, Math.round((Date.parse(w.resetsAt) - now.value) / 60_000))
  if (minutes < 60) return `${minutes} Min.`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} Std.`
  return `${Math.floor(hours / 24)} T ${hours % 24} Std.`
}

const tooltip = (a: AccountQuota) =>
  [a.email ?? a.label, a.error].filter(Boolean).join(' · ')
</script>

<template>
  <!-- Grid with auto columns: the rows stay aligned while each column is only as wide as its
       widest value. Fixed widths wasted room the crowded header needs, and wrapped "100 % frei". -->
  <div
    v-for="a in accounts"
    :key="a.label"
    class="grid grid-cols-[repeat(4,auto)] items-center gap-x-2 gap-y-1 whitespace-nowrap text-[11px]"
    :class="{ 'opacity-60': a.error }"
    :title="tooltip(a)"
  >
    <template v-for="w in a.windows" :key="w.key">
      <span class="text-ink-2">{{ WINDOW_LABELS[w.key] ?? w.key }}</span>
      <span class="h-1.5 w-24 overflow-hidden rounded-full bg-surface-3">
        <span class="block h-full rounded-full" :class="barColor(w)" :style="{ width: `${freePercent(w)}%` }" />
      </span>
      <b class="num text-right text-ink">{{ freePercent(w) }} % frei</b>
      <span class="num text-ink-3">{{ countdown(w) }}</span>
    </template>
  </div>
</template>
