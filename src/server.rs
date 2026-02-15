use crate::config::Config;
use crate::tools::{ToolContext, repo, staging, history, branching, remote, advanced};
use crate::git::GitExecutor;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;

pub async fn run_server(config: Config) -> anyhow::Result<()> {
    let executor = GitExecutor::new(config.clone());
    let ctx = ToolContext {
        config: config.clone(),
        executor: Arc::new(RwLock::new(executor)),
    };

    tracing::info!("Running in STDIO mode");
    run_stdio_server(ctx).await
}

async fn run_stdio_server(ctx: ToolContext) -> anyhow::Result<()> {
    use std::io::{self, BufRead, Write};
    
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    
    let mut line = String::new();
    let stdin_lock = stdin.lock();
    
    for line_result in stdin_lock.lines() {
        line.clear();
        match line_result {
            Ok(input) => {
                line = input;
                let response = handle_request(&ctx, &line).await;
                writeln!(stdout, "{}", response)?;
                stdout.flush()?;
            }
            Err(e) => {
                tracing::error!("Error reading stdin: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

async fn handle_request(ctx: &ToolContext, input: &str) -> String {
    let request: Result<JsonRpcRequest, _> = serde_json::from_str(input);
    
    match request {
        Ok(req) => {
            let result = process_request(ctx, req).await;
            match result {
                Ok(response) => json_rpc_response(response),
                Err(e) => json_rpc_error(&e.to_string(), -32603),
            }
        }
        Err(e) => {
            json_rpc_error(&format!("Parse error: {}", e), -32700)
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn json_rpc_response(result: Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "result": result
    }).to_string()
}

fn json_rpc_error(message: &str, code: i32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": null
    }).to_string()
}

async fn process_request(ctx: &ToolContext, req: JsonRpcRequest) -> anyhow::Result<Value> {
    match req.method.as_str() {
        "initialize" => {
            Ok(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "git-mcp-server",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }))
        }
        
        "tools/list" => {
            Ok(serde_json::json!({
                "tools": get_tool_definitions()
            }))
        }
        
        "tools/call" => {
            let params = req.params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            let name = params.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
            let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
            
            execute_tool(ctx, name, arguments).await
        }
        
        _ => {
            Err(anyhow::anyhow!("Unknown method: {}", req.method))
        }
    }
}

fn get_tool_definitions() -> Vec<Value> {
    vec![
        tool_def("git_status", "Show the working tree status", "GitStatusInput"),
        tool_def("git_init", "Initialize a new Git repository", "GitInitInput"),
        tool_def("git_clone", "Clone a repository from a remote URL", "GitCloneInput"),
        tool_def("git_clean", "Remove untracked files from the working tree", "GitCleanInput"),
        tool_def("git_add", "Stage files for commit", "GitAddInput"),
        tool_def("git_commit", "Create a new commit", "GitCommitInput"),
        tool_def("git_diff", "View differences", "GitDiffInput"),
        tool_def("git_log", "View commit history", "GitLogInput"),
        tool_def("git_show", "Show details of a git object", "GitShowInput"),
        tool_def("git_blame", "Show line-by-line authorship", "GitBlameInput"),
        tool_def("git_reflog", "View the reference logs", "GitReflogInput"),
        tool_def("git_branch", "Manage branches", "GitBranchInput"),
        tool_def("git_checkout", "Switch branches or restore working tree files", "GitCheckoutInput"),
        tool_def("git_merge", "Merge branches together", "GitMergeInput"),
        tool_def("git_rebase", "Rebase commits onto another branch", "GitRebaseInput"),
        tool_def("git_cherry_pick", "Cherry-pick commits", "GitCherryPickInput"),
        tool_def("git_remote", "Manage remote repositories", "GitRemoteInput"),
        tool_def("git_fetch", "Fetch updates from a remote repository", "GitFetchInput"),
        tool_def("git_pull", "Pull changes from a remote repository", "GitPullInput"),
        tool_def("git_push", "Push changes to a remote repository", "GitPushInput"),
        tool_def("git_tag", "Manage tags", "GitTagInput"),
        tool_def("git_stash", "Manage stashes", "GitStashInput"),
        tool_def("git_reset", "Reset current HEAD to specified state", "GitResetInput"),
        tool_def("git_worktree", "Manage multiple working trees", "GitWorktreeInput"),
        tool_def("git_set_working_dir", "Set the session working directory", "GitSetWorkingDirInput"),
        tool_def("git_clear_working_dir", "Clear the session working directory", "GitClearWorkingDirInput"),
    ]
}

fn tool_def(name: &str, description: &str, input_schema: &str) -> Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "$ref": format!("#/definitions/{}", input_schema)
        }
    })
}

