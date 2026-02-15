use crate::error::Result;
use crate::tools::repo;
use crate::tools::ToolContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitTagInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Operation: list, create, delete")]
    pub mode: Option<String>,

    #[schemars(description = "Tag name")]
    pub tag_name: Option<String>,

    #[schemars(description = "Commit to tag")]
    pub commit: Option<String>,

    #[schemars(description = "Tag message")]
    pub message: Option<String>,

    #[schemars(description = "Create annotated tag")]
    pub annotated: Option<bool>,

    #[schemars(description = "Force tag creation")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitTagInfo {
    pub name: String,
    pub commit_hash: String,
    pub message: Option<String>,
    pub tagger: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitTagOutput {
    pub success: bool,
    pub tags: Vec<GitTagInfo>,
    pub message: String,
}

pub async fn git_tag(ctx: ToolContext, input: GitTagInput) -> Result<GitTagOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    match input.mode.as_deref() {
        Some("create") => {
            let mut args = vec!["tag"];

            if input.annotated.unwrap_or(false) || input.message.is_some() {
                args.push("-a");
            }

            if let Some(msg) = &input.message {
                args.push("-m");
                args.push(msg);
            }

            if input.force.unwrap_or(false) {
                args.push("-f");
            }

            if let Some(name) = &input.tag_name {
                args.push(name);
            }

            if let Some(commit) = &input.commit {
                args.push(commit);
            }

            if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?;
            } else {
                executor.execute(&args)?;
            }

            Ok(GitTagOutput {
                success: true,
                tags: vec![],
                message: format!("Created tag: {}", input.tag_name.unwrap_or_default()),
            })
        }
        Some("delete") => {
            let mut args = vec!["tag", "-d"];
            if let Some(name) = &input.tag_name {
                args.push(name);
            }
            if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?;
            } else {
                executor.execute(&args)?;
            }

            Ok(GitTagOutput {
                success: true,
                tags: vec![],
                message: format!("Deleted tag: {}", input.tag_name.unwrap_or_default()),
            })
        }
        _ => {
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(
                    p,
                    &[
                        "tag",
                        "-l",
                        "--format=%(refname:short)|%(objectname:short)|%(subject)|%(taggername)",
                    ],
                )?
            } else {
                executor.execute(&[
                    "tag",
                    "-l",
                    "--format=%(refname:short)|%(objectname:short)|%(subject)|%(taggername)",
                ])?
            };

            let tags: Vec<GitTagInfo> = output
                .stdout
                .lines()
                .filter(|l| !l.is_empty())
                .filter_map(|line| {
                    let parts: Vec<&str> = line.splitn(4, '|').collect();
                    if parts.len() >= 2 {
                        Some(GitTagInfo {
                            name: parts[0].to_string(),
                            commit_hash: parts[1].to_string(),
                            message: parts
                                .get(2)
                                .map(|s| s.to_string())
                                .filter(|s| !s.is_empty()),
                            tagger: parts
                                .get(3)
                                .map(|s| s.to_string())
                                .filter(|s| !s.is_empty()),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            Ok(GitTagOutput {
                success: true,
                tags,
                message: String::new(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStashInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Operation: push, pop, apply, list, drop, clear")]
    pub mode: Option<String>,

    #[schemars(description = "Stash message")]
    pub message: Option<String>,

    #[schemars(description = "Stash reference (e.g., stash@{0})")]
    pub stash_ref: Option<String>,

    #[schemars(description = "Include untracked files")]
    pub include_untracked: Option<bool>,

    #[schemars(description = "Keep staged changes")]
    pub keep_index: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStashEntry {
    pub stash_ref: String,
    pub branch: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStashOutput {
    pub success: bool,
    pub stashes: Vec<GitStashEntry>,
    pub message: String,
}

pub async fn git_stash(ctx: ToolContext, input: GitStashInput) -> Result<GitStashOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    match input.mode.as_deref() {
        Some("push") | None => {
            let mut args = vec!["stash", "push"];

            if let Some(msg) = &input.message {
                args.push("-m");
                args.push(msg);
            }

            if input.include_untracked.unwrap_or(false) {
                args.push("-u");
            }

            if input.keep_index.unwrap_or(false) {
                args.push("--keep-index");
            }

            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?
            } else {
                executor.execute(&args)?
            };

            Ok(GitStashOutput {
                success: true,
                stashes: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("pop") => {
            let mut args = vec!["stash", "pop"];
            if let Some(r) = &input.stash_ref {
                args.push(r);
            }
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?
            } else {
                executor.execute(&args)?
            };

            Ok(GitStashOutput {
                success: true,
                stashes: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("apply") => {
            let mut args = vec!["stash", "apply"];
            if let Some(r) = &input.stash_ref {
                args.push(r);
            }
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?
            } else {
                executor.execute(&args)?
            };

            Ok(GitStashOutput {
                success: true,
                stashes: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("drop") => {
            let mut args = vec!["stash", "drop"];
            if let Some(r) = &input.stash_ref {
                args.push(r);
            }
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?
            } else {
                executor.execute(&args)?
            };

            Ok(GitStashOutput {
                success: true,
                stashes: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("clear") => {
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &["stash", "clear"])?
            } else {
                executor.execute(&["stash", "clear"])?
            };

            Ok(GitStashOutput {
                success: true,
                stashes: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("list") => {
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &["stash", "list", "--format=%gd|%gD|%s"])?
            } else {
                executor.execute(&["stash", "list", "--format=%gd|%gD|%s"])?
            };

            let stashes: Vec<GitStashEntry> = output
                .stdout
                .lines()
                .filter(|l| !l.is_empty())
                .filter_map(|line| {
                    let parts: Vec<&str> = line.splitn(3, '|').collect();
                    if parts.len() >= 3 {
                        Some(GitStashEntry {
                            stash_ref: parts[0].to_string(),
                            branch: parts[1].to_string(),
                            message: parts[2].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            Ok(GitStashOutput {
                success: true,
                stashes,
                message: String::new(),
            })
        }
        _ => {
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &["stash", "list"])?
            } else {
                executor.execute(&["stash", "list"])?
            };

            Ok(GitStashOutput {
                success: true,
                stashes: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitResetInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Reset mode: soft, mixed, hard, merge, keep")]
    pub mode: Option<String>,

    #[schemars(description = "Target commit/branch")]
    pub target: Option<String>,

    #[schemars(description = "Specific file paths")]
    pub paths: Option<Vec<String>>,

    #[schemars(description = "Confirmation for hard reset")]
    pub confirmed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitResetOutput {
    pub success: bool,
    pub previous_head: Option<String>,
    pub new_head: Option<String>,
    pub message: String,
}

pub async fn git_reset(ctx: ToolContext, input: GitResetInput) -> Result<GitResetOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    let mode = input.mode.as_deref().unwrap_or("mixed");

    if mode == "hard" && !input.confirmed.unwrap_or(false) {
        return Ok(GitResetOutput {
            success: false,
            previous_head: None,
            new_head: None,
            message: "Hard reset requires confirmation (confirmed=true)".to_string(),
        });
    }

    let mut args = vec!["reset"];

    match mode {
        "soft" => args.push("--soft"),
        "hard" => args.push("--hard"),
        "merge" => args.push("--merge"),
        "keep" => args.push("--keep"),
        _ => {} // mixed is default
    }

    if let Some(target) = &input.target {
        args.push(target);
    }

    if let Some(paths) = &input.paths {
        args.push("--");
        for path in paths {
            args.push(path);
        }
    }

    let output = if let Some(ref p) = path {
        executor.execute_in_dir(p, &args)?
    } else {
        executor.execute(&args)?
    };

    Ok(GitResetOutput {
        success: true,
        previous_head: None,
        new_head: input.target.clone(),
        message: output.stdout.trim().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitWorktreeInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Operation: list, add, remove, move, prune")]
    pub mode: Option<String>,

    #[schemars(description = "Worktree path")]
    pub worktree_path: Option<String>,

    #[schemars(description = "Branch for new worktree")]
    pub branch: Option<String>,

    #[schemars(description = "Force operation")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitWorktreeInfo {
    pub worktree_path: String,
    pub head_hash: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitWorktreeOutput {
    pub success: bool,
    pub worktrees: Vec<GitWorktreeInfo>,
    pub message: String,
}

pub async fn git_worktree(ctx: ToolContext, input: GitWorktreeInput) -> Result<GitWorktreeOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    match input.mode.as_deref() {
        Some("add") => {
            let mut args = vec!["worktree", "add"];

            if input.force.unwrap_or(false) {
                args.push("-f");
            }

            if let Some(path) = &input.worktree_path {
                args.push(path);
            }

            if let Some(branch) = &input.branch {
                args.push("-b");
                args.push(branch);
            }

            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?
            } else {
                executor.execute(&args)?
            };

            Ok(GitWorktreeOutput {
                success: true,
                worktrees: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("remove") => {
            let mut args = vec!["worktree", "remove"];

            if input.force.unwrap_or(false) {
                args.push("-f");
            }

            if let Some(path) = &input.worktree_path {
                args.push(path);
            }

            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &args)?
            } else {
                executor.execute(&args)?
            };

            Ok(GitWorktreeOutput {
                success: true,
                worktrees: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        Some("prune") => {
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &["worktree", "prune"])?
            } else {
                executor.execute(&["worktree", "prune"])?
            };

            Ok(GitWorktreeOutput {
                success: true,
                worktrees: vec![],
                message: output.stdout.trim().to_string(),
            })
        }
        _ => {
            let output = if let Some(ref p) = path {
                executor.execute_in_dir(p, &["worktree", "list", "--porcelain"])?
            } else {
                executor.execute(&["worktree", "list", "--porcelain"])?
            };

            let mut worktrees = Vec::new();
            let mut current: Option<GitWorktreeInfo> = None;

            for line in output.stdout.lines() {
                if let Some(rest) = line.strip_prefix("worktree ") {
                    if let Some(curr) = current.take() {
                        worktrees.push(curr);
                    }
                    current = Some(GitWorktreeInfo {
                        worktree_path: rest.to_string(),
                        head_hash: String::new(),
                        branch: None,
                    });
                } else if let Some(ref mut curr) = current {
                    if let Some(rest) = line.strip_prefix("HEAD ") {
                        curr.head_hash = rest.to_string();
                    } else if let Some(rest) = line.strip_prefix("branch ") {
                        curr.branch = Some(rest.to_string());
                    }
                }
            }

            if let Some(curr) = current {
                worktrees.push(curr);
            }

            Ok(GitWorktreeOutput {
                success: true,
                worktrees,
                message: String::new(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitSetWorkingDirInput {
    #[schemars(description = "Path to set as working directory")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitSetWorkingDirOutput {
    pub success: bool,
    pub path: String,
    pub is_git_repo: bool,
    pub message: String,
}

pub async fn git_set_working_dir(
    ctx: ToolContext,
    input: GitSetWorkingDirInput,
) -> Result<GitSetWorkingDirOutput> {
    let mut executor = ctx.executor.write().await;

    let path = std::path::PathBuf::from(&input.path);

    if !path.exists() {
        return Ok(GitSetWorkingDirOutput {
            success: false,
            path: input.path.clone(),
            is_git_repo: false,
            message: format!("Path does not exist: {}", input.path),
        });
    }

    let is_git_repo = path.join(".git").exists();

    if is_git_repo {
        executor.set_working_dir(path)?;
    }

    Ok(GitSetWorkingDirOutput {
        success: true,
        path: input.path.clone(),
        is_git_repo,
        message: if is_git_repo {
            format!("Working directory set to: {}", input.path)
        } else {
            format!("Path set but not a git repository: {}", input.path)
        },
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitClearWorkingDirInput {
    #[schemars(description = "Confirmation")]
    pub confirm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitClearWorkingDirOutput {
    pub success: bool,
    pub message: String,
}

pub async fn git_clear_working_dir(
    ctx: ToolContext,
    input: GitClearWorkingDirInput,
) -> Result<GitClearWorkingDirOutput> {
    let confirm = input.confirm.to_lowercase();
    if confirm != "y" && confirm != "yes" {
        return Ok(GitClearWorkingDirOutput {
            success: false,
            message: "Confirmation required (confirm='Y' or 'Yes')".to_string(),
        });
    }

    let mut executor = ctx.executor.write().await;
    executor.clear_working_dir();

    Ok(GitClearWorkingDirOutput {
        success: true,
        message: "Working directory cleared".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitWrapupInstructionsInput {
    #[schemars(description = "Acknowledgement to initiate wrap-up workflow (Y/Yes)")]
    pub acknowledgement: String,

    #[schemars(description = "Include instruction to update agent meta files (Y/Yes)")]
    #[serde(rename = "updateAgentMetaFiles")]
    pub update_agent_meta_files: Option<String>,

    #[schemars(description = "If true, instruct to create an annotated tag")]
    #[serde(rename = "createTag")]
    pub create_tag: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitWrapupGitStatus {
    pub branch: String,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitWrapupInstructionsOutput {
    pub instructions: String,
    #[serde(rename = "gitStatus")]
    pub git_status: Option<GitWrapupGitStatus>,
    #[serde(rename = "gitStatusError")]
    pub git_status_error: Option<String>,
}

pub async fn git_wrapup_instructions(
    ctx: ToolContext,
    input: GitWrapupInstructionsInput,
) -> Result<GitWrapupInstructionsOutput> {
    let ack = input.acknowledgement.to_lowercase();
    if ack != "y" && ack != "yes" {
        return Ok(GitWrapupInstructionsOutput {
            instructions: "Acknowledgement required (acknowledgement='Y' or 'Yes')".to_string(),
            git_status: None,
            git_status_error: None,
        });
    }

    let mut instructions = default_wrapup_instructions();

    if let Some(path) = &ctx.config.git_wrapup_instructions_path {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if !contents.trim().is_empty() {
                instructions = contents;
            }
        }
    }

    if input
        .update_agent_meta_files
        .as_deref()
        .map(|s| {
            let s = s.to_lowercase();
            s == "y" || s == "yes"
        })
        .unwrap_or(false)
    {
        instructions.push_str("\n\nAgent meta files:\n- Update any agent-specific meta files required by your workflow.\n");
    }

    if input.create_tag.unwrap_or(false) {
        instructions.push_str("\n\nTagging:\n- After all commits are created, create an annotated tag for the release.\n");
    }

    let (git_status, git_status_error) = match repo::git_status(
        ctx.clone(),
        repo::GitStatusInput {
            path: None,
            include_untracked: Some(true),
        },
    )
    .await
    {
        Ok(st) => (
            Some(GitWrapupGitStatus {
                branch: st.branch.unwrap_or_else(|| "".to_string()),
                staged: st.staged,
                unstaged: st.unstaged,
                untracked: st.untracked,
            }),
            None,
        ),
        Err(e) => (None, Some(e.to_string())),
    };

    Ok(GitWrapupInstructionsOutput {
        instructions,
        git_status,
        git_status_error,
    })
}

fn default_wrapup_instructions() -> String {
    let mut s = String::new();
    s.push_str("Wrap-up workflow:\n");
    s.push_str("1) Set working directory with git_set_working_dir if needed.\n");
    s.push_str("2) Review changes using git_diff (include untracked if needed).\n");
    s.push_str("3) Update CHANGELOG.md for the release notes.\n");
    s.push_str("4) Review/update README.md and documentation for significant changes.\n");
    s.push_str("5) Create atomic commits with clear messages.\n");
    s
}
