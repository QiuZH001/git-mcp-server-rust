# AGENTS.md

## Project Overview

Git MCP Server (Rust) - A Model Context Protocol server providing 26 Git tools for AI agents.

## Build Commands

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Check for compilation errors (fast)
cargo check

# Run the server
cargo run
# or directly:
./target/release/git-mcp-server
```

## Test Commands

```bash
# Run all tests
cargo test

# Run integration tests only
cargo test --test integration_test

# Run a single test
cargo test --test integration_test test_git_status
cargo test --test integration_test test_git_commit

# Run tests with output
cargo test -- --nocapture

# Run specific test by pattern
cargo test -p git-mcp-server test_git_log
```

## Lint and Format

```bash
# Check for warnings
cargo clippy

# Format code
cargo fmt

# Check formatting without changes
cargo fmt -- --check
```

## Project Structure

```
src/
├── main.rs           # Entry point, logging setup
├── server.rs         # MCP STDIO server implementation
├── config/mod.rs     # Configuration from environment
├── error/mod.rs      # Custom error types
├── git/
│   ├── mod.rs        # Git module exports
│   └── executor.rs   # Git CLI wrapper
└── tools/
    ├── mod.rs        # ToolContext, module exports
    ├── repo.rs       # init, clone, status, clean
    ├── staging.rs    # add, commit, diff
    ├── history.rs    # log, show, blame, reflog
    ├── branching.rs  # branch, checkout, merge, rebase, cherry-pick
    ├── remote.rs     # remote, fetch, pull, push
    └── advanced.rs   # tag, stash, reset, worktree, set/clear_working_dir

tests/
└── integration_test.rs  # 19 integration tests
```

## Code Style Guidelines

### Imports

```rust
// Order: external crates first, then internal modules
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

use crate::config::Config;
use crate::error::{GitMcpError, Result};
use crate::tools::ToolContext;
```

### Naming Conventions

- **Modules**: `snake_case` (e.g., `git_executor`, `tool_context`)
- **Types/Structs**: `PascalCase` (e.g., `GitStatusInput`, `ToolContext`)
- **Functions**: `snake_case` (e.g., `git_status`, `execute_command`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Async functions**: Same as sync, prefixed with `async`

### Struct Definitions

```rust
// Input structs: derive Debug, Clone, Serialize, Deserialize, JsonSchema
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusInput {
    #[schemars(description = "Path to the git repository")]
    pub path: Option<String>,
    
    #[schemars(description = "Include untracked files")]
    pub include_untracked: Option<bool>,
}

// Output structs: include success field
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusOutput {
    pub success: bool,
    pub branch: Option<String>,
    // ... other fields
}
```

### Function Signatures

```rust
// Tool functions are async and take ToolContext + Input
pub async fn git_status(ctx: ToolContext, input: GitStatusInput) -> Result<GitStatusOutput> {
    let executor = ctx.executor.read().await;
    // implementation
    Ok(result)
}
```

### Error Handling

```rust
// Use the crate's Result type (alias for Result<T, GitMcpError>)
use crate::error::Result;

// Use ? for error propagation
let output = executor.execute(&args)?;

// Use map_err for custom error messages
path.canonicalize()
    .map_err(|_| GitMcpError::InvalidPath(path.display().to_string()))?;
```

### Git Command Execution

```rust
// Build args as Vec<String> for dynamic values
let mut args: Vec<String> = vec!["log".into(), "--format=%H".into()];

if let Some(n) = input.max_count {
    args.push("-n".into());
    args.push(n.to_string());
}

// Convert to &str slice for execution
let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
let output = executor.execute(&args_refs)?;
```

### MCP Tool Registration

Tools are registered in `src/server.rs`. To add a new tool:

1. Create Input/Output structs in appropriate `src/tools/*.rs`
2. Implement the async function
3. Add to `get_tool_definitions()` in server.rs
4. Add to `execute_tool()` match statement

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `GIT_BASE_DIR` | Restrict operations to this directory |
| `GIT_USERNAME` | Git author name |
| `GIT_EMAIL` | Git author email |
| `GIT_SIGN_COMMITS` | Enable commit signing |
| `MCP_LOG_LEVEL` | Log level (debug, info, warn, error) |
| `MCP_TRANSPORT_TYPE` | Transport (stdio, http) |

## Key Dependencies

- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `schemars` - JSON Schema generation
- `thiserror` - Custom error types
- `anyhow` - Error handling in main
- `tracing` - Logging
- `rmcp` - MCP SDK

## Testing Guidelines

- Tests create temporary git repositories using `tempfile`
- Use `TestServer` helper for STDIO communication
- Always set working directory before testing git operations
- Configure git user before commits:

```rust
Command::new("git").args(["config", "user.email", "test@test.com"])
    .current_dir(temp_dir.path())
    .output()
    .expect("Failed to config");
```
