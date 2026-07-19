use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkill {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub content_hash: Option<String>,
    pub installed_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrigin {
    pub skill_id: String,
    pub origin_path: String,
    pub found_in: String,
    pub imported_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLink {
    pub skill_id: String,
    pub scope: String,
    pub project_id: Option<String>,
    pub agent_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub global_subpath: String,
    pub project_subpath: String,
    pub installed: bool,
    pub source_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnmanagedSkill {
    pub directory: String,
    pub name: String,
    pub description: Option<String>,
    pub origins: Vec<UnmanagedOrigin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnmanagedOrigin {
    pub path: String,
    pub found_in: String,
}

/// Aggregated view returned to frontend: skill + its links + its origins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    #[serde(flatten)]
    pub skill: InstalledSkill,
    pub links: Vec<SkillLink>,
    pub origins: Vec<SkillOrigin>,
    pub any_enabled: bool,
}
