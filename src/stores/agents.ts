import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Agent } from "./types";
import { detectAgents, ensureAgentDir } from "../lib/tauri";

export const useAgentsStore = defineStore("agents", () => {
  const agents = ref<Agent[]>([]);
  const loaded = ref(false);
  const installedAgents = computed(() => agents.value.filter(a => a.installed));

  // 每次启动都重新按磁盘目录存在性检测 installed(手动新建的 agent 目录也能被发现)
  async function load() {
    agents.value = await detectAgents();
    loaded.value = true;
  }
  async function refresh() {
    agents.value = await detectAgents();
  }
  /** 创建 agent 的全局 skills 目录并把该 agent 置为已安装(设置页「创建目录」按钮) */
  async function ensureInstalled(id: string): Promise<boolean> {
    const a = await ensureAgentDir(id);
    if (!a) return false;
    const idx = agents.value.findIndex(x => x.id === a.id);
    if (idx >= 0) agents.value[idx] = a; else agents.value.push(a);
    return true;
  }
  function get(id: string) { return agents.value.find(a => a.id === id); }

  return { agents, loaded, installedAgents, load, refresh, ensureInstalled, get };
});
