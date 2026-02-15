use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::tools::ToolContext;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitRemoteInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Operation: list, add, remove, rename, get-url, set-url")]
    pub mode: Option<String>,
    
    #[schemars(description = "Remote name")]
    pub name: Option<String>,
    
    #[schemars(description = "Remote URL")]
    pub url: Option<String>,
    
    #[schemars(description = "New remote name (for rename)")]
    pub new_name: Option<String>,
    
    #[schemars(description = "Push URL")]
    pub push: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitRemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitRemoteOutput {
    pub success: bool,
    pub remotes: Vec<GitRemoteInfo>,
    pub message: String,
}

pub async fn git_remote(ctx: ToolContext, input: GitRemoteInput) -> Result<GitRemoteOutput> {
    let executor = ctx.executor.read().await;
    
    match input.mode.as_deref() {
        Some("add") => {
            let mut args = vec!["remote", "add"];
            if let Some(name) = &input.name {
                args.push(name);
            }
            if let Some(url) = &input.url {
                args.push(url);
            }
            if let Some(push) = &input.push {
                args.push("--push");
                args.push(push);
            }
            executor.execute(&args)?;
            Ok(GitRemoteOutput {
                success: true,
                remotes: vec![],
                message: format!("Added remote: {}", input.name.unwrap_or_default()),
            })
        }
        Some("remove") => {
            let mut args = vec!["remote", "remove"];
            if let Some(name) = &input.name {
                args.push(name);
            }
            executor.execute(&args)?;
            Ok(GitRemoteOutput {
                success: true,
                remotes: vec![],
                message: format!("Removed remote: {}", input.name.unwrap_or_default()),
            })
        }
        Some("rename") => {
            let mut args = vec!["remote", "rename"];
            if let Some(name) = &input.name {
                args.push(name);
            }
            if let Some(new_name) = &input.new_name {
                args.push(new_name);
            }
            executor.execute(&args)?;
            Ok(GitRemoteOutput {
                success: true,
                remotes: vec![],
                message: format!("Renamed remote to: {}", input.new_name.unwrap_or_default()),
            })
        }
        Some("get-url") => {
            let mut args = vec!["remote", "get-url"];
            if let Some(name) = &input.name {
                args.push(name);
            }
            let output = executor.execute(&args)?;
            Ok(GitRemoteOutput {
                success: true,
                remotes: vec![GitRemoteInfo {
                    name: input.name.unwrap_or_default(),
                    fetch_url: output.stdout.trim().to_string(),
                    push_url: None,
                }],
                message: String::new(),
            })
        }
        Some("set-url") => {
            let mut args = vec!["remote", "set-url"];
            if let Some(name) = &input.name {
                args.push(name);
            }
            if let Some(url) = &input.url {
                args.push(url);
            }
            executor.execute(&args)?;
            Ok(GitRemoteOutput {
                success: true,
                remotes: vec![],
                message: format!("Set URL for remote: {}", input.name.unwrap_or_default()),
            })
        }
        _ => {
            let output = executor.execute(&["remote", "-v"])?;
            
            let mut remotes: Vec<GitRemoteInfo> = Vec::new();
            let mut current_name = String::new();
            let mut current_fetch = String::new();
            
            for line in output.stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0].to_string();
                    let url = parts[1].to_string();
                    let is_push = parts.get(2).map(|s| *s == "(push)").unwrap_or(false);
                    
                    if name != current_name {
                        if !current_name.is_empty() {
                            remotes.push(GitRemoteInfo {
                                name: current_name.clone(),
                                fetch_url: current_fetch.clone(),
                                push_url: None,
                            });
                        }
                        current_name = name;
                        current_fetch = url;
                    } else if is_push {
                        if let Some(last) = remotes.last_mut() {
                            last.push_url = Some(url);
                        }
                    }
                }
            }
            
            if !current_name.is_empty() {
                remotes.push(GitRemoteInfo {
                    name: current_name,
                    fetch_url: current_fetch,
                    push_url: None,
                });
            }
            
            Ok(GitRemoteOutput {
                success: true,
                remotes,
                message: String::new(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitFetchInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Remote name")]
    pub remote: Option<String>,
    
    #[schemars(description = "Prune deleted remote branches")]
    pub prune: Option<bool>,
    
    #[schemars(description = "Fetch all tags")]
    pub tags: Option<bool>,
    
    #[schemars(description = "Shallow fetch depth")]
    pub depth: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitFetchOutput {
    pub success: bool,
    pub remote: String,
    pub fetched_refs: Vec<String>,
    pub message: String,
}

pub async fn git_fetch(ctx: ToolContext, input: GitFetchInput) -> Result<GitFetchOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args: Vec<String> = vec!["fetch".into()];
    
    if input.prune.unwrap_or(false) {
        args.push("--prune".into());
    }
    
    if input.tags.unwrap_or(false) {
        args.push("--tags".into());
    }
    
    if let Some(depth) = input.depth {
        args.push("--depth".into());
        args.push(depth.to_string());
    }
    
    let remote = input.remote.clone().unwrap_or_else(|| "--all".to_string());
    args.push(remote.clone());
    
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = executor.execute(&args_refs)?;
    
    let fetched_refs: Vec<String> = output.stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    
    Ok(GitFetchOutput {
        success: true,
        remote,
        fetched_refs,
        message: "Fetch completed".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitPullInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Remote name")]
    pub remote: Option<String>,
    
    #[schemars(description = "Branch to pull")]
    pub branch: Option<String>,
    
    #[schemars(description = "Rebase instead of merge")]
    pub rebase: Option<bool>,
    
    #[schemars(description = "Only fast-forward")]
    pub fast_forward_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitPullOutput {
    pub success: bool,
    pub merged_branches: Vec<String>,
    pub fast_forwarded: bool,
    pub message: String,
}

pub async fn git_pull(ctx: ToolContext, input: GitPullInput) -> Result<GitPullOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args = vec!["pull"];
    
    if input.rebase.unwrap_or(false) {
        args.push("--rebase");
    }
    
    if input.fast_forward_only.unwrap_or(false) {
        args.push("--ff-only");
    }
    
    if let Some(remote) = &input.remote {
        args.push(remote);
    }
    
    if let Some(branch) = &input.branch {
        args.push(branch);
    }
    
    let output = executor.execute(&args)?;
    
    let fast_forwarded = output.stdout.contains("Fast-forward");
    
    Ok(GitPullOutput {
        success: true,
        merged_branches: input.branch.iter().cloned().collect(),
        fast_forwarded,
        message: output.stdout.trim().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitPushInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Remote name")]
    pub remote: Option<String>,
    
    #[schemars(description = "Branch to push")]
    pub branch: Option<String>,
    
    #[schemars(description = "Force push")]
    pub force: Option<bool>,
    
    #[schemars(description = "Force with lease")]
    pub force_with_lease: Option<bool>,
    
    #[schemars(description = "Set upstream")]
    pub set_upstream: Option<bool>,
    
    #[schemars(description = "Push all tags")]
    pub tags: Option<bool>,
    
    #[schemars(description = "Dry run")]
    pub dry_run: Option<bool>,
    
    #[schemars(description = "Delete remote branch")]
    pub delete: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitPushOutput {
    pub success: bool,
    pub remote: String,
    pub branch: Option<String>,
    pub message: String,
}

pub async fn git_push(ctx: ToolContext, input: GitPushInput) -> Result<GitPushOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args = vec!["push"];
    
    if input.force.unwrap_or(false) {
        args.push("--force");
    }
    
    if input.force_with_lease.unwrap_or(false) {
        args.push("--force-with-lease");
    }
    
    if input.set_upstream.unwrap_or(false) {
        args.push("--set-upstream");
    }
    
    if input.tags.unwrap_or(false) {
        args.push("--tags");
    }
    
    if input.dry_run.unwrap_or(false) {
        args.push("--dry-run");
    }
    
    if input.delete.unwrap_or(false) {
        args.push("--delete");
    }
    
    let remote = input.remote.clone().unwrap_or_else(|| "origin".to_string());
    args.push(&remote);
    
    if let Some(branch) = &input.branch {
        args.push(branch);
    }
    
    let output = executor.execute(&args)?;
    
    Ok(GitPushOutput {
        success: true,
        remote,
        branch: input.branch.clone(),
        message: output.stdout.trim().to_string(),
    })
}
