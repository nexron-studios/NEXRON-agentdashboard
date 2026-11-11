<script setup lang="ts">
import { computed, ref } from 'vue'
import { Globe } from 'lucide-vue-next'
import { HoverCardRoot, HoverCardTrigger } from 'reka-ui'
import { openUrl, type Source } from '../api'
import { useFavicon } from '../favicon'
import HoverCardContent from './ui/hover-card/HoverCardContent.vue'

// Quelle nach prompt-kit „Source“: Kapsel mit Favicon und Domain, beim Drüberfahren Titel,
// Beschreibung und Adresse. Ein Klick öffnet die Seite im Standardbrowser.
const props = defineProps<{ source: Source }>()

const favicon = useFavicon(computed(() => props.source.domain))
const error = ref('')

const open = async () => {
  try {
    await openUrl(props.source.url)
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}
</script>

<template>
  <HoverCardRoot :open-delay="250" :close-delay="100">
    <HoverCardTrigger as-child>
      <button
        class="inline-flex h-6 max-w-48 items-center gap-1.5 rounded-full bg-surface-2 pr-2.5 pl-1.5 text-[12px] text-ink-2 transition-colors duration-150 hover:bg-surface-3 hover:text-ink"
        :aria-label="`${source.title || source.domain} im Browser öffnen`"
        @click.stop="open"
      >
        <img v-if="favicon" :src="favicon" alt="" class="size-3.5 shrink-0 rounded-sm" />
        <Globe v-else :size="14" :stroke-width="2" class="shrink-0 text-ink-3" aria-hidden="true" />
        <span class="truncate">{{ source.domain }}</span>
      </button>
    </HoverCardTrigger>
    <HoverCardContent>
      <p class="flex items-center gap-1.5 text-[12px] text-ink-3">
        <img v-if="favicon" :src="favicon" alt="" class="size-3.5 rounded-sm" />
        <Globe v-else :size="14" :stroke-width="2" aria-hidden="true" />
        {{ source.domain }}
      </p>
      <p v-if="source.title" class="mt-1.5 line-clamp-2 font-medium text-ink">{{ source.title }}</p>
      <p v-if="source.description" class="mt-1 line-clamp-4 leading-snug text-ink-2">{{ source.description }}</p>
      <p class="mt-1.5 truncate font-mono text-[11.5px] text-ink-3">{{ source.url }}</p>
      <p v-if="error" class="mt-1.5 text-[12px] text-critical">{{ error }}</p>
    </HoverCardContent>
  </HoverCardRoot>
</template>
