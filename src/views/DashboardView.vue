<script setup lang="ts">
import { computed, ref } from "vue";
import { useSkillsStore } from "../stores/skills";
import { useAgentsStore } from "../stores/agents";
import { RouterLink } from "vue-router";
import Button from "../lib/ui/button.vue";

const skills = useSkillsStore();
const agents = useAgentsStore();

const previewMode = computed(() => skills.showPreview || skills.scanning || skills.unmanaged.length > 0);
const picked = ref<Set<string>>(new Set());

function togglePick(dir: string) {
  const next = new Set(picked.value);
  next.has(dir) ? next.delete(dir) : next.add(dir);
  picked.value = next;
}
const importList = computed(() => skills.unmanaged.filter(s => picked.value.has(s.directory)));

async function runScan() {
  picked.value = new Set();
  skills.showPreview = true;
  await skills.fetchUnmanaged();
  // default select all
  picked.value = new Set(skills.unmanaged.map(s => s.directory));
}
async function cancelScan() {
  picked.value = new Set();
  skills.cancelScan();
}
async function runImport() {
  await skills.doImport(importList.value.map(s => ({ dir: s.directory, origins: s.origins })));
  picked.value = new Set();
}

const sourceSummary = computed(() => {
  const counts: Record<string, number> = {};
  for (const s of skills.unmanaged) for (const o of s.origins) counts[o.foundIn] = (counts[o.foundIn] || 0) + 1;
  return Object.entries(counts).map(([k, v]) => `${labelFor(k)}(${v})`).join(" · ");
});
function labelFor(f: string) {
  if (f.startsWith("agent:")) return agents.get(f.slice(6))?.name ?? f.slice(6);
  if (f.startsWith("project:")) return f;
  return "SSOT";
}
</script>

