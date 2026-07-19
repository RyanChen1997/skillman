<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useSkillsStore } from "../stores/skills";
import { useAgentsStore } from "../stores/agents";
import { useProjectsStore } from "../stores/projects";
import { batchAddToProject } from "../lib/tauri";
import Dialog from "../lib/ui/dialog.vue";
import Button from "../lib/ui/button.vue";
import AgentIcon from "../components/AgentIcon.vue";

const props = defineProps<{ projectId: string; open: boolean }>();
const emit = defineEmits<{ (e: "update:open", v: boolean): void; (e: "added"): void }>();

const skills = useSkillsStore();
const agents = useAgentsStore();
const projects = useProjectsStore();

const project = computed(() => projects.projects.find(p => p.id === props.projectId));
const usableAgents = computed(() => agents.installedAgents.filter(a => !a.sourceOnly));

const selectedSkills = ref<Set<string>>(new Set());
const selectedAgents = ref<Set<string>>(new Set());
const adding = ref(false);

const availableSkills = computed(() => {
  return skills.skills.filter(s => !s.links.some(l => l.scope === "project" && l.projectId === props.projectId && l.enabled));
});

function toggleSkill(id: string) {
  const next = new Set(selectedSkills.value);
  next.has(id) ? next.delete(id) : next.add(id);
  selectedSkills.value = next;
}
function toggleAgent(id: string) {
  const next = new Set(selectedAgents.value);
  next.has(id) ? next.delete(id) : next.add(id);
  selectedAgents.value = next;
}
function close() { emit("update:open", false); }

watch(() => props.open, (v) => {
  if (v) {
    selectedSkills.value = new Set();
    selectedAgents.value = new Set();
  }
});

async function confirm() {
  if (!selectedSkills.value.size || !selectedAgents.value.size) return;
  adding.value = true;
  try {
    await batchAddToProject(props.projectId, Array.from(selectedSkills.value), Array.from(selectedAgents.value));
    selectedSkills.value = new Set();
    selectedAgents.value = new Set();
    close();
    emit("added");
  } catch (e) {
    console.error("添加失败", e);
  } finally {
    adding.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" size="lg" @update:open="(v: boolean) => emit('update:open', v)">
    <div class="mb-4">
      <h3 class="text-lg font-semibold">从技能库添加到项目</h3>
      <p class="text-[13px] text-[var(--color-muted)] mt-1">选择要添加到「{{ project?.name ?? projectId }}」的 Skills 和 Agent。</p>
    </div>

    <div class="grid gap-5 [grid-template-columns:1fr_280px] min-w-0">
      <div class="min-w-0">
        <h4 class="text-[12.5px] font-semibold text-[var(--color-muted)] uppercase tracking-wide mb-2.5">Skills</h4>
        <div class="border border-[var(--color-border)] rounded-md overflow-hidden max-h-[360px] overflow-y-auto">
          <div v-for="s in availableSkills" :key="s.id" class="flex items-start gap-3 p-3 border-b border-[var(--color-border-soft)] last:border-b-0 hover:bg-[var(--color-surface-2)] cursor-pointer"
            @click="toggleSkill(s.id)">
            <span class="mt-0.5 w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center flex-shrink-0"
              :class="selectedSkills.has(s.id) ? 'bg-[var(--color-accent)] border-[var(--color-accent)]' : 'border-[var(--color-border)]'">
              <svg v-if="selectedSkills.has(s.id)" width="10" height="8" viewBox="0 0 10 8" fill="none"><path d="M1 4l3 3 5-6" stroke="var(--color-accent-on)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </span>
            <div class="flex-1 min-w-0">
              <div class="text-sm font-medium">{{ s.name }}</div>
              <div class="text-[12px] text-[var(--color-muted)] truncate">{{ s.description ?? "无描述" }}</div>
            </div>
          </div>
          <div v-if="!availableSkills.length" class="p-4 text-center text-[var(--color-muted)] text-[13px]">当前项目已启用所有 Skills</div>
        </div>
      </div>

      <div class="min-w-0">
        <h4 class="text-[12.5px] font-semibold text-[var(--color-muted)] uppercase tracking-wide mb-2.5">Agents</h4>
        <div class="border border-[var(--color-border)] rounded-md p-2 space-y-2 max-h-[360px] overflow-y-auto">
          <div v-for="a in usableAgents" :key="a.id" class="flex items-center gap-2.5 p-2 rounded-md border cursor-pointer"
            :class="selectedAgents.has(a.id) ? 'border-[var(--color-accent)]/35 bg-[var(--color-accent)]/12' : 'border-[var(--color-border)] bg-[var(--color-surface)]'"
            @click="toggleAgent(a.id)">
            <span class="mt-0.5 w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center flex-shrink-0"
              :class="selectedAgents.has(a.id) ? 'bg-[var(--color-accent)] border-[var(--color-accent)]' : 'border-[var(--color-border)]'">
              <svg v-if="selectedAgents.has(a.id)" width="10" height="8" viewBox="0 0 10 8" fill="none"><path d="M1 4l3 3 5-6" stroke="var(--color-accent-on)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </span>
            <AgentIcon :agent-id="a.id" class="w-4 h-4 text-[var(--color-muted)]" />
            <span class="text-[13px]">{{ a.name }}</span>
          </div>
          <div v-if="!usableAgents.length" class="p-2 text-[13px] text-[var(--color-muted)]">没有可用的已安装 Agent</div>
        </div>
      </div>
    </div>

    <div class="flex items-center justify-end gap-3 mt-5">
      <Button variant="ghost" size="sm" @click="close">取消</Button>
      <Button :disabled="!selectedSkills.size || !selectedAgents.size || adding" @click="confirm">
        {{ adding ? "添加中…" : `添加 ${selectedSkills.size} 个 skill 到 ${selectedAgents.size} 个 agent` }}
      </Button>
    </div>
  </Dialog>
</template>
