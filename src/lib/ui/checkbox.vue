<script setup lang="ts">
import { computed } from "vue";
import { cn } from "../utils";
const props = defineProps<{ modelValue: boolean; indeterminate?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [v: boolean] }>();
const cls = computed(() => cn(
  "w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center cursor-pointer transition-colors flex-shrink-0",
  (props.modelValue || props.indeterminate) ? "bg-[var(--color-accent)] border-[var(--color-accent)]" : "bg-[var(--color-bg)] border-[var(--color-border)] hover:border-[var(--color-meta)]",
));
function click() { emit("update:modelValue", !props.modelValue); }
</script>
<template>
  <span :class="cls" @click="click">
    <svg v-if="modelValue && !indeterminate" width="10" height="8" viewBox="0 0 10 8" fill="none">
      <path d="M1 4l3 3 5-6" stroke="var(--color-accent-on)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <span v-else-if="indeterminate" class="w-2 h-[1.5px] bg-[var(--color-accent-on)]" />
  </span>
</template>
