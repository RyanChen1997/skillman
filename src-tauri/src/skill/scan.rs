use crate::agent::{list_agents, resolve_global_dest};
use crate::db::Database;
use crate::models::{Project, UnmanagedOrigin, UnmanagedSkill};
use crate::paths;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

fn collect_skills(
    managed: &HashSet<String>,
    candidates: &[(PathBuf, String)],
) -> HashMap<String, UnmanagedSkill> {
    let mut map: HashMap<String, UnmanagedSkill> = HashMap::new();
    for (dir, found_in) in candidates {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if !ft.is_dir() && !ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if managed.contains(&name) {
                continue;
            }
            let path = entry.path();
            let md = match crate::skill::md::read_skill_md(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let origin = UnmanagedOrigin {
                path: path.to_string_lossy().to_string(),
                found_in: found_in.clone(),
            };
            let skill = map.entry(name.clone()).or_insert_with(|| UnmanagedSkill {
                directory: name.clone(),
                name: md.name.clone(),
                description: Some(md.description),
                origins: Vec::new(),
            });
            skill.origins.push(origin);
        }
    }
    map
}

pub fn scan_project(db: &Arc<Database>, project: &Project) -> Vec<UnmanagedSkill> {
    let managed: HashSet<String> = {
        let c = db.conn();
        let mut stmt = c.prepare("SELECT directory FROM skills").unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };

    let agents = list_agents(db);
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    for a in &agents {
        if a.installed {
            candidates.push((
                PathBuf::from(&project.path).join(&a.project_subpath),
                format!("project:{}", project.id),
            ));
        }
    }

    let mut out: Vec<UnmanagedSkill> = collect_skills(&managed, &candidates).into_values().collect();
    out.sort_by(|a, b| a.directory.cmp(&b.directory));
    out
}

pub fn scan_unmanaged(db: &Arc<Database>, projects: &[Project]) -> Vec<UnmanagedSkill> {
    let managed: HashSet<String> = {
        let c = db.conn();
        let mut stmt = c.prepare("SELECT directory FROM skills").unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };

    let agents = list_agents(db);
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    for a in &agents {
        if a.installed {
            candidates.push((resolve_global_dest(a), format!("agent:{}", a.id)));
        }
    }
    for p in projects {
        for a in &agents {
            if a.installed {
                candidates.push((
                    PathBuf::from(&p.path).join(&a.project_subpath),
                    format!("project:{}", p.id),
                ));
            }
        }
    }
    candidates.push((paths::ssot_dir(), "ssot".to_string()));

    let mut out: Vec<UnmanagedSkill> = collect_skills(&managed, &candidates).into_values().collect();
    out.sort_by(|a, b| a.directory.cmp(&b.directory));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::Project;
    use std::sync::atomic::{AtomicU64, Ordering};

    static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_db() -> Arc<Database> {
        let n = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("skillman_scan_{}_{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&p);
        Database::open(&p).unwrap()
    }

    #[test]
    fn finds_unmanaged_and_dedups() {
        let root = std::env::temp_dir().join(format!("skillman_scanroot_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let skill_home = root.join("home");
        let agent_dir = skill_home.join(".x/skills");
        std::fs::create_dir_all(agent_dir.join("foo")).unwrap();
        std::fs::write(
            agent_dir.join("foo").join("SKILL.md"),
            "---\nname: Foo\ndescription: Foo desc.\n---\n# foo\n\nFoo desc.",
        )
        .unwrap();
        std::fs::create_dir_all(agent_dir.join("bar")).unwrap();
        std::fs::write(
            agent_dir.join("bar").join("SKILL.md"),
            "---\nname: Bar\ndescription: Bar desc.\n---\n# bar\n\nBar desc.",
        )
        .unwrap();
        // not a skill (no SKILL.md)
        std::fs::create_dir_all(agent_dir.join("notaskill")).unwrap();
        // hidden
        std::fs::create_dir_all(agent_dir.join(".hidden")).unwrap();
        std::fs::write(
            agent_dir.join(".hidden").join("SKILL.md"),
            "---\nname: Hidden\ndescription: Hidden desc.\n---\nx",
        )
        .unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','Test','.x/skills','.x/skills',1,0)",
                [],
            )
            .unwrap();
        }
        let projects: Vec<Project> = vec![];
        let result = crate::paths::with_test_home(&skill_home, || scan_unmanaged(&db, &projects));
        let dirs: Vec<String> = result.iter().map(|s| s.directory.clone()).collect();
        assert!(dirs.contains(&"foo".to_string()));
        assert!(dirs.contains(&"bar".to_string()));
        assert!(!dirs.contains(&"notaskill".to_string()));
        assert!(!dirs.contains(&".hidden".to_string()));
        // foo has one origin (agent:testagent)
        let foo = result.iter().find(|s| s.directory == "foo").unwrap();
        assert_eq!(foo.origins.len(), 1);
        assert_eq!(foo.origins[0].found_in, "agent:testagent");
        assert_eq!(foo.description.as_deref(), Some("Foo desc."));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_project_finds_skills_under_agent_subpaths() {
        let root =
            std::env::temp_dir().join(format!("skillman_projscan_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let claude_dir = root.join(".claude/skills/proj-foo");
        let codex_dir = root.join(".codex/skills/proj-bar");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::write(
            claude_dir.join("SKILL.md"),
            "---\nname: Proj Foo\ndescription: Foo desc.\n---\n# proj-foo\n\nFoo desc.",
        )
        .unwrap();
        std::fs::write(
            codex_dir.join("SKILL.md"),
            "---\nname: Proj Bar\ndescription: Bar desc.\n---\n# proj-bar\n\nBar desc.",
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

        let project = Project {
            id: "demo".into(),
            name: "Demo".into(),
            path: root.to_string_lossy().to_string(),
        };
        let result = scan_project(&db, &project);
        let dirs: Vec<String> = result.iter().map(|s| s.directory.clone()).collect();
        assert!(dirs.contains(&"proj-foo".to_string()));
        assert!(dirs.contains(&"proj-bar".to_string()));
        let foo = result.iter().find(|s| s.directory == "proj-foo").unwrap();
        assert_eq!(foo.origins.len(), 1);
        assert!(foo.origins[0].found_in.starts_with("project:"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_skill_without_valid_frontmatter() {
        let root = std::env::temp_dir().join(format!("skillman_scan_invalid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let skill_home = root.join("home");
        let agent_dir = skill_home.join(".x/skills");
        std::fs::create_dir_all(agent_dir.join("good")).unwrap();
        std::fs::write(agent_dir.join("good").join("SKILL.md"), "---\nname: good\ndescription: Good desc.\n---\n").unwrap();
        std::fs::create_dir_all(agent_dir.join("bad")).unwrap();
        std::fs::write(agent_dir.join("bad").join("SKILL.md"), "# bad\n\nNo frontmatter.").unwrap();

        let db = tmp_db();
        {
            let c = db.conn();
            c.execute(
                "INSERT INTO agents(id,name,global_subpath,project_subpath,installed,source_only) VALUES('testagent','Test','.x/skills','.x/skills',1,0)",
                [],
            ).unwrap();
        }
        let result = crate::paths::with_test_home(&skill_home, || scan_unmanaged(&db, &[]));
        let dirs: Vec<String> = result.iter().map(|s| s.directory.clone()).collect();
        assert!(dirs.contains(&"good".to_string()));
        assert!(!dirs.contains(&"bad".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }
}
