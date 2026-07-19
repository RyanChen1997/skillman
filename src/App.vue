<script setup lang="ts">
import { onMounted } from "vue";
import AppSidebar from "./components/AppSidebar.vue";
import LinkProjectModal from "./components/LinkProjectModal.vue";
import { useAgentsStore } from "./stores/agents";
import { useProjectsStore } from "./stores/projects";
import { useSettingsStore } from "./stores/settings";
import { useSkillsStore } from "./stores/skills";

const agents = useAgentsStore();
const projects = useProjectsStore();
const settings = useSettingsStore();
const skills = useSkillsStore();

onMounted(async () => {
  await Promise.all([agents.load(), projects.load(), settings.load(), skills.load()]);
});
</script>

<template>
  <div class="grid h-screen w-screen grid-cols-[260px_1fr] overflow-hidden bg-[var(--color-bg)]">
    <AppSidebar />
    <main class="overflow-y-auto">
      <RouterView />
    </main>
    <LinkProjectModal />
  </div>
</template>
