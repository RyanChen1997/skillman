import { defineStore } from "pinia";
import { ref } from "vue";
import type { Project } from "./types";
import { listProjects, addProject as apiAdd, removeProject as apiRemove } from "../lib/tauri";

export const useProjectsStore = defineStore("projects", () => {
  const projects = ref<Project[]>([]);
  const linkModalOpen = ref(false);

  async function load() { projects.value = await listProjects(); }
  async function add(p: { id: string; name: string; path: string }) {
    const created = await apiAdd(p);
    projects.value = [...projects.value, created];
    linkModalOpen.value = false;
  }
  async function remove(id: string) {
    await apiRemove(id);
    projects.value = projects.value.filter(p => p.id !== id);
  }
  return { projects, linkModalOpen, load, add, remove };
});
