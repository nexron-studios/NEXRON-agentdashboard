import { invoke } from '@tauri-apps/api/core'

export interface Context {
  tokens: number
  window: number
  model: string
}

export interface HerdrPlace {
  workspace: string
  tab: string
  tabId: string
  paneId: string
  title: string
  status: string
  /** herdr-Machine, auf der der Tab liegt; `null` auf diesem Rechner */
  machine: string | null
}

export interface Profile {
  label: string
  dir: string
  email: string | null
}

export interface SessionRow {
  sessionId: string
  live: boolean
  status: string
  name: string
  pid: number | null
  entrypoint: string
  account: string
  herdr: HerdrPlace | null
  cwd: string
  startedAt: number
  title: string
  firstPrompt: string
  currentTask: string
  currentTaskIsFirst: boolean
  /** Von Haiku formulierter Titel des aktuellen Themas; leer, solange Claudes Titel noch passt */
  topic: string
  /** Ein Titel ist unterwegs (Claudes eigener oder ein Thema von Haiku) */
  titlePending: boolean
  lastPrompt: string
  prompts: number
  lastAction: string
  lastModel: string
  lastT: number
  /** Wann Claude mit der letzten Aufgabe fertig wurde (ms); nur aussagekräftig bei idle/done */
  finishedAt: number
  context: Context | null
  agents: AgentRun[]
  agentsTotal: number
  agentsRunning: number
}

/** Ein Rechner im Umschalter: dieser oder eine SSH-Machine aus herdr */
export interface MachineSummary {
  id: string
  label: string
  /** Rechnername bzw. SSH-Ziel */
  host: string
  /** windows | macos | linux; leer, solange die Machine noch nie geantwortet hat */
  os: string
  local: boolean
  /** Noch keine Antwort und kein Fehler seit dem Start */
  connecting: boolean
  connected: boolean
  error: string | null
  live: number
  approval: number
  busy: number
  waiting: number
}

export interface Overview {
  /** Die Machine, deren Sessions `live` und `recent` zeigen */
  machine: string
  machines: MachineSummary[]
  live: SessionRow[]
  recent: SessionRow[]
  profiles: Profile[]
  herdrAvailable: boolean
}

export interface ModelUsage {
  input: number
  output: number
  cacheRead: number
  cacheWrite: number
  requests: number
}

export interface AgentRun {
  id: string
  description: string
  agentType: string
  model: string
  background: boolean
  started: number
  finished: number | null
  status: string
}

export interface ErrorItem {
  t: number
  source: string
  message: string
}

export interface TimelineEvent {
  t: number
  kind: 'prompt' | 'tool' | 'error'
  label: string
  sidechain: boolean
}

export interface StepField {
  key: string
  value: string
}

export interface Source {
  url: string
  domain: string
  /** Seitentitel; nur WebSearch liefert einen */
  title: string
  /** Bei WebFetch Claudes Zusammenfassung der Seite */
  description: string
}

export interface Step {
  t: number
  /** `thought` ist Text, den Claude zwischen zwei Tool-Aufrufen schreibt */
  kind: 'prompt' | 'thought' | 'tool'
  label: string
  detail: string
  sidechain: boolean
  failed: boolean
  /** Das Ergebnis des Tool-Aufrufs ist da */
  done: boolean
  input: StepField[]
  output: string
  sources: Source[]
}

export interface Cost {
  totalUsd: number
  byModel: Record<string, number>
  linesAdded: number
  linesRemoved: number
}

export interface Prompt {
  t: number
  text: string
}

export interface Stats {
  title: string
  firstPrompt: string
  currentTask: string
  currentTaskIsFirst: boolean
  lastPrompt: string
  lastAction: string
  lastModel: string
  firstT: number
  lastT: number
  cwd: string
  gitBranch: string
  version: string
  permissionMode: string
  prompts: number
  turns: number
  toolCalls: number
  compactions: number
  models: Record<string, ModelUsage>
  context: Context | null
  tools: Record<string, number>
  agents: AgentRun[]
  errors: ErrorItem[]
  errorCount: number
  events: TimelineEvent[]
  steps: Step[]
  promptLog: Prompt[]
  cost: Cost | null
  fileBytes: number
}

export interface LiveSession {
  sessionId: string
  pid: number | null
  cwd: string | null
  name: string | null
  status: string | null
  statusUpdatedAt: number | null
  entrypoint: string | null
  account: string
  startedAt: number | null
}

export interface Detail {
  sessionId: string
  live: LiveSession | null
  status: string
  account: string
  herdr: HerdrPlace | null
  topic: string
  titlePending: boolean
  stats: Stats
}

export const fetchOverview = (recentLimit: number, machine: string) => invoke<Overview>('overview', { recentLimit, machine })
export const fetchDetail = (sessionId: string, machine: string) => invoke<Detail>('session_detail', { sessionId, machine })
export const focusHerdr = (place: HerdrPlace) =>
  invoke<void>('focus_herdr', { tabId: place.tabId, paneId: place.paneId, machine: place.machine })
/** herdr in Ghostty wechselt auf dieselbe Machine wie der Umschalter */
export const followInHerdr = (machine: string) => invoke<void>('follow_in_herdr', { machine })
export const clearHerdr = (place: HerdrPlace) => invoke<void>('clear_herdr', { paneId: place.paneId, machine: place.machine })
export const stopHerdr = (place: HerdrPlace) => invoke<void>('stop_herdr', { paneId: place.paneId, machine: place.machine })
export const openFolder = (path: string) => invoke<void>('open_folder', { path })
export const openSession = (pid: number | null, cwd: string, entrypoint: string) =>
  invoke<void>('open_session', { pid, cwd, entrypoint })
export const fetchFavicon = (domain: string) => invoke<string | null>('favicon', { domain })
export const openUrl = (url: string) => invoke<void>('open_url', { url })

export interface QuotaWindow {
  key: string
  usedPercent: number
  resetsAt: string | null
}

export interface AccountQuota {
  label: string
  email: string | null
  windows: QuotaWindow[]
  fetchedAt: number
  error: string | null
}

export const fetchQuota = () => invoke<AccountQuota[]>('quota')
