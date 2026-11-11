<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { TooltipProvider, TooltipRoot } from 'reka-ui'
import TooltipContent from './ui/tooltip/TooltipContent.vue'

// Ein Tooltip für die ganze App: Jedes Element mit `title` bekommt statt des grauen Windows-Kästchens
// den shadcn-Tooltip. Der Text wandert dafür von `title` nach `data-tip`, damit der Browser keinen
// eigenen mehr zeigt. Komponenten schreiben weiter einfach `title="…"`, auch mit Zeilenumbrüchen.
// Steht der Tooltip an einem gekürzten Text (`truncate`, `line-clamp-*`), erscheint er nur, wenn der Text
// gerade wirklich abgeschnitten ist; sonst wiederholt er bloß, was schon dasteht.
const DELAY_MS = 600
const anchor = shallowRef<HTMLElement>()
const text = ref('')
const open = ref(false)
let current: HTMLElement | null = null
let timer = 0

function claim(el: HTMLElement): string {
  const t = el.getAttribute('title')
  if (t !== null) {
    if (t) el.dataset.tip = t
    else delete el.dataset.tip
    el.removeAttribute('title')
  }
  return el.dataset.tip ?? ''
}

const isClipText = (el: HTMLElement) => {
  const css = getComputedStyle(el)
  return css.textOverflow === 'ellipsis' || !['', 'none'].includes(css.getPropertyValue('-webkit-line-clamp'))
}
const isClipped = (el: HTMLElement) => el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1

function show(el: HTMLElement, delay: number) {
  const t = claim(el)
  window.clearTimeout(timer)
  if (!t || (isClipText(el) && !isClipped(el))) return void (open.value = false)
  // Von Tooltip zu Tooltip wandern: sofort, wie bei shadcn mit skipDelayDuration
  timer = window.setTimeout(
    () => {
      anchor.value = el
      text.value = t
      open.value = true
    },
    open.value ? 0 : delay,
  )
}

function hide() {
  window.clearTimeout(timer)
  current = null
  open.value = false
}

const target = (e: Event) => (e.target instanceof Element ? e.target.closest<HTMLElement>('[title], [data-tip]') : null)
const contains = (el: Element, x: number, y: number) => {
  const r = el.getBoundingClientRect()
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}

// Nicht `e.target`: Über jeder Karte liegt das Klickfeld des Titels (`.card-link::after`), das würde
// überall den Titel melden. Gesucht wird das oberste Element mit Tooltip, das wirklich unter dem Zeiger liegt.
function tipAt(x: number, y: number) {
  for (const hit of document.elementsFromPoint(x, y)) {
    const el = hit.closest<HTMLElement>('[title], [data-tip]')
    if (el && contains(el, x, y)) return el
  }
  return null
}

let frame = 0
function onMove(e: PointerEvent) {
  if (e.pointerType === 'touch') return
  const { clientX: x, clientY: y } = e
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => {
    const el = tipAt(x, y)
    if (el === current) return
    if (!el) return hide()
    current = el
    show(el, DELAY_MS)
  })
}
function onOut(e: PointerEvent) {
  if (!e.relatedTarget) hide()
}
function onFocusIn(e: FocusEvent) {
  const el = target(e)
  if (!el || !(e.target as HTMLElement).matches(':focus-visible')) return
  current = el
  show(el, 0)
}

// Vue setzt `title` neu, wenn sich der Text ändert: sofort wieder übernehmen, auch am offenen Tooltip
const observer = new MutationObserver((list) => {
  for (const m of list) {
    const el = m.target as HTMLElement
    if (!el.hasAttribute('title')) continue
    const t = claim(el)
    if (el === anchor.value && open.value) {
      if (t) text.value = t
      else hide()
    }
  }
})

onMounted(() => {
  document.addEventListener('pointermove', onMove)
  document.addEventListener('pointerout', onOut)
  document.addEventListener('focusin', onFocusIn)
  document.addEventListener('focusout', hide)
  document.addEventListener('pointerdown', hide)
  document.addEventListener('scroll', hide, true)
  observer.observe(document.body, { subtree: true, attributes: true, attributeFilter: ['title'] })
})
onBeforeUnmount(() => {
  document.removeEventListener('pointermove', onMove)
  document.removeEventListener('pointerout', onOut)
  document.removeEventListener('focusin', onFocusIn)
  document.removeEventListener('focusout', hide)
  document.removeEventListener('pointerdown', hide)
  document.removeEventListener('scroll', hide, true)
  observer.disconnect()
  window.clearTimeout(timer)
  cancelAnimationFrame(frame)
})
</script>

<template>
  <TooltipProvider>
    <TooltipRoot :open="open" disable-hoverable-content @update:open="(v) => !v && hide()">
      <TooltipContent v-if="anchor" :reference="anchor">{{ text }}</TooltipContent>
    </TooltipRoot>
  </TooltipProvider>
</template>
