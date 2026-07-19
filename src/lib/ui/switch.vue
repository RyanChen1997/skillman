<script setup lang="ts">
import { computed } from "vue";
import { cn } from "../utils";
const props = defineProps<{ modelValue: boolean; disabled?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [v: boolean] }>();
const cls = computed(() => cn(
  "relative inline-flex h-[18px] w-8 flex-shrink-0 rounded-full border cursor-pointer transition-colors duration-100",
  props.modelValue ? "bg-[var(--color-accent)] border-[var(--color-accent)]" : "bg-[var(--color-surface-2)] border-[var(--color-border)]",
  props.disabled && "opacity-45 cursor-not-allowed",
));
function toggle() { if (!props.disabled) emit("update:modelValue", !props.modelValue); }
</script>
<template>
  <button type="button" :class="cls" :disabled="disabled" @click="toggle"
    :style="{ '--tw': '0' }">
    <span class="absolute top-[1px] left-[1px] h-[14px] w-[14px] rounded-full bg-[var(--color-accent-on)] transition-transform duration-100"
      :style="{ transform: modelValue ? 'translateX(14px)' : 'translateX(0)', background: modelValue ? 'var(--color-accent-on)' : 'var(--color-meta)' }" />
  </button>
</template>
