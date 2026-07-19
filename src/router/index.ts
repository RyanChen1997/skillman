import { createRouter, createWebHashHistory } from "vue-router";

const routes = [
  { path: "/", name: "dashboard", component: () => import("../views/DashboardView.vue") },
  { path: "/skills", name: "skills", component: () => import("../views/SkillsView.vue") },
  { path: "/skills/:id", name: "skill-detail", component: () => import("../views/SkillDetailView.vue"), props: true },
  { path: "/settings", name: "settings", component: () => import("../views/SettingsView.vue") },
];

export default createRouter({ history: createWebHashHistory(), routes });
