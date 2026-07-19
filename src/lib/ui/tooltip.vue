<script setup lang="ts">
defineProps<{ text: string }>();
</script>
<template>
  <span class="group relative inline-flex">
    <slot />
    <span class="pointer-events-none absolute left-1/2 -translate-x-1/2 bottom-full mb-1 whitespace-nowrap rounded bg-[var(--color-surface-2)] border border-[var(--color-border)] px-2 py-1 text-[11px] text-[var(--color-fg)] opacity-0 group-hover:opacity-100 transition-opacity">
      {{ text }}
    </span>
  </span>
</template>