<template>
  <!-- Empty state -->
  <div v-if="!skills.loaded || (skills.skills.length === 0 && !previewMode)" class="flex flex-col items-center justify-center h-full text-center px-12">
    <div class="w-[72px] h-[72px] rounded-[18px] bg-[var(--color-surface-2)] grid place-items-center mb-6 text-[var(--color-accent)]">
      <span class="text-4xl">◆</span>
    </div>
    <h1 class="text-2xl font-semibold mb-2">欢迎使用 Skillman</h1>
    <p class="text-[14.5px] text-[var(--color-muted)] max-w-[440px] mb-8 leading-relaxed">
      还没有导入任何 Skills。扫描本地磁盘,自动发现已安装的 AI coding agent 和 Skills 目录,一键导入并接管。
    </p>
    <Button @click="runScan" :disabled="skills.scanning">
      {{ skills.scanning ? "扫描中…" : "扫描本地 Skills" }}
    </Button>
    <p class="text-xs text-[var(--color-meta)] mt-4">检测 Claude Code · Codex · OpenCode · Cursor · Grok · Antigravity</p>
  </div>

  <!-- Scan preview (step 1) -->
  <div v-else-if="previewMode" class="p-10">
    <h1 class="text-[26px] font-semibold mb-1">扫描结果</h1>
    <p class="text-[13.5px] text-[var(--color-muted)] mb-1">发现 <span class="font-mono">{{ skills.unmanaged.length }}</span> 个未托管的 Skills</p>
    <p v-if="sourceSummary" class="text-[12.5px] text-[var(--color-meta)] mb-6">来自:{{ sourceSummary }}</p>

    <div class="border border-[var(--color-border)] rounded-lg overflow-hidden mb-4">
      <div v-for="s in skills.unmanaged" :key="s.directory" class="flex items-start gap-3 p-3.5 border-b border-[var(--color-border-soft)] last:border-b-0 hover:bg-[var(--color-surface-2)]">
        <span class="mt-0.5 w-4 h-4 rounded-[4px] border-[1.5px] grid place-items-center cursor-pointer flex-shrink-0"
          :class="picked.has(s.directory) ? 'bg-[var(--color-accent)] border-[var(--color-accent)]' : 'border-[var(--color-border)] hover:border-[var(--color-meta)]'"
          @click="togglePick(s.directory)">
          <svg v-if="picked.has(s.directory)" width="10" height="8" viewBox="0 0 10 8" fill="none"><path d="M1 4l3 3 5-6" stroke="var(--color-accent-on)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </span>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2"><span class="font-mono font-semibold text-sm">{{ s.name }}</span>
            <span v-for="o in s.origins" :key="o.path" class="text-[11px] px-2 py-0.5 rounded bg-[var(--color-surface-2)] border border-[var(--color-border)] text-[var(--color-muted)]">{{ labelFor(o.foundIn) }}</span>
          </div>
          <p class="text-[12.5px] text-[var(--color-muted)] mt-0.5">{{ s.description ?? "无描述" }}</p>
        </div>
      </div>
    </div>
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="sm" @click="picked = new Set(skills.unmanaged.map(s => s.directory))">全选</Button>
      <Button variant="ghost" size="sm" @click="picked = new Set()">取消选择</Button>
      <span class="flex-1 text-[13px] text-[var(--color-muted)]">共 {{ picked.size }} 个已选</span>
      <Button variant="ghost" size="sm" @click="cancelScan">关闭</Button>
      <Button @click="runImport" :disabled="!picked.size || skills.importing">
        {{ skills.importing ? "导入中…" : "确认导入并替换为 symlink" }}
      </Button>
    </div>
  </div>

  <!-- Imported dashboard -->
  <div v-else>
    <header class="px-10 pt-8 pb-6 border-b border-[var(--color-border)]">
      <h1 class="text-[26px] font-semibold mb-1">你好!</h1>
      <p class="text-[13.5px] text-[var(--color-muted)]">Library 中 <span class="font-mono">{{ skills.totalCount }}</span> 个 Skills · 全局已启用 <span class="font-mono">{{ skills.enabledCount }}</span> · 已检测 agent <span class="font-mono">{{ agents.installedAgents.length }}</span></p>
    </header>
    <div class="p-10">
      <div class="grid grid-cols-3 gap-4 mb-8">
        <div class="flex items-center justify-between p-5 border border-[var(--color-border)] rounded-lg bg-[var(--color-surface)]">
          <div><div class="text-[11.5px] text-[var(--color-muted)] mb-2.5">Library Skills</div><div class="text-3xl font-semibold">{{ skills.totalCount }}</div></div>
        </div>
        <div class="flex items-center justify-between p-5 border border-[var(--color-border)] rounded-lg bg-[var(--color-surface)]">
          <div><div class="text-[11.5px] text-[var(--color-muted)] mb-2.5">已启用 / 已禁用</div><div class="text-3xl font-semibold">{{ skills.enabledCount }} · {{ skills.disabledCount }}</div></div>
        </div>
        <div class="flex items-center justify-between p-5 border border-[var(--color-border)] rounded-lg bg-[var(--color-surface)]">
          <div><div class="text-[11.5px] text-[var(--color-muted)] mb-2.5">已检测 agent</div><div class="text-3xl font-semibold">{{ agents.installedAgents.length }}</div></div>
        </div>
      </div>
      <Button @click="runScan" :disabled="skills.scanning">扫描并导入新 Skills</Button>

      <section class="mt-8">
        <div class="flex items-baseline justify-between mb-3.5">
          <h2 class="text-sm font-semibold">最近导入</h2>
          <RouterLink class="text-[12.5px] text-[var(--color-muted)] hover:text-[var(--color-accent)]" to="/skills">查看更多 →</RouterLink>
        </div>
        <div class="border border-[var(--color-border)] rounded-lg overflow-hidden bg-[var(--color-surface)]">
          <RouterLink v-for="s in [...skills.skills].sort((a,b)=>b.installedAt-a.installedAt).slice(0,7)" :key="s.id" :to="`/skills/${s.id}`"
            class="flex items-center gap-3 p-3.5 border-b border-[var(--color-border-soft)] last:border-b-0 hover:bg-[var(--color-surface-2)]">
            <span class="w-8 h-8 rounded-md bg-[var(--color-surface-2)] grid place-items-center font-mono text-[12.5px] font-semibold text-[var(--color-muted)]">{{ s.name.charAt(0).toUpperCase() }}</span>
            <div class="flex-1 min-w-0"><div class="text-sm font-medium">{{ s.name }}</div><div class="text-[12.5px] text-[var(--color-muted)] truncate">{{ s.description ?? "无描述" }}</div></div>
          </RouterLink>
        </div>
      </section>
    </div>
  </div>
</template>
