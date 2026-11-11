import { ref, watchEffect, type Ref } from 'vue'
import { fetchFavicon } from './api'

// Eine Anfrage pro Domain und App-Lauf: Der Ablauf wird alle zwei Sekunden neu gezeichnet, und
// dieselbe Quelle steht oft in mehreren Schritten. `null` heißt „die Seite hat keins“.
const cache = new Map<string, Promise<string | null>>()

const load = (domain: string) => {
  const hit = cache.get(domain)
  if (hit) return hit
  const pending = fetchFavicon(domain).catch((e: unknown) => {
    console.warn(`Favicon für ${domain} nicht geladen`, e)
    return null
  })
  cache.set(domain, pending)
  return pending
}

/** Das Favicon als `data:`-URL, solange es lädt oder fehlt `null` */
export const useFavicon = (domain: Ref<string>): Ref<string | null> => {
  const src = ref<string | null>(null)
  watchEffect(async (onCleanup) => {
    let isStale = false
    onCleanup(() => (isStale = true))
    src.value = null
    const url = await load(domain.value)
    if (!isStale) src.value = url
  })
  return src
}
