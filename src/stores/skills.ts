import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { SkillView, UnmanagedSkill, ImportReq, SkillLink } from "./types";
import { listSkills, getSkill, scanUnmanaged, confirmImport, reconcileDuplicates, toggleLink, batchSetLinks, restoreSkill, uninstallSkill } from "../lib/tauri";

export const useSkillsStore = defineStore("skills", () => {
  const skills = ref<SkillView[]>([]);
  const unmanaged = ref<UnmanagedSkill[]>([]);
  const scanning = ref(false);
  const importing = ref(false);
  const selectedIds = ref<Set<string>>(new Set());
  const loaded = ref(false);
  const showPreview = ref(false);
  // 上次扫描/启动时自动接管的同名重复 Skill 数量(agent 目录→symlink+开链接;standard→删除)
  const reconciledCount = ref(0);

  const totalCount = computed(() => skills.value.length);
  const enabledCount = computed(() => skills.value.filter(s => s.anyEnabled).length);
  const disabledCount = computed(() => totalCount.value - enabledCount.value);

  async function load() {
    // 已导入 skill 的同名副本自动接管(symlink+开链接 / standard 删除)
    await reconcile();
    skills.value = await listSkills();
    loaded.value = true;
  }
  async function reconcile() {
    try {
      reconciledCount.value = await reconcileDuplicates();
    } catch {
      reconciledCount.value = 0;
    }
  }
  async function fetchUnmanaged() {
    scanning.value = true;
    unmanaged.value = await scanUnmanaged();
    scanning.value = false;
    // 扫描后立即接管已导入 skill 的同名副本
    await reconcile();
    if (reconciledCount.value > 0) skills.value = await listSkills();
  }
  async function doImport(imports: ImportReq[]) { importing.value = true; skills.value = await confirmImport(imports); unmanaged.value = []; showPreview.value = false; importing.value = false; }
  function cancelScan() { unmanaged.value = []; showPreview.value = false; }
  function dismissReconciled() { reconciledCount.value = 0; }
  async function refresh() { await load(); }
  function get(id: string) { return skills.value.find(s => s.id === id); }

  function isLinkOn(skill: SkillView, scope: string, projectId: string | null, agentId: string): boolean {
    return skill.links.some(l => l.scope === scope && (l.projectId ?? null) === (projectId ?? null) && l.agentId === agentId && l.enabled);
  }
  async function setLink(skillId: string, scope: string, projectId: string | null, agentId: string, on: boolean) {
    await toggleLink({ skillId, scope, projectId, agentId, on });
    const s = skills.value.find(x => x.id === skillId); if (!s) return;
    const existing = s.links.find(l => l.scope === scope && (l.projectId ?? null) === (projectId ?? null) && l.agentId === agentId);
    if (existing) { existing.enabled = on; } else { s.links.push({ skillId, scope, projectId, agentId, enabled: on } as SkillLink); }
    s.anyEnabled = s.links.some(l => l.enabled);
  }
  async function toggleAggregate(skillId: string) {
    const s = skills.value.find(x => x.id === skillId); if (!s) return;
    if (s.anyEnabled) {
      // disable all links
      const ids = [skillId];
      await batchSetLinks(ids, false);
      s.links.forEach(l => l.enabled = false);
      s.anyEnabled = false;
    } else {
      await batchSetLinks([skillId], true);
      // refresh from server to get accurate link list
      const fresh = await getSkill(skillId);
      if (fresh) { const idx = skills.value.findIndex(x => x.id === skillId); if (idx >= 0) skills.value[idx] = fresh; }
    }
  }

  function toggleSelect(id: string) {
    const next = new Set(selectedIds.value);
    next.has(id) ? next.delete(id) : next.add(id); selectedIds.value = next;
  }
  function clearSelection() { selectedIds.value = new Set(); }
  function selectAll(ids: string[]) { selectedIds.value = new Set(ids); }
  async function batchEnable(on: boolean) {
    const ids = Array.from(selectedIds.value);
    await batchSetLinks(ids, on);
    await load();
  }
  async function removeSkill(id: string) { await uninstallSkill(id); skills.value = skills.value.filter(s => s.id !== id); }
  async function doRestore(id: string) { await restoreSkill(id); skills.value = skills.value.filter(s => s.id !== id); }

  return {
    skills, unmanaged, scanning, importing, selectedIds, loaded, showPreview, reconciledCount,
    totalCount, enabledCount, disabledCount,
    load, fetchUnmanaged, doImport, refresh, get, isLinkOn, setLink, toggleAggregate,
    toggleSelect, clearSelection, selectAll, batchEnable, removeSkill, doRestore, cancelScan,
    dismissReconciled,
  };
});
