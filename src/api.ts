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

export interface Overview {
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

export const fetchOverview = (recentLimit: number) => invoke<Overview>('overview', { recentLimit })
export const fetchDetail = (sessionId: string) => invoke<Detail>('session_detail', { sessionId })
export const focusHerdr = (tabId: string) => invoke<void>('focus_herdr', { tabId })
export const clearHerdr = (paneId: string) => invoke<void>('clear_herdr', { paneId })
export const openFolder = (path: string) => invoke<void>('open_folder', { path })
export const openSession = (pid: number | null, cwd: string, entrypoint: string) =>
  invoke<void>('open_session', { pid, cwd, entrypoint })
