<script setup lang="ts">
import { computed, ref, onMounted } from "vue";
import { useRoute, useRouter, RouterLink } from "vue-router";
import { useSkillsStore } from "../stores/skills";
import { useAgentsStore } from "../stores/agents";
import { useProjectsStore } from "../stores/projects";
import { useSettingsStore } from "../stores/settings";
import { readSkillMdSource } from "../lib/tauri";
import AgentIcon from "../components/AgentIcon.vue";
import MarkdownPreview from "../components/MarkdownPreview.vue";
import Button from "../lib/ui/button.vue";
import Switch from "../lib/ui/switch.vue";

const route = useRoute();
const router = useRouter();
const skills = useSkillsStore();
const agents = useAgentsStore();
const projects = useProjectsStore();
const settings = useSettingsStore();

const skillId = computed(() => route.params.id as string);
const skill = computed(() => skills.get(skillId.value));
const previewMode = ref<"rendered" | "source">("rendered");
const mdSource = ref("");

async function loadMd() {
  if (skill.value) mdSource.value = (await readSkillMdSource(skill.value.id)) ?? "";
}
onMounted(loadMd);

function goBack(e: Event) { e.preventDefault(); window.history.length > 1 ? router.back() : router.push("/skills"); }

const shownAgents = computed(() =>
  (settings.showUninstalled ? agents.agents : agents.installedAgents).filter(a => !a.sourceOnly)
);

function isGlobalOn(agentId: string): boolean {
  return skill.value ? skills.isLinkOn(skill.value, "global", null, agentId) : false;
}
async function toggleGlobal(agentId: string, on: boolean) {
  if (skill.value) await skills.setLink(skill.value.id, "global", null, agentId, on);
}
function isProjectOn(projectId: string, agentId: string): boolean {
  return skill.value ? skills.isLinkOn(skill.value, "project", projectId, agentId) : false;
}
async function toggleProject(projectId: string, agentId: string, on: boolean) {
  if (skill.value) await skills.setLink(skill.value.id, "project", projectId, agentId, on);
}
async function onAggregate() { if (skill.value) await skills.toggleAggregate(skill.value.id); }
async function onDelete() { if (skill.value) { await skills.removeSkill(skill.value.id); router.push("/skills"); } }
async function onRestore() { if (skill.value) { await skills.doRestore(skill.value.id); router.push("/skills"); } }
</script>

