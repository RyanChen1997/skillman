export type Scope = "global" | "project";

export interface Agent {
  id: string; name: string; globalSubpath: string; projectSubpath: string;
  installed: boolean; sourceOnly: boolean;
}
export interface Project { id: string; name: string; path: string; }
export interface SkillLink { skillId: string; scope: Scope; projectId: string | null; agentId: string; enabled: boolean; }
export interface SkillOrigin { skillId: string; originPath: string; foundIn: string; importedAt: number; }
export interface UnmanagedOrigin { path: string; foundIn: string; }
export interface UnmanagedSkill { directory: string; name: string; description: string | null; origins: UnmanagedOrigin[]; }
export interface InstalledSkill {
  id: string; name: string; directory: string; description: string | null;
  source: string | null; contentHash: string | null; installedAt: number; updatedAt: number;
}
export interface SkillView extends InstalledSkill {
  links: SkillLink[]; origins: SkillOrigin[]; anyEnabled: boolean;
}
export interface ImportReq { dir: string; origins: UnmanagedOrigin[]; }
