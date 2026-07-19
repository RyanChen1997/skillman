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

function onToggleSelect(e: Event) { e.preventDefault(); e.stopPropagation(); skills.toggleSelect(props.skill.id); }
async function onAggregate(e: Event) { e.preventDefault(); e.stopPropagation(); await skills.toggleAggregate(props.skill.id); }

// Toggle this skill's GLOBAL link for a given agent, directly from the card
// (without navigating into the detail page).
function isGlobalOn(agentId: string): boolean {
  return skills.isLinkOn(props.skill, "global", null, agentId);
}
async function onToggleAgent(e: Event, agentId: string) {
  e.preventDefault(); e.stopPropagation();
  await skills.setLink(props.skill.id, "global", null, agentId, !isGlobalOn(agentId));
}
</script>

<template>
  <RouterLink :to="`/skills/${skill.id}`"
    class="block p-[18px] border rounded-lg bg-[var(--color-surface)] hover:border-[var(--color-meta)] transition-colors"
    :class="[skill.anyEnabled ? '' : 'opacity-55', isSelected ? '!border-[var(--color-accent)] bg-[var(--color-accent)]/5' : '']">
    <div class="flex items-center gap-2.5 mb-3">
      <span class="w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center cursor-pointer flex-shrink-0"
        :class="isSelected ? 'bg-[var(--color-accent)] border-[var(--color-accent)]' : 'border-[var(--color-border)] hover:border-[var(--color-meta)]'" @click="onToggleSelect">
        <svg v-if="isSelected" width="10" height="8" viewBox="0 0 10 8" fill="none"><path d="M1 4l3 3 5-6" stroke="var(--color-accent-on)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </span>
      <span class="w-8 h-8 rounded-md bg-[var(--color-surface-2)] grid place-items-center font-mono text-[12.5px] font-semibold text-[var(--color-muted)]">{{ skill.name.charAt(0).toUpperCase() }}</span>
      <span class="flex-1 min-w-0 font-mono font-semibold text-[14.5px] truncate">{{ skill.name }}</span>
      <span class="text-[11.5px] font-medium px-2.5 py-0.5 rounded-full border cursor-pointer"
        :class="skill.anyEnabled ? 'text-[var(--color-success)] border-[var(--color-success)]/35 bg-[var(--color-success)]/14' : 'text-[var(--color-meta)] border-[var(--color-border)] bg-[var(--color-surface-2)]'"
        @click="onAggregate">{{ skill.anyEnabled ? "已启用" : "已禁用" }}</span>
    </div>
    <p class="text-[13px] text-[var(--color-muted)] leading-relaxed line-clamp-2 mb-2">{{ skill.description ?? "无描述" }}</p>
    <div class="flex items-center gap-1.5 flex-wrap mt-2">
      <button v-for="a in agentList" :key="a.id"
        class="inline-flex items-center gap-1 px-1.5 py-1 rounded border transition-colors cursor-pointer"
        :class="isGlobalOn(a.id) ? 'bg-[var(--color-success)]/10 border-[var(--color-success)]/25 text-[var(--color-success)]' : 'bg-[var(--color-surface-2)] border-[var(--color-border)] text-[var(--color-meta)]'"
        :title="a.name + (isGlobalOn(a.id) ? ' · 全局已开启,点击关闭' : ' · 点击开启全局开关')"
        @click="onToggleAgent($event, a.id)">
        <AgentIcon :agent-id="a.id" class="w-3.5 h-3.5" />
        <span class="text-[10.5px] ml-0.5">{{ isGlobalOn(a.id) ? '开' : '关' }}</span>
      </button>
    </div>
  </RouterLink>
</template>
