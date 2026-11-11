import { Bot, FilePen, FilePlus, FileText, Globe, ListTodo, Search, Wrench } from 'lucide-vue-next'
import type { Component } from 'vue'

// Ein Icon je Werkzeug, damit man ohne Lesen sieht, was Claude gerade tut. Bash fehlt absichtlich:
// dafür gibt es den animierten TerminalLoader.
const TOOL_ICONS: Record<string, Component> = {
  Read: FileText,
  Write: FilePlus,
  Edit: FilePen,
  MultiEdit: FilePen,
  NotebookEdit: FilePen,
  Grep: Search,
  Glob: Search,
  ToolSearch: Search,
  WebFetch: Globe,
  WebSearch: Globe,
  Agent: Bot,
  Task: Bot,
  TodoWrite: ListTodo,
}

export const toolIcon = (tool: string): Component => TOOL_ICONS[tool] ?? Wrench
