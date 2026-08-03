use crate::agent::{list_agents, resolve_global_dest, resolve_project_dest};
use crate::db::Database;
use crate::models::{SkillLink, SkillOrigin, SkillView};
use crate::paths;
use crate::skill::fsutil::{copy_dir_recursive, is_symlink_to, remove_recursive};
use rusqlite::params;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn list_skills(db: &Arc<Database>) -> Vec<SkillView> {
    // Collect skills in a block so the connection guard (Mutex) is dropped
    // BEFORE we call load_links/load_origins, which each acquire their own
    // guard. std::sync::Mutex is NOT reentrant -> holding here would deadlock.
    let skills: Vec<crate::models::InstalledSkill> = {
        let c = db.conn();
        let mut stmt = c.prepare(
            "SELECT id,name,directory,description,source,content_hash,installed_at,updated_at FROM skills ORDER BY installed_at DESC, name"
        ).unwrap();
        stmt.query_map([], |r| {
            Ok(crate::models::InstalledSkill {
                id: r.get(0)?,
                name: r.get(1)?,
                directory: r.get(2)?,
                description: r.get(3)?,
                source: r.get(4)?,
                content_hash: r.get(5)?,
                installed_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    };
    let mut views = Vec::new();
    for s in skills {
        let links = load_links(db, &s.id);
        let origins = load_origins(db, &s.id);
        let any_enabled = links.iter().any(|l| l.enabled);
        views.push(SkillView {
            skill: s,
            links,
            origins,
            any_enabled,
        });
    }
    views
}

pub fn get_skill(db: &Arc<Database>, id: &str) -> Option<SkillView> {
    list_skills(db).into_iter().find(|s| s.skill.id == id)
}

fn load_links(db: &Arc<Database>, skill_id: &str) -> Vec<SkillLink> {
    let c = db.conn();
    let mut stmt = c
        .prepare(
            "SELECT skill_id,scope,project_id,agent_id,enabled FROM skill_links WHERE skill_id=?1",
        )
        .unwrap();
    stmt.query_map(params![skill_id], |r| {
        let raw: Option<String> = r.get(2)?;
        Ok(SkillLink {
            skill_id: r.get(0)?,
            scope: r.get(1)?,
            project_id: raw.filter(|p| !p.is_empty()),
            agent_id: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

fn load_origins(db: &Arc<Database>, skill_id: &str) -> Vec<SkillOrigin> {
    let c = db.conn();
    let mut stmt = c
        .prepare(
            "SELECT skill_id,origin_path,found_in,imported_at FROM skill_origins WHERE skill_id=?1",
        )
        .unwrap();
    stmt.query_map(params![skill_id], |r| {
        Ok(SkillOrigin {
            skill_id: r.get(0)?,
            origin_path: r.get(1)?,
            found_in: r.get(2)?,
            imported_at: r.get(3)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

/// Resolve the exact skill directory (not the whole agent dir) for a link.
/// Must join the skill's `directory` — returning the bare agent dir here would
/// make restore/uninstall `remove_recursive` the ENTIRE agent skills directory
/// (data loss: unrelated skills placed there get wiped).
fn dest_for_link(
    db: &Arc<Database>,
    link: &SkillLink,
    agents: &[crate::models::Agent],
    projects: &[crate::models::Project],
) -> Option<std::path::PathBuf> {
    let agent = agents.iter().find(|a| a.id == link.agent_id)?;
    let dir: String = {
        let c = db.conn();
        c.query_row(
            "SELECT directory FROM skills WHERE id=?1",
            params![link.skill_id],
            |r| r.get(0),
        )
        .ok()?
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

pub fn restore_skill(db: &Arc<Database>, projects: &[crate::models::Project], id: &str) {
    let agents = list_agents(db);
    let ssot = paths::ssot_dir().join(skill_dir_of(db, id));
    // 1. remove all symlinks/copy for enabled links
    for link in load_links(db, id) {
        if !link.enabled {
            continue;
        }
        if let Some(dest) = dest_for_link(db, &link, &agents, projects) {
            let _ = remove_recursive(&dest);
        }
    }
    // 2. copy SSOT back to each origin (if currently symlink/missing)
    for o in load_origins(db, id) {
        let origin = Path::new(&o.origin_path);
        if !origin.exists() || is_symlink_to(origin, &ssot) {
            if is_symlink_to(origin, &ssot) {
                let _ = remove_recursive(origin);
            }
            if let Some(parent) = origin.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = copy_dir_recursive(&ssot, origin);
        }
    }
    // 3. delete SSOT
    let _ = remove_recursive(&ssot);
    // 4. delete DB rows
    let c = db.conn();
    c.execute("DELETE FROM skills WHERE id=?1", params![id])
        .ok();
}

pub fn uninstall_skill(db: &Arc<Database>, projects: &[crate::models::Project], id: &str) {
    let agents = list_agents(db);
    let dir = skill_dir_of(db, id);
    let ssot = paths::ssot_dir().join(&dir);
    // 1. remove all symlinks/copy
    for link in load_links(db, id) {
        if !link.enabled {
            continue;
        }
        if let Some(dest) = dest_for_link(db, &link, &agents, projects) {
            let _ = remove_recursive(&dest);
        }
    }
    // 2. backup SSOT
    if ssot.exists() {
        let _ = fs::create_dir_all(paths::backups_dir());
        let backup = paths::backups_dir().join(format!("{}-uninstall-{}", dir, now_ts()));
        let _ = copy_dir_recursive(&ssot, &backup);
    }
    // 3. delete SSOT
    let _ = remove_recursive(&ssot);
    // 4. delete DB rows
    let c = db.conn();
    c.execute("DELETE FROM skills WHERE id=?1", params![id])
        .ok();
}

pub fn reset_all(db: &Arc<Database>, projects: &[crate::models::Project]) {
    // Collect ids first so we can delete rows while iterating without skipping.
    let skill_ids: Vec<String> = {
        let c = db.conn();
        let mut stmt = c.prepare("SELECT id FROM skills").unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };

    for id in skill_ids {
        restore_skill(db, projects, &id);
    }

    // Clear projects table.
    {
        let c = db.conn();
        c.execute("DELETE FROM projects", []).ok();
    }

    // NOTE: skill-backups 目录刻意保留,不清空。它是操作安全网:导入接管/
    // 自动删除重复/卸载时都会先备份原文。万一后续代码出 bug 弄丢了真实文件,
    // 用户可以来这里手动找回。重置只恢复 skill 与清库,不动备份。
}

fn skill_dir_of(db: &Arc<Database>, id: &str) -> String {
    let c = db.conn();
    c.query_row(
        "SELECT directory FROM skills WHERE id=?1",
        params![id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Regression: list_skills previously held the Mutex guard across calls to
    /// load_links/load_origins (each acquiring their own), deadlocking on a
    /// non-empty DB. With >0 skills this must return without hanging.
    #[test]
    fn list_skills_with_links_does_not_deadlock() {
        let p = std::env::temp_dir().join(format!("skillman_list_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let db = Database::open(&p).unwrap();
        {
            let c = db.conn();
            c.execute("INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('codex','Codex','.codex/skills','.codex/skills',1)", []).unwrap();
            c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:foo','foo','foo',1,1)", []).unwrap();
            c.execute("INSERT INTO skill_origins(skill_id,origin_path,found_in,imported_at) VALUES('local:foo','/x','agent:codex',1)", []).unwrap();
            c.execute("INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES('local:foo','global','','codex',1)", []).unwrap();
        }
        let views = list_skills(&db);
        assert_eq!(views.len(), 1, "should return the one skill");
        assert_eq!(views[0].links.len(), 1, "should load its link");
        assert!(views[0].any_enabled, "link is enabled");
        assert_eq!(views[0].origins.len(), 1, "should load its origin");
        drop(db);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn reset_all_restores_origins_and_clears_db() {
        use crate::paths;
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("skillman_reset_all_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let agent_dir = home.join(".x/skills");
        let skill_src = agent_dir.join("resetfoo");
        std::fs::create_dir_all(&skill_src).unwrap();
        std::fs::write(
            skill_src.join("SKILL.md"),
            "---\nname: resetfoo\ndescription: Desc.\n---\n# resetfoo\n\nDesc.\n",
        )
        .unwrap();

        let db = {
            let p = std::env::temp_dir().join(format!(
                "skillman_reset_all_db_{}_{}.db",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_file(&p);
            Database::open(&p).unwrap()
        };
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }

        let projects: Vec<crate::models::Project> = vec![];

        crate::paths::with_test_home(&home, || {
            // Import the skill
            let imports = vec![crate::skill::import::ImportReq {
                dir: "resetfoo".into(),
                origins: vec![crate::models::UnmanagedOrigin {
                    path: skill_src.to_string_lossy().to_string(),
                    found_in: "agent:testagent".into(),
                }],
            }];
            let _ = crate::skill::import::confirm_import(&db, &projects, imports);

            // At this point the agent dir should be a symlink to SSOT
            assert!(skill_src.exists(), "symlink should exist after import");

            // Call reset_all
            reset_all(&db, &projects);

            // Verify origin is restored as real directory (not symlink)
            assert!(skill_src.exists(), "origin should exist after reset");
            let meta = std::fs::symlink_metadata(&skill_src).unwrap();
            assert!(
                !meta.file_type().is_symlink(),
                "origin should not be a symlink after reset"
            );

            // Verify SSOT is gone
            assert!(
                !paths::ssot_dir().join("resetfoo").exists(),
                "SSOT should be deleted"
            );

            // Verify DB is empty
            let c = db.conn();
            let skill_count: i64 = c
                .query_row("SELECT COUNT(*) FROM skills", [], |r| r.get(0))
                .unwrap();
            let link_count: i64 = c
                .query_row("SELECT COUNT(*) FROM skill_links", [], |r| r.get(0))
                .unwrap();
            let origin_count: i64 = c
                .query_row("SELECT COUNT(*) FROM skill_origins", [], |r| r.get(0))
                .unwrap();
            let project_count: i64 = c
                .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
                .unwrap();
            assert_eq!(skill_count, 0, "skills should be empty");
            assert_eq!(link_count, 0, "skill_links should be empty");
            assert_eq!(origin_count, 0, "skill_origins should be empty");
            assert_eq!(project_count, 0, "projects should be empty");
        });

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Regression: reset previously removed the ENTIRE agent skills directory
    /// (`dest_for_link` missed `.join(dir)`), wiping unrelated skills the user
    /// had placed in that agent dir. After reset, only the skill's own dir may
    /// be touched; unrelated files must survive untouched.
    #[test]
    fn reset_all_keeps_unrelated_files_in_agent_dir() {
        use crate::paths;
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("skillman_reset_unrel_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let agent_dir = home.join(".x/skills");
        let skill_src = agent_dir.join("resetfoo");
        std::fs::create_dir_all(&skill_src).unwrap();
        std::fs::write(
            skill_src.join("SKILL.md"),
            "---\nname: resetfoo\ndescription: Desc.\n---\n# resetfoo\n\nDesc.\n",
        )
        .unwrap();

        let db = {
            let p = std::env::temp_dir().join(format!(
                "skillman_reset_unrel_db_{}_{}.db",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_file(&p);
            Database::open(&p).unwrap()
        };
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }

        let projects: Vec<crate::models::Project> = vec![];

        crate::paths::with_test_home(&home, || {
            // 1. import resetfoo -> agent dir entry becomes a symlink to SSOT
            let imports = vec![crate::skill::import::ImportReq {
                dir: "resetfoo".into(),
                origins: vec![crate::models::UnmanagedOrigin {
                    path: skill_src.to_string_lossy().to_string(),
                    found_in: "agent:testagent".into(),
                }],
            }];
            let _ = crate::skill::import::confirm_import(&db, &projects, imports);
            assert!(
                crate::skill::fsutil::is_symlink_to(
                    &skill_src,
                    &paths::ssot_dir().join("resetfoo")
                ),
                "import should leave a symlink in the agent dir"
            );

            // 2. user then drops an UNRELATED skill (never imported) into the same agent dir
            let unrelated = agent_dir.join("brand-new-skill");
            std::fs::create_dir_all(&unrelated).unwrap();
            std::fs::write(
                unrelated.join("SKILL.md"),
                "---\nname: brand-new-skill\ndescription: Never imported.\n---\n# brand-new-skill\n\nFresh.\n",
            )
            .unwrap();

            // 3. reset
            reset_all(&db, &projects);

            // 4. the unrelated skill must survive untouched
            assert!(
                unrelated.exists(),
                "unrelated skill in agent dir must survive reset"
            );
            let meta = std::fs::symlink_metadata(&unrelated).unwrap();
            assert!(
                !meta.file_type().is_symlink(),
                "unrelated skill must stay a real dir"
            );
            assert!(
                std::fs::read_to_string(unrelated.join("SKILL.md"))
                    .unwrap()
                    .contains("brand-new-skill"),
                "unrelated skill content must be intact"
            );
            // the imported skill itself is restored as a real dir
            let meta2 = std::fs::symlink_metadata(&skill_src).unwrap();
            assert!(
                !meta2.file_type().is_symlink(),
                "imported skill should be restored as real dir"
            );
        });

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Regression: reset must NOT clear the skill-backups directory. Backups are
    /// the safety net for recovering files after bugs; reset only restores
    /// skills and clears the DB, leaving backups untouched for manual recovery.
    #[test]
    fn reset_all_keeps_backups_dir() {
        use crate::paths;
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "skillman_reset_backup_{}_{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("home");
        let agent_dir = home.join(".x/skills");
        let skill_src = agent_dir.join("bkfoo");
        std::fs::create_dir_all(&skill_src).unwrap();
        std::fs::write(
            skill_src.join("SKILL.md"),
            "---\nname: bkfoo\ndescription: Desc.\n---\n# bkfoo\n\nDesc.\n",
        )
        .unwrap();

        let db = {
            let p = std::env::temp_dir().join(format!(
                "skillman_reset_backup_db_{}_{}.db",
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_file(&p);
            Database::open(&p).unwrap()
        };
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','T','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }

        let projects: Vec<crate::models::Project> = vec![];

        crate::paths::with_test_home(&home, || {
            // import -> writes a preimport backup into skill-backups
            let imports = vec![crate::skill::import::ImportReq {
                dir: "bkfoo".into(),
                origins: vec![crate::models::UnmanagedOrigin {
                    path: skill_src.to_string_lossy().to_string(),
                    found_in: "agent:testagent".into(),
                }],
            }];
            let _ = crate::skill::import::confirm_import(&db, &projects, imports);

            let backups_dir = paths::backups_dir();
            let before: Vec<_> = std::fs::read_dir(&backups_dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name())
                .collect();
            assert!(
                !before.is_empty(),
                "import should have created a preimport backup"
            );

            // reset
            reset_all(&db, &projects);

            // backups must survive reset untouched
            let after: Vec<_> = std::fs::read_dir(&backups_dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name())
                .collect();
            assert_eq!(
                before.len(),
                after.len(),
                "reset must not clear the backups dir: before={:?} after={:?}",
                before,
                after
            );
            assert!(backups_dir.exists(), "backups dir should still exist");
        });

        let _ = std::fs::remove_dir_all(&root);
    }
}
