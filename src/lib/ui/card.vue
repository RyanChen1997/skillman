<script setup lang="ts">
import { computed } from "vue";
import { cn } from "../utils";
const cls = computed(() => cn("bg-[var(--color-surface)] border border-[var(--color-border)] rounded-lg overflow-hidden"));
</script>
<template><div :class="cls"><slot /></div></template>
