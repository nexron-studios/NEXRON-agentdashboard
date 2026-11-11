import {
  AlarmClock, BookOpen, Bot, CircleStop, Earth, Eye, FilePen, FilePlus, FileText, GitFork, Globe, ListTodo, Map,
  MessageCircleQuestion, Plug, Search, Send, Settings2, Sparkles, SquareTerminal, Telescope, Users, Workflow, Wrench, Zap,
} from 'lucide-vue-next'
import type { Component } from 'vue'

// Ebene „Aktivität“ der Farbordnung in style.css: drei kühle Töne nach Art der Arbeit, unterschieden
// wird vor allem über das Icon. Was keine dieser drei Arten ist, bleibt neutral, damit Status und
// Alarm (gelb, grün, rot) die einzigen lauten Farben auf der Karte bleiben.
// Klassen stehen ausgeschrieben, weil Tailwind nur vollständige Klassennamen im Quelltext findet.
const TONES = {
  read: { text: 'text-act-read', bar: 'bg-act-read', color: 'var(--color-act-read)' },
  change: { text: 'text-act-change', bar: 'bg-act-change', color: 'var(--color-act-change)' },
  outside: { text: 'text-act-outside', bar: 'bg-act-outside', color: 'var(--color-act-outside)' },
  neutral: { text: 'text-ink-3', bar: 'bg-ink-3', color: 'var(--color-ink-3)' },
} as const

export interface Kind {
  icon: Component
  /** Farbe für das Icon; Text daneben bleibt ink */
  text: string
  bar: string
  /** Für SVG-Füllungen, wo keine Klasse greift */
  color: string
}

const kind = (icon: Component, tone: keyof typeof TONES): Kind => ({ icon, ...TONES[tone] })

const TOOL_KINDS: Record<string, Kind> = {
  Read: kind(FileText, 'read'),
  Grep: kind(Search, 'read'),
  Glob: kind(Search, 'read'),
  ToolSearch: kind(Search, 'read'),
  Write: kind(FilePlus, 'change'),
  Edit: kind(FilePen, 'change'),
  MultiEdit: kind(FilePen, 'change'),
  NotebookEdit: kind(FilePen, 'change'),
  // Auf der Karte ersetzt der animierte TerminalLoader dieses Icon, die Farbe bleibt dieselbe
  Bash: kind(SquareTerminal, 'change'),
  Monitor: kind(Eye, 'change'),
  KillShell: kind(CircleStop, 'change'),
  WebFetch: kind(Globe, 'outside'),
  WebSearch: kind(Earth, 'outside'),
  Agent: kind(Bot, 'neutral'),
  Task: kind(Bot, 'neutral'),
  TaskStop: kind(CircleStop, 'neutral'),
  TaskOutput: kind(Bot, 'neutral'),
  SendMessage: kind(Send, 'neutral'),
  ListAgents: kind(Users, 'neutral'),
  Workflow: kind(Workflow, 'neutral'),
  TodoWrite: kind(ListTodo, 'neutral'),
  Skill: kind(Zap, 'neutral'),
  AskUserQuestion: kind(MessageCircleQuestion, 'neutral'),
  ScheduleWakeup: kind(AlarmClock, 'neutral'),
}
const MCP_KIND = kind(Plug, 'outside')
const DEFAULT_TOOL = kind(Wrench, 'neutral')

// Das Backend kürzt `mcp__server__tool` auf `tool`. Eingebaute Tools heißen in PascalCase, MCP-Tools
// fast immer klein (`search_vault_simple`, `navigate`), daran werden sie erkannt.
const MCP_NAME = /^[a-z]/
export const toolKind = (tool: string): Kind => TOOL_KINDS[tool] ?? (MCP_NAME.test(tool) ? MCP_KIND : DEFAULT_TOOL)

/** Claude schreibt gerade selbst Text, statt ein Tool aufzurufen */
export const SAID_KIND = kind(Sparkles, 'neutral')

// Ein Subagent bekommt die Farbe der Arbeit, für die er gebaut ist
const AGENT_KINDS: Record<string, Kind> = {
  'general-purpose': kind(Bot, 'change'),
  fork: kind(GitFork, 'change'),
  'statusline-setup': kind(Settings2, 'change'),
  Explore: kind(Telescope, 'read'),
  Plan: kind(Map, 'read'),
  'claude-code-guide': kind(BookOpen, 'read'),
  'web-fetch': kind(Globe, 'outside'),
}
const CUSTOM_AGENT = kind(Sparkles, 'neutral')

export const agentKind = (type: string): Kind => AGENT_KINDS[type] ?? CUSTOM_AGENT

/** Gleiche Kapsel für jeden Typ-Chip; die Farbe trägt nur das Icon */
export const KIND_CHIP = 'bg-surface-2 text-ink-2'
