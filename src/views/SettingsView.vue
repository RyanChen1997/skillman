<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useAgentsStore } from "../stores/agents";
import { useSettingsStore } from "../stores/settings";
import { useSkillsStore } from "../stores/skills";
import { getPaths, resetAll } from "../lib/tauri";
import Button from "../lib/ui/button.vue";
import Switch from "../lib/ui/switch.vue";
import Dialog from "../lib/ui/dialog.vue";

const agents = useAgentsStore();
const settings = useSettingsStore();
const skills = useSkillsStore();
const router = useRouter();
const tab = ref<"general" | "about">("general");
const showResetConfirm = ref(false);
const paths = ref<Record<string, string>>({});

const themeOptions = [
  { k: "light" as const, label: "浅色" },
  { k: "dark" as const, label: "深色" },
  { k: "system" as const, label: "跟随系统" },
];

function agentGlobalPath(subpath: string) {
  const home = paths.value.home || "~";
  const sep = paths.value.separator || "/";
  const normalized = subpath.replace(/\//g, sep);
  return `${home}${sep}${normalized}`;
}

onMounted(async () => {
  paths.value = await getPaths();
});

function doReset() {
  showResetConfirm.value = true;
}

async function confirmReset() {
  showResetConfirm.value = false;
  await resetAll();
  await skills.refresh();
  router.push("/");
}

function cancelReset() {
  showResetConfirm.value = false;
}
</script>

<template>
  <header class="px-10 pt-8 pb-6 border-b border-[var(--color-border)]">
    <h1 class="text-[26px] font-semibold mb-1">设置</h1>
    <p class="text-[13.5px] text-[var(--color-muted)]">{{ tab === "general" ? "通用" : "关于" }}</p>
  </header>
  <div class="p-10 grid grid-cols-[200px_1fr] gap-10">
    <nav class="flex flex-col gap-0.5">
      <button class="flex items-center gap-2.5 px-3 py-2 rounded-md text-[13.5px] text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': tab==='general' }" @click="tab='general'">通用</button>
      <button class="flex items-center gap-2.5 px-3 py-2 rounded-md text-[13.5px] text-[var(--color-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-fg)]" :class="{ '!bg-[var(--color-surface-2)] !text-[var(--color-fg)]': tab==='about' }" @click="tab='about'">关于</button>
    </nav>
    <div>
      <section v-show="tab==='general'">
        <section class="mb-8">
          <h2 class="text-sm font-semibold mb-3">外观</h2>
          <div class="border border-[var(--color-border)] rounded-lg overflow-hidden bg-[var(--color-surface)] p-[18px]">
            <div class="grid grid-cols-[200px_1fr] gap-6 items-center">
              <div>
                <div class="text-[13.5px] font-medium">主题</div>
                <div class="text-xs text-[var(--color-muted)] mt-0.5">选择界面颜色风格</div>
              </div>
              <div class="inline-flex bg-[var(--color-surface-2)] border border-[var(--color-border)] rounded-md p-0.5">
                <button v-for="opt in themeOptions" :key="opt.k" type="button"
                  class="px-3 py-1 text-[13px] rounded text-[var(--color-muted)]"
                  :class="{ '!bg-[var(--color-surface)] !text-[var(--color-fg)] shadow-sm': settings.theme === opt.k }"
                  @click="settings.setTheme(opt.k)">
                  {{ opt.label }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <h2 class="text-sm font-semibold mb-3">Library</h2>
        <div class="border border-[var(--color-border)] rounded-lg overflow-hidden bg-[var(--color-surface)] mb-8">
          <div class="grid grid-cols-[200px_1fr] gap-6 p-[18px] border-b border-[var(--color-border-soft)] items-center">
            <div><div class="text-[13.5px] font-medium">本地 Library 路径</div><div class="text-xs text-[var(--color-muted)] mt-0.5">存放全部 Skills 真实文件</div></div>
            <code class="font-mono text-[12.5px] bg-[var(--color-surface-2)] border border-[var(--color-border)] rounded-md px-3 py-1.5">{{ paths.ssot || "~/.skillman/skills" }}</code>
          </div>
        </div>

        <h2 class="text-sm font-semibold mb-3">Agent</h2>
        <div class="border border-[var(--color-border)] rounded-lg overflow-hidden bg-[var(--color-surface)] mb-8">
          <div class="grid grid-cols-[200px_1fr_32px] gap-6 p-[18px] border-b border-[var(--color-border-soft)] items-center">
            <div><div class="text-[13.5px] font-medium">显示未安装的 agent</div><div class="text-xs text-[var(--color-muted)] mt-0.5">关闭则隐藏未安装 agent</div></div>
            <div><Switch :model-value="settings.showUninstalled" @update:model-value="settings.setShowUninstalled" /></div>
          </div>
          <div v-for="a in agents.agents.filter(x => !x.sourceOnly)" :key="a.id" class="grid grid-cols-[200px_1fr_auto] gap-6 p-[18px] border-b border-[var(--color-border-soft)] items-center last:border-b-0">
            <div class="text-[13.5px] font-medium">{{ a.name }}</div>
            <div class="flex items-center gap-2 text-xs text-[var(--color-muted)]"><span class="font-mono">{{ agentGlobalPath(a.globalSubpath) }}</span><span :class="a.installed ? 'text-[var(--color-success)]' : 'text-[var(--color-warn)]'">{{ a.installed ? "已安装" : "未安装" }}</span></div>
            <Button v-if="!a.installed" variant="secondary" size="sm" @click="agents.ensureInstalled(a.id)">创建目录</Button>
          </div>
        </div>

        <h2 class="text-sm font-semibold mb-3">数据管理</h2>
        <div class="border border-[var(--color-border)] rounded-lg overflow-hidden bg-[var(--color-surface)]">
          <div class="grid grid-cols-[200px_1fr_32px] gap-6 p-[18px] items-center">
            <div><div class="text-[13.5px] font-medium">重置数据</div><div class="text-xs text-[var(--color-muted)] mt-0.5">将所有 skill 恢复到原目录并清空数据(skill-backups 备份会保留,便于找回)</div></div>
            <div><Button variant="destructive" size="sm" @click="doReset">重置</Button></div>
          </div>
        </div>
      </section>

      <section v-show="tab==='about'">
        <h2 class="text-sm font-semibold mb-3">关于</h2>
        <div class="border border-[var(--color-border)] rounded-lg overflow-hidden bg-[var(--color-surface)]">
          <div class="grid grid-cols-[200px_1fr_32px] gap-6 p-[18px] items-center">
            <div><div class="text-[13.5px] font-medium">版本号</div></div>
            <div class="font-mono text-[13px]">v0.2.0</div>
          </div>
        </div>
      </section>
    </div>
  </div>

  <Dialog v-model:open="showResetConfirm">
    <div class="text-[16px] font-semibold mb-2">确认重置?</div>
    <p class="text-[13.5px] text-[var(--color-muted)] mb-6 leading-relaxed">
      这将把所有 skill 从 SSOT 恢复到原来的 agent 目录，删除所有 symlinks，并清空数据库与项目。skill-backups 目录中的备份会保留，出问题时可以手动找回文件。此操作不可撤销。
    </p>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" size="sm" @click="cancelReset">取消</Button>
      <Button variant="destructive" size="sm" @click="confirmReset">确认重置</Button>
    </div>
  </Dialog>
</template>
