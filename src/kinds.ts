import {
  BookOpen, Bot, Earth, FilePen, FilePlus, FileText, GitFork, Globe, ListTodo, Map, MessageCircleQuestion,
  Plug, Search, Send, Settings2, Sparkles, Telescope, Wrench,
} from 'lucide-vue-next'
import type { Component } from 'vue'

// Farbe und Icon je Tool und je Subagent-Typ, damit man ohne Lesen sieht, was gerade passiert.
// Gleiche Farbe heißt gleiche Art Arbeit: blau liest, orange schreibt, cyan sucht, limettengrün geht ins Web.
// Klassen stehen ausgeschrieben, weil Tailwind nur vollständige Klassennamen im Quelltext findet.
const TONES = {
  blue: { text: 'text-series-1', chip: 'bg-series-1/15 text-series-1', bar: 'bg-series-1' },
  orange: { text: 'text-series-2', chip: 'bg-series-2/15 text-series-2', bar: 'bg-series-2' },
  green: { text: 'text-series-3', chip: 'bg-series-3/15 text-series-3', bar: 'bg-series-3' },
  amber: { text: 'text-series-4', chip: 'bg-series-4/15 text-series-4', bar: 'bg-series-4' },
  lime: { text: 'text-series-5', chip: 'bg-series-5/15 text-series-5', bar: 'bg-series-5' },
  cyan: { text: 'text-series-6', chip: 'bg-series-6/15 text-series-6', bar: 'bg-series-6' },
  purple: { text: 'text-series-7', chip: 'bg-series-7/15 text-series-7', bar: 'bg-series-7' },
  neutral: { text: 'text-ink-2', chip: 'bg-surface-2 text-ink-2', bar: 'bg-ink-3' },
} as const

export interface Kind {
  icon: Component
  text: string
  chip: string
  bar: string
}

const kind = (icon: Component, tone: keyof typeof TONES): Kind => ({ icon, ...TONES[tone] })

// Bash fehlt absichtlich: dafür gibt es den animierten TerminalLoader.
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
  Agent: kind(Bot, 'purple'),
  Task: kind(Bot, 'purple'),
  SendMessage: kind(Send, 'purple'),
  TodoWrite: kind(ListTodo, 'amber'),
  Skill: kind(Sparkles, 'amber'),
  AskUserQuestion: kind(MessageCircleQuestion, 'amber'),
}
const MCP_KIND = kind(Plug, 'green')
const DEFAULT_TOOL = kind(Wrench, 'neutral')

// Das Backend kürzt `mcp__server__tool` auf `tool`. Eingebaute Tools heißen in PascalCase, MCP-Tools
// fast immer klein (`search_vault_simple`, `navigate`), daran werden sie erkannt.
const MCP_NAME = /^[a-z]/
export const toolKind = (tool: string): Kind => TOOL_KINDS[tool] ?? (MCP_NAME.test(tool) ? MCP_KIND : DEFAULT_TOOL)

const AGENT_KINDS: Record<string, Kind> = {
  'general-purpose': kind(Bot, 'purple'),
  Explore: kind(Telescope, 'cyan'),
  Plan: kind(Map, 'blue'),
  'web-fetch': kind(Globe, 'lime'),
  fork: kind(GitFork, 'orange'),
  'claude-code-guide': kind(BookOpen, 'amber'),
  'statusline-setup': kind(Settings2, 'neutral'),
}
// Eigene Agents aus .claude/agents bekommen eine gemeinsame Farbe, damit sie sich von den eingebauten abheben
const CUSTOM_AGENT = kind(Bot, 'green')

export const agentKind = (type: string): Kind => AGENT_KINDS[type] ?? CUSTOM_AGENT
