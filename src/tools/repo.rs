use crate::error::Result;
use crate::tools::ToolContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusInput {
    #[schemars(description = "Path to the git repository")]
    pub path: Option<String>,

    #[schemars(description = "Include untracked files in the output")]
    pub include_untracked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusOutput {
    pub success: bool,
    pub branch: Option<String>,
    pub ahead: Option<i32>,
    pub behind: Option<i32>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
    pub conflicts: Vec<String>,
}

pub async fn git_status(ctx: ToolContext, input: GitStatusInput) -> Result<GitStatusOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    let mut args = vec!["status", "--porcelain=v2", "--branch"];
    if input.include_untracked.unwrap_or(true) {
        args.push("-u");
    }

    let output = if let Some(ref p) = path {
        executor.execute_in_dir(p, &args)?
    } else {
        executor.execute(&args)?
    };
    let stdout = output.stdout.trim();

    let mut result = GitStatusOutput {
        success: true,
        branch: None,
        ahead: None,
        behind: None,
        staged: Vec::new(),
        unstaged: Vec::new(),
        untracked: Vec::new(),
        conflicts: Vec::new(),
    };

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            result.branch = Some(rest.to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut it = rest.split_whitespace();
            let ahead = it.next().unwrap_or("0");
            let behind = it.next().unwrap_or("0");
            result.ahead = ahead.trim_start_matches('+').parse().ok();
            result.behind = behind.trim_start_matches('-').parse().ok();
            continue;
        }

        if line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("? ") {
            result.untracked.push(rest.to_string());
            continue;
        }

        if line.starts_with("u ") {
            if let Some(path) = line.split_whitespace().last() {
                result.conflicts.push(path.to_string());
            }
            continue;
        }

        if line.starts_with("1 ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let xy = parts[1];
            let x = xy.chars().next().unwrap_or('.');
            let y = xy.chars().nth(1).unwrap_or('.');
            let path = parts.last().copied().unwrap_or("");

            if x != '.' {
                result.staged.push(path.to_string());
            }
            if y != '.' {
                result.unstaged.push(path.to_string());
            }
            continue;
        }

        if line.starts_with("2 ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let xy = parts[1];
            let x = xy.chars().next().unwrap_or('.');
            let y = xy.chars().nth(1).unwrap_or('.');

            let path = parts
                .get(parts.len().saturating_sub(2))
                .copied()
                .unwrap_or("");

            if x != '.' {
                result.staged.push(path.to_string());
            }
            if y != '.' {
                result.unstaged.push(path.to_string());
            }
            continue;
        }
    }

    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitInitInput {
    #[schemars(description = "Path where the repository should be initialized")]
    pub path: String,

    #[schemars(description = "Initial branch name (default: main)")]
    pub initial_branch: Option<String>,

    #[schemars(description = "Create a bare repository")]
    pub bare: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitInitOutput {
    pub success: bool,
    pub path: String,
    pub message: String,
}

pub async fn git_init(ctx: ToolContext, input: GitInitInput) -> Result<GitInitOutput> {
    let executor = ctx.executor.read().await;

    let mut args = vec!["init"];

    if let Some(branch) = &input.initial_branch {
        args.push("--initial-branch");
        args.push(branch);
    }

    if input.bare.unwrap_or(false) {
        args.push("--bare");
    }

    args.push(&input.path);

    executor.execute(&args)?;

    Ok(GitInitOutput {
        success: true,
        path: input.path.clone(),
        message: format!("Initialized empty Git repository in {}", input.path),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCloneInput {
    #[schemars(description = "URL of the repository to clone")]
    pub url: String,

    #[schemars(description = "Local path to clone into")]
    pub local_path: Option<String>,

    #[schemars(description = "Clone only this branch")]
    pub branch: Option<String>,

    #[schemars(description = "Create a shallow clone with this depth")]
    pub depth: Option<i32>,

    #[schemars(description = "Clone as a bare repository")]
    pub bare: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCloneOutput {
    pub success: bool,
    pub path: String,
    pub message: String,
}

pub async fn git_clone(ctx: ToolContext, input: GitCloneInput) -> Result<GitCloneOutput> {
    let executor = ctx.executor.read().await;

    let mut args: Vec<String> = vec!["clone".into()];

    if let Some(branch) = &input.branch {
        args.push("--branch".into());
        args.push(branch.clone());
    }

    if let Some(depth) = input.depth {
        args.push("--depth".into());
        args.push(depth.to_string());
    }

    if input.bare.unwrap_or(false) {
        args.push("--bare".into());
    }

    args.push(input.url.clone());

    if let Some(path) = &input.local_path {
        args.push(path.clone());
    }

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    executor.execute(&args_refs)?;

    let path = input.local_path.clone().unwrap_or_else(|| {
        let url = &input.url;
        let name = url.rsplit('/').next().unwrap_or("repo");
        name.trim_end_matches(".git").to_string()
    });

    Ok(GitCloneOutput {
        success: true,
        path,
        message: format!("Cloned repository from {}", input.url),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCleanInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Force cleaning (required)")]
    pub force: bool,

    #[schemars(description = "Show what would be deleted without actually deleting")]
    pub dry_run: Option<bool>,

    #[schemars(description = "Remove untracked directories in addition to files")]
    pub directories: Option<bool>,

    #[schemars(description = "Remove ignored files as well")]
    pub ignored: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCleanOutput {
    pub success: bool,
    pub cleaned_files: Vec<String>,
    pub message: String,
}

pub async fn git_clean(ctx: ToolContext, input: GitCleanInput) -> Result<GitCleanOutput> {
    if !input.force {
        return Ok(GitCleanOutput {
            success: false,
            cleaned_files: vec![],
            message: "Clean operation requires force=true confirmation".to_string(),
        });
    }

    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    let mut args = vec!["clean"];

    if input.dry_run.unwrap_or(false) {
        args.push("--dry-run");
    } else {
        args.push("-f");
    }

    if input.directories.unwrap_or(false) {
        args.push("-d");
    }

    if input.ignored.unwrap_or(false) {
        args.push("-X");
    }

    let output = if let Some(ref p) = path {
        executor.execute_in_dir(p, &args)?
    } else {
        executor.execute(&args)?
    };

    let cleaned_files: Vec<String> = output
        .stdout
        .lines()
        .filter_map(|l| {
            l.strip_prefix("Would remove ")
                .or_else(|| l.strip_prefix("Removing "))
                .map(|rest| rest.trim().to_string())
        })
        .collect();

    Ok(GitCleanOutput {
        success: true,
        cleaned_files,
        message: if input.dry_run.unwrap_or(false) {
            "Dry run completed".to_string()
        } else {
            "Files cleaned successfully".to_string()
        },
    })
}
