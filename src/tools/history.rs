use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::tools::ToolContext;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitLogInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Maximum number of commits to show")]
    pub max_count: Option<i32>,
    
    #[schemars(description = "Skip N commits")]
    pub skip: Option<i32>,
    
    #[schemars(description = "Show commits after this date")]
    pub since: Option<String>,
    
    #[schemars(description = "Show commits before this date")]
    pub until: Option<String>,
    
    #[schemars(description = "Filter by author")]
    pub author: Option<String>,
    
    #[schemars(description = "Search pattern")]
    pub grep: Option<String>,
    
    #[schemars(description = "Branch or commit to show")]
    pub branch: Option<String>,
    
    #[schemars(description = "Filter by file path")]
    pub file_path: Option<String>,
    
    #[schemars(description = "Show one commit per line")]
    pub oneline: Option<bool>,
    
    #[schemars(description = "Show diffstat")]
    pub stat: Option<bool>,
    
    #[schemars(description = "Show full diff")]
    pub patch: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitLogOutput {
    pub success: bool,
    pub commits: Vec<GitCommit>,
    pub total: Option<i32>,
}

pub async fn git_log(ctx: ToolContext, input: GitLogInput) -> Result<GitLogOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args: Vec<String> = vec!["log".into(), "--format=%H|%h|%an|%ae|%ad|%s".into(), "--date=iso".into()];
    
    if let Some(n) = input.max_count {
        args.push("-n".into());
        args.push(n.to_string());
    }
    
    if let Some(n) = input.skip {
        args.push("--skip".into());
        args.push(n.to_string());
    }
    
    if let Some(since) = &input.since {
        args.push("--since".into());
        args.push(since.clone());
    }
    
    if let Some(until) = &input.until {
        args.push("--until".into());
        args.push(until.clone());
    }
    
    if let Some(author) = &input.author {
        args.push("--author".into());
        args.push(author.clone());
    }
    
    if let Some(grep) = &input.grep {
        args.push("--grep".into());
        args.push(grep.clone());
    }
    
    if input.oneline.unwrap_or(false) {
        args.push("--oneline".into());
    }
    
    if input.stat.unwrap_or(false) {
        args.push("--stat".into());
    }
    
    if input.patch.unwrap_or(false) {
        args.push("--patch".into());
    }
    
    if let Some(branch) = &input.branch {
        args.push(branch.clone());
    }
    
    if let Some(file_path) = &input.file_path {
        args.push("--".into());
        args.push(file_path.clone());
    }
    
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = executor.execute(&args_refs)?;
    
    let commits: Vec<GitCommit> = output.stdout
        .lines()
        .filter(|l| l.contains('|'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(6, '|').collect();
            if parts.len() >= 6 {
                Some(GitCommit {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    email: parts[3].to_string(),
                    date: parts[4].to_string(),
                    message: parts[5].to_string(),
                })
            } else {
                None
            }
        })
        .collect();
    
    let total = Some(commits.len() as i32);
    
    Ok(GitLogOutput {
        success: true,
        commits,
        total,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitShowInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Object to show (commit, tag, tree, blob)")]
    pub object: Option<String>,
    
    #[schemars(description = "Pretty format")]
    pub format: Option<String>,
    
    #[schemars(description = "Show diffstat")]
    pub stat: Option<bool>,
    
    #[schemars(description = "Specific file path")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitShowOutput {
    pub success: bool,
    pub content: String,
    pub commit: Option<GitCommit>,
}

pub async fn git_show(ctx: ToolContext, input: GitShowInput) -> Result<GitShowOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args: Vec<String> = vec!["show".into()];
    
    if let Some(fmt) = &input.format {
        args.push(format!("--format={}", fmt));
    } else {
        args.push("--format=%H|%h|%an|%ae|%ad|%s".into());
    }
    
    if input.stat.unwrap_or(false) {
        args.push("--stat".into());
    }
    
    if let Some(object) = &input.object {
        args.push(object.clone());
    }
    
    if let Some(file_path) = &input.file_path {
        args.push("--".into());
        args.push(file_path.clone());
    }
    
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = executor.execute(&args_refs)?;
    
    let commit = output.stdout
        .lines()
        .next()
        .filter(|l| l.contains('|'))
        .and_then(|line| {
            let parts: Vec<&str> = line.splitn(6, '|').collect();
            if parts.len() >= 6 {
                Some(GitCommit {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    email: parts[3].to_string(),
                    date: parts[4].to_string(),
                    message: parts[5].to_string(),
                })
            } else {
                None
            }
        });
    
    Ok(GitShowOutput {
        success: true,
        content: output.stdout,
        commit,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitBlameInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "File to blame")]
    pub file: String,
    
    #[schemars(description = "Start line number")]
    pub start_line: Option<i32>,
    
    #[schemars(description = "End line number")]
    pub end_line: Option<i32>,
    
    #[schemars(description = "Ignore whitespace changes")]
    pub ignore_whitespace: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitBlameLine {
    pub line_number: i32,
    pub commit_hash: String,
    pub author: String,
    pub date: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitBlameOutput {
    pub success: bool,
    pub file: String,
    pub lines: Vec<GitBlameLine>,
}

pub async fn git_blame(ctx: ToolContext, input: GitBlameInput) -> Result<GitBlameOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args: Vec<String> = vec!["blame".into(), "--line-porcelain".into()];
    
    if input.ignore_whitespace.unwrap_or(false) {
        args.push("-w".into());
    }
    
    if let (Some(start), Some(end)) = (input.start_line, input.end_line) {
        args.push(format!("-L{},{}", start, end));
    }
    
    args.push(input.file.clone());
    
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = executor.execute(&args_refs)?;
    
    let mut lines = Vec::new();
    let mut current_line: Option<GitBlameLine> = None;
    let mut line_num = 0;
    
    for line in output.stdout.lines() {
        if line.starts_with("author ") {
            if let Some(ref mut curr) = current_line {
                curr.author = line[7..].to_string();
            }
        } else if line.starts_with("author-mail ") {
        } else if line.starts_with("author-time ") {
            if let Some(ref mut curr) = current_line {
                curr.date = line[12..].to_string();
            }
        } else if line.starts_with('\t') {
            if let Some(curr) = current_line.take() {
                lines.push(GitBlameLine {
                    content: line[1..].to_string(),
                    ..curr
                });
            }
        } else if !line.starts_with(' ') && line.contains(' ') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                line_num += 1;
                current_line = Some(GitBlameLine {
                    line_number: parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(line_num),
                    commit_hash: parts[0].to_string(),
                    author: String::new(),
                    date: String::new(),
                    content: String::new(),
                });
            }
        }
    }
    
    Ok(GitBlameOutput {
        success: true,
        file: input.file.clone(),
        lines,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitReflogInput {
    #[schemars(description = "Path to the repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Reference to show (default: HEAD)")]
    pub r#ref: Option<String>,
    
    #[schemars(description = "Maximum number of entries")]
    pub max_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitReflogEntry {
    pub hash: String,
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitReflogOutput {
    pub success: bool,
    pub entries: Vec<GitReflogEntry>,
}

pub async fn git_reflog(ctx: ToolContext, input: GitReflogInput) -> Result<GitReflogOutput> {
    let executor = ctx.executor.read().await;
    
    let mut args: Vec<String> = vec!["reflog".into(), "--format=%H|%gs|%gd".into()];
    
    if let Some(n) = input.max_count {
        args.push("-n".into());
        args.push(n.to_string());
    }
    
    if let Some(ref_name) = &input.r#ref {
        args.push(ref_name.clone());
    }
    
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = executor.execute(&args_refs)?;
    
    let entries: Vec<GitReflogEntry> = output.stdout
        .lines()
        .filter(|l| l.contains('|'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() >= 3 {
                Some(GitReflogEntry {
                    hash: parts[0].to_string(),
                    action: parts[1].to_string(),
                    message: parts[2].to_string(),
                })
            } else {
                None
            }
        })
        .collect();
    
    Ok(GitReflogOutput {
        success: true,
        entries,
    })
}
