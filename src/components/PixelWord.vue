<script setup lang="ts">
import { computed } from 'vue'

// Pixel lettering as in the homeserver header: white block letters, dark outline, offset shadow in
// the brand colour, a dot at the end. SVG instead of a web font, so the app stays offline and the
// edges stay sharp at any size. Only the glyphs that are needed.
const GLYPHS: Record<string, string[]> = {
  A: ['.###.', '#...#', '#####', '#...#', '#...#'],
  B: ['####.', '#...#', '####.', '#...#', '####.'],
  D: ['####.', '#...#', '#...#', '#...#', '####.'],
  H: ['#...#', '#...#', '#####', '#...#', '#...#'],
  O: ['.###.', '#...#', '#...#', '#...#', '.###.'],
  R: ['####.', '#...#', '####.', '#..#.', '#...#'],
  S: ['.####', '#....', '.###.', '....#', '####.'],
}
const ROWS = 5
const GAP = 1
const SHADOW = 0.6
const OUTLINE = 0.12

const props = defineProps<{ text: string; height?: number }>()

const layout = computed(() => {
  const cells: { x: number; y: number }[] = []
  let x = 0
  for (const ch of props.text.toUpperCase()) {
    const glyph = GLYPHS[ch]
    if (!glyph) {
      if (import.meta.env.DEV) console.warn(`PixelWord: no glyph for "${ch}"`)
      continue
    }
    glyph.forEach((row, y) => [...row].forEach((c, dx) => c === '#' && cells.push({ x: x + dx, y })))
    x += glyph[0].length + GAP
  }
  // Dot: one rounded cell in the brand colour, on the baseline
  const dot = { x, y: ROWS - 1 }
  return { cells, dot, width: x + 1 + SHADOW + OUTLINE }
})
</script>

<template>
  <svg
    :viewBox="`${-OUTLINE} ${-OUTLINE} ${layout.width + OUTLINE} ${ROWS + SHADOW + OUTLINE * 2}`"
    :style="{ height: `${height ?? 40}px` }"
    class="block w-auto overflow-visible"
    role="img"
    :aria-label="text"
    shape-rendering="crispEdges"
  >
    <g class="fill-brand">
      <rect v-for="(c, i) in layout.cells" :key="`s${i}`" :x="c.x + SHADOW" :y="c.y + SHADOW" width="1.02" height="1.02" />
    </g>
    <g class="fill-app">
      <rect
        v-for="(c, i) in layout.cells"
        :key="`o${i}`"
        :x="c.x - OUTLINE"
        :y="c.y - OUTLINE"
        :width="1 + OUTLINE * 2"
        :height="1 + OUTLINE * 2"
      />
    </g>
    <g class="fill-ink">
      <rect v-for="(c, i) in layout.cells" :key="`f${i}`" :x="c.x" :y="c.y" width="1.02" height="1.02" />
    </g>
    <rect
      class="fill-brand"
      :x="layout.dot.x + SHADOW / 2"
      :y="layout.dot.y + SHADOW / 2"
      width="1.1"
      height="1.1"
      rx="0.28"
      shape-rendering="geometricPrecision"
    />
  </svg>
</template>
