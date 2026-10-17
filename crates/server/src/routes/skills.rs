use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use axum::{Json, Router, response::Json as ResponseJson, routing::get};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::DeploymentImpl;

const SKILLS_DIR: &str = ".agents/skills";
const MAX_SKILLS: usize = 128;
const MAX_DESCRIPTION_BYTES: usize = 1024;
const MAX_INSTRUCTIONS_BYTES: usize = 1024 * 1024;

static PUBLISH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn publish_lock() -> &'static Mutex<()> {
    PUBLISH_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSkill {
    name: String,
    description: String,
    instructions: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsCatalog {
    available: bool,
    repository_path: Option<String>,
    base_branch: String,
    skills: Vec<ManagedSkill>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillProposal {
    title: String,
    #[serde(default)]
    body: String,
    changes: Vec<SkillChange>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
enum SkillChange {
    Upsert {
        name: String,
        description: String,
        instructions: String,
        #[serde(default)]
        previous: Option<ManagedSkill>,
    },
    Delete {
        name: String,
        previous: ManagedSkill,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillProposalResult {
    pull_request_url: String,
    branch: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/skills", get(list_skills))
        .route("/skills/proposals", axum::routing::post(create_proposal))
}

fn configured_repo() -> Option<PathBuf> {
    std::env::var_os("VIBE_KANBAN_SKILLS_REPO").map(PathBuf::from)
}

fn base_branch() -> String {
    std::env::var("VIBE_KANBAN_SKILLS_BASE_BRANCH").unwrap_or_else(|_| "main".into())
}

fn valid_name(name: &str) -> bool {
    const YAML_TYPED_NAMES: [&str; 7] = ["null", "true", "false", "yes", "no", "on", "off"];
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name != "synced"
        && !YAML_TYPED_NAMES.contains(&name)
}

fn validate_skill(name: &str, description: &str, instructions: &str) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!(
            "Skill name '{name}' must use lowercase letters, digits, and single hyphens"
        ));
    }
    if description.trim().is_empty() || description.len() > MAX_DESCRIPTION_BYTES {
        return Err(format!(
            "Skill '{name}' needs a description of at most {MAX_DESCRIPTION_BYTES} bytes"
        ));
    }
    if description.contains('\n') || description.contains('\r') {
        return Err(format!("Skill '{name}' description must be one line"));
    }
    let normalized_description = description.trim().to_ascii_lowercase();
    if !description
        .trim()
        .starts_with(|character: char| character.is_ascii_alphabetic())
        || description.contains([':', '#'])
        || matches!(
            normalized_description.as_str(),
            "null" | "true" | "false" | "yes" | "no" | "on" | "off"
        )
    {
        return Err(format!("Skill '{name}' description must be a plain scalar"));
    }
    if instructions.trim().is_empty() || instructions.len() > MAX_INSTRUCTIONS_BYTES {
        return Err(format!(
            "Skill '{name}' needs instructions of at most {MAX_INSTRUCTIONS_BYTES} bytes"
        ));
    }
    Ok(())
}

fn parse_skill(path: &Path) -> Result<ManagedSkill, String> {
    let metadata = path
        .symlink_metadata()
        .map_err(|error| format!("Cannot inspect {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{} must be a regular file", path.display()));
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return Err(format!("{} has invalid frontmatter", path.display()));
    }
    let mut name = None;
    let mut description = None;
    for line in &mut lines {
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix("name: ") {
            name = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("description: ") {
            description = Some(value.to_string());
        }
    }
    let name = name.ok_or_else(|| format!("{} is missing name", path.display()))?;
    let description =
        description.ok_or_else(|| format!("{} is missing description", path.display()))?;
    let instructions = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    validate_skill(&name, &description, &instructions)?;
    Ok(ManagedSkill {
        name,
        description,
        instructions,
    })
}

fn safe_skills_root(repo: &Path) -> Result<Option<PathBuf>, String> {
    let agents = repo.join(".agents");
    let root = agents.join("skills");
    for path in [&agents, &root] {
        match path.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            }
            Ok(_) => {
                return Err(format!(
                    "Managed skills path must contain only real directories: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Cannot inspect {}: {error}", path.display())),
        }
    }
    Ok(Some(root))
}

fn load_catalog(repo: &Path) -> Result<Vec<ManagedSkill>, String> {
    let Some(root) = safe_skills_root(repo)? else {
        return Ok(Vec::new());
    };
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) => return Err(format!("Cannot read {}: {error}", root.display())),
    };
    let mut skills = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if !file_type.is_dir() || file_type.is_symlink() {
            return Err(format!(
                "Managed skill source contains an unsupported entry: {}",
                entry.path().display()
            ));
        }
        skills.push(parse_skill(&entry.path().join("SKILL.md"))?);
        if skills
            .last()
            .is_some_and(|skill| skill.name != entry.file_name().to_string_lossy())
        {
            return Err(format!(
                "Skill name must match directory {}",
                entry.path().display()
            ));
        }
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    if skills.len() > MAX_SKILLS {
        return Err(format!("Managed catalog exceeds {MAX_SKILLS} skills"));
    }
    Ok(skills)
}

