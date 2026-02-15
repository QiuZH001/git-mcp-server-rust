use crate::config::Config;
use crate::git::GitExecutor;
use crate::tools::{advanced, analysis, branching, history, remote, repo, staging, ToolContext};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use schemars::schema_for;
use schemars::JsonSchema;
use serde_json::Map;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run_server(config: Config) -> anyhow::Result<()> {
    let executor = GitExecutor::new(config.clone());
    let ctx = ToolContext {
        config: config.clone(),
        executor: Arc::new(RwLock::new(executor)),
    };

    match config.transport_type {
        crate::config::TransportType::Http => {
            tracing::info!("Running in HTTP mode");
            run_http_server(ctx).await
        }
        crate::config::TransportType::Stdio => {
            tracing::info!("Running in STDIO mode");
            run_stdio_server(ctx).await
        }
    }
}

async fn run_http_server(ctx: ToolContext) -> anyhow::Result<()> {
    use axum::extract::State;
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Router;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    #[derive(Clone)]
    struct AppState {
        config: Config,
        session_mode: crate::config::SessionMode,
        sessions: Arc<RwLock<HashMap<String, ToolContext>>>,
    }

    async fn handler(
        State(state): State<AppState>,
        headers: HeaderMap,
        body: String,
    ) -> impl IntoResponse {
        let protocol_version = headers
            .get("MCP-Protocol-Version")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("2025-11-25")
            .to_string();

        let session_id = headers
            .get("MCP-Session-Id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let parsed: Result<JsonRpcRequest, _> = serde_json::from_str(&body);
        let req = match parsed {
            Ok(r) => r,
            Err(e) => {
                let resp = json_rpc_error(None, &format!("Parse error: {}", e), -32700);
                let mut out_headers = HeaderMap::new();
                out_headers.insert("Content-Type", HeaderValue::from_static("application/json"));
                out_headers.insert(
                    "MCP-Protocol-Version",
                    HeaderValue::from_str(&protocol_version)
                        .unwrap_or(HeaderValue::from_static("2025-11-25")),
                );
                return (StatusCode::OK, out_headers, resp);
            }
        };

        let mut response_headers = HeaderMap::new();
        response_headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        response_headers.insert(
            "MCP-Protocol-Version",
            HeaderValue::from_str(&protocol_version)
                .unwrap_or(HeaderValue::from_static("2025-11-25")),
        );

        if !state.config.allowed_origins.is_empty() {
            if let Some(origin) = headers.get("Origin").and_then(|v| v.to_str().ok()) {
                if !state
                    .config
                    .allowed_origins
                    .iter()
                    .any(|allowed| allowed == origin)
                {
                    let resp = json_rpc_error(req.id.clone(), "Forbidden origin", -32003);
                    return (StatusCode::FORBIDDEN, response_headers, resp);
                }
            }
        }

        if let Err(msg) = validate_auth(&state.config, &headers) {
            let resp = json_rpc_error(req.id.clone(), &msg, -32001);
            return (StatusCode::UNAUTHORIZED, response_headers, resp);
        }

        let (ctx_for_call, new_session_id) = if req.method == "initialize"
            && state.session_mode != crate::config::SessionMode::Stateless
        {
            let new_id = Uuid::new_v4().to_string();
            let new_ctx = ToolContext::new(state.config.clone());
            {
                let mut sessions = state.sessions.write().await;
                sessions.insert(new_id.clone(), new_ctx.clone());
            }
            (new_ctx, Some(new_id))
        } else {
            match (state.session_mode, session_id) {
                (crate::config::SessionMode::Stateful, Some(id))
                | (crate::config::SessionMode::Auto, Some(id)) => {
                    let sessions = state.sessions.read().await;
                    if let Some(existing) = sessions.get(&id) {
                        (existing.clone(), None)
                    } else {
                        let resp = json_rpc_error(req.id.clone(), "Invalid MCP session", -32602);
                        return (StatusCode::OK, response_headers, resp);
                    }
                }
                (crate::config::SessionMode::Stateful, None) => {
                    let resp = json_rpc_error(req.id.clone(), "Missing MCP-Session-Id", -32602);
                    return (StatusCode::OK, response_headers, resp);
                }
                (crate::config::SessionMode::Stateless, _) => {
                    (ToolContext::new(state.config.clone()), None)
                }
                (_, None) => (ToolContext::new(state.config.clone()), None),
            }
        };

        if let Some(id) = &new_session_id {
            if let Ok(val) = HeaderValue::from_str(id) {
                response_headers.insert("MCP-Session-Id", val);
            }
        }

        let id = req.id.clone();
        let result = process_request(&ctx_for_call, req).await;
        let resp = match result {
            Ok(value) => json_rpc_response(id, value),
            Err(e) => json_rpc_error(id, &e.to_string(), -32603),
        };

        (StatusCode::OK, response_headers, resp)
    }

    let state = AppState {
        config: ctx.config.clone(),
        session_mode: ctx.config.session_mode,
        sessions: Arc::new(RwLock::new(HashMap::new())),
    };

    let endpoint_path = ctx.config.http_endpoint_path.clone();
    let router = Router::new()
        .route(&endpoint_path, post(handler))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", ctx.config.http_host, ctx.config.http_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid HTTP bind address: {}", e))?;

    axum::serve(tokio::net::TcpListener::bind(addr).await?, router).await?;
    Ok(())
}

