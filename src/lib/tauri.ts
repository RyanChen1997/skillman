import { invoke } from "@tauri-apps/api/core";
import type { Agent, Project, SkillView, UnmanagedSkill, ImportReq } from "../stores/types";

export const detectAgents = () => invoke<Agent[]>("detect_agents");
export const ensureAgentDir = (agentId: string) => invoke<Agent | null>("ensure_agent_dir", { agentId });
export const scanUnmanaged = () => invoke<UnmanagedSkill[]>("scan_unmanaged");
export const scanProject = (projectId: string) => invoke<UnmanagedSkill[]>("scan_project", { projectId });
export const confirmImport = (imports: ImportReq[]) => invoke<SkillView[]>("confirm_import", { imports });
export const reconcileDuplicates = () => invoke<number>("reconcile_duplicates");
export const listSkills = () => invoke<SkillView[]>("list_skills");
export const getSkill = (id: string) => invoke<SkillView | null>("get_skill", { id });
export const toggleLink = (p: { skillId: string; scope: string; projectId: string | null; agentId: string; on: boolean }) =>
  invoke<void>("toggle_link", p);
export const batchSetLinks = (skillIds: string[], on: boolean) => invoke<void>("batch_set_links", { skillIds, on });
export const syncAll = () => invoke<void>("sync_all");
export const restoreSkill = (id: string) => invoke<void>("restore_skill", { id });
export const uninstallSkill = (id: string) => invoke<void>("uninstall_skill", { id });
export const listProjects = () => invoke<Project[]>("list_projects");
export const addProject = (p: { id: string; name: string; path: string }) => invoke<Project>("add_project", p);
export const removeProject = (id: string) => invoke<void>("remove_project", { id });
export const getSetting = (key: string) => invoke<string | null>("get_setting", { key });
export const setSetting = (key: string, value: string) => invoke<void>("set_setting", { key, value });
export const readSkillMdSource = (id: string) => invoke<string | null>("read_skill_md_source", { id });
export const getPaths = () => invoke<Record<string, string>>("get_paths");
export const resetAll = () => invoke<void>("reset_all");
export const batchAddToProject = (projectId: string, skillIds: string[], agentIds: string[]) =>
  invoke<void>("batch_add_to_project", { projectId, skillIds, agentIds });
