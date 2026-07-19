import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Agent } from "./types";
import { detectAgents, listAgents } from "../lib/tauri";

export const useAgentsStore = defineStore("agents", () => {
  const agents = ref<Agent[]>([]);
  const loaded = ref(false);
  const installedAgents = computed(() => agents.value.filter(a => a.installed));

  async function load() {
    agents.value = await listAgents();
    loaded.value = true;
  }
  async function refresh() {
    agents.value = await detectAgents();
  }
  function get(id: string) { return agents.value.find(a => a.id === id); }

  return { agents, loaded, installedAgents, load, refresh, get };
});
