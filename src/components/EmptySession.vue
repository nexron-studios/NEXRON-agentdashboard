<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import sleepingGif from '../assets/clawd-sleeping.gif'
import sleepingStill from '../assets/clawd-sleeping-still.png'

// Leerer Zustand einer frischen Session: Clawd döst, bis die erste Anfrage kommt.
defineProps<{ vscode?: boolean }>()

// Nur zur Deko: in der Eingabezeile tippt sich ein echter Befehl ein, bleibt kurz stehen und wird
// wieder gelöscht. Slash-Befehle von Claude Code, und mit „!“ Shell-Befehle (Bash-Modus von Claude
// Code, dann steht „!“ statt „>“ vorn). Kurz halten, die Zeile ist schmal (max. ~24 Zeichen).
const COMMANDS = [
  '/init',
  '/review',
  '/compact',
  '/model opus',
  '/clear',
  '/resume',
  '/context',
  '/agents',
  '/mcp',
  '/memory',
  '/config',
  '/status',
  '/permissions',
  '/doctor',
  '!git status',
  '!git diff',
  '!git log --oneline',
  '!git pull',
  '!npm run dev',
  '!npm test',
  '!npm run build',
  '!cargo check',
  '!ls',
  '!pwd',
]
const typed = ref('')
const shell = ref(false)
const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
let timer = 0

const wait = (ms: number) => new Promise<void>((r) => (timer = window.setTimeout(r, ms)))

// Keine feste Reihenfolge: jede Karte mischt die Liste für sich neu (Fisher-Yates) und spielt sie
// ohne Wiederholung ab, dann neu gemischt. Der erste Befehl einer Runde ist nie der letzte der vorigen.
let bag: string[] = []
let last = ''
const pick = () => {
  if (!bag.length) {
    bag = [...COMMANDS]
    for (let i = bag.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1))
      ;[bag[i], bag[j]] = [bag[j], bag[i]]
    }
    if (bag[bag.length - 1] === last) bag.unshift(bag.pop()!)
  }
  last = bag.pop()!
  return last
}

let alive = true
async function loop() {
  // Versetzt starten, damit nicht alle leeren Karten im Gleichtakt tippen
  await wait(400 + Math.random() * 1800)
  while (alive) {
    const raw = pick()
    shell.value = raw.startsWith('!')
    const cmd = shell.value ? raw.slice(1) : raw
    for (let n = 1; n <= cmd.length && alive; n++) {
      typed.value = cmd.slice(0, n)
      await wait(45 + Math.random() * 70)
    }
    await wait(1400 + Math.random() * 800)
    while (typed.value && alive) {
      typed.value = typed.value.slice(0, -1)
      await wait(22)
    }
    await wait(500 + Math.random() * 500)
  }
}

onMounted(() => {
  if (reduced) {
    const raw = pick()
    shell.value = raw.startsWith('!')
    typed.value = shell.value ? raw.slice(1) : raw
  } else loop()
})
onBeforeUnmount(() => {
  alive = false
  window.clearTimeout(timer)
})
</script>

<template>
  <div class="relative mt-4 flex flex-1 flex-col items-center justify-center gap-3 overflow-hidden rounded-xl border border-dashed border-line-strong/70 px-5 py-5 text-center">
    <!-- Warmer Schein hinter Clawd, nach außen ganz weg -->
    <div aria-hidden="true" class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_50%_32%,color-mix(in_oklab,var(--color-brand)_14%,transparent),transparent_55%)]" />
    <!-- Feines Punktraster, zur Mitte hin sichtbar -->
    <div
      aria-hidden="true"
      class="pointer-events-none absolute inset-0 opacity-70 [background-image:radial-gradient(var(--color-line-strong)_1px,transparent_1px)] [background-size:16px_16px] [mask-image:radial-gradient(circle_at_50%_45%,black,transparent_70%)]"
    />

    <!-- Clawd schläft (aus TokenScope); bei reduzierter Bewegung nur das erste Bild -->
    <picture class="relative">
      <source :srcset="sleepingStill" media="(prefers-reduced-motion: reduce)" />
      <img :src="sleepingGif" alt="" width="54" height="68" class="[image-rendering:pixelated]" />
    </picture>

    <div class="relative space-y-0.5">
      <p class="text-[14px] font-semibold text-ink-2">Clawd wartet auf die erste Anfrage</p>
      <p class="mx-auto max-w-[36ch] text-[12.5px] text-balance text-ink-3">
        Sobald du {{ vscode ? 'in VS Code' : 'im Terminal' }} loslegst, stehen hier Anfrage, Schritte und Kontext.
      </p>
    </div>

    <div class="relative flex w-full max-w-[220px] items-center gap-2 rounded-md border border-line-strong bg-app/60 px-2.5 py-1 font-mono text-[12px] text-ink-3">
      <span class="w-[1ch] shrink-0" :class="shell ? 'text-series-7' : 'text-brand'">{{ shell ? '!' : '>' }}</span>
      <span class="-mr-1 truncate whitespace-pre text-ink-2" aria-hidden="true">{{ typed }}</span>
      <span class="h-3.5 w-[6px] shrink-0 rounded-[1px] bg-ink-2" :class="{ 'caret-blink': !typed }" aria-hidden="true" />
    </div>
  </div>
</template>
