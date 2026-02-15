use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::tools::ToolContext;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitBranchInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Operation: list, create, delete, rename, show-current")]
    pub operation: Option<String>,
    
    #[schemars(description = "Branch name")]
    pub name: Option<String>,
    
    #[schemars(description = "New branch name (for rename)")]
    pub new_name: Option<String>,
    
    #[schemars(description = "Start point for new branch")]
    pub start_point: Option<String>,
    
    #[schemars(description = "Force operation")]
    pub force: Option<bool>,
    
    #[schemars(description = "List all branches (remote and local)")]
    pub all: Option<bool>,
    
    #[schemars(description = "List remote branches")]
    pub remote: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitBranch {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitBranchOutput {
    pub success: bool,
    pub branches: Vec<GitBranch>,
    pub current_branch: Option<String>,
    pub message: String,
}

pub async fn git_branch(ctx: ToolContext, input: GitBranchInput) -> Result<GitBranchOutput> {
    let executor = ctx.executor.read().await;
    
    match input.operation.as_deref() {
        Some("create") => {
            let mut args = vec!["branch"];
            if input.force.unwrap_or(false) {
                args.push("-f");
            }
            if let Some(name) = &input.name {
                args.push(name);
            }
            if let Some(start) = &input.start_point {
                args.push(start);
            }
            executor.execute(&args)?;
            Ok(GitBranchOutput {
                success: true,
                branches: vec![],
                current_branch: None,
                message: format!("Created branch: {}", input.name.unwrap_or_default()),
            })
        }
        Some("delete") => {
            let mut args = vec!["branch", "-d"];
            if input.force.unwrap_or(false) {
                args = vec!["branch", "-D"];
            }
            if let Some(name) = &input.name {
                args.push(name);
            }
            executor.execute(&args)?;
            Ok(GitBranchOutput {
                success: true,
                branches: vec![],
                current_branch: None,
                message: format!("Deleted branch: {}", input.name.unwrap_or_default()),
            })
        }
        Some("rename") => {
            let mut args = vec!["branch", "-m"];
            if let Some(name) = &input.name {
                args.push(name);
            }
            if let Some(new_name) = &input.new_name {
                args.push(new_name);
            }
            executor.execute(&args)?;
            Ok(GitBranchOutput {
                success: true,
                branches: vec![],
                current_branch: None,
                message: format!("Renamed branch to: {}", input.new_name.unwrap_or_default()),
            })
        }
        Some("show-current") | None => {
            let output = executor.execute(&["branch", "--show-current"])?;
            let current = output.stdout.trim().to_string();
            Ok(GitBranchOutput {
                success: true,
                branches: vec![],
                current_branch: if current.is_empty() { None } else { Some(current) },
                message: String::new(),
            })
        }
        Some("list") | _ => {
            let mut args = vec!["branch", "-vv", "--format=%(refname:short)|%(HEAD)|%(upstream:short)"];
            if input.all.unwrap_or(false) {
                args.push("-a");
            }
            if input.remote.unwrap_or(false) {
                args.push("-r");
            }
            
            let output = executor.execute(&args)?;
            
            let branches: Vec<GitBranch> = output.stdout
                .lines()
                .filter(|l| !l.is_empty())
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split('|').collect();
                    if parts.len() >= 2 {
                        Some(GitBranch {
                            name: parts[0].trim().to_string(),
                            is_current: parts.get(1).map(|s| s.trim() == "*").unwrap_or(false),
                            is_remote: parts[0].starts_with("remotes/"),
                            upstream: parts.get(2).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            
            let current_branch = branches.iter()
                .find(|b| b.is_current)
                .map(|b| b.name.clone());
            
            Ok(GitBranchOutput {
                success: true,
                branches,
                current_branch,
                message: String::new(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCheckoutInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Target: branch, commit, or file")]
    pub target: String,
    
    #[schemars(description = "Create new branch")]
    pub create_branch: Option<bool>,
    
    #[schemars(description = "Force checkout")]
    pub force: Option<bool>,
    
    #[schemars(description = "File paths to restore")]
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCheckoutOutput {
    pub success: bool,
    pub previous_head: Option<String>,
    pub new_head: String,
    pub message: String,
}

pub async fn git_checkout(ctx: ToolContext, input: GitCheckoutInput) -> Result<GitCheckoutOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args = vec!["checkout"];
    
    if input.create_branch.unwrap_or(false) {
        args.push("-b");
    }
    
    if input.force.unwrap_or(false) {
        args.push("-f");
    }
    
    args.push(&input.target);
    
    if let Some(paths) = &input.paths {
        args.push("--");
        for path in paths {
            args.push(path);
        }
    }
    
    let output = executor.execute(&args)?;
    
    Ok(GitCheckoutOutput {
        success: true,
        previous_head: None,
        new_head: input.target.clone(),
        message: output.stdout.trim().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitMergeInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Branch to merge")]
    pub branch: String,
    
    #[schemars(description = "Merge strategy")]
    pub strategy: Option<String>,
    
    #[schemars(description = "Create a merge commit")]
    pub no_fast_forward: Option<bool>,
    
    #[schemars(description = "Squash commits")]
    pub squash: Option<bool>,
    
    #[schemars(description = "Merge commit message")]
    pub message: Option<String>,
    
    #[schemars(description = "Abort current merge")]
    pub abort: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitMergeOutput {
    pub success: bool,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<String>,
    pub message: String,
}

pub async fn git_merge(ctx: ToolContext, input: GitMergeInput) -> Result<GitMergeOutput> {
    let executor = ctx.executor.read().await;
    
    if input.abort.unwrap_or(false) {
        executor.execute(&["merge", "--abort"])?;
        return Ok(GitMergeOutput {
            success: true,
            merged_branches: vec![],
            conflicts: vec![],
            message: "Merge aborted".to_string(),
        });
    }
    
    let mut args = vec!["merge"];
    
    if input.no_fast_forward.unwrap_or(false) {
        args.push("--no-ff");
    }
    
    if input.squash.unwrap_or(false) {
        args.push("--squash");
    }
    
    if let Some(msg) = &input.message {
        args.push("-m");
        args.push(msg);
    }
    
    if let Some(strategy) = &input.strategy {
        args.push("--strategy");
        args.push(strategy);
    }
    
    args.push(&input.branch);
    
    let output = executor.execute(&args)?;
    
    let has_conflicts = output.stdout.contains("CONFLICT") || output.stderr.contains("CONFLICT");
    
    Ok(GitMergeOutput {
        success: !has_conflicts,
        merged_branches: vec![input.branch.clone()],
        conflicts: if has_conflicts { vec!["Conflicts detected".to_string()] } else { vec![] },
        message: output.stdout.trim().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitRebaseInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Rebase mode: start, continue, abort, skip")]
    pub mode: Option<String>,
    
    #[schemars(description = "Upstream branch")]
    pub upstream: Option<String>,
    
    #[schemars(description = "Branch to rebase")]
    pub branch: Option<String>,
    
    #[schemars(description = "Interactive rebase")]
    pub interactive: Option<bool>,
    
    #[schemars(description = "Rebase onto specific commit")]
    pub onto: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitRebaseOutput {
    pub success: bool,
    pub message: String,
}

pub async fn git_rebase(ctx: ToolContext, input: GitRebaseInput) -> Result<GitRebaseOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args = vec!["rebase"];
    
    match input.mode.as_deref() {
        Some("continue") => {
            args.push("--continue");
        }
        Some("abort") => {
            args.push("--abort");
        }
        Some("skip") => {
            args.push("--skip");
        }
        _ => {
            if input.interactive.unwrap_or(false) {
                args.push("-i");
            }
            
            if let Some(onto) = &input.onto {
                args.push("--onto");
                args.push(onto);
            }
            
            if let Some(upstream) = &input.upstream {
                args.push(upstream);
            }
            
            if let Some(branch) = &input.branch {
                args.push(branch);
            }
        }
    }
    
    let output = executor.execute(&args)?;
    
    Ok(GitRebaseOutput {
        success: true,
        message: output.stdout.trim().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCherryPickInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Commit hashes to cherry-pick")]
    pub commits: Vec<String>,
    
    #[schemars(description = "Don't commit, only stage")]
    pub no_commit: Option<bool>,
    
    #[schemars(description = "Continue cherry-pick")]
    pub continue_op: Option<bool>,
    
    #[schemars(description = "Abort cherry-pick")]
    pub abort: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCherryPickOutput {
    pub success: bool,
    pub cherry_picked: Vec<String>,
    pub conflicts: bool,
    pub message: String,
}

pub async fn git_cherry_pick(ctx: ToolContext, input: GitCherryPickInput) -> Result<GitCherryPickOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args = vec!["cherry-pick"];
    
    if input.no_commit.unwrap_or(false) {
        args.push("--no-commit");
    }
    
    if input.continue_op.unwrap_or(false) {
        args.push("--continue");
    } else if input.abort.unwrap_or(false) {
        args.push("--abort");
    } else {
        for commit in &input.commits {
            args.push(commit);
        }
    }
    
    let output = executor.execute(&args)?;
    
    let has_conflicts = output.stdout.contains("CONFLICT") || output.stderr.contains("conflict");
    
    Ok(GitCherryPickOutput {
        success: !has_conflicts,
        cherry_picked: input.commits.clone(),
        conflicts: has_conflicts,
        message: output.stdout.trim().to_string(),
    })
}