fn load_base_catalog(repo: &Path) -> Result<Vec<ManagedSkill>, String> {
    let id = Uuid::new_v4().simple().to_string();
    let checkout = std::env::temp_dir().join(format!("vibe-kanban-skills-read-{id}"));
    let base = base_branch();
    let remote = run(repo, "git", &["remote", "get-url", "origin"])?;
    let result = run(
        repo,
        "git",
        &[
            "clone",
            "--single-branch",
            "--no-tags",
            "--branch",
            &base,
            &remote,
            checkout.to_str().ok_or("Invalid temporary checkout path")?,
        ],
    )
    .and_then(|_| load_catalog(&checkout));
    let _ = fs::remove_dir_all(checkout);
    result
}

async fn list_skills() -> ResponseJson<ApiResponse<SkillsCatalog>> {
    let branch = base_branch();
    let Some(repo) = configured_repo() else {
        return ResponseJson(ApiResponse::success(SkillsCatalog {
            available: false,
            repository_path: None,
            base_branch: branch,
            skills: Vec::new(),
        }));
    };
    let repository_path = repo.display().to_string();
    match tokio::task::spawn_blocking(move || load_base_catalog(&repo)).await {
        Ok(Ok(skills)) => ResponseJson(ApiResponse::success(SkillsCatalog {
            available: true,
            repository_path: Some(repository_path),
            base_branch: branch,
            skills,
        })),
        Ok(Err(error)) => ResponseJson(ApiResponse::error(&error)),
        Err(error) => ResponseJson(ApiResponse::error(&format!(
            "Skill catalog task failed: {error}"
        ))),
    }
}

fn run(repo: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|error| format!("Could not run {program}: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{program} failed: {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write_skill(
    root: &Path,
    name: &str,
    description: &str,
    instructions: &str,
) -> Result<(), String> {
    validate_skill(name, description, instructions)?;
    let directory = root.join(name);
    if directory
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!("Refusing to replace symlinked skill '{name}'"));
    }
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let content = format!(
        "---\nname: {name}\ndescription: {}\n---\n\n{}\n",
        description.trim(),
        instructions.trim()
    );
    fs::write(directory.join("SKILL.md"), content).map_err(|error| error.to_string())
}

fn publish(repo: PathBuf, proposal: SkillProposal) -> Result<SkillProposalResult, String> {
    if proposal.title.trim().is_empty() || proposal.title.len() > 200 {
        return Err("A proposal title of at most 200 bytes is required".into());
    }
    if proposal.changes.is_empty() || proposal.changes.len() > MAX_SKILLS {
        return Err("A proposal must contain between 1 and 128 changes".into());
    }
    let mut changed_names = HashSet::new();
    for change in &proposal.changes {
        let changed_name = match change {
            SkillChange::Upsert { name, .. } | SkillChange::Delete { name, .. } => name,
        };
        if !changed_names.insert(changed_name) {
            return Err(format!("Skill '{changed_name}' is changed more than once"));
        }
        match change {
            SkillChange::Upsert {
                name,
                description,
                instructions,
                ..
            } => {
                validate_skill(name, description, instructions)?;
            }
            SkillChange::Delete { name, .. } if !valid_name(name) => {
                return Err(format!("Invalid skill name '{name}'"));
            }
            SkillChange::Delete { .. } => {}
        }
    }

    let id = Uuid::new_v4().simple().to_string();
    let branch = format!("skills/{id}");
    let worktree = std::env::temp_dir().join(format!("vibe-kanban-skills-{id}"));
    let base = base_branch();
    let remote = run(&repo, "git", &["remote", "get-url", "origin"])?;
    let prepare_result = run(
        &repo,
        "git",
        &[
            "clone",
            "--single-branch",
            "--no-tags",
            "--branch",
            &base,
            &remote,
            worktree.to_str().ok_or("Invalid temporary worktree path")?,
        ],
    )
    .and_then(|_| run(&worktree, "git", &["checkout", "-b", &branch]));
    if let Err(error) = prepare_result {
        let _ = fs::remove_dir_all(&worktree);
        return Err(error);
    }

    let result = (|| {
        let root = worktree.join(SKILLS_DIR);
        let current = load_catalog(&worktree)?;
        for change in &proposal.changes {
            let (name, previous) = match change {
                SkillChange::Upsert { name, previous, .. } => (name, previous.as_ref()),
                SkillChange::Delete { name, previous } => (name, Some(previous)),
            };
            let actual = current.iter().find(|skill| &skill.name == name);
            if actual != previous {
                return Err(format!(
                    "Skill '{name}' changed on {base}; refresh Settings before proposing"
                ));
            }
        }
        for change in &proposal.changes {
            match change {
                SkillChange::Upsert {
                    name,
                    description,
                    instructions,
                    ..
                } => {
                    write_skill(&root, name, description, instructions)?;
                }
                SkillChange::Delete { name, .. } => {
                    let directory = root.join(name);
                    if !directory.is_dir() || directory.is_symlink() {
                        return Err(format!("Skill '{name}' does not exist"));
                    }
                    fs::remove_dir_all(directory).map_err(|error| error.to_string())?;
                }
            }
        }
        load_catalog(&worktree)?;
        run(&worktree, "git", &["add", "--", SKILLS_DIR])?;
        if Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&worktree)
            .status()
            .map_err(|error| error.to_string())?
            .success()
        {
            return Err("The proposal does not change the managed catalog".into());
        }
        run(&worktree, "git", &["config", "user.name", "Vibe Kanban"])?;
        run(
            &worktree,
            "git",
            &["config", "user.email", "vibe-kanban@localhost"],
        )?;
        run(&worktree, "git", &["commit", "-m", proposal.title.trim()])?;
        run(&worktree, "git", &["push", "-u", "origin", &branch])?;
        let pr_url = run(
            &worktree,
            "gh",
            &[
                "pr",
                "create",
                "--base",
                &base,
                "--head",
                &branch,
                "--title",
                proposal.title.trim(),
                "--body",
                proposal.body.trim(),
            ],
        )?;
        Ok(SkillProposalResult {
            pull_request_url: pr_url,
            branch: branch.clone(),
        })
    })();

    if result.is_err() {
        let _ = run(&worktree, "git", &["push", "origin", "--delete", &branch]);
    }
    let _ = fs::remove_dir_all(&worktree);
    result
}

