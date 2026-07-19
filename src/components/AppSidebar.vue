<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRouter, useRoute } from "vue-router";
import { useAgentsStore } from "../stores/agents";
import { useProjectsStore } from "../stores/projects";
import { useSettingsStore } from "../stores/settings";
import { useSkillsStore } from "../stores/skills";
import AgentIcon from "./AgentIcon.vue";

const agents = useAgentsStore();
const projects = useProjectsStore();
const settings = useSettingsStore();
const skills = useSkillsStore();
const router = useRouter();
const route = useRoute();

const shownAgents = computed(() =>
  (settings.showUninstalled ? agents.agents : agents.installedAgents).filter(a => !a.sourceOnly));

const activeAgent = computed(() => (route.query.agent as string) || "");
const activeProject = computed(() => (route.query.project as string) || "");

function goAgent(id: string) { router.push({ path: "/skills", query: { agent: id } }); }
function goProject(id: string) { router.push({ path: "/skills", query: { project: id } }); }

function agentSkillCount(agentId: string): number {
  return skills.skills.filter(s => s.links.some(l => l.agentId === agentId && l.scope === "global" && l.enabled)).length;
}
function projectSkillCount(projectId: string): number {
  return skills.skills.filter(s => s.links.some(l => l.scope === "project" && l.projectId === projectId && l.enabled)).length;
}
</script>

<template>
  <aside class="flex flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)] overflow-hidden h-screen">
    <div class="flex items-center gap-2.5 px-4 py-4">
      <span class="w-[26px] h-[26px] rounded-[7px] bg-[var(--color-accent)] text-[var(--color-accent-on)] grid place-items-center font-bold text-sm">S</span>
      <span class="text-[14.5px] font-semibold text-[var(--color-fg)]">SkillMan</span>
    </div>
    <div class="flex-1 overflow-y-auto pb-4">
      <nav class="px-3">
        <RouterLink class="flex items-center gap-2.5 px-2.5 py-[7px] rounded-md text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)] text-sm" active-class="!bg-[var(--color-surface-2)] !text-[var(--color-fg)]" to="/" exact-active-class="!bg-[var(--color-surface-2)] !text-[var(--color-fg)]">
          <span class="text-base">▦</span><span>Dashboard</span>
        </RouterLink>
        <RouterLink class="flex items-center gap-2.5 px-2.5 py-[7px] rounded-md text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)] text-sm" active-class="!bg-[var(--color-surface-2)] !text-[var(--color-fg)]" to="/skills">
          <span class="text-base">◆</span><span>技能库</span>
          <span v-if="skills.enabledCount" class="ml-auto text-[11px] text-[var(--color-meta)]">{{ skills.enabledCount }}</span>
        </RouterLink>
      </nav>

      <div v-if="shownAgents.length" class="mt-1 px-3 pt-3 border-t border-[var(--color-border-soft)]">
        <div class="text-[11.5px] text-[var(--color-meta)] px-2.5 py-1.5 font-medium">全局工作区</div>
        <a v-for="a in shownAgents" :key="a.id" class="flex items-center gap-2.5 pl-[30px] pr-2.5 py-[7px] rounded-md text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)] text-[13.5px] cursor-pointer"
          :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': activeAgent === a.id }" @click="goAgent(a.id)">
          <AgentIcon :agent-id="a.id" class="w-4 h-4" />
          <span>{{ a.name }}</span>
          <span v-if="!a.installed" class="ml-auto text-[10.5px] text-[var(--color-warn)]">未安装</span>
          <span v-else-if="agentSkillCount(a.id)" class="ml-auto text-[11px] text-[var(--color-meta)]">{{ agentSkillCount(a.id) }}</span>
        </a>
      </div>

      <div class="mt-1 px-3 pt-3 border-t border-[var(--color-border-soft)]">
        <div class="text-[11.5px] text-[var(--color-meta)] px-2.5 py-1.5 font-medium">项目工作区</div>
        <a v-for="p in projects.projects" :key="p.id" class="flex items-center gap-2.5 pl-[30px] pr-2.5 py-[7px] rounded-md text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)] text-[13.5px] cursor-pointer"
          :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': activeProject === p.id }" @click="goProject(p.id)">
          <span class="text-base">📁</span><span>{{ p.name }}</span>
          <span v-if="projectSkillCount(p.id)" class="ml-auto text-[11px] text-[var(--color-meta)]">{{ projectSkillCount(p.id) }}</span>
        </a>
        <a class="flex items-center gap-2.5 pl-[30px] pr-2.5 py-[7px] rounded-md text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)] text-[13.5px] cursor-pointer" @click="projects.linkModalOpen = true">
          <span class="text-base">＋</span><span>关联项目</span>
        </a>
      </div>
    </div>
    <div class="border-t border-[var(--color-border-soft)] p-3">
      <RouterLink class="flex items-center gap-2.5 px-2.5 py-[7px] rounded-md text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)] text-sm" active-class="!bg-[var(--color-surface-2)] !text-[var(--color-fg)]" to="/settings">
        <span class="text-base">⚙</span><span>设置</span>
      </RouterLink>
    </div>
  </aside>
</template>
