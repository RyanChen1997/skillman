use serde::Deserialize;
use std::fs;
use std::io;
use std::path::Path;

pub struct SkillMd {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

/// Read the raw SKILL.md file without parsing or validating frontmatter.
/// Used for the preview renderer so older skills lacking frontmatter still render.
pub fn read_skill_md_raw(dir: &Path) -> io::Result<String> {
    let path = dir.join("SKILL.md");
    fs::read_to_string(&path)
}

pub fn read_skill_md(dir: &Path) -> io::Result<SkillMd> {
    let path = dir.join("SKILL.md");
    let raw = fs::read_to_string(&path)?;
    let (front_yaml, _) = split_frontmatter(&raw);
    let front_yaml = front_yaml.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "SKILL.md has no frontmatter")
    })?;
    let front: Frontmatter = serde_yaml::from_str(front_yaml).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("SKILL.md frontmatter is malformed YAML: {}", e),
        )
    })?;

    let name = front
        .name
        .filter(|n| !n.trim().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "SKILL.md frontmatter missing name",
            )
        })?
        .trim()
        .to_string();

    let description = front
        .description
        .filter(|d| !d.trim().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "SKILL.md frontmatter missing description",
            )
        })?
        .trim()
        .to_string();

    Ok(SkillMd { name, description })
}

/// Split raw markdown into optional YAML frontmatter content and the body.
fn split_frontmatter(raw: &str) -> (Option<&str>, &str) {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return (None, raw);
    }
    let after_open = &trimmed[3..];
    if let Some(end) = after_open.find("\n---") {
        let yaml = &after_open[..end];
        // Strip a trailing carriage return left over from CRLF line endings.
        let yaml = yaml.strip_suffix('\r').unwrap_or(yaml);
        let body = &trimmed[3 + end + 4..];
        // Skip the newline after the closing delimiter, whether LF or CRLF.
        let body = body.strip_prefix("\r\n").or_else(|| body.strip_prefix('\n')).unwrap_or(body);
        (Some(yaml), body)
    } else {
        (None, raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_skill_dir(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!("{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let dir = tmp.join("my-skill");
        std::fs::create_dir_all(&dir).unwrap();
        (tmp, dir)
    }

    #[test]
    fn parses_frontmatter_name_and_description() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_parse");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: Agent Reach\ndescription: Multi-platform internet research hub.\n---\n# Agent Reach\n\nOther body.\n",
        )
        .unwrap();

        let md = read_skill_md(&dir).unwrap();
        assert_eq!(md.name, "Agent Reach");
        assert_eq!(md.description, "Multi-platform internet research hub.");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rejects_missing_description() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_no_desc");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: Agent Reach\n---\n# Agent Reach\n\nOther body.\n",
        )
        .unwrap();

        assert!(read_skill_md(&dir).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rejects_missing_name() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_no_name");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\ndescription: Multi-platform internet research hub.\n---\n# Agent Reach\n\nOther body.\n",
        )
        .unwrap();

        assert!(read_skill_md(&dir).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rejects_empty_description() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_empty_desc");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: Agent Reach\ndescription: \"\"\n---\n# Agent Reach\n\nOther body.\n",
        )
        .unwrap();

        assert!(read_skill_md(&dir).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rejects_no_frontmatter() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_no_front");
        std::fs::write(dir.join("SKILL.md"), "# Agent Reach\n\nOther body.\n").unwrap();

        assert!(read_skill_md(&dir).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rejects_malformed_yaml() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_bad_yaml");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: : bad\n---\n# Agent Reach\n\nOther body.\n",
        )
        .unwrap();

        assert!(read_skill_md(&dir).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parses_frontmatter_with_crlf_line_endings() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_crlf");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\r\nname: Agent Reach\r\ndescription: Multi-platform internet research hub.\r\n---\r\n# Agent Reach\r\n\r\nOther body.\r\n",
        )
        .unwrap();

        let md = read_skill_md(&dir).unwrap();
        assert_eq!(md.name, "Agent Reach");
        assert_eq!(md.description, "Multi-platform internet research hub.");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
