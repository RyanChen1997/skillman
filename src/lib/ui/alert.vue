<script setup lang="ts">
import { computed } from "vue";
import { cn } from "../utils";
const props = withDefaults(defineProps<{ variant?: "info" | "warn" | "danger" }>(), { variant: "info" });
const cls = computed(() => cn("flex items-center gap-3 p-3 rounded-md border text-sm",
  props.variant === "warn" && "bg-[var(--color-surface)] border-[var(--color-border)]",
  props.variant === "danger" && "text-[var(--color-danger)] border-[var(--color-border)]"));
</script>
<template><div :class="cls"><slot /></div></template>
