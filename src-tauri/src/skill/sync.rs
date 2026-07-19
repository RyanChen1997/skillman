use crate::agent::{list_agents, resolve_global_dest, resolve_project_dest};
use crate::db::Database;
use crate::models::{Agent, Project, SkillLink};
use crate::paths;
use crate::skill::fsutil::{copy_dir_recursive, create_symlink_or_copy, is_symlink_to, remove_recursive};
use rusqlite::params;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ts() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

fn dest_for(db: &Arc<Database>, link: &SkillLink, agents: &[Agent], projects: &[Project]) -> Option<PathBuf> {
    let agent = agents.iter().find(|a| a.id == link.agent_id)?;
    let dir: String = {
        let c = db.conn();
        c.query_row("SELECT directory FROM skills WHERE id=?1", params![link.skill_id], |r| r.get(0)).ok()?
    };
    match link.scope.as_str() {
        "global" => Some(resolve_global_dest(agent).join(dir)),
        "project" => {
            let pid = link.project_id.as_ref().filter(|p| !p.is_empty())?;
            let proj = projects.iter().find(|p| p.id == *pid)?;
            Some(resolve_project_dest(agent, Path::new(&proj.path)).join(dir))
        }
        _ => None,
    }
}

fn ssot_of(db: &Arc<Database>, skill_id: &str) -> PathBuf {
    let c = db.conn();
    let dir: String = c.query_row("SELECT directory FROM skills WHERE id=?1", params![skill_id], |r| r.get(0)).unwrap_or_default();
    paths::ssot_dir().join(dir)
}

pub fn toggle_link(db: &Arc<Database>, projects: &[Project], skill_id: &str, scope: &str, project_id: Option<&str>, agent_id: &str, on: bool) {
    let pid = project_id.unwrap_or("");
    {
        let c = db.conn();
        c.execute(
            "INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(skill_id,scope,project_id,agent_id) DO UPDATE SET enabled=excluded.enabled",
            params![skill_id, scope, pid, agent_id, on as i64],
        ).ok();
    }
    let pid_arg: Option<&str> = if pid.is_empty() { None } else { Some(pid) };
    reconcile_dest(db, projects, skill_id, scope, pid_arg, agent_id, on);
}

fn reconcile_dest(db: &Arc<Database>, projects: &[Project], skill_id: &str, scope: &str, project_id: Option<&str>, agent_id: &str, on: bool) {
    let agents = list_agents(db);
    let agent = match agents.iter().find(|a| a.id == agent_id) {
        Some(a) => a,
        None => return,
    };
    if agent.source_only { return; }
    let link = SkillLink { skill_id: skill_id.into(), scope: scope.into(), project_id: project_id.map(String::from), agent_id: agent_id.into(), enabled: on };
    let Some(dest) = dest_for(db, &link, &agents, projects) else { return };
    let ssot = ssot_of(db, skill_id);
    if !ssot.exists() { return; }
    if on {
        if dest.exists() && !is_symlink_to(&dest, &ssot) {
            // takeover: backup + ensure origin row
            let dir = ssot.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let backup = paths::backups_dir().join(format!("{}-link-{}-{}", dir, agent_id, now_ts()));
            let _ = fs::create_dir_all(paths::backups_dir());
            let _ = copy_dir_recursive(&dest, &backup);
            ensure_origin(db, skill_id, &dest, found_in_for(scope, project_id, agent_id));
        }
        let _ = create_symlink_or_copy(&ssot, &dest);
    } else {
        let _ = remove_recursive(&dest);
    }
}

fn found_in_for(scope: &str, project_id: Option<&str>, agent_id: &str) -> String {
    match scope {
        "project" => format!("project:{}", project_id.unwrap_or("")),
        _ => format!("agent:{}", agent_id),
    }
}

fn ensure_origin(db: &Arc<Database>, skill_id: &str, path: &Path, found_in: String) {
    let c = db.conn();
    c.execute(
        "INSERT OR IGNORE INTO skill_origins(skill_id,origin_path,found_in,imported_at) VALUES(?1,?2,?3,?4)",
        params![skill_id, path.to_string_lossy(), found_in, now_ts()],
    ).ok();
}

pub fn batch_add_to_project(
    db: &Arc<Database>,
    projects: &[Project],
    project_id: &str,
    skill_ids: &[String],
    agent_ids: &[String],
) {
    for skill_id in skill_ids {
        for agent_id in agent_ids {
            toggle_link(db, projects, skill_id, "project", Some(project_id), agent_id, true);
        }
    }
}

