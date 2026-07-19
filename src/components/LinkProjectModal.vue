<script setup lang="ts">
import { ref, watch } from "vue";
import { useProjectsStore } from "../stores/projects";
import { useSkillsStore } from "../stores/skills";
import type { Project, UnmanagedSkill } from "../stores/types";
import { addProject, scanProject, confirmImport } from "../lib/tauri";
import Button from "../lib/ui/button.vue";
import Dialog from "../lib/ui/dialog.vue";

const projects = useProjectsStore();
const skills = useSkillsStore();

const name = ref("");
const path = ref("");
const step = ref<"form" | "preview">("form");
const project = ref<Project | null>(null);
const found = ref<UnmanagedSkill[]>([]);
const picked = ref<Set<string>>(new Set());
const scanning = ref(false);
const importing = ref(false);

watch(
  () => projects.linkModalOpen,
  (open) => {
    if (open) {
      name.value = "";
      path.value = "";
      step.value = "form";
      project.value = null;
      found.value = [];
      picked.value = new Set();
    }
  }
);

function togglePick(dir: string) {
  const next = new Set(picked.value);
  if (next.has(dir)) next.delete(dir);
  else next.add(dir);
  picked.value = next;
}

async function scan() {
  if (!path.value.trim()) return;
  const n = name.value.trim() || "project";
  const created = await addProject({ id: n, name: n, path: path.value.trim() });
  project.value = created;
  projects.projects = [...projects.projects, created];

  scanning.value = true;
  found.value = await scanProject(created.id);
  scanning.value = false;
  picked.value = new Set(found.value.map((s) => s.directory));
  if (found.value.length) {
    step.value = "preview";
  } else {
    alert("未在项目中找到 Skills");
    projects.linkModalOpen = false;
  }
}

async function doImport() {
  if (!project.value) return;
  const list = found.value.filter((s) => picked.value.has(s.directory));
  if (!list.length) return;
  importing.value = true;
  await confirmImport(list.map((s) => ({ dir: s.directory, origins: s.origins })));
  importing.value = false;
  await skills.load();
  projects.linkModalOpen = false;
}
</script>

<template>
  <Dialog :open="projects.linkModalOpen" @update:open="projects.linkModalOpen = $event">
    <template v-if="step === 'form'">
      <h3 class="text-base font-semibold mb-2">关联项目</h3>
      <p class="text-[13.5px] text-[var(--color-muted)] mb-4">
        输入项目根目录，skillman 将扫描其下的 agent skills 子目录。
      </p>
      <label class="block text-[12.5px] text-[var(--color-muted)] mb-1">项目名称</label>
      <input
        v-model="name"
        class="w-full mb-3 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-sm outline-none focus:border-[var(--color-accent)]"
        placeholder="my-project"
      />
      <label class="block text-[12.5px] text-[var(--color-muted)] mb-1">项目根目录路径</label>
      <input
        v-model="path"
        class="w-full mb-4 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-sm outline-none focus:border-[var(--color-accent)] font-mono"
        placeholder="例如 C:\Users\你的名字\Documents\my-project"
      />
      <span class="block text-[12px] text-[var(--color-meta)] mb-4">
        将扫描该目录下的 .claude\skills、.codex\skills 等 agent 子目录
      </span>
      <div class="flex justify-end gap-2">
        <Button variant="ghost" size="sm" @click="projects.linkModalOpen = false">取消</Button>
        <Button size="sm" @click="scan" :disabled="!path.trim() || scanning">
          {{ scanning ? "扫描中…" : "扫描并导入" }}
        </Button>
      </div>
    </template>

    <template v-else>
      <h3 class="text-base font-semibold mb-2">扫描结果</h3>
      <p class="text-[13.5px] text-[var(--color-muted)] mb-4">
        发现 <span class="font-mono">{{ found.length }}</span> 个未托管的 Skills
      </p>
      <div class="border border-[var(--color-border)] rounded-lg overflow-hidden mb-4">
        <div
          v-for="s in found"
          :key="s.directory"
          class="flex items-start gap-3 p-3.5 border-b border-[var(--color-border-soft)] last:border-b-0 hover:bg-[var(--color-surface-2)]"
        >
          <span
            class="mt-0.5 w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center cursor-pointer flex-shrink-0"
            :class="
              picked.has(s.directory)
                ? 'bg-[var(--color-accent)] border-[var(--color-accent)]'
                : 'border-[var(--color-border)] hover:border-[var(--color-meta)]'
            "
            @click="togglePick(s.directory)"
          >
            <svg
              v-if="picked.has(s.directory)"
              width="10"
              height="8"
              viewBox="0 0 10 8"
              fill="none"
            >
              <path
                d="M1 4l3 3 5-6"
                stroke="var(--color-accent-on)"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </span>
          <div class="flex-1 min-w-0">
            <div class="font-mono font-semibold text-sm">{{ s.name }}</div>
            <p class="text-[12.5px] text-[var(--color-muted)] mt-0.5">
              {{ s.description ?? "无描述" }}
            </p>
          </div>
        </div>
      </div>
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="sm" @click="picked = new Set(found.map((s) => s.directory))">
          全选
        </Button>
        <Button variant="ghost" size="sm" @click="picked = new Set()">取消选择</Button>
        <span class="flex-1 text-[13px] text-[var(--color-muted)]">共 {{ picked.size }} 个已选</span>
        <Button variant="ghost" size="sm" @click="projects.linkModalOpen = false">取消</Button>
        <Button @click="doImport" :disabled="!picked.size || importing">
          {{ importing ? "导入中…" : "确认导入" }}
        </Button>
      </div>
    </template>
  </Dialog>
</template>
