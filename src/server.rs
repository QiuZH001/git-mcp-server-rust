use crate::config::Config;
use crate::git::GitExecutor;
use crate::tools::{advanced, branching, history, remote, repo, staging, ToolContext};
use schemars::schema_for;
use schemars::JsonSchema;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

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
            let id = req.id.clone();
            let result = process_request(ctx, req).await;
            match result {
                Ok(response) => json_rpc_response(id, response),
                Err(e) => json_rpc_error(id, &e.to_string(), -32603),
            }
        }
        Err(e) => json_rpc_error(None, &format!("Parse error: {}", e), -32700),
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn json_rpc_response(id: Option<Value>, result: Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    })
    .to_string()
}

fn json_rpc_error(id: Option<Value>, message: &str, code: i32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": id
    })
    .to_string()
}

async fn process_request(ctx: &ToolContext, req: JsonRpcRequest) -> anyhow::Result<Value> {
    match req.method.as_str() {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "git-mcp-server",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),

        "tools/list" => Ok(serde_json::json!({
            "tools": get_tool_definitions()
        })),

        "tools/call" => {
            let params = req
                .params
                .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            match execute_tool(ctx, name, arguments).await {
                Ok(value) => Ok(call_tool_ok(value)),
                Err(e) => Ok(call_tool_error(e.to_string())),
            }
        }

        _ => Err(anyhow::anyhow!("Unknown method: {}", req.method)),
    }
}

fn call_tool_ok(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "isError": false
    })
}

fn call_tool_error(message: String) -> Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": message
        }],
        "isError": true
    })
}

fn get_tool_definitions() -> Vec<Value> {
    vec![
        tool_def::<repo::GitStatusInput>("git_status", "Show the working tree status"),
        tool_def::<repo::GitInitInput>("git_init", "Initialize a new Git repository"),
        tool_def::<repo::GitCloneInput>("git_clone", "Clone a repository from a remote URL"),
        tool_def::<repo::GitCleanInput>(
            "git_clean",
            "Remove untracked files from the working tree",
        ),
        tool_def::<staging::GitAddInput>("git_add", "Stage files for commit"),
        tool_def::<staging::GitCommitInput>("git_commit", "Create a new commit"),
        tool_def::<staging::GitDiffInput>("git_diff", "View differences"),
        tool_def::<history::GitLogInput>("git_log", "View commit history"),
        tool_def::<history::GitShowInput>("git_show", "Show details of a git object"),
        tool_def::<history::GitBlameInput>("git_blame", "Show line-by-line authorship"),
        tool_def::<history::GitReflogInput>("git_reflog", "View the reference logs"),
        tool_def::<branching::GitBranchInput>("git_branch", "Manage branches"),
        tool_def::<branching::GitCheckoutInput>(
            "git_checkout",
            "Switch branches or restore working tree files",
        ),
        tool_def::<branching::GitMergeInput>("git_merge", "Merge branches together"),
        tool_def::<branching::GitRebaseInput>("git_rebase", "Rebase commits onto another branch"),
        tool_def::<branching::GitCherryPickInput>("git_cherry_pick", "Cherry-pick commits"),
        tool_def::<remote::GitRemoteInput>("git_remote", "Manage remote repositories"),
        tool_def::<remote::GitFetchInput>("git_fetch", "Fetch updates from a remote repository"),
        tool_def::<remote::GitPullInput>("git_pull", "Pull changes from a remote repository"),
        tool_def::<remote::GitPushInput>("git_push", "Push changes to a remote repository"),
        tool_def::<advanced::GitTagInput>("git_tag", "Manage tags"),
        tool_def::<advanced::GitStashInput>("git_stash", "Manage stashes"),
        tool_def::<advanced::GitResetInput>("git_reset", "Reset current HEAD to specified state"),
        tool_def::<advanced::GitWorktreeInput>("git_worktree", "Manage multiple working trees"),
        tool_def::<advanced::GitSetWorkingDirInput>(
            "git_set_working_dir",
            "Set the session working directory",
        ),
        tool_def::<advanced::GitClearWorkingDirInput>(
            "git_clear_working_dir",
            "Clear the session working directory",
        ),
    ]
}

fn tool_def<T: JsonSchema>(name: &str, description: &str) -> Value {
    let schema = schema_for!(T);
    let input_schema = serde_json::to_value(&schema.schema)
        .unwrap_or_else(|_| serde_json::json!({"type": "object"}));

    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
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
        _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
    }
}
