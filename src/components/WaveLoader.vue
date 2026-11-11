<!-- Drei Balken statt Drehkreis: läuft ruhiger neben Text und liest sich als „arbeitet“, nicht als „lädt“ -->
<template>
  <span class="wave-bars inline-flex h-3.5 w-3.5 shrink-0 items-center justify-center gap-[2px]" role="img" aria-label="läuft">
    <i v-for="n in 3" :key="n" class="block h-3 w-[2.5px] rounded-full bg-current" />
  </span>
</template>
