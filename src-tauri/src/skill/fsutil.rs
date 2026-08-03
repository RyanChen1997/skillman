use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !src.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("src missing: {}", src.display()),
        ));
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ft.is_symlink() {
            let target = fs::read_link(&from)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &to)?;
            #[cfg(windows)]
            {
                if target.is_dir() {
                    std::os::windows::fs::symlink_dir(&target, &to)?;
                } else {
                    std::os::windows::fs::symlink_file(&target, &to)?;
                }
            }
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Replace `dst` with a symlink to `src`. If symlink creation fails, fallback to a copy.
pub fn create_symlink_or_copy(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() || fs::symlink_metadata(dst).is_ok() {
        remove_recursive(dst)?;
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let res = symlink(src, dst);
    if res.is_err() {
        // fallback copy
        copy_dir_recursive(src, dst)?;
    }
    Ok(())
}

#[cfg(unix)]
fn symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}
#[cfg(windows)]
fn symlink(src: &Path, dst: &Path) -> io::Result<()> {
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, dst)
    } else {
        std::os::windows::fs::symlink_file(src, dst)
    }
}

pub fn remove_recursive(path: &Path) -> io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn is_symlink_to(path: &Path, target: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => {
            fs::read_link(path)
                .map(|t| {
                    // Resolve relative symlink targets before comparing; on Windows
                    // symlinks often point to relative paths, and strict == would fail.
                    let resolved = if t.is_absolute() {
                        t.clone()
                    } else {
                        path.parent().unwrap_or(Path::new(".")).join(&t)
                    };
                    let resolved = fs::canonicalize(&resolved).unwrap_or(resolved);
                    let target_canon = fs::canonicalize(target).unwrap_or(target.to_path_buf());
                    resolved == target_canon
                })
                .unwrap_or(false)
        }
        _ => false,
    }
}

pub fn content_hash(dir: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut entries: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    entries.sort();
    for path in entries {
        let rel = path.strip_prefix(dir).unwrap_or(&path);
        hasher.update(rel.to_string_lossy().as_bytes());
        if let Ok(bytes) = fs::read(&path) {
            hasher.update(&bytes);
        }
    }
    Ok(hex::encode(&hasher.finalize()))
}

// hex encode (avoid extra dep)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("skillman_fsutil_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn copy_recursive_copies_files() {
        let root = tmp("copy");
        let src = root.join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("SKILL.md"), "# hi\n").unwrap();
        fs::create_dir(src.join("sub")).unwrap();
        fs::write(src.join("sub").join("a.txt"), "abc").unwrap();
        let dst = root.join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("SKILL.md").exists());
        assert_eq!(
            fs::read_to_string(dst.join("sub").join("a.txt")).unwrap(),
            "abc"
        );
    }

    #[test]
    fn symlink_or_copy_creates_symlink_then_copy_fallback() {
        let root = tmp("sym");
        let src = root.join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("f"), "1").unwrap();
        let dst = root.join("dst");
        create_symlink_or_copy(&src, &dst).unwrap();
        // dst should exist and resolve to src content
        assert_eq!(fs::read_to_string(dst.join("f")).unwrap(), "1");
        assert!(is_symlink_to(&dst, &src));
    }

    #[test]
    fn content_hash_is_stable_and_distinct() {
        let root = tmp("hash");
        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        fs::write(a.join("f"), "x").unwrap();
        let h1 = content_hash(&a).unwrap();
        let h2 = content_hash(&a).unwrap();
        assert_eq!(h1, h2);
        fs::write(a.join("f"), "y").unwrap();
        let h3 = content_hash(&a).unwrap();
        assert_ne!(h1, h3);
    }
}
