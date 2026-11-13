<template>
  <!-- Stiller Platzhalter für freie Plätze in der letzten Kartenreihe. Ohne Puls, sonst sähe er nach
       Laden aus. Er übernimmt den Aufbau von SessionCard und wird über das Grid so hoch wie die Reihe. -->
  <div class="card flex flex-col p-5 opacity-50" aria-hidden="true">
    <div class="flex items-center gap-3">
      <span class="h-3.5 w-36 rounded bg-surface-2" />
      <span class="ml-auto h-6 w-20 rounded-full bg-surface-2" />
    </div>
    <span class="mt-5 h-4 w-3/4 rounded bg-surface-2" />
    <span class="mt-3 h-3 w-1/3 rounded bg-surface-2" />
    <span class="mt-5 h-1.5 w-full rounded-full bg-surface-2" />
    <div class="mt-auto flex items-center gap-3 border-t border-line pt-5">
      <span class="h-9 w-36 rounded-lg bg-surface-2" />
      <span class="ml-auto h-9 w-28 rounded-lg bg-surface-2" />
    </div>
  </div>
</template>