fn validate_auth(config: &Config, headers: &axum::http::HeaderMap) -> Result<(), String> {
    match config.auth_mode {
        crate::config::AuthMode::None => Ok(()),
        crate::config::AuthMode::Jwt => {
            let token = extract_bearer_token(headers)?;
            let secret = config
                .auth_secret_key
                .as_ref()
                .ok_or_else(|| "MCP_AUTH_SECRET_KEY is required in jwt mode".to_string())?;

            let mut validation = Validation::new(Algorithm::HS256);
            validation.validate_aud = false;
            decode::<Map<String, Value>>(
                &token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &validation,
            )
            .map_err(|e| format!("Invalid JWT token: {}", e))?;
            Ok(())
        }
        crate::config::AuthMode::Oauth => {
            let token = extract_bearer_token(headers)?;
            let issuer = config
                .oauth_issuer_url
                .as_ref()
                .ok_or_else(|| "OAUTH_ISSUER_URL is required in oauth mode".to_string())?;
            let audience = config
                .oauth_audience
                .as_ref()
                .ok_or_else(|| "OAUTH_AUDIENCE is required in oauth mode".to_string())?;
            let public_key_pem = config
                .oauth_public_key_pem
                .as_ref()
                .ok_or_else(|| "OAUTH_PUBLIC_KEY_PEM is required in oauth mode".to_string())?;

            let header = decode_header(&token).map_err(|e| format!("Invalid JWT header: {}", e))?;
            let algorithm = header.alg;
            if algorithm != Algorithm::RS256 {
                return Err("OAuth token must use RS256 algorithm".to_string());
            }

            let mut validation = Validation::new(Algorithm::RS256);
            validation.set_issuer(&[issuer.as_str()]);
            validation.set_audience(&[audience.as_str()]);

            decode::<Map<String, Value>>(
                &token,
                &DecodingKey::from_rsa_pem(public_key_pem.as_bytes())
                    .map_err(|e| format!("Invalid OAUTH_PUBLIC_KEY_PEM: {}", e))?,
                &validation,
            )
            .map_err(|e| format!("Invalid OAuth token: {}", e))?;
            Ok(())
        }
    }
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Result<String, String> {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Authorization header".to_string())?;
    let prefix = "Bearer ";
    if !auth.starts_with(prefix) {
        return Err("Authorization header must use Bearer token".to_string());
    }
    let token = auth[prefix.len()..].trim();
    if token.is_empty() {
        return Err("Bearer token is empty".to_string());
    }
    Ok(token.to_string())
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
            "protocolVersion": "2025-11-25",
            "capabilities": {
                "tools": {},
                "resources": {
                    "subscribe": false,
                    "listChanged": false
                },
                "prompts": {
                    "listChanged": false
                }
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
                Ok(value) => Ok(call_tool_ok(ctx, value)),
                Err(e) => Ok(call_tool_error(e.to_string())),
            }
        }

        "resources/list" => Ok(serde_json::json!({
            "resources": [
                {
                    "uri": "git://working-directory",
                    "name": "Git Working Directory",
                    "description": "Current session working directory for git operations.",
                    "mimeType": "text/plain"
                }
            ]
        })),

        "resources/read" => {
            let params = req
                .params
                .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            let uri = params
                .get("uri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing uri"))?;
            if uri != "git://working-directory" {
                return Err(anyhow::anyhow!("Resource not found: {}", uri));
            }

            let executor = ctx.executor.read().await;
            let wd = executor
                .get_working_dir()
                .map(|p| p.display().to_string())
                .or_else(|| {
                    ctx.config
                        .git_base_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                })
                .unwrap_or_default();

            Ok(serde_json::json!({
                "contents": [{
                    "uri": "git://working-directory",
                    "mimeType": "text/plain",
                    "text": wd
                }]
            }))
        }

        "prompts/list" => Ok(serde_json::json!({
            "prompts": [
                {
                    "name": "git_wrapup",
                    "title": "Git Wrap-up",
                    "description": "Guides agents through reviewing, documenting, committing, and tagging changes.",
                    "arguments": [
                        {"name": "changelogPath", "description": "Path to CHANGELOG.md", "required": false},
                        {"name": "skipDocumentation", "description": "Skip documentation review/update", "required": false},
                        {"name": "createTag", "description": "Create a tag after wrap-up", "required": false},
                        {"name": "updateAgentFiles", "description": "Update agent meta files", "required": false}
                    ]
                }
            ]
        })),

        "prompts/get" => {
            let params = req
                .params
                .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing name"))?;
            if name != "git_wrapup" {
                return Err(anyhow::anyhow!("Prompt not found: {}", name));
            }

            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let changelog_path = args
                .get("changelogPath")
                .and_then(|v| v.as_str())
                .unwrap_or("CHANGELOG.md");
            let skip_docs = args
                .get("skipDocumentation")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let create_tag = args
                .get("createTag")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let update_agent_files = args
                .get("updateAgentFiles")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let mut text = String::new();
            text.push_str("Follow this wrap-up protocol:\n\n");
            text.push_str("1) Inspect changes with git_diff (include untracked if needed).\n");
            text.push_str("2) Update changelog and docs if applicable.\n");
            text.push_str(&format!(
                "3) Ensure {} is updated appropriately.\n",
                changelog_path
            ));
            if !skip_docs {
                text.push_str("4) Review/update README.md and other documentation as needed.\n");
            } else {
                text.push_str("4) Documentation step skipped (skipDocumentation=true).\n");
            }
            text.push_str("5) Commit changes in atomic units.\n");
            if create_tag {
                text.push_str("6) Create an annotated tag for the release.\n");
            }
            if update_agent_files {
                text.push_str("7) Update agent meta files if your workflow requires it.\n");
            }

            Ok(serde_json::json!({
                "description": "Git Wrap-up prompt",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": text
                    }
                }]
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown method: {}", req.method)),
    }
}

fn call_tool_ok(ctx: &ToolContext, value: Value) -> Value {
    let text = match ctx.config.response_verbosity {
        crate::config::ResponseVerbosity::Minimal => {
            serde_json::to_string(&value).unwrap_or_else(|_| value.to_string())
        }
        _ => serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
    };
    let text = match ctx.config.response_format {
        crate::config::ResponseFormat::Markdown => format!("```json\n{}\n```", text),
        _ => text,
    };
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
        tool_def::<analysis::GitChangelogAnalyzeInput>(
            "git_changelog_analyze",
            "Gather git context for changelog analysis",
        ),
        tool_def::<advanced::GitWrapupInstructionsInput>(
            "git_wrapup_instructions",
            "Get git wrap-up workflow instructions",
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
        "git_changelog_analyze" => {
            let input: analysis::GitChangelogAnalyzeInput = serde_json::from_value(arguments)?;
            let result = analysis::git_changelog_analyze(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        "git_wrapup_instructions" => {
            let input: advanced::GitWrapupInstructionsInput = serde_json::from_value(arguments)?;
            let result = advanced::git_wrapup_instructions(ctx.clone(), input).await?;
            Ok(serde_json::to_value(result)?)
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
    }
}
