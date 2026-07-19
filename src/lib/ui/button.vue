<script setup lang="ts">
import { computed } from "vue";
import { cn } from "../utils";
const props = withDefaults(defineProps<{
  variant?: "default" | "secondary" | "ghost" | "destructive";
  size?: "default" | "sm";
  as?: string;
}>(), { variant: "default", size: "default", as: "button" });
const cls = computed(() => cn(
  "inline-flex items-center justify-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-fg)] cursor-pointer transition-colors duration-100 whitespace-nowrap font-medium",
  "hover:bg-[var(--color-surface-2)] hover:border-[var(--color-meta)]",
  "disabled:opacity-40 disabled:cursor-not-allowed",
  props.size === "sm" ? "px-3 py-1.5 text-[13px]" : "px-[18px] py-[11px] text-sm",
  props.variant === "default" && "bg-[var(--color-accent)] text-[var(--color-accent-on)] border-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] hover:border-[var(--color-accent-hover)]",
  props.variant === "ghost" && "bg-transparent border-[var(--color-border)] hover:bg-[var(--color-surface-2)]",
  props.variant === "secondary" && "bg-[var(--color-surface-2)] hover:bg-[var(--color-border)]",
  props.variant === "destructive" && "text-[var(--color-danger)] border-[var(--color-border)] hover:bg-[var(--color-surface-2)]",
));
</script>
<template>
  <component :is="as" :class="cls"><slot /></component>
</template>
