use crate::agent::list_agents;
use crate::db::Database;
use crate::models::{Project, SkillView, UnmanagedOrigin};
use crate::paths;
use crate::skill::fsutil::{copy_dir_recursive, create_symlink_or_copy, is_symlink_to, remove_recursive};
use crate::skill::md::read_skill_md;
use rusqlite::params;
use serde::Deserialize;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
pub struct ImportReq {
    pub dir: String,
    pub origins: Vec<UnmanagedOrigin>,
}

fn now_ts() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn confirm_import(db: &Arc<Database>, projects: &[Project], imports: Vec<ImportReq>) -> Vec<SkillView> {
    let ssot = paths::ssot_dir();
    let _ = fs::create_dir_all(&ssot);
    let backups = paths::backups_dir();
    let _ = fs::create_dir_all(&backups);
    let agents = list_agents(db);
    let ts = now_ts();

    for imp in &imports {
        let dir = imp.dir.clone();
        let ssot_path = ssot.join(&dir);
        // 1. copy to SSOT if missing
        if !ssot_path.exists() {
            if let Some(first) = imp.origins.first() {
                let _ = copy_dir_recursive(std::path::Path::new(&first.path), &ssot_path);
            }
        }
        let md = read_skill_md(&ssot_path).ok();
        let name = md.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| dir.clone());
        let description = md.as_ref().map(|m| m.description.clone());
        let hash = crate::skill::fsutil::content_hash(&ssot_path).ok();
        let id = format!("local:{}", dir);
        // 2. upsert skill
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO skills(id,name,directory,description,source,content_hash,installed_at,updated_at)
                 VALUES(?1,?2,?3,?4,'local',?5,?6,?6)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description,
                   content_hash=excluded.content_hash, updated_at=excluded.updated_at",
                params![id, name, dir, description, hash, ts],
            ).ok();
            for o in &imp.origins {
                c.execute(
                    "INSERT OR IGNORE INTO skill_origins(skill_id,origin_path,found_in,imported_at) VALUES(?1,?2,?3,?4)",
                    params![id, o.path, o.found_in, ts],
                ).ok();
            }
        }
        // 3. takeover each origin: backup original.
        //    - Regular agents: replace with SSOT symlink and write default enabled link.
        //    - Source-only agents (e.g. standard): delete original, no symlink, no default link.
        for o in &imp.origins {
            let src = std::path::Path::new(&o.path);
            if !src.exists() { continue; }
            if is_symlink_to(src, &ssot_path) { continue; }
            let backup = backups.join(format!("{}-preimport-{}", dir, ts));
            let _ = copy_dir_recursive(src, &backup);
            let _ = remove_recursive(src);

            let (scope, pid_opt) = parse_found_in(&o.found_in);
            let pid = pid_opt.as_deref().unwrap_or("");
            let agent_id = match &scope[..] {
                "project" => infer_agent_for_project(&agents, projects, &o.found_in, src),
                _ => infer_agent_for_global(&agents, &o.found_in),
            };
            if let Some(aid) = agent_id {
                if let Some(agent) = agents.iter().find(|a| a.id == aid) {
                    if !agent.source_only {
                        let _ = create_symlink_or_copy(&ssot_path, src);
                        let c = db.conn();
                        c.execute(
                            "INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES(?1,?2,?3,?4,1)
                             ON CONFLICT(skill_id,scope,project_id,agent_id) DO UPDATE SET enabled=1",
                            params![id, scope, pid, aid],
                        ).ok();
                    }
                }
            }
        }
    }

    // Return the FULL library state, not just the imported subset. The frontend
    // store mirrors this return value as its complete skills list; returning only
    // the newly imported skills caused every previously imported skill to vanish
    // from the UI on each subsequent import.
    crate::skill::lifecycle::list_skills(db)
}

fn parse_found_in(found_in: &str) -> (String, Option<String>) {
    if let Some(rest) = found_in.strip_prefix("project:") {
        ( "project".to_string(), Some(rest.to_string()) )
    } else {
        ( "global".to_string(), None )
    }
}

/// For a project origin, infer the agent from the directory layout.
/// The origin path is <projectRoot>/<agentProjectSubpath>/<skillDir>.
/// Match the agent whose project_subpath is a prefix segment of the path.
fn infer_agent_for_project(
    agents: &[crate::models::Agent],
    projects: &[Project],
    found_in: &str,
    origin_path: &std::path::Path,
) -> Option<String> {
    let pid = found_in.strip_prefix("project:")?;
    let proj = projects.iter().find(|p| p.id == pid)?;
    let proj_root = std::path::Path::new(&proj.path);
    let rel = origin_path.strip_prefix(proj_root).ok()?;
    for a in agents {
        let sub = std::path::Path::new(&a.project_subpath);
        if rel.starts_with(sub) {
            return Some(a.id.clone());
        }
    }
    agents.iter().find(|a| a.installed).map(|a| a.id.clone())
}