async fn create_proposal(
    Json(proposal): Json<SkillProposal>,
) -> ResponseJson<ApiResponse<SkillProposalResult>> {
    let Some(repo) = configured_repo() else {
        return ResponseJson(ApiResponse::error(
            "Skill management is not configured on this machine",
        ));
    };
    let _guard = publish_lock().lock().await;
    match tokio::task::spawn_blocking(move || publish(repo, proposal)).await {
        Ok(Ok(result)) => ResponseJson(ApiResponse::success(result)),
        Ok(Err(error)) => ResponseJson(ApiResponse::error(&error)),
        Err(error) => ResponseJson(ApiResponse::error(&format!(
            "Skill proposal task failed: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn validates_portable_skill_fields() {
        assert!(validate_skill("browser-tools", "Use for browser work", "# Instructions").is_ok());
        assert!(validate_skill("Bad_Name", "description", "instructions").is_err());
        assert!(validate_skill("synced", "description", "instructions").is_err());
        assert!(validate_skill("true", "description", "instructions").is_err());
        assert!(validate_skill("valid", "line one\nline two", "instructions").is_err());
        assert!(validate_skill("valid", "# browser work", "instructions").is_err());
        assert!(validate_skill("valid", "Use this skill:", "instructions").is_err());
        assert!(validate_skill("valid", "true", "instructions").is_err());
        assert!(validate_skill("valid", "description", " ").is_err());
    }

    #[test]
    fn writes_and_loads_a_deterministic_catalog() {
        let temporary = tempdir().unwrap();
        assert!(load_catalog(temporary.path()).unwrap().is_empty());
        let root = temporary.path().join(SKILLS_DIR);
        write_skill(
            &root,
            "zeta",
            "Use for the last task",
            "# Instructions\n\nDo the last task.",
        )
        .unwrap();
        write_skill(
            &root,
            "alpha",
            "Use for the first task",
            "# Instructions\n\nDo the first task.",
        )
        .unwrap();

        let catalog = load_catalog(temporary.path()).unwrap();
        assert_eq!(
            catalog
                .iter()
                .map(|skill| skill.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
        assert_eq!(
            catalog[0].instructions,
            "# Instructions\n\nDo the first task."
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_catalog_root() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::create_dir(temporary.path().join(".agents")).unwrap();
        symlink(outside.path(), temporary.path().join(SKILLS_DIR)).unwrap();
        assert!(
            load_catalog(temporary.path())
                .unwrap_err()
                .contains("real directories")
        );

        let second = tempdir().unwrap();
        symlink(outside.path(), second.path().join(".agents")).unwrap();
        assert!(
            load_catalog(second.path())
                .unwrap_err()
                .contains("real directories")
        );
    }
}
