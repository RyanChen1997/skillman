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
    // Strict parse first; fall back to a lenient line-based parse for real-world
    // frontmatter that strict YAML rejects (e.g. an unquoted `: ` inside a plain
    // scalar description like "Do NOT use for: 单篇文章...").
    match serde_yaml::from_str::<Frontmatter>(front_yaml) {
        Ok(front) => build_skill_md(front.name, front.description),
        Err(strict_err) => match parse_frontmatter_lenient(front_yaml) {
            Ok(md) => Ok(md),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("SKILL.md frontmatter is malformed YAML: {}", strict_err),
            )),
        },
    }
}

fn build_skill_md(name: Option<String>, description: Option<String>) -> io::Result<SkillMd> {
    let name = name
        .filter(|n| !n.trim().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "SKILL.md frontmatter missing name",
            )
        })?
        .trim()
        .to_string();

    let description = description
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

/// Lenient fallback: extract the first `name:` and `description:` top-level keys
/// line-by-line. Handles folded blocks (`description: >`) by joining indented
/// continuation lines. Strict YAML remains the primary path; this only rescues
/// frontmatter that strict YAML cannot parse.
fn parse_frontmatter_lenient(front_yaml: &str) -> io::Result<SkillMd> {
    let lines: Vec<&str> = front_yaml.lines().collect();
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with('#') {
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("name:") {
            if name.is_none() {
                let v = rest.trim();
                if !v.is_empty() {
                    name = Some(v.to_string());
                }
            }
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("description:") {
            if description.is_none() {
                let rest = rest.trim();
                if rest.is_empty() || rest.starts_with('>') || rest.starts_with('|') {
                    // folded/literal block: join indented continuation lines
                    let mut parts: Vec<String> = Vec::new();
                    i += 1;
                    while i < lines.len() && (lines[i].starts_with(' ') || lines[i].starts_with('\t')) {
                        let t = lines[i].trim();
                        if !t.is_empty() {
                            parts.push(t.to_string());
                        }
                        i += 1;
                    }
                    if !parts.is_empty() {
                        description = Some(parts.join(" "));
                    }
                    continue;
                }
                description = Some(rest.to_string());
            }
            i += 1;
            continue;
        }
        i += 1;
    }

    build_skill_md(name, description)
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

    /// Regression: real-world frontmatter whose description is a single plain
    /// scalar containing an unquoted `: ` (e.g. "Do NOT use for: 单篇文章...")
    /// fails strict YAML parsing. The lenient fallback must still extract
    /// name/description so such skills are not silently skipped by the scan.
    #[test]
    fn lenient_parse_rescues_unquoted_colon_in_description() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_colon");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: ai-hotspots\ndescription: 跨平台 AI 资讯速报工作流。Do NOT use for: 单篇文章深度技术分析 / AI 概念知识问答。\n---\n# ai-hotspots\n\nBody.\n",
        )
        .unwrap();

        let md = read_skill_md(&dir).unwrap();
        assert_eq!(md.name, "ai-hotspots");
        assert!(md.description.contains("Do NOT use for: 单篇文章"));
        assert!(serde_yaml::from_str::<Frontmatter>(
            &format!(
                "name: {}\ndescription: {}",
                md.name, md.description
            )
        )
        .is_err(),
        "sanity: this description is indeed not strict-parseable");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lenient_parse_handles_folded_description_block() {
        let (tmp, dir) = tmp_skill_dir("skillman_md_folded");
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: Folded Skill\ndescription: >\n  First line of the description\n  Second line, joined with a space.\n  Also has: colon inside.\n---\n# Folded\n\nBody.\n",
        )
        .unwrap();

        let md = read_skill_md(&dir).unwrap();
        assert_eq!(md.name, "Folded Skill");
        assert_eq!(
            md.description,
            "First line of the description Second line, joined with a space. Also has: colon inside."
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lenient_parse_still_rejects_missing_fields() {
        // No description at all -> must stay an error even under the fallback.
        let (tmp, dir) = tmp_skill_dir("skillman_md_lenient_missing");
        std::fs::write(dir.join("SKILL.md"), "---\nname: Only Name\n---\n# x\n\nBody.\n").unwrap();
        assert!(read_skill_md(&dir).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
