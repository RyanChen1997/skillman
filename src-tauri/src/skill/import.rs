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

/// Take over same-name copies of ALREADY-MANAGED skills found in agent (or
/// standard) skill directories:
/// - regular agent dir    -> backup + delete original, symlink to SSOT, enable
///                           that agent's link (global or project scope, matching
///                           where the copy was found);
/// - source-only standard -> backup + delete only; no symlink, no link.
/// Returns the number of duplicate origins handled.
///
/// Invoked automatically after each scan (and on app load). Scan itself stays
/// read-only; this is the write phase for managed duplicates, mirroring the
/// confirm_import semantics for unmanaged skills.
pub fn reconcile_duplicates(db: &Arc<Database>, projects: &[Project], dups: Vec<UnmanagedOrigin>) -> usize {
    let ssot = paths::ssot_dir();
    let backups = paths::backups_dir();
    let _ = fs::create_dir_all(&backups);
    let agents = list_agents(db);
    let ts = now_ts();
    let mut handled = 0usize;

    for o in &dups {
        let src = std::path::Path::new(&o.path);
        let dir = match src.file_name().map(|n| n.to_string_lossy().to_string()) {
            Some(d) if !d.is_empty() => d,
            _ => continue,
        };
        if dir.starts_with('.') || o.found_in == "ssot" {
            continue;
        }
        let ssot_path = ssot.join(&dir);
        if !ssot_path.exists() || !src.exists() {
            continue;
        }
        if is_symlink_to(src, &ssot_path) {
            continue; // already taken over
        }

        // 1. backup + delete the duplicate copy
        let backup = backups.join(format!("{}-reconcile-{}", dir, ts));
        let _ = copy_dir_recursive(src, &backup);
        let _ = remove_recursive(src);

        // 2. resolve which agent owns this location
        let (scope, pid_opt) = parse_found_in(&o.found_in);
        let pid = pid_opt.as_deref().unwrap_or("");
        let agent_id: Option<String> = if scope == "project" {
            infer_agent_for_project(&agents, projects, &o.found_in, src)
        } else {
            o.found_in.strip_prefix("agent:").map(String::from)
        };
        let Some(aid) = agent_id else { continue };
        let Some(agent) = agents.iter().find(|a| a.id == aid) else { continue };

        if agent.source_only {
            // standard dir: delete only (already done above), no link
            handled += 1;
            continue;
        }

        // 3. regular agent: symlink to SSOT + enable the agent's link
        let skill_id = format!("local:{}", dir);
        let _ = create_symlink_or_copy(&ssot_path, src);
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES(?1,?2,?3,?4,1)
                 ON CONFLICT(skill_id,scope,project_id,agent_id) DO UPDATE SET enabled=1",
                params![skill_id, scope, pid, aid],
            )
            .ok();
            c.execute(
                "INSERT OR IGNORE INTO skill_origins(skill_id,origin_path,found_in,imported_at) VALUES(?1,?2,?3,?4)",
                params![skill_id, o.path, o.found_in, ts],
            )
            .ok();
        }
        handled += 1;
    }
    handled
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

    /// Regression: same-name copies of ALREADY-imported skills re-appearing in
    /// an agent dir must be replaced with a symlink to the existing SSOT and the
    /// agent's link must be enabled; copies in the standard (source-only) dir
    /// must be deleted without a link or symlink.
    #[test]
    fn reconcile_duplicates_takes_over_agent_copy_and_deletes_standard_copy() {
        let root = std::env::temp_dir().join(format!("skillman_reconcile_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let ssot = home.join(".skillman/skills");
        let agent_dir = home.join(".x/skills");
        let std_dir = home.join(".agents/skills");

        let mk = |dir: &std::path::Path| {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), "---\nname: dup\ndescription: D.\n---\n").unwrap();
        };
        mk(&ssot.join("dupf"));
        mk(&agent_dir.join("dupf")); // dup in a regular agent dir
        mk(&std_dir.join("dupf")); // dup in the standard dir
        mk(&ssot.join("dupg"));
        mk(&std_dir.join("dupg")); // another standard dup

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('standard','Standard','.agents/skills','.agents/skills',1,1)",
                [],
            ).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:dupf','dupf','dupf',1,1)", []).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:dupg','dupg','dupg',1,1)", []).unwrap();
        }

        let dups = vec![
            UnmanagedOrigin { path: agent_dir.join("dupf").to_string_lossy().to_string(), found_in: "agent:testagent".into() },
            UnmanagedOrigin { path: std_dir.join("dupf").to_string_lossy().to_string(), found_in: "agent:standard".into() },
            UnmanagedOrigin { path: std_dir.join("dupg").to_string_lossy().to_string(), found_in: "agent:standard".into() },
        ];

        let (n, link_on, std_links, origins) = crate::paths::with_test_home(&home, || {
            let n = reconcile_duplicates(&db, &[], dups);
            let link_on: i64 = {
                let c = db.conn();
                c.query_row(
                    "SELECT COUNT(*) FROM skill_links WHERE skill_id='local:dupf' AND scope='global' AND project_id='' AND agent_id='testagent' AND enabled=1",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
            };
            let std_links: i64 = {
                let c = db.conn();
                c.query_row("SELECT COUNT(*) FROM skill_links WHERE agent_id='standard'", [], |r| r.get(0)).unwrap()
            };
            let origins: i64 = {
                let c = db.conn();
                c.query_row("SELECT COUNT(*) FROM skill_origins WHERE skill_id='local:dupf'", [], |r| r.get(0)).unwrap()
            };
            (n, link_on, std_links, origins)
        });

        assert_eq!(n, 3, "all three duplicate origins should be handled");
        // agent copy: replaced by symlink to SSOT + enabled link recorded
        assert!(
            crate::skill::fsutil::is_symlink_to(&agent_dir.join("dupf"), &ssot.join("dupf")),
            "agent dir copy should become a symlink to SSOT"
        );
        assert_eq!(link_on, 1, "testagent global link should be enabled");
        assert_eq!(origins, 1, "reconciled origin should be recorded");
        // standard copies: deleted, no symlink, no link
        assert!(!std_dir.join("dupf").exists(), "standard dup should be deleted");
        assert!(!std_dir.join("dupg").exists(), "standard dup should be deleted");
        assert_eq!(std_links, 0, "standard must never get a link");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Regression: reconcile must not touch an agent dir that is already a
    /// symlink to the SSOT, nor the SSOT candidate itself.
    #[test]
    fn reconcile_duplicates_skips_already_linked_and_ssot() {
        let root = std::env::temp_dir().join(format!("skillman_reconcile_skip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let ssot = home.join(".skillman/skills");
        let agent_dir = home.join(".x/skills");
        std::fs::create_dir_all(&ssot.join("dupx")).unwrap();
        std::fs::write(ssot.join("dupx").join("SKILL.md"), "# dupx").unwrap();
        // already a symlink (previous takeover)
        crate::skill::fsutil::create_symlink_or_copy(&ssot.join("dupx"), &agent_dir.join("dupx")).unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:dupx','dupx','dupx',1,1)", []).unwrap();
        }

        let dups = vec![
            UnmanagedOrigin { path: agent_dir.join("dupx").to_string_lossy().to_string(), found_in: "agent:testagent".into() },
            UnmanagedOrigin { path: ssot.join("dupx").to_string_lossy().to_string(), found_in: "ssot".into() },
        ];
        let n = crate::paths::with_test_home(&home, || reconcile_duplicates(&db, &[], dups));
        assert_eq!(n, 0, "nothing new to handle");
        assert!(
            crate::skill::fsutil::is_symlink_to(&agent_dir.join("dupx"), &ssot.join("dupx")),
            "already-linked copy must remain untouched"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// End-to-end user flow regression. Reproduces the two reported bugs at once:
    /// 1. a SKILL.md whose frontmatter description is a plain scalar containing an
    ///    unquoted `: ` (the real "ai-hotspots" style, e.g. "Do NOT use for: 单篇...")
    ///    must still be found by the scan;
    /// 2. after importing from agent alpha, same-name copies re-added in agent beta
    ///    and the standard dir must be auto-taken-over on the next scan/reconcile:
    ///    beta -> symlink to SSOT + enabled link; standard -> deleted without a link.
    #[test]
    fn reconcile_e2e_full_user_flow() {
        let root = std::env::temp_dir().join(format!("skillman_reconcile_e2e_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let ssot = home.join(".skillman/skills");
        let alpha_dir = home.join(".a/skills");
        let beta_dir = home.join(".b/skills");
        let std_dir = home.join(".agents/skills");

        let frontmatter = "---\nname: dupflow\ndescription: 跨平台 AI 资讯速报工作流。Do NOT use for: 单篇文章深度技术分析 / AI 概念知识问答 / 翻译。\n---\n# dupflow\n\nBody.\n";
        let mk_skill = |dir: &std::path::Path| {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), frontmatter).unwrap();
        };
        // fresh copy in agent alpha
        mk_skill(&alpha_dir.join("dupflow"));

        let db = tmp_db();
        {
            let c = db.conn();
            for (id, name, sub, installed, source_only) in [
                ("alpha", "Alpha", ".a/skills", 1i64, 0i64),
                ("beta", "Beta", ".b/skills", 1i64, 0i64),
                ("standard", "Standard", ".agents/skills", 1i64, 1i64),
            ] {
                c.execute(
                    "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES(?1,?2,?3,?3,?4,?5)",
                    params![id, name, sub, installed, source_only],
                )
                .unwrap();
            }
        }
        let projects: Vec<Project> = vec![];

        // Everything that touches paths::* or the DB must run under the test home.
        // Assertions are deferred to after the closure so a failing assert cannot
        // leave SKILLMAN_HOME set (with_test_home does not clean up on panic).
        let (
            scan_found,
            alpha_is_link,
            dup_count,
            handled,
            alpha_link,
            beta_link,
            std_links,
            beta_origin,
            beta_is_link,
            std_still_exists,
        ) = crate::paths::with_test_home(&home, || {
            // step 1: scan must find the skill despite the unquoted colon
            let unmanaged = crate::skill::scan::scan_unmanaged(&db, &projects);
            let scan_found = unmanaged.iter().any(|s| s.directory == "dupflow");

            // step 2: import from alpha -> symlink + enabled global link
            let imports = vec![ImportReq {
                dir: "dupflow".into(),
                origins: vec![UnmanagedOrigin {
                    path: alpha_dir.join("dupflow").to_string_lossy().to_string(),
                    found_in: "agent:alpha".into(),
                }],
            }];
            confirm_import(&db, &projects, imports);
            let alpha_is_link = crate::skill::fsutil::is_symlink_to(&alpha_dir.join("dupflow"), &ssot.join("dupflow"));

            // step 3: same-name copies appear in beta + standard
            mk_skill(&beta_dir.join("dupflow"));
            mk_skill(&std_dir.join("dupflow"));

            let dups = crate::skill::scan::find_managed_duplicates(&db, &projects);
            let dup_count = dups.len();
            let handled = reconcile_duplicates(&db, &projects, dups);
            let beta_is_link = crate::skill::fsutil::is_symlink_to(&beta_dir.join("dupflow"), &ssot.join("dupflow"));
            let std_still_exists = std_dir.join("dupflow").exists();

            // collect DB state; each query in its own scoped guard (Mutex is not
            // reentrant), and never hold a guard while calling another db fn.
            let alpha_link: i64 = {
                let c = db.conn();
                c.query_row(
                    "SELECT COUNT(*) FROM skill_links WHERE skill_id='local:dupflow' AND scope='global' AND project_id='' AND agent_id='alpha' AND enabled=1",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
            };
            let beta_link: i64 = {
                let c = db.conn();
                c.query_row(
                    "SELECT COUNT(*) FROM skill_links WHERE skill_id='local:dupflow' AND scope='global' AND project_id='' AND agent_id='beta' AND enabled=1",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
            };
            let std_links: i64 = {
                let c = db.conn();
                c.query_row("SELECT COUNT(*) FROM skill_links WHERE agent_id='standard'", [], |r| r.get(0)).unwrap()
            };
            let beta_origin: i64 = {
                let c = db.conn();
                c.query_row(
                    "SELECT COUNT(*) FROM skill_origins WHERE skill_id='local:dupflow' AND found_in='agent:beta'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
            };

            (
                scan_found,
                alpha_is_link,
                dup_count,
                handled,
                alpha_link,
                beta_link,
                std_links,
                beta_origin,
                beta_is_link,
                std_still_exists,
            )
        });

        assert!(scan_found, "scan must find dupflow despite unquoted colon in description");
        assert!(alpha_is_link, "alpha copy should become a symlink to SSOT after import");
        assert_eq!(dup_count, 2, "beta + standard copies should be reported as managed duplicates");
        assert_eq!(handled, 2, "both duplicate origins should be taken over");
        assert_eq!(alpha_link, 1, "alpha global link stays enabled");
        assert_eq!(beta_link, 1, "beta global link should be enabled after takeover");
        assert_eq!(std_links, 0, "standard must never get any link");
        assert_eq!(beta_origin, 1, "beta origin should be recorded in skill_origins");
        assert!(beta_is_link, "beta copy should become a symlink to SSOT");
        assert!(!std_still_exists, "standard copy should be deleted");

        let _ = std::fs::remove_dir_all(&root);
    }
}
