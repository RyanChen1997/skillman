<script setup lang="ts">
import { computed } from "vue";
import { useSkillsStore } from "../stores/skills";
import Button from "../lib/ui/button.vue";

const skills = useSkillsStore();
const count = computed(() => skills.selectedIds.size);
</script>

<template>
  <div v-if="count > 0" class="sticky top-0 z-[6] flex items-center gap-3 px-10 py-2.5 bg-[var(--color-accent)]/10 border-b border-[var(--color-accent)] text-sm">
    <span class="font-semibold text-[var(--color-accent)]">{{ count }}</span> 个已选中
    <Button variant="ghost" size="sm" @click="skills.batchEnable(true)">启用</Button>
    <Button variant="ghost" size="sm" @click="skills.batchEnable(false)">禁用</Button>
    <span class="flex-1"></span>
    <button class="text-[13px] text-[var(--color-muted)] hover:text-[var(--color-fg)]" @click="skills.clearSelection()">清除选择</button>
  </div>
</template>