async fn execute_tool(ctx: &ToolContext, name: &str, arguments: Value) -> anyhow::Result<Value> {
    match name {
        "git_status" => {
            let input: repo::GitStatusInput = serde_json::from_value(arguments)?;
            let result = repo::git_status(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_init" => {
            let input: repo::GitInitInput = serde_json::from_value(arguments)?;
            let result = repo::git_init(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_clone" => {
            let input: repo::GitCloneInput = serde_json::from_value(arguments)?;
            let result = repo::git_clone(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_clean" => {
            let input: repo::GitCleanInput = serde_json::from_value(arguments)?;
            let result = repo::git_clean(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_add" => {
            let input: staging::GitAddInput = serde_json::from_value(arguments)?;
            let result = staging::git_add(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_commit" => {
            let input: staging::GitCommitInput = serde_json::from_value(arguments)?;
            let result = staging::git_commit(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_diff" => {
            let input: staging::GitDiffInput = serde_json::from_value(arguments)?;
            let result = staging::git_diff(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_log" => {
            let input: history::GitLogInput = serde_json::from_value(arguments)?;
            let result = history::git_log(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_show" => {
            let input: history::GitShowInput = serde_json::from_value(arguments)?;
            let result = history::git_show(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_blame" => {
            let input: history::GitBlameInput = serde_json::from_value(arguments)?;
            let result = history::git_blame(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_reflog" => {
            let input: history::GitReflogInput = serde_json::from_value(arguments)?;
            let result = history::git_reflog(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_branch" => {
            let input: branching::GitBranchInput = serde_json::from_value(arguments)?;
            let result = branching::git_branch(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_checkout" => {
            let input: branching::GitCheckoutInput = serde_json::from_value(arguments)?;
            let result = branching::git_checkout(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_merge" => {
            let input: branching::GitMergeInput = serde_json::from_value(arguments)?;
            let result = branching::git_merge(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_rebase" => {
            let input: branching::GitRebaseInput = serde_json::from_value(arguments)?;
            let result = branching::git_rebase(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_cherry_pick" => {
            let input: branching::GitCherryPickInput = serde_json::from_value(arguments)?;
            let result = branching::git_cherry_pick(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_remote" => {
            let input: remote::GitRemoteInput = serde_json::from_value(arguments)?;
            let result = remote::git_remote(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_fetch" => {
            let input: remote::GitFetchInput = serde_json::from_value(arguments)?;
            let result = remote::git_fetch(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_pull" => {
            let input: remote::GitPullInput = serde_json::from_value(arguments)?;
            let result = remote::git_pull(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_push" => {
            let input: remote::GitPushInput = serde_json::from_value(arguments)?;
            let result = remote::git_push(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_tag" => {
            let input: advanced::GitTagInput = serde_json::from_value(arguments)?;
            let result = advanced::git_tag(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_stash" => {
            let input: advanced::GitStashInput = serde_json::from_value(arguments)?;
            let result = advanced::git_stash(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_reset" => {
            let input: advanced::GitResetInput = serde_json::from_value(arguments)?;
            let result = advanced::git_reset(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_worktree" => {
            let input: advanced::GitWorktreeInput = serde_json::from_value(arguments)?;
            let result = advanced::git_worktree(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_set_working_dir" => {
            let input: advanced::GitSetWorkingDirInput = serde_json::from_value(arguments)?;
            let result = advanced::git_set_working_dir(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_clear_working_dir" => {
            let input: advanced::GitClearWorkingDirInput = serde_json::from_value(arguments)?;
            let result = advanced::git_clear_working_dir(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        _ => {
            Err(anyhow::anyhow!("Unknown tool: {}", name))
        }
    }
}
