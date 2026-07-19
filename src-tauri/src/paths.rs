use std::path::PathBuf;

pub fn home() -> PathBuf {
    // Test/dev isolation: if SKILLMAN_HOME is set, all skillman paths (SSOT,
    // agent detection dirs, agent dest dirs) resolve under it instead of the
    // real user home. Leaves Tauri's own HOME usage untouched. Unset = prod.
    if let Ok(h) = std::env::var("SKILLMAN_HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn skillman_dir() -> PathBuf {
    home().join(".skillman")
}

pub fn ssot_dir() -> PathBuf {
    skillman_dir().join("skills")
}

pub fn db_path() -> PathBuf {
    skillman_dir().join("skillman.db")
}

pub fn backups_dir() -> PathBuf {
    skillman_dir().join("skill-backups")
}

#[cfg(test)]
pub fn with_test_home<F, R>(path: &std::path::Path, f: F) -> R
where
    F: FnOnce() -> R,
{
    use std::sync::Mutex;
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap();
    unsafe { std::env::set_var("SKILLMAN_HOME", path) };
    let r = f();
    unsafe { std::env::remove_var("SKILLMAN_HOME") };
    r
}