fn infer_agent_for_global(agents: &[crate::models::Agent], found_in: &str) -> Option<String> {
    if let Some(aid) = found_in.strip_prefix("agent:") {
        if agents.iter().any(|a| a.id == aid) { return Some(aid.to_string()); }
    }
    agents.iter().find(|a| a.installed).map(|a| a.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Project, UnmanagedOrigin};
    use crate::paths;
    use crate::skill::fsutil::remove_recursive;
    use std::sync::atomic::{AtomicU64, Ordering};

    static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_db() -> Arc<Database> {
        let n = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("skillman_import_test_{}_{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&p);
        Database::open(&p).unwrap()
    }

    /// Regression: importing a second skill must not hide previously imported
    /// skills. `confirm_import` previously returned only the newly imported
    /// skills (filtered by `ids`); the frontend store overwrites its full list
    /// with that return value, so every prior skill vanished from the UI.
    #[test]
    fn second_import_keeps_existing_skill() {
        let root = std::env::temp_dir().join(format!("skillman_import_root_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let agent_dir = home.join(".x/skills");
        std::fs::create_dir_all(agent_dir.join("impkeep-a")).unwrap();
        std::fs::write(
            agent_dir.join("impkeep-a").join("SKILL.md"),
            "---\nname: Skill A\ndescription: A desc.\n---\n# a\n\nA desc.",
        )
        .unwrap();
        std::fs::create_dir_all(agent_dir.join("impkeep-b")).unwrap();
        std::fs::write(
            agent_dir.join("impkeep-b").join("SKILL.md"),
            "---\nname: Skill B\ndescription: B desc.\n---\n# b\n\nB desc.",
        )
        .unwrap();
        // clear leftover SSOT entries from a crashed prior run
        let _ = remove_recursive(&paths::ssot_dir().join("impkeep-a"));
        let _ = remove_recursive(&paths::ssot_dir().join("impkeep-b"));

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }
        let projects: Vec<Project> = vec![];

        let imp_a = vec![ImportReq {
            dir: "impkeep-a".into(),
            origins: vec![UnmanagedOrigin { path: agent_dir.join("impkeep-a").to_string_lossy().to_string(), found_in: "agent:testagent".into() }],
        }];
        crate::paths::with_test_home(&home, || { confirm_import(&db, &projects, imp_a); });

        let imp_b = vec![ImportReq {
            dir: "impkeep-b".into(),
            origins: vec![UnmanagedOrigin { path: agent_dir.join("impkeep-b").to_string_lossy().to_string(), found_in: "agent:testagent".into() }],
        }];
        let result = crate::paths::with_test_home(&home, || confirm_import(&db, &projects, imp_b));

        let dirs: Vec<String> = result.iter().map(|s| s.skill.directory.clone()).collect();
        assert!(dirs.contains(&"impkeep-a".to_string()), "existing skill a missing after second import: {:?}", dirs);
        assert!(dirs.contains(&"impkeep-b".to_string()), "new skill b missing: {:?}", dirs);

        let _ = remove_recursive(&paths::ssot_dir().join("impkeep-a"));
        let _ = remove_recursive(&paths::ssot_dir().join("impkeep-b"));
        let _ = std::fs::remove_dir_all(&root);
        drop(db);
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!("skillman_import_test_{}.db", std::process::id())));
    }

    #[test]
    fn project_import_infers_agent_from_path() {
        let root = std::env::temp_dir().join(format!("skillman_projimport_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let claude_dir = root.join(".claude/skills/projinf-a");
        let codex_dir = root.join(".codex/skills/projinf-b");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::write(
            claude_dir.join("SKILL.md"),
            "---\nname: Skill A\ndescription: A.\n---\n# a\n\nA.",
        )
        .unwrap();
        std::fs::write(
            codex_dir.join("SKILL.md"),
            "---\nname: Skill B\ndescription: B.\n---\n# b\n\nB.",
        )
        .unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('claude-code','Claude Code','.claude/skills','.claude/skills',1)",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('codex','Codex','.codex/skills','.codex/skills',1)",
                [],
            )
            .unwrap();
        }

        let projects = vec![Project {
            id: "demo".into(),
            name: "Demo".into(),
            path: root.to_string_lossy().to_string(),
        }];
        let imports = vec![
            ImportReq {
                dir: "projinf-a".into(),
                origins: vec![UnmanagedOrigin {
                    path: claude_dir.to_string_lossy().to_string(),
                    found_in: "project:demo".into(),
                }],
            },
            ImportReq {
                dir: "projinf-b".into(),
                origins: vec![UnmanagedOrigin {
                    path: codex_dir.to_string_lossy().to_string(),
                    found_in: "project:demo".into(),
                }],
            },
        ];
        confirm_import(&db, &projects, imports);

        let claude_link: i64 = {
            let c = db.conn();
            c.query_row(
                "SELECT COUNT(*) FROM skill_links WHERE scope='project' AND agent_id='claude-code' AND project_id='demo'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        let codex_link: i64 = {
            let c = db.conn();
            c.query_row(
                "SELECT COUNT(*) FROM skill_links WHERE scope='project' AND agent_id='codex' AND project_id='demo'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(claude_link, 1, "projinf-a should link to claude-code");
        assert_eq!(codex_link, 1, "projinf-b should link to codex");

        let _ = std::fs::remove_dir_all(&root);
        let _ = remove_recursive(&paths::ssot_dir().join("projinf-a"));
        let _ = remove_recursive(&paths::ssot_dir().join("projinf-b"));
    }

    #[test]
    fn source_only_origin_deletes_without_symlink_or_link() {
        let root = std::env::temp_dir().join(format!("skillman_import_std_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let std_dir = home.join(".agents/skills/std-a");
        std::fs::create_dir_all(&std_dir).unwrap();
        std::fs::write(
            std_dir.join("SKILL.md"),
            "---\nname: Std A\ndescription: Std desc.\n---\n# std-a\n\nStd desc.",
        )
        .unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('standard','Standard','.agents/skills','.agents/skills',1,1)",
                [],
            ).unwrap();
        }
        let _ = remove_recursive(&paths::ssot_dir().join("std-a"));

        let imports = vec![ImportReq {
            dir: "std-a".into(),
            origins: vec![UnmanagedOrigin { path: std_dir.to_string_lossy().to_string(), found_in: "agent:standard".into() }],
        }];
        let mut ssot_exists = false;
        let mut link_count: i64 = 0;
        crate::paths::with_test_home(&home, || {
            confirm_import(&db, &[], imports);
            ssot_exists = paths::ssot_dir().join("std-a").exists();
            let c = db.conn();
            link_count = c.query_row("SELECT COUNT(*) FROM skill_links WHERE agent_id='standard'", [], |r| r.get(0)).unwrap();
        });

        assert!(!std_dir.exists(), "standard origin should be deleted");
        assert!(ssot_exists, "SSOT should exist");
        assert_eq!(link_count, 0, "standard should not have default link");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_only_and_regular_origin_merges_and_enables_regular_only() {
        let root = std::env::temp_dir().join(format!("skillman_import_merge_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let std_dir = home.join(".agents/skills/merge-a");
        let reg_dir = home.join(".x/skills/merge-a");
        std::fs::create_dir_all(&std_dir).unwrap();
        std::fs::create_dir_all(&reg_dir).unwrap();
        std::fs::write(
            std_dir.join("SKILL.md"),
            "---\nname: Merge A\ndescription: Merged skill.\n---\n# merge-a",
        )
        .unwrap();
        std::fs::write(
            reg_dir.join("SKILL.md"),
            "---\nname: Merge A\ndescription: Merged skill.\n---\n# merge-a",
        )
        .unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('standard','Standard','.agents/skills','.agents/skills',1,1)",
                [],
            ).unwrap();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','Test','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }
        let _ = remove_recursive(&paths::ssot_dir().join("merge-a"));

        let imports = vec![ImportReq {
            dir: "merge-a".into(),
            origins: vec![
                UnmanagedOrigin { path: std_dir.to_string_lossy().to_string(), found_in: "agent:standard".into() },
                UnmanagedOrigin { path: reg_dir.to_string_lossy().to_string(), found_in: "agent:testagent".into() },
            ],
        }];
        crate::paths::with_test_home(&home, || { confirm_import(&db, &[], imports); });

        let result = crate::skill::lifecycle::list_skills(&db);
        let skill = result.iter().find(|s| s.skill.directory == "merge-a").unwrap();
        assert_eq!(skill.origins.len(), 2);
        let standard_link = skill.links.iter().find(|l| l.agent_id == "standard");
        let test_link = skill.links.iter().find(|l| l.agent_id == "testagent");
        assert!(standard_link.is_none(), "standard link should not exist");
        assert!(test_link.is_some() && test_link.unwrap().enabled, "testagent link should be enabled");

        let _ = remove_recursive(&paths::ssot_dir().join("merge-a"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn import_uses_frontmatter_name_and_description() {
        let root = std::env::temp_dir().join(format!("skillman_import_fm_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let agent_dir = home.join(".x/skills");
        std::fs::create_dir_all(agent_dir.join("fm-skill")).unwrap();
        std::fs::write(agent_dir.join("fm-skill").join("SKILL.md"), "---\nname: Frontmatter Name\ndescription: Frontmatter desc.\n---\n# Old Title\n\nBody desc.").unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }
        let projects: Vec<Project> = vec![];
        let imports = vec![ImportReq {
            dir: "fm-skill".into(),
            origins: vec![UnmanagedOrigin { path: agent_dir.join("fm-skill").to_string_lossy().to_string(), found_in: "agent:testagent".into() }],
        }];
        let result = crate::paths::with_test_home(&home, || confirm_import(&db, &projects, imports));
        let skill = result.iter().find(|s| s.skill.directory == "fm-skill").unwrap();
        assert_eq!(skill.skill.name, "Frontmatter Name");
        assert_eq!(skill.skill.description.as_deref(), Some("Frontmatter desc."));

        let _ = std::fs::remove_dir_all(&root);
        let _ = remove_recursive(&paths::ssot_dir().join("fm-skill"));
    }
}
