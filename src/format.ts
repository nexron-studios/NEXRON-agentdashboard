import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue'

export function tokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1)}K`
  return String(n)
}

export function usd(n: number): string {
  return n < 0.01 && n > 0 ? '<$0.01' : `$${n.toFixed(2)}`
}

export function duration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000))
  if (s < 60) return `${s} s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} min`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h} h ${m % 60} min`
  return `${Math.floor(h / 24)} d ${h % 24} h`
}

export function ago(t: number, now: number): string {
  if (!t) return ''
  const s = Math.round((now - t) / 1000)
  if (s < 10) return 'gerade eben'
  return `vor ${duration(s * 1000)}`
}

const dateFmt = new Intl.DateTimeFormat('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' })
const timeFmt = new Intl.DateTimeFormat('de-DE', { hour: '2-digit', minute: '2-digit' })
const shortDayFmt = new Intl.DateTimeFormat('de-DE', { day: '2-digit', month: '2-digit' })
const dayFmt = new Intl.DateTimeFormat('de-DE', { weekday: 'long', day: 'numeric', month: 'long' })
export const date = (t: number) => (t ? dateFmt.format(t) : '')
export const time = (t: number) => (t ? timeFmt.format(t) : '')

/** Uhrzeit, ab gestern mit Tag davor: „14:32“, „gestern 18:05“, „30.10. 09:12“ */
export function clock(t: number, now: number): string {
  if (!t) return ''
  const day = (x: number) => new Date(x).setHours(0, 0, 0, 0)
  const diff = Math.round((day(now) - day(t)) / 86_400_000)
  if (diff === 0) return time(t)
  if (diff === 1) return `gestern ${time(t)}`
  return `${shortDayFmt.format(t)} ${time(t)}`
}

/** „Heute“, „Gestern“ oder „Freitag, 30. Oktober“ */
export function dayLabel(t: number, now: number): string {
  const day = (x: number) => new Date(x).setHours(0, 0, 0, 0)
  const diff = Math.round((day(now) - day(t)) / 86_400_000)
  if (diff === 0) return 'Heute'
  if (diff === 1) return 'Gestern'
  return dayFmt.format(t)
}

/** `claude-opus-5-5` → `Opus 5.5` */
export function modelName(id: string): string {
  const m = /claude-([a-z]+)-(\d+)(?:-(\d+))?/.exec(id)
  if (!m) return id
  const family = m[1].charAt(0).toUpperCase() + m[1].slice(1)
  return m[3] && m[3].length <= 2 ? `${family} ${m[2]}.${m[3]}` : `${family} ${m[2]}`
}

const GENERIC_DIRS = new Set(['frontend', 'backend', 'app', 'src', 'code', 'repos', 'web', 'client', 'server', 'api'])

/** Lesbarer Projektname aus dem Arbeitsordner: `…\repos\app\frontend` → `app / frontend` */
export function projectName(cwd: string): string {
  const parts = cwd.split(/[\\/]+/).filter((p) => p && !/^[A-Za-z]:$/.test(p))
  if (!parts.length) return cwd
  const last = parts[parts.length - 1]
  if (GENERIC_DIRS.has(last.toLowerCase()) && parts.length > 1) return `${parts[parts.length - 2]} / ${last}`
  return last
}

/** Claude hält an und braucht eine Freigabe oder Auswahl */
export const needsApproval = (status: string) => status === 'blocked' || status === 'waiting'
/** Arbeitet gerade: denkt nach oder führt einen Befehl aus */
export const isBusy = (status: string) => status === 'busy' || status === 'shell'
/** Hat seine Aufgabe abgeschlossen */
export const awaitsYou = (status: string) => status === 'idle' || status === 'done'

/** `claude-2` vor `claude-10`; Sessions ohne herdr-Tab ans Ende */
const collator = new Intl.Collator('de', { numeric: true, sensitivity: 'base' })
export const byTab = (a: string | undefined, b: string | undefined) =>
  a && b ? collator.compare(a, b) : a ? -1 : b ? 1 : 0

/**
 * Ruft `task` sofort und danach alle `ms` auf, ohne dass sich Aufrufe überholen. `refresh` holt den
 * nächsten Aufruf vor (z. B. nach einem Wechsel der Machine); läuft gerade einer, folgt er direkt danach.
 */
export function usePolling(task: () => Promise<void>, ms: number): { now: Ref<number>; refresh: () => void } {
  const now = ref(Date.now())
  let timer: number | undefined
  let stopped = false
  let running = false
  let again = false
  const tick = async () => {
    if (running) {
      again = true
      return
    }
    running = true
    window.clearTimeout(timer)
    try {
      await task()
    } finally {
      running = false
      now.value = Date.now()
      if (stopped) return
      if (again) {
        again = false
        void tick()
      } else timer = window.setTimeout(tick, ms)
    }
  }
  onMounted(tick)
  onBeforeUnmount(() => {
    stopped = true
    window.clearTimeout(timer)
  })
  return { now, refresh: () => void tick() }
}

/**
 * Überschrift einer Session. Claude Code vergibt den Titel nur einmal am Anfang; ist die Arbeit
 * seitdem zu einem anderen Thema gewandert, wird das aktuelle Thema zur Überschrift und der Titel
 * rückt als „Gestartet als“ darunter.
 */
export function headline(s: {
  title: string
  currentTask: string
  currentTaskIsFirst: boolean
  firstPrompt: string
  topic?: string
  titlePending?: boolean
}) {
  const switched = !!s.currentTask && !s.currentTaskIsFirst
  // Ist ein Titel unterwegs, bleibt der bisherige stehen (oder ein Platzhalter, `pending`), statt kurz die
  // Anfrage zu zeigen. Die Anfrage ist nur der Rückfall, wenn kein Titel kommt.
  if (switched) {
    if (!s.topic && s.titlePending) return { text: s.title, startedAs: '', switched, pending: !s.title }
    return { text: s.topic || s.currentTask, startedAs: s.title || s.firstPrompt, switched, pending: false }
  }
  if (!s.title && s.titlePending) return { text: '', startedAs: '', switched, pending: true }
  return { text: s.title || s.currentTask || s.firstPrompt, startedAs: '', switched, pending: false }
}

export const PENDING_TITLE = 'Titel wird erstellt …'

/** Anfrage ohne Bild-Platzhalter und überzählige Leerzeichen, zum Vergleichen */
export const plainPrompt = (t: string) => t.replace(/\[(Image|Pasted)[^\]]*\]/g, '').replace(/\s+/g, ' ').trim()
