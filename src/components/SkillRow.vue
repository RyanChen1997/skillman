<script setup lang="ts">
import { computed } from "vue";
import { RouterLink } from "vue-router";
import type { SkillView } from "../stores/types";
import { useSkillsStore } from "../stores/skills";
import { useAgentsStore } from "../stores/agents";
import { useSettingsStore } from "../stores/settings";
import AgentIcon from "./AgentIcon.vue";

const props = defineProps<{ skill: SkillView }>();
const skills = useSkillsStore();
const agents = useAgentsStore();
const settings = useSettingsStore();
const isSelected = computed(() => skills.selectedIds.has(props.skill.id));
const agentList = computed(() =>
  (settings.showUninstalled ? agents.agents : agents.installedAgents).filter(a => !a.sourceOnly)
);
function onToggle(e: Event) { e.preventDefault(); e.stopPropagation(); skills.toggleSelect(props.skill.id); }
function isGlobalOn(agentId: string): boolean { return skills.isLinkOn(props.skill, "global", null, agentId); }
async function onToggleAgent(e: Event, agentId: string) {
  e.preventDefault(); e.stopPropagation();
  await skills.setLink(props.skill.id, "global", null, agentId, !isGlobalOn(agentId));
}
async function onAggregate(e: Event) {
  e.preventDefault();
  e.stopPropagation();
  await skills.toggleAggregate(props.skill.id);
}
</script>

<template>
  <RouterLink :to="`/skills/${skill.id}`" class="grid grid-cols-[24px_32px_1.4fr_1fr_0.7fr_0.5fr] items-center gap-3 p-3 border-b border-[var(--color-border-soft)] last:border-b-0 bg-[var(--color-surface)] hover:bg-[var(--color-surface-2)]"
    :class="[skill.anyEnabled ? '' : 'opacity-55', isSelected ? '!border-[var(--color-accent)]' : '']">
    <span class="w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center cursor-pointer" :class="isSelected ? 'bg-[var(--color-accent)] border-[var(--color-accent)]' : 'border-[var(--color-border)]'" @click="onToggle">
      <svg v-if="isSelected" width="10" height="8" viewBox="0 0 10 8" fill="none"><path d="M1 4l3 3 5-6" stroke="var(--color-accent-on)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </span>
    <span class="w-7 h-7 rounded bg-[var(--color-surface-2)] grid place-items-center font-mono text-xs font-semibold text-[var(--color-muted)]">{{ skill.name.charAt(0).toUpperCase() }}</span>
    <span class="font-mono font-semibold text-sm">{{ skill.name }}</span>
    <span class="text-[12.5px] text-[var(--color-muted)] truncate">{{ skill.description ?? "" }}</span>
    <span class="flex gap-1">
      <button v-for="a in agentList" :key="a.id"
        class="inline-flex items-center px-1 py-0.5 rounded border transition-colors cursor-pointer"
        :class="isGlobalOn(a.id) ? 'bg-[var(--color-success)]/10 border-[var(--color-success)]/25 text-[var(--color-success)]' : 'bg-[var(--color-surface-2)] border-[var(--color-border)] text-[var(--color-meta)]'"
        :title="a.name + (isGlobalOn(a.id) ? ' · 全局已开启,点击关闭' : ' · 点击开启全局开关')"
        @click="onToggleAgent($event, a.id)">
        <AgentIcon :agent-id="a.id" class="w-3.5 h-3.5" />
      </button>
    </span>
    <span class="text-[11.5px] text-right cursor-pointer"
      :class="skill.anyEnabled ? 'text-[var(--color-success)]' : 'text-[var(--color-meta)]'"
      @click="onAggregate">
      {{ skill.anyEnabled ? "已启用" : "已禁用" }}
    </span>
  </RouterLink>
</template>
