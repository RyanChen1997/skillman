use crate::db::Database;
use crate::models::Agent;
use crate::paths;
use rusqlite::params;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct AgentSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub global_subpath: &'static str,
    pub project_subpath: &'static str,
    pub source_only: bool,
}

pub static BUILTIN_AGENTS: &[AgentSpec] = &[
    AgentSpec { id: "claude-code", name: "Claude Code", global_subpath: ".claude/skills", project_subpath: ".claude/skills", source_only: false },
    AgentSpec { id: "codex", name: "Codex", global_subpath: ".codex/skills", project_subpath: ".codex/skills", source_only: false },
    AgentSpec { id: "opencode", name: "OpenCode", global_subpath: ".config/opencode/skills", project_subpath: ".opencode/skills", source_only: false },
    AgentSpec { id: "cursor", name: "Cursor", global_subpath: ".cursor/skills", project_subpath: ".cursor/skills", source_only: false },
    AgentSpec { id: "grok", name: "Grok", global_subpath: ".grok/skills", project_subpath: ".grok/skills", source_only: false },
    AgentSpec { id: "antigravity", name: "Antigravity", global_subpath: ".gemini/config/skills", project_subpath: ".gemini/config/skills", source_only: false },
    AgentSpec { id: "standard", name: "Standard", global_subpath: ".agents/skills", project_subpath: ".agents/skills", source_only: true },
];

/// UPSERT all builtin agents.
/// Sets `installed` from directory existence. Returns all agents in DB.
pub fn detect_agents(db: &Arc<Database>) -> Vec<Agent> {
    {
        let c = db.conn();
        for spec in BUILTIN_AGENTS {
            let installed = paths::home().join(spec.global_subpath).is_dir();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only)
                 VALUES(?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name,
                   global_subpath=excluded.global_subpath,
                   project_subpath=excluded.project_subpath,
                   installed=excluded.installed,
                   source_only=excluded.source_only",
                params![spec.id, spec.name, spec.global_subpath, spec.project_subpath, installed as i64, spec.source_only as i64],
            ).ok();
        }
    }
    list_agents(db)
}

pub fn list_agents(db: &Arc<Database>) -> Vec<Agent> {
    let c = db.conn();
    let mut stmt = c.prepare("SELECT id,name,global_subpath,project_subpath,installed,source_only FROM agents ORDER BY id").unwrap();
    stmt.query_map([], |r| Ok(Agent {
        id: r.get(0)?,
        name: r.get(1)?,
        global_subpath: r.get(2)?,
        project_subpath: r.get(3)?,
        installed: r.get::<_, i64>(4)? != 0,
        source_only: r.get::<_, i64>(5)? != 0,
    })).unwrap().filter_map(|r| r.ok()).collect()
}

pub fn resolve_global_dest(agent: &Agent) -> PathBuf {
    paths::home().join(&agent.global_subpath)
}

pub fn resolve_project_dest(agent: &Agent, project_root: &Path) -> PathBuf {
    project_root.join(&agent.project_subpath)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_has_seven() {
        assert_eq!(BUILTIN_AGENTS.len(), 7);
    }

    #[test]
    fn dest_resolution() {
        let a = Agent { id: "codex".into(), name: "Codex".into(), global_subpath: ".codex/skills".into(), project_subpath: ".codex/skills".into(), installed: true, source_only: false };
        assert_eq!(resolve_global_dest(&a), paths::home().join(".codex/skills"));
        assert_eq!(resolve_project_dest(&a, Path::new("/proj")), PathBuf::from("/proj/.codex/skills"));
    }
}