/// Set all links for each skill to `on`.
/// If on: create a global link for every installed agent. If off: all links disabled.
pub fn batch_set_links(db: &Arc<Database>, projects: &[Project], skill_ids: &[String], on: bool) {
    let agents = list_agents(db);
    for sid in skill_ids {
        if on {
            for a in agents.iter().filter(|a| a.installed && !a.source_only) {
                toggle_link(db, projects, sid, "global", None, &a.id, true);
            }
        } else {
            // disable all existing links
            // Scope the connection guard to the query only; toggle_link below
            // acquires its own guard -> holding here would deadlock (Mutex is
            // NOT reentrant).
            let links: Vec<(String, Option<String>, String)> = {
                let c = db.conn();
                let mut stmt = c.prepare("SELECT scope,project_id,agent_id FROM skill_links WHERE skill_id=?1").unwrap();
                stmt.query_map(params![sid], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?))).unwrap()
                    .filter_map(|r| r.ok()).collect()
            };
            for (scope, pid, aid) in links {
                toggle_link(db, projects, sid, &scope, pid.as_deref(), &aid, false);
            }
        }
    }
}

pub fn sync_all(db: &Arc<Database>, projects: &[Project]) {
    let agents = list_agents(db);
    // global dests
    for a in &agents {
        if !a.installed { continue; }
        let dest_root = resolve_global_dest(a);
        let entries = match fs::read_dir(&dest_root) { Ok(e) => e, Err(_) => continue };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            if is_symlink_to(&p, &paths::ssot_dir().join(&name)) {
                // orphan symlink: keep only if a matching enabled link exists
                let has_enabled = link_enabled(db, &name, "global", None, &a.id);
                if !has_enabled { let _ = remove_recursive(&p); }
            }
        }
    }
    // project dests
    for proj in projects {
        for a in &agents {
            if !a.installed { continue; }
            let dest_root = resolve_project_dest(a, Path::new(&proj.path));
            let entries = match fs::read_dir(&dest_root) { Ok(e) => e, Err(_) => continue };
            for entry in entries.flatten() {
                let p = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }
                if is_symlink_to(&p, &paths::ssot_dir().join(&name)) {
                    let has_enabled = link_enabled(db, &name, "project", Some(&proj.id), &a.id);
                    if !has_enabled { let _ = remove_recursive(&p); }
                }
            }
        }
    }
    // ensure enabled links have a symlink
    let skill_ids: Vec<String> = {
        let c = db.conn();
        let mut stmt = c.prepare("SELECT DISTINCT skill_id FROM skill_links WHERE enabled=1").unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0)).unwrap().filter_map(|r| r.ok()).collect()
    };
    for sid in skill_ids {
        let links: Vec<(String, Option<String>, String)> = {
            let c = db.conn();
            let mut stmt = c.prepare("SELECT scope,project_id,agent_id FROM skill_links WHERE skill_id=?1 AND enabled=1").unwrap();
            stmt.query_map(params![sid], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?))).unwrap()
                .filter_map(|r| r.ok()).collect()
        };
        for (scope, pid, aid) in links {
            reconcile_dest(db, projects, &sid, &scope, pid.as_deref(), &aid, true);
        }
    }
}

