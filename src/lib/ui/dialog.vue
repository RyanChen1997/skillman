<script setup lang="ts">
import { watch, computed } from "vue";
const props = defineProps<{ open: boolean; size?: "sm" | "md" | "lg" }>();
const emit = defineEmits<{ "update:open": [v: boolean] }>();
function close() { emit("update:open", false); }
watch(() => props.open, (o) => {
  if (o) document.body.style.overflow = "hidden"; else document.body.style.overflow = "";
});
const widthClass = computed(() => {
  switch (props.size) {
    case "lg": return "w-[min(720px,calc(100%-48px))]";
    case "sm": return "w-[min(360px,calc(100%-48px))]";
    default: return "w-[min(440px,calc(100%-48px))]";
  }
});
</script>
<template>
  <Teleport to="body">
    <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center bg-black/55" @click.self="close">
      <div :class="['bg-[var(--color-surface)] border border-[var(--color-border)] rounded-lg p-5', widthClass]">
        <slot />
      </div>
    </div>
  </Teleport>
</template>
