mod models;
mod paths;
mod db;
mod agent;
mod skill;
mod services;

use crate::db::Database;
use crate::models::{Agent, Project, SkillView, UnmanagedSkill};
use crate::skill::import::ImportReq;
use std::collections::HashMap;
use std::sync::Arc;

struct AppState {
    db: Arc<Database>,
}

#[tauri::command]
fn detect_agents(state: tauri::State<'_, AppState>) -> Vec<Agent> {
    crate::agent::detect_agents(&state.db)
}

#[tauri::command]
fn list_agents(state: tauri::State<'_, AppState>) -> Vec<Agent> {
    crate::agent::list_agents(&state.db)
}

#[tauri::command]
fn scan_unmanaged(state: tauri::State<'_, AppState>) -> Vec<UnmanagedSkill> {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::scan::scan_unmanaged(&state.db, &projects)
}

#[tauri::command]
fn scan_project(state: tauri::State<'_, AppState>, project_id: String) -> Vec<UnmanagedSkill> {
    let projects = crate::services::list_projects(&state.db);
    match projects.iter().find(|p| p.id == project_id) {
        Some(p) => crate::skill::scan::scan_project(&state.db, p),
        None => Vec::new(),
    }
}

#[tauri::command]
fn confirm_import(state: tauri::State<'_, AppState>, imports: Vec<ImportReq>) -> Vec<SkillView> {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::import::confirm_import(&state.db, &projects, imports)
}

#[tauri::command]
fn list_skills(state: tauri::State<'_, AppState>) -> Vec<SkillView> {
    crate::skill::lifecycle::list_skills(&state.db)
}

#[tauri::command]
fn get_skill(state: tauri::State<'_, AppState>, id: String) -> Option<SkillView> {
    crate::skill::lifecycle::get_skill(&state.db, &id)
}

#[tauri::command]
fn toggle_link(state: tauri::State<'_, AppState>, skill_id: String, scope: String, project_id: Option<String>, agent_id: String, on: bool) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::sync::toggle_link(&state.db, &projects, &skill_id, &scope, project_id.as_deref(), &agent_id, on);
}

#[tauri::command]
fn batch_set_links(state: tauri::State<'_, AppState>, skill_ids: Vec<String>, on: bool) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::sync::batch_set_links(&state.db, &projects, &skill_ids, on);
}

#[tauri::command]
fn batch_add_to_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
    skill_ids: Vec<String>,
    agent_ids: Vec<String>,
) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::sync::batch_add_to_project(&state.db, &projects, &project_id, &skill_ids, &agent_ids);
}

#[tauri::command]
fn sync_all(state: tauri::State<'_, AppState>) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::sync::sync_all(&state.db, &projects);
}

#[tauri::command]
fn restore_skill(state: tauri::State<'_, AppState>, id: String) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::lifecycle::restore_skill(&state.db, &projects, &id);
}

#[tauri::command]
fn uninstall_skill(state: tauri::State<'_, AppState>, id: String) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::lifecycle::uninstall_skill(&state.db, &projects, &id);
}

#[tauri::command]
fn list_projects(state: tauri::State<'_, AppState>) -> Vec<Project> {
    crate::services::list_projects(&state.db)
}

#[tauri::command]
fn add_project(state: tauri::State<'_, AppState>, id: String, name: String, path: String) -> Project {
    crate::services::add_project(&state.db, id, name, path)
}

#[tauri::command]
fn remove_project(state: tauri::State<'_, AppState>, id: String) {
    crate::services::remove_project(&state.db, &id)
}

#[tauri::command]
fn get_setting(state: tauri::State<'_, AppState>, key: String) -> Option<String> {
    crate::services::get_setting(&state.db, &key)
}

#[tauri::command]
fn set_setting(state: tauri::State<'_, AppState>, key: String, value: String) {
    crate::services::set_setting(&state.db, &key, &value);
}

#[tauri::command]
fn read_skill_md_source(state: tauri::State<'_, AppState>, id: String) -> Option<String> {
    crate::services::read_skill_md_source(&state.db, &id)
}

#[tauri::command]
fn get_paths() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("home".to_string(), crate::paths::home().to_string_lossy().to_string());
    map.insert("ssot".to_string(), crate::paths::ssot_dir().to_string_lossy().to_string());
    map.insert("backups".to_string(), crate::paths::backups_dir().to_string_lossy().to_string());
    map.insert("separator".to_string(), std::path::MAIN_SEPARATOR.to_string());
    map
}

#[tauri::command]
fn reset_all(state: tauri::State<'_, AppState>) {
    let projects = crate::services::list_projects(&state.db);
    crate::skill::lifecycle::reset_all(&state.db, &projects);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Database::open(crate::paths::db_path()).expect("failed to open db");
    // ensure agents detected on first run
    let _ = crate::agent::detect_agents(&db);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { db })
        .invoke_handler(tauri::generate_handler![
            detect_agents, list_agents, scan_unmanaged, scan_project, confirm_import,
            list_skills, get_skill, toggle_link, batch_set_links, batch_add_to_project, sync_all,
            restore_skill, uninstall_skill, list_projects, add_project, remove_project,
            get_setting, set_setting, read_skill_md_source, get_paths, reset_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
