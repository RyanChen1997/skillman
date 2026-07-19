<script setup lang="ts">
import { ref, computed } from "vue";
import { useRoute } from "vue-router";
import { LayoutGrid, List } from "lucide-vue-next";
import { useSkillsStore } from "../stores/skills";
import { useProjectsStore } from "../stores/projects";
import SkillCard from "../components/SkillCard.vue";
import SkillRow from "../components/SkillRow.vue";
import BatchBar from "../components/BatchBar.vue";
import AddSkillsToProjectModal from "../components/AddSkillsToProjectModal.vue";
import Button from "../lib/ui/button.vue";

const skills = useSkillsStore();
const projects = useProjectsStore();
const addModalOpen = ref(false);
const route = useRoute();
const search = ref("");
const statusFilter = ref<"all" | "enabled" | "disabled">("all");
const view = ref<"grid" | "list">("grid");

const qAgent = computed(() => (route.query.agent as string) || "");
const qProject = computed(() => (route.query.project as string) || "");
const projectName = computed(() => projects.projects.find(p => p.id === qProject.value)?.name ?? qProject.value);

const filtered = computed(() => {
  let list = skills.skills;
  if (qAgent.value) list = list.filter(s => s.links.some(l => l.agentId === qAgent.value && l.scope === "global" && l.enabled));
  if (qProject.value) list = list.filter(s => s.links.some(l => l.scope === "project" && l.projectId === qProject.value && l.enabled));
  if (statusFilter.value !== "all") {
    const want = statusFilter.value === "enabled";
    list = list.filter(s => s.anyEnabled === want);
  }
  const q = search.value.trim().toLowerCase();
  if (q) list = list.filter(s => s.name.toLowerCase().includes(q) || (s.description ?? "").toLowerCase().includes(q));
  return list;
});
const enabledN = computed(() => filtered.value.filter(s => s.anyEnabled).length);
const disabledN = computed(() => filtered.value.length - enabledN.value);

function selectVisible() { skills.selectAll(filtered.value.map(s => s.id)); }
</script>

<template>
  <template v-if="!skills.skills.length">
    <div class="flex flex-col items-center justify-center h-full text-center px-12">
      <p class="text-[14.5px] text-[var(--color-muted)] mb-5">还没有导入任何 Skills</p>
      <RouterLink class="px-[18px] py-[11px] rounded-md bg-[var(--color-accent)] text-[var(--color-accent-on)]" to="/">前往 Dashboard 导入</RouterLink>
    </div>
  </template>
  <template v-else>
    <header class="px-10 pt-8 pb-6 border-b border-[var(--color-border)]">
      <div class="flex items-start justify-between">
        <div>
          <h1 class="text-[26px] font-semibold mb-1">{{ qProject ? `项目：${projectName}` : "技能库" }}</h1>
          <p class="text-[13.5px] text-[var(--color-muted)]"><span class="font-mono">{{ filtered.length }}</span> 个 Skills · <span class="font-mono">{{ enabledN }}</span> 个已启用 · <span class="font-mono">{{ disabledN }}</span> 个已禁用</p>
        </div>
        <Button v-if="qProject" size="sm" @click="addModalOpen = true">从技能库中添加</Button>
      </div>
    </header>
    <BatchBar />
    <div class="sticky top-0 z-[5] flex items-center gap-3 px-10 py-3.5 border-b border-[var(--color-border)] bg-[var(--color-bg)]">
      <div class="flex-1 max-w-[420px] relative">
        <input v-model="search" type="search" placeholder="搜索 Skill 名称、描述…" class="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] pl-9 pr-3 py-2 text-sm outline-none focus:border-[var(--color-accent)]" />
        <span class="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-muted)]">⌕</span>
      </div>
      <div class="inline-flex bg-[var(--color-surface)] border border-[var(--color-border)] rounded-md p-0.5">
        <button class="px-3 py-1 text-[13px] rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': statusFilter==='all' }" @click="statusFilter='all'">全部</button>
        <button class="px-3 py-1 text-[13px] rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': statusFilter==='enabled' }" @click="statusFilter='enabled'">已启用</button>
        <button class="px-3 py-1 text-[13px] rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': statusFilter==='disabled' }" @click="statusFilter='disabled'">已禁用</button>
      </div>
      <span class="flex-1"></span>
      <button class="text-[12.5px] text-[var(--color-muted)] hover:text-[var(--color-accent)]" @click="selectVisible">全选可见</button>
      <div class="inline-flex bg-[var(--color-surface)] border border-[var(--color-border)] rounded-md p-0.5">
        <button class="w-7 h-7 grid place-items-center rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': view==='grid' }" @click="view='grid'" title="网格">
          <LayoutGrid class="w-4 h-4" />
        </button>
        <button class="w-7 h-7 grid place-items-center rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': view==='list' }" @click="view='list'" title="列表">
          <List class="w-4 h-4" />
        </button>
      </div>
    </div>
    <div class="p-10">
      <div v-if="view==='grid'" class="grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(296px,1fr))]">
        <SkillCard v-for="s in filtered" :key="s.id" :skill="s" />
      </div>
      <div v-else class="border border-[var(--color-border)] rounded-lg overflow-hidden">
        <SkillRow v-for="s in filtered" :key="s.id" :skill="s" />
      </div>
      <div v-if="!filtered.length" class="text-center py-16 text-[var(--color-muted)]">没有匹配的 Skills</div>
    </div>
    <AddSkillsToProjectModal v-if="qProject" :project-id="qProject" v-model:open="addModalOpen" @added="skills.load()" />
  </template>
</template>
