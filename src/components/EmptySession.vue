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
  <!-- Größen-Container: zählt nicht zur Höhe der Karte, sondern füllt nur den Platz, den die
       Kartenreihe hergibt. Steht die Karte neben einer laufenden Session, wird sie so hoch wie diese;
       sind alle Karten der Reihe frisch, bleibt es bei der kompakten Mindesthöhe. -->
  <div class="empty relative mt-4 flex min-h-[176px] flex-1 overflow-hidden rounded-xl border border-dashed border-line-strong/70">
    <!-- Warmer Schein hinter Clawd, nach außen ganz weg -->
    <div aria-hidden="true" class="empty-glow pointer-events-none absolute inset-0" />
    <!-- Feines Punktraster, zur Mitte hin sichtbar -->
    <div aria-hidden="true" class="empty-dots pointer-events-none absolute inset-0 opacity-70" />

    <div class="empty-body relative flex flex-1 flex-col items-center justify-center gap-3 px-5 py-4 text-center">
      <!-- Clawd schläft (aus TokenScope); bei reduzierter Bewegung nur das erste Bild -->
      <picture class="shrink-0">
        <source :srcset="sleepingStill" media="(prefers-reduced-motion: reduce)" />
        <img :src="sleepingGif" alt="" width="54" height="68" class="[image-rendering:pixelated]" />
      </picture>

      <div class="empty-text flex min-w-0 flex-col items-center gap-3">
        <div class="space-y-0.5">
          <p class="text-[14px] font-semibold text-ink-2">Clawd wartet auf die erste Anfrage</p>
          <p class="empty-hint mx-auto max-w-[36ch] text-[12.5px] text-balance text-ink-3">
            Sobald du {{ vscode ? 'in VS Code' : 'im Terminal' }} loslegst, stehen hier Anfrage, Schritte und Kontext.
          </p>
        </div>

        <div class="flex w-full max-w-[220px] items-center gap-2 rounded-md border border-line-strong bg-app/60 px-2.5 py-1 font-mono text-[12px] text-ink-3">
          <span class="w-[1ch] shrink-0" :class="shell ? 'text-series-7' : 'text-brand'">{{ shell ? '!' : '>' }}</span>
          <span class="-mr-1 truncate whitespace-pre text-ink-2" aria-hidden="true">{{ typed }}</span>
          <span class="h-3.5 w-[6px] shrink-0 rounded-[1px] bg-ink-2" :class="{ 'caret-blink': !typed }" aria-hidden="true" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.empty {
  container-type: size;
  --gx: 50%;
  --gy: 32%;
}
.empty-glow {
  background: radial-gradient(circle at var(--gx) var(--gy), color-mix(in oklab, var(--color-brand) 14%, transparent), transparent 55%);
}
.empty-dots {
  background-image: radial-gradient(var(--color-line-strong) 1px, transparent 1px);
  background-size: 16px 16px;
  mask-image: radial-gradient(circle at var(--gx) 45%, black, transparent 70%);
}
/* Wenig Platz: erst der Hinweistext weg … */
@container (max-height: 236px) {
  .empty-hint {
    display: none;
  }
}
/* … und noch enger: Clawd etwas kleiner, alles bleibt mittig untereinander */
@container (max-height: 168px) {
  .empty-body {
    gap: 0.5rem;
    padding-block: 0.75rem;
  }
  .empty-body img {
    width: 40px;
    height: 50px;
  }
}
</style>
