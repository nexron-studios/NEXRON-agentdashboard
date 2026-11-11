import { ref } from 'vue'
import type { Profile } from './api'

/** Claude-Profile aus der letzten Übersicht; für E-Mail und Farbe eines Accounts. */
export const profiles = ref<Profile[]>([])

const ACCOUNT_COLORS = ['var(--color-series-1)', 'var(--color-series-7)', 'var(--color-ink-2)', 'var(--color-series-3)']

export function accountInfo(label: string) {
  const i = profiles.value.findIndex((p) => p.label === label)
  return {
    email: i >= 0 ? profiles.value[i].email : null,
    // Feste Farbe je Profil (Reihenfolge der Ordner), nie nach Rang
    color: i >= 0 ? ACCOUNT_COLORS[i % ACCOUNT_COLORS.length] : 'var(--color-ink-3)',
  }
}