fn link_enabled(db: &Arc<Database>, skill_dir: &str, scope: &str, project_id: Option<&str>, agent_id: &str) -> bool {
    let sid = format!("local:{}", skill_dir);
    let pid = project_id.unwrap_or("");
    let c = db.conn();
    c.query_row(
        "SELECT enabled FROM skill_links WHERE skill_id=?1 AND scope=?2 AND project_id=?3 AND agent_id=?4",
        params![sid, scope, pid, agent_id],
        |r| r.get::<_, i64>(0),
    ).map(|v| v != 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Regression: batch_set_links(on=false) previously held the Mutex guard
    /// across toggle_link calls in the disable loop -> deadlock. With >1 link
    /// this must return without hanging and disable all links.
    #[test]
    fn batch_set_links_disable_does_not_deadlock() {
        let p = std::env::temp_dir().join(format!("skillman_batch_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let db = Database::open(&p).unwrap();
        {
            let c = db.conn();
            c.execute("INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('codex','Codex','.codex/skills','.codex/skills',1)", []).unwrap();
            c.execute("INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('claude-code','Claude Code','.claude/skills','.claude/skills',1)", []).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:foo','foo','foo',1,1)", []).unwrap();
            c.execute("INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES('local:foo','global','','codex',1)", []).unwrap();
            c.execute("INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES('local:foo','global','','claude-code',1)", []).unwrap();
        }
        batch_set_links(&db, &[], &["local:foo".to_string()], false);
        let n: i64 = {
            let c = db.conn();
            c.query_row("SELECT COUNT(*) FROM skill_links WHERE skill_id='local:foo' AND enabled=1", [], |r| r.get(0)).unwrap()
        };
        assert_eq!(n, 0, "all links should be disabled");
        drop(db);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn toggle_link_upserts_and_removes() {
        // Use a unique skill dir name ("tgfoo") so this test's SSOT dir under
        // ~/.skillman/skills cannot collide with the scan test's "foo"/"bar"
        // when tests run in parallel (scan_unmanaged scans the shared SSOT).
        let p = std::env::temp_dir().join(format!("skillman_sync_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let db = Database::open(&p).unwrap();
        let root = std::env::temp_dir().join(format!("skillman_syncroot_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let ssot = home.join(".skillman/skills/tgfoo");
        let dest = home.join(".x/s/tgfoo");
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/s','.x/s',1,0)",
                [],
            ).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:tgfoo','tgfoo','tgfoo',1,1)", []).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dest);
        crate::paths::with_test_home(&home, || {
            // create a fake SSOT inside the test home so reconcile has something to link
            let _ = std::fs::remove_dir_all(&ssot);
            std::fs::create_dir_all(&ssot).unwrap();
            std::fs::write(ssot.join("SKILL.md"), "# tgfoo").unwrap();
            toggle_link(&db, &[], "local:tgfoo", "global", None, "testagent", true);
            assert!(dest.exists(), "dest should be created on enable");
            toggle_link(&db, &[], "local:tgfoo", "global", None, "testagent", false);
            assert!(!dest.exists(), "dest should be removed on disable");
        });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn batch_add_to_project_creates_links_and_symlinks() {
        use crate::db::Database;
        use crate::models::Project;
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("skillman_batchproj_{}_{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&p);
        let db = Database::open(&p).unwrap();
        let root = std::env::temp_dir().join(format!("skillman_batchprojroot_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let ssot_a = home.join(".skillman/skills/bp-a");
        let ssot_b = home.join(".skillman/skills/bp-b");
        let proj_dir = root.join("proj");
        let claude_dest = proj_dir.join(".claude/skills");
        let codex_dest = proj_dir.join(".codex/skills");

        std::fs::create_dir_all(&ssot_a).unwrap();
        std::fs::create_dir_all(&ssot_b).unwrap();
        std::fs::write(ssot_a.join("SKILL.md"), "---\nname: bp-a\ndescription: A.\n---\n").unwrap();
        std::fs::write(ssot_b.join("SKILL.md"), "---\nname: bp-b\ndescription: B.\n---\n").unwrap();

        {
            let c = db.conn();
            c.execute("INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('claude-code','Claude Code','.claude/skills','.claude/skills',1)", []).unwrap();
            c.execute("INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('codex','Codex','.codex/skills','.codex/skills',1)", []).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:bp-a','bp-a','bp-a',1,1)", []).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:bp-b','bp-b','bp-b',1,1)", []).unwrap();
        }
        let projects = vec![Project { id: "demo".into(), name: "Demo".into(), path: proj_dir.to_string_lossy().to_string() }];

        crate::paths::with_test_home(&home, || {
            batch_add_to_project(&db, &projects, "demo", &["local:bp-a".into(), "local:bp-b".into()], &["claude-code".into(), "codex".into()]);
        });

        let count: i64 = {
            let c = db.conn();
            c.query_row("SELECT COUNT(*) FROM skill_links WHERE scope='project' AND project_id='demo' AND enabled=1", [], |r| r.get(0)).unwrap()
        };
        assert_eq!(count, 4, "should create 4 enabled project links");
        assert!(claude_dest.join("bp-a").exists(), "claude bp-a symlink should exist");
        assert!(claude_dest.join("bp-b").exists(), "claude bp-b symlink should exist");
        assert!(codex_dest.join("bp-a").exists(), "codex bp-a symlink should exist");
        assert!(codex_dest.join("bp-b").exists(), "codex bp-b symlink should exist");

        let _ = std::fs::remove_dir_all(&root);
    }
}