<template>
  <template v-if="skill">
    <header class="px-10 pt-8 pb-6 border-b border-[var(--color-border)] flex items-start gap-6">
      <button class="w-9 h-9 rounded-md border border-[var(--color-border)] text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)]" @click="goBack">←</button>
      <div class="flex-1">
        <p class="text-[13.5px] text-[var(--color-muted)] mb-2"><RouterLink to="/skills" class="text-[var(--color-muted)]">技能库</RouterLink> <span class="text-[var(--color-meta)]">/</span> <span class="font-mono">{{ skill.name }}</span></p>
        <div class="flex items-center gap-3 flex-wrap">
          <h1 class="text-[26px] font-semibold">{{ skill.name }}</h1>
          <span class="text-[11.5px] font-medium px-2.5 py-0.5 rounded-full border cursor-pointer"
            :class="skill.anyEnabled ? 'text-[var(--color-success)] border-[var(--color-success)]/35 bg-[var(--color-success)]/14' : 'text-[var(--color-meta)] border-[var(--color-border)] bg-[var(--color-surface-2)]'"
            @click="onAggregate">{{ skill.anyEnabled ? "已启用" : "已禁用" }}</span>
          <span v-if="skill.source" class="text-[11px] px-2 py-0.5 rounded bg-[var(--color-accent)]/12 border border-[var(--color-accent)]/25 text-[var(--color-accent)]">{{ skill.source }}</span>
        </div>
      </div>
      <div class="flex gap-2">
        <Button variant="ghost" size="sm" @click="onRestore">恢复</Button>
        <Button variant="destructive" size="sm" @click="onDelete">删除</Button>
      </div>
    </header>

    <div class="p-10">
      <section class="mb-8">
        <h2 class="text-sm font-semibold mb-3">描述</h2>
        <div class="p-4 border border-[var(--color-border)] rounded-md bg-[var(--color-surface)] text-sm leading-7">
          <p>{{ skill.description ?? "无描述" }}</p>
        </div>
      </section>

      <section class="mb-8">
        <div class="p-4 border border-[var(--color-border)] rounded-md bg-[var(--color-surface)]">
          <h3 class="text-[12.5px] font-semibold text-[var(--color-muted)] uppercase tracking-wide mb-2.5">全局工作区</h3>
          <div class="flex items-center gap-2.5 flex-wrap">
            <div v-for="a in shownAgents" :key="a.id" class="inline-flex items-center gap-2 px-2 py-1.5 rounded-md border"
              :class="isGlobalOn(a.id) ? 'border-[var(--color-success)]/35 bg-[var(--color-success)]/12' : 'border-[var(--color-border)] bg-[var(--color-surface-2)]'"
              :style="{ opacity: a.installed ? 1 : 0.45, cursor: a.installed ? 'pointer' : 'not-allowed' }"
              @click="a.installed && toggleGlobal(a.id, !isGlobalOn(a.id))">
              <AgentIcon :agent-id="a.id" class="w-4 h-4" :class="isGlobalOn(a.id) ? 'text-[var(--color-success)]' : 'text-[var(--color-muted)]'" />
              <span class="text-[13px]" :class="isGlobalOn(a.id) ? 'text-[var(--color-success)]' : 'text-[var(--color-muted)]'">{{ a.name }}</span>
              <Switch :model-value="isGlobalOn(a.id)" :disabled="!a.installed" @update:model-value="(v: boolean) => toggleGlobal(a.id, v)" />
            </div>
          </div>
        </div>
      </section>

      <section class="mb-8">
        <div class="flex items-center gap-2.5 mb-3">
          <h3 class="text-[12.5px] font-semibold text-[var(--color-muted)] uppercase tracking-wide">项目工作区</h3>
          <span class="flex-1"></span>
          <Button variant="ghost" size="sm" @click="projects.linkModalOpen = true">关联项目</Button>
        </div>
        <div v-if="!projects.projects.length" class="p-6 text-center text-[var(--color-muted)] text-[13px] border border-[var(--color-border)] rounded-md">还没有关联项目</div>
        <div v-else class="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(280px,1fr))]">
          <div v-for="p in projects.projects" :key="p.id" class="p-3.5 border border-[var(--color-border)] rounded-md bg-[var(--color-surface)]">
            <div class="flex items-center justify-between mb-3">
              <span class="text-sm font-semibold">📁 {{ p.name }}</span>
              <span class="font-mono text-[11px] text-[var(--color-meta)] truncate ml-2">{{ p.path }}</span>
            </div>
            <div class="flex flex-wrap gap-1.5">
              <button v-for="a in shownAgents" :key="a.id" class="inline-flex items-center gap-1.5 px-2 py-1 rounded border text-[11.5px] transition-colors"
                :class="isProjectOn(p.id, a.id) ? 'text-[var(--color-success)] border-[var(--color-success)]/25 bg-[var(--color-success)]/10' : 'text-[var(--color-muted)] border-[var(--color-border)] bg-[var(--color-surface-2)]'"
                :style="{ opacity: a.installed ? 1 : 0.45, cursor: a.installed ? 'pointer' : 'not-allowed' }"
                @click="a.installed && toggleProject(p.id, a.id, !isProjectOn(p.id, a.id))">
                <AgentIcon :agent-id="a.id" class="w-3 h-3" />
                <span>{{ isProjectOn(p.id, a.id) ? "已添加" : "+ 添加" }}</span>
              </button>
            </div>
          </div>
        </div>
      </section>

      <section>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-sm font-semibold">SKILL.md</h2>
          <div class="inline-flex bg-[var(--color-surface)] border border-[var(--color-border)] rounded-md p-0.5">
            <button class="px-3 py-1 text-[12.5px] rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': previewMode==='rendered' }" @click="previewMode='rendered'">渲染</button>
            <button class="px-3 py-1 text-[12.5px] rounded text-[var(--color-muted)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': previewMode==='source' }" @click="previewMode='source'">源码</button>
          </div>
        </div>
        <MarkdownPreview v-if="previewMode==='rendered'" :source="mdSource" />
        <pre v-else class="p-4 border border-[var(--color-border)] rounded-md bg-[var(--color-surface)] font-mono text-[12.5px] whitespace-pre-wrap">{{ mdSource }}</pre>
      </section>
    </div>
  </template>
  <div v-else class="p-20 text-center text-[var(--color-muted)]">未找到该 Skill</div>
</template>
