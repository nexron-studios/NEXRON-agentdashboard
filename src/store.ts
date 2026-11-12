import { computed, ref, watch } from 'vue'
import type { MachineSummary, Profile } from './api'

/** Claude-Profile aus der letzten Übersicht; für E-Mail und Farbe eines Accounts. */
export const profiles = ref<Profile[]>([])

const MACHINE_KEY = 'agentdashboard.machine'
export const LOCAL_MACHINE = 'local'

/** Alle Rechner aus der letzten Übersicht, dieser zuerst */
export const machines = ref<MachineSummary[]>([])
/** Gewählter Rechner; bleibt über Neustarts gewählt */
export const machineId = ref(localStorage.getItem(MACHINE_KEY) ?? LOCAL_MACHINE)
watch(machineId, (id) => localStorage.setItem(MACHINE_KEY, id))
/** Der gezeigte Rechner; unbekannte IDs (Machine aus herdr entfernt) fallen auf diesen Rechner zurück */
export const machine = computed(
  () => machines.value.find((m) => m.id === machineId.value) ?? machines.value.find((m) => m.local) ?? null,
)
/** Warum herdr dem letzten Umschalten nicht gefolgt ist; leer, wenn alles geklappt hat */
export const herdrNotice = ref('')

/** Sessions liegen auf einem anderen Rechner: Ordner und Fenster gibt es hier nicht */
export const isRemote = computed(() => !!machine.value && !machine.value.local)

const ACCOUNT_COLORS = ['var(--color-series-1)', 'var(--color-series-7)', 'var(--color-ink-2)', 'var(--color-series-3)']

export function accountInfo(label: string) {
  const i = profiles.value.findIndex((p) => p.label === label)
  return {
    email: i >= 0 ? profiles.value[i].email : null,
    // Feste Farbe je Profil (Reihenfolge der Ordner), nie nach Rang
    color: i >= 0 ? ACCOUNT_COLORS[i % ACCOUNT_COLORS.length] : 'var(--color-ink-3)',
  }
}
