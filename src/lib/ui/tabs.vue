<script setup lang="ts">
import { provide, type Ref } from "vue";
const active = defineModel<string>();
const set = (v: string) => { active.value = v; };
provide("tabs", { active: active as Ref<string>, set });
</script>
<template>
  <div class="flex gap-1 border-b border-[var(--color-border)] mb-6">
    <slot />
  </div>
</template>
