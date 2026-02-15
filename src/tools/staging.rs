use crate::error::Result;
use crate::tools::ToolContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitAddInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Files to add")]
    pub files: Vec<String>,

    #[schemars(description = "Update tracked files only")]
    pub update: Option<bool>,

    #[schemars(description = "Add all files")]
    pub all: Option<bool>,

    #[schemars(description = "Force adding ignored files")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitAddOutput {
    pub success: bool,
    pub files_added: Vec<String>,
    pub message: String,
}

pub async fn git_add(ctx: ToolContext, input: GitAddInput) -> Result<GitAddOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    let mut args = vec!["add"];

    if input.update.unwrap_or(false) {
        args.push("--update");
    }

    if input.all.unwrap_or(false) {
        args.push("--all");
    }

    if input.force.unwrap_or(false) {
        args.push("--force");
    }

    for file in &input.files {
        args.push(file);
    }

    if let Some(ref p) = path {
        executor.execute_in_dir(p, &args)?;
    } else {
        executor.execute(&args)?;
    }

    Ok(GitAddOutput {
        success: true,
        files_added: input.files.clone(),
        message: format!("Added {} file(s) to staging", input.files.len()),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCommitInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Commit message")]
    pub message: String,

    #[schemars(description = "Author override (name <email>)")]
    pub author: Option<String>,

    #[schemars(description = "Amend previous commit")]
    pub amend: Option<bool>,

    #[schemars(description = "Allow empty commit")]
    pub allow_empty: Option<bool>,

    #[schemars(description = "Files to stage before commit")]
    pub files_to_stage: Option<Vec<String>>,

    #[schemars(description = "Skip pre-commit hooks")]
    pub no_verify: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCommitOutput {
    pub success: bool,
    pub commit_hash: Option<String>,
    pub branch: Option<String>,
    pub message: String,
}

pub async fn git_commit(ctx: ToolContext, input: GitCommitInput) -> Result<GitCommitOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    // Stage files if provided
    if let Some(files) = &input.files_to_stage {
        let mut add_args = vec!["add"];
        for file in files {
            add_args.push(file);
        }
        if let Some(ref p) = path {
            executor.execute_in_dir(p, &add_args)?;
        } else {
            executor.execute(&add_args)?;
        }
    }

    let mut args = vec!["commit", "-m", &input.message];

    if let Some(author) = &input.author {
        args.push("--author");
        args.push(author);
    }

    if input.amend.unwrap_or(false) {
        args.push("--amend");
    }

    if input.allow_empty.unwrap_or(false) {
        args.push("--allow-empty");
    }

    if input.no_verify.unwrap_or(false) {
        args.push("--no-verify");
    }

    let output = if let Some(ref p) = path {
        executor.execute_in_dir(p, &args)?
    } else {
        executor.execute(&args)?
    };

    // Extract commit hash from output
    let commit_hash = output
        .stdout
        .lines()
        .find(|l| l.contains('[') && l.contains(']'))
        .and_then(|l| {
            let start = l.find('[')?;
            let end = l.find(']')?;
            Some(l[start + 1..end].split_whitespace().last()?.to_string())
        });

    // Get current branch
    let branch_output = if let Some(ref p) = path {
        executor.execute_in_dir(p, &["branch", "--show-current"])?
    } else {
        executor.execute(&["branch", "--show-current"])?
    };
    let branch = branch_output.stdout.trim().to_string();
    let branch = if branch.is_empty() {
        None
    } else {
        Some(branch)
    };

    Ok(GitCommitOutput {
        success: true,
        commit_hash,
        branch,
        message: format!("Created commit: {}", input.message),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitDiffInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,

    #[schemars(description = "Target commit/branch to compare")]
    pub target: Option<String>,

    #[schemars(description = "Source commit/branch to compare from")]
    pub source: Option<String>,

    #[schemars(description = "Specific file paths")]
    pub paths: Option<Vec<String>>,

    #[schemars(description = "Show staged changes")]
    pub staged: Option<bool>,

    #[schemars(description = "Include untracked files")]
    pub include_untracked: Option<bool>,

    #[schemars(description = "Show only file names")]
    pub name_only: Option<bool>,

    #[schemars(description = "Show diffstat")]
    pub stat: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitDiffOutput {
    pub success: bool,
    pub diff: String,
    pub files_changed: Option<i32>,
    pub insertions: Option<i32>,
    pub deletions: Option<i32>,
}

pub async fn git_diff(ctx: ToolContext, input: GitDiffInput) -> Result<GitDiffOutput> {
    let executor = ctx.executor.read().await;

    let path = input.path.as_ref().map(PathBuf::from);

    let mut args = vec!["diff"];

    if input.staged.unwrap_or(false) {
        args.push("--staged");
    }

    if input.name_only.unwrap_or(false) {
        args.push("--name-only");
    }

    if input.stat.unwrap_or(false) {
        args.push("--stat");
    }

    if let Some(source) = &input.source {
        args.push(source);
        if let Some(target) = &input.target {
            args.push(target);
        }
    } else if let Some(target) = &input.target {
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

    // Parse stat if available
    let (files_changed, insertions, deletions) = if input.stat.unwrap_or(false) {
        let last_line = output.stdout.lines().last().unwrap_or("");
        // Parse "X files changed, Y insertions(+), Z deletions(-)"
        let parts: Vec<&str> = last_line.split(',').collect();
        let fc = parts
            .get(0)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse().ok());
        let ins = parts
            .get(1)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse().ok());
        let del = parts
            .get(2)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse().ok());
        (fc, ins, del)
    } else {
        (None, None, None)
    };

    Ok(GitDiffOutput {
        success: true,
        diff: output.stdout,
        files_changed,
        insertions,
        deletions,
    })
}
