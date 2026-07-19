use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Arc<Database>> {
        let parent = path.as_ref().parent();
        if let Some(p) = parent {
            std::fs::create_dir_all(p)?;
        }
        let conn = Connection::open(path).map_err(io_err)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").ok();
        let db = Arc::new(Database { conn: Mutex::new(conn) });
        db.migrate();
        Ok(db)
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    fn migrate(&self) {
        let c = self.conn();
        c.execute_batch(SCHEMA).expect("migration failed");
        // Remove legacy columns if they still exist (SQLite >=3.35.0) and ensure
        // source_only column exists for older databases.
        let cols: Vec<String> = {
            let mut stmt = c.prepare("PRAGMA table_info(agents)").unwrap();
            stmt.query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        if cols.contains(&"custom_global_dir".to_string()) {
            c.execute("ALTER TABLE agents DROP COLUMN custom_global_dir", []).ok();
        }
        if cols.contains(&"custom_project_base".to_string()) {
            c.execute("ALTER TABLE agents DROP COLUMN custom_project_base", []).ok();
        }
        if !cols.contains(&"source_only".to_string()) {
            c.execute(
                "ALTER TABLE agents ADD COLUMN source_only INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .expect("add source_only column failed");
        }
    }
}

fn io_err(e: rusqlite::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS skills (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  directory     TEXT NOT NULL UNIQUE,
  description   TEXT,
  source        TEXT,
  content_hash  TEXT,
  installed_at  INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS skill_origins (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  skill_id     TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  origin_path  TEXT NOT NULL,
  found_in     TEXT NOT NULL,
  imported_at  INTEGER NOT NULL,
  UNIQUE(skill_id, origin_path)
);
CREATE TABLE IF NOT EXISTS skill_links (
  skill_id    TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  scope       TEXT NOT NULL,
  project_id  TEXT NOT NULL DEFAULT '',
  agent_id    TEXT NOT NULL REFERENCES agents(id),
  enabled     INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (skill_id, scope, project_id, agent_id)
);
CREATE TABLE IF NOT EXISTS agents (
  id                  TEXT PRIMARY KEY,
  name                TEXT NOT NULL,
  global_subpath      TEXT NOT NULL,
  project_subpath     TEXT NOT NULL,
  installed           INTEGER NOT NULL DEFAULT 0,
  source_only         INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS projects (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL,
  path  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_migrates_and_cascades() {
        let tmp = std::env::temp_dir().join(format!("skillman_db_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let db = Database::open(&tmp).unwrap();
        let c = db.conn();
        let names: Vec<String> = {
            let mut stmt = c.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap();
            stmt.query_map([], |r| r.get::<_, String>(0)).unwrap()
                .filter_map(|r| r.ok()).collect()
        };
        for t in ["skills", "skill_origins", "skill_links", "agents", "projects", "settings"] {
            assert!(names.contains(&t.to_string()), "missing table {t}");
        }
        c.execute("INSERT INTO agents(id,name,global_subpath,project_subpath,installed) VALUES('codex','Codex','.codex/skills','.codex/skills',1)", []).unwrap();
        c.execute("INSERT INTO skills(id,name,directory,installed_at,updated_at) VALUES('local:foo','foo','foo',1,1)", []).unwrap();
        c.execute("INSERT INTO skill_links(skill_id,scope,project_id,agent_id,enabled) VALUES('local:foo','global','','codex',1)", []).unwrap();
        c.execute("DELETE FROM skills WHERE id='local:foo'", []).unwrap();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM skill_links", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "cascade delete failed");
        drop(c);
        drop(db);
        let _ = std::fs::remove_file(&tmp);
    }
}
