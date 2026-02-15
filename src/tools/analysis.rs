use crate::error::Result;
use crate::tools::ToolContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitChangelogAnalyzeInput {
    #[schemars(description = "Path to the repository")]
    pub path: String,

    #[schemars(
        description = "Review types to generate instructions for: security, features, storyline, gaps, breaking_changes, quality"
    )]
    #[serde(rename = "reviewTypes")]
    pub review_types: Vec<String>,

    #[schemars(description = "Maximum recent commits to fetch (1-1000, default 200)")]
    #[serde(rename = "maxCommits")]
    pub max_commits: Option<u32>,

    #[schemars(description = "Only include history since this tag")]
    #[serde(rename = "sinceTag")]
    pub since_tag: Option<String>,

    #[schemars(description = "Branch or ref to analyze (defaults to current branch)")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitChangelogCommit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub timestamp: i64,
    pub refs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitChangelogTag {
    pub name: String,
    pub commit: String,
    pub timestamp: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitChangelogGitContext {
    #[serde(rename = "currentBranch")]
    pub current_branch: Option<String>,
    #[serde(rename = "totalCommitsFetched")]
    pub total_commits_fetched: u32,
    pub commits: Vec<GitChangelogCommit>,
    pub tags: Vec<GitChangelogTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitChangelogAnalyzeOutput {
    pub success: bool,
    #[serde(rename = "reviewTypes")]
    pub review_types: Vec<String>,
    #[serde(rename = "gitContext")]
    pub git_context: GitChangelogGitContext,
    #[serde(rename = "reviewInstructions")]
    pub review_instructions: String,
}

pub async fn git_changelog_analyze(
    ctx: ToolContext,
    input: GitChangelogAnalyzeInput,
) -> Result<GitChangelogAnalyzeOutput> {
    if input.review_types.is_empty() {
        return Err(crate::error::GitMcpError::InvalidInput(
            "reviewTypes must contain at least one value".to_string(),
        ));
    }

    let allowed = [
        "security",
        "features",
        "storyline",
        "gaps",
        "breaking_changes",
        "quality",
    ];
    for t in &input.review_types {
        if !allowed.contains(&t.as_str()) {
            return Err(crate::error::GitMcpError::InvalidInput(format!(
                "Unknown review type: {}",
                t
            )));
        }
    }

    let executor = ctx.executor.read().await;
    let repo_path = PathBuf::from(&input.path);

    let current_branch = executor
        .execute_in_dir(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .map(|o| o.stdout.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD");

    let max_commits = input.max_commits.unwrap_or(200).clamp(1, 1000);
    let target_ref = input.branch.as_deref().unwrap_or("HEAD");

    let range = input
        .since_tag
        .as_ref()
        .map(|t| format!("{}..{}", t, target_ref));

    let mut log_args: Vec<String> = vec![
        "log".into(),
        "--format=%h|%s|%an|%ct|%D".into(),
        "-n".into(),
        max_commits.to_string(),
    ];
    if let Some(r) = &range {
        log_args.push(r.clone());
    } else {
        log_args.push(target_ref.to_string());
    }
    let log_args_refs: Vec<&str> = log_args.iter().map(|s| s.as_str()).collect();

    let log_output = executor.execute_in_dir(&repo_path, &log_args_refs)?;
    let commits: Vec<GitChangelogCommit> = log_output
        .stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, '|').collect();
            if parts.len() < 4 {
                return None;
            }
            let refs = parts
                .get(4)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.split(',')
                        .map(|r| r.trim().to_string())
                        .filter(|r| !r.is_empty())
                        .collect::<Vec<String>>()
                })
                .filter(|v| !v.is_empty());

            Some(GitChangelogCommit {
                hash: parts[0].to_string(),
                subject: parts[1].to_string(),
                author: parts[2].to_string(),
                timestamp: parts[3].parse::<i64>().unwrap_or(0),
                refs,
            })
        })
        .collect();

    let tag_output = executor.execute_in_dir(
        &repo_path,
        &[
            "for-each-ref",
            "refs/tags",
            "--format=%(refname:short)|%(objectname:short)|%(creatordate:unix)|%(subject)",
        ],
    )?;
    let tags: Vec<GitChangelogTag> = tag_output
        .stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() < 2 {
                return None;
            }
            let ts = parts.get(2).and_then(|s| s.parse::<i64>().ok());
            let msg = parts
                .get(3)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            Some(GitChangelogTag {
                name: parts[0].to_string(),
                commit: parts[1].to_string(),
                timestamp: ts,
                message: msg,
            })
        })
        .collect();

    let review_instructions = build_review_instructions(&input.review_types);

    Ok(GitChangelogAnalyzeOutput {
        success: true,
        review_types: input.review_types,
        git_context: GitChangelogGitContext {
            current_branch,
            total_commits_fetched: commits.len() as u32,
            commits,
            tags,
        },
        review_instructions,
    })
}

fn build_review_instructions(review_types: &[String]) -> String {
    let mut out = String::new();
    out.push_str(
        "Use the provided gitContext (commits and tags) to cross-check the changelog.\n\n",
    );

    for t in review_types {
        match t.as_str() {
            "security" => {
                out.push_str("Security:\n- Identify security-related commits not reflected in the changelog.\n- Check for missing CVE references, severity notes, and mitigation guidance.\n\n");
            }
            "features" => {
                out.push_str("Features:\n- Summarize major features and ensure they are documented.\n- Call out incomplete or partially documented feature work.\n\n");
            }
            "storyline" => {
                out.push_str("Storyline:\n- Describe the project evolution arc reflected by commits/tags.\n- Highlight major milestones and transitions.\n\n");
            }
            "gaps" => {
                out.push_str("Gaps:\n- Find commits that should appear in the changelog but do not.\n- Flag areas with frequent fixes but little documentation.\n\n");
            }
            "breaking_changes" => {
                out.push_str("Breaking Changes:\n- Detect breaking changes and verify migration notes exist.\n- Ensure deprecations and removals are clearly described.\n\n");
            }
            "quality" => {
                out.push_str("Quality:\n- Analyze commit patterns for stability, hotfix frequency, and refactor/chore trends.\n- Suggest improvements to changelog clarity and completeness.\n\n");
            }
            _ => {}
        }
    }

    out
}
