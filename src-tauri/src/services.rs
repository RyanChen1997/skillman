use crate::db::Database;
use crate::models::Project;
use crate::skill::md::read_skill_md_raw;
use rusqlite::params;
use std::path::Path;
use std::sync::Arc;

pub fn list_projects(db: &Arc<Database>) -> Vec<Project> {
    let c = db.conn();
    let mut stmt = c.prepare("SELECT id,name,path FROM projects ORDER BY name").unwrap();
    stmt.query_map([], |r| Ok(Project { id: r.get(0)?, name: r.get(1)?, path: r.get(2)? })).unwrap()
        .filter_map(|r| r.ok()).collect()
}

pub fn add_project(db: &Arc<Database>, id: String, name: String, path: String) -> Project {
    let c = db.conn();
    c.execute("INSERT OR REPLACE INTO projects(id,name,path) VALUES(?1,?2,?3)", params![id, name, path]).ok();
    Project { id, name, path }
}

pub fn remove_project(db: &Arc<Database>, id: &str) {
    let c = db.conn();
    c.execute("DELETE FROM projects WHERE id=?1", params![id]).ok();
}

pub fn get_setting(db: &Arc<Database>, key: &str) -> Option<String> {
    let c = db.conn();
    c.query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get::<_, String>(0)).ok()
}

pub fn set_setting(db: &Arc<Database>, key: &str, value: &str) {
    let c = db.conn();
    c.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,?2)", params![key, value]).ok();
}

pub fn read_skill_md_source(db: &Arc<Database>, skill_id: &str) -> Option<String> {
    let c = db.conn();
    let dir: String = c.query_row("SELECT directory FROM skills WHERE id=?1", params![skill_id], |r| r.get(0)).ok()?;
    let ssot = crate::paths::ssot_dir().join(&dir);
    read_skill_md_raw(Path::new(&ssot)).ok()
}

