import {
  AlarmClock, BookOpen, Bot, CircleStop, Earth, Eye, FilePen, FilePlus, FileText, GitFork, Globe, ListTodo, Map,
  MessageCircleQuestion, Plug, Search, Send, Settings2, Sparkles, SquareTerminal, Telescope, Users, Workflow, Wrench, Zap,
} from 'lucide-vue-next'
import type { Component } from 'vue'

// Eine Farbsprache für alles, was Claude tut, überall gleich: Karte, Ablauf, Tool-Nutzung, Zeitleiste.
// Blau liest, orange schreibt, cyan sucht, limette geht ins Web, gelb führt Befehle aus,
// lila delegiert an Subagents und Hintergrund-Tasks, grün spricht mit MCP-Servern. In der Markenfarbe
// steht, was Claude selbst tut: schreiben, planen, nachfragen, Skills laden.
// Klassen stehen ausgeschrieben, weil Tailwind nur vollständige Klassennamen im Quelltext findet.
const TONES = {
  blue: { text: 'text-series-1', chip: 'bg-series-1/15 text-series-1', bar: 'bg-series-1', color: 'var(--color-series-1)' },
  orange: { text: 'text-series-2', chip: 'bg-series-2/15 text-series-2', bar: 'bg-series-2', color: 'var(--color-series-2)' },
  green: { text: 'text-series-3', chip: 'bg-series-3/15 text-series-3', bar: 'bg-series-3', color: 'var(--color-series-3)' },
  amber: { text: 'text-series-4', chip: 'bg-series-4/15 text-series-4', bar: 'bg-series-4', color: 'var(--color-series-4)' },
  lime: { text: 'text-series-5', chip: 'bg-series-5/15 text-series-5', bar: 'bg-series-5', color: 'var(--color-series-5)' },
  cyan: { text: 'text-series-6', chip: 'bg-series-6/15 text-series-6', bar: 'bg-series-6', color: 'var(--color-series-6)' },
  purple: { text: 'text-series-7', chip: 'bg-series-7/15 text-series-7', bar: 'bg-series-7', color: 'var(--color-series-7)' },
  brand: { text: 'text-brand', chip: 'bg-brand/15 text-brand', bar: 'bg-brand', color: 'var(--color-brand)' },
  neutral: { text: 'text-ink-2', chip: 'bg-surface-2 text-ink-2', bar: 'bg-ink-3', color: 'var(--color-ink-3)' },
} as const

export interface Kind {
  icon: Component
  text: string
  chip: string
  bar: string
  /** Für SVG-Füllungen, wo keine Klasse greift */
  color: string
}

const kind = (icon: Component, tone: keyof typeof TONES): Kind => ({ icon, ...TONES[tone] })

const TOOL_KINDS: Record<string, Kind> = {
  Read: kind(FileText, 'blue'),
  Write: kind(FilePlus, 'orange'),
  Edit: kind(FilePen, 'orange'),
  MultiEdit: kind(FilePen, 'orange'),
  NotebookEdit: kind(FilePen, 'orange'),
  Grep: kind(Search, 'cyan'),
  Glob: kind(Search, 'cyan'),
  ToolSearch: kind(Search, 'cyan'),
  WebFetch: kind(Globe, 'lime'),
  WebSearch: kind(Earth, 'lime'),
  // Auf der Karte ersetzt der animierte TerminalLoader dieses Icon, die Farbe bleibt dieselbe
  Bash: kind(SquareTerminal, 'amber'),
  Monitor: kind(Eye, 'amber'),
  KillShell: kind(CircleStop, 'amber'),
  Agent: kind(Bot, 'purple'),
  Task: kind(Bot, 'purple'),
  SendMessage: kind(Send, 'purple'),
  ListAgents: kind(Users, 'purple'),
  Workflow: kind(Workflow, 'purple'),
  TaskStop: kind(CircleStop, 'purple'),
  TaskOutput: kind(Bot, 'purple'),
  TodoWrite: kind(ListTodo, 'brand'),
  Skill: kind(Zap, 'brand'),
  AskUserQuestion: kind(MessageCircleQuestion, 'brand'),
  ScheduleWakeup: kind(AlarmClock, 'brand'),
}
const MCP_KIND = kind(Plug, 'green')
const DEFAULT_TOOL = kind(Wrench, 'neutral')

// Das Backend kürzt `mcp__server__tool` auf `tool`. Eingebaute Tools heißen in PascalCase, MCP-Tools
// fast immer klein (`search_vault_simple`, `navigate`), daran werden sie erkannt.
const MCP_NAME = /^[a-z]/
export const toolKind = (tool: string): Kind => TOOL_KINDS[tool] ?? (MCP_NAME.test(tool) ? MCP_KIND : DEFAULT_TOOL)

/** Claude schreibt gerade selbst Text, statt ein Tool aufzurufen */
export const SAID_KIND = kind(Sparkles, 'brand')

const AGENT_KINDS: Record<string, Kind> = {
  'general-purpose': kind(Bot, 'purple'),
  Explore: kind(Telescope, 'cyan'),
  Plan: kind(Map, 'blue'),
  'web-fetch': kind(Globe, 'lime'),
  fork: kind(GitFork, 'orange'),
  'claude-code-guide': kind(BookOpen, 'amber'),
  'statusline-setup': kind(Settings2, 'neutral'),
}
// Eigene Agents aus .claude/agents teilen sich Lila mit general-purpose; das Icon unterscheidet sie
const CUSTOM_AGENT = kind(Sparkles, 'purple')

export const agentKind = (type: string): Kind => AGENT_KINDS[type] ?? CUSTOM_AGENT
