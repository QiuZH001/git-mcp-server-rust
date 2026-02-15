use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::TempDir;

struct TestServer {
    child: std::process::Child,
    reader: BufReader<std::process::ChildStdout>,
}

impl TestServer {
    fn new() -> Self {
        let binary = get_binary_path();
        let mut child = Command::new(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start server");

        let reader = BufReader::new(child.stdout.take().unwrap());
        Self { child, reader }
    }

    fn send(&mut self, request: &str) -> String {
        self.child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
        self.child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();

        let mut response = String::new();
        self.reader
            .read_line(&mut response)
            .expect("Failed to read response");
        response
    }

    fn set_working_dir(&mut self, path: &str) -> String {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "tools/call",
            "params": {
                "name": "git_set_working_dir",
                "arguments": {
                    "path": path
                }
            }
        })
        .to_string();
        self.send(&request)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn get_binary_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest_dir)
        .join("target")
        .join("debug")
        .join("git-mcp-server")
}

#[test]
fn test_initialize() {
    let mut server = TestServer::new();
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
    let response = server.send(request);

    assert!(
        response.contains("git-mcp-server"),
        "Response should contain server name"
    );
}

#[test]
fn test_tools_list() {
    let mut server = TestServer::new();
    let request = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    let response = server.send(request);

    let tools = vec![
        "git_status",
        "git_init",
        "git_clone",
        "git_add",
        "git_commit",
        "git_log",
        "git_branch",
        "git_push",
        "git_pull",
        "git_fetch",
        "git_remote",
        "git_tag",
        "git_stash",
        "git_reset",
        "git_worktree",
        "git_diff",
        "git_show",
        "git_blame",
        "git_reflog",
        "git_checkout",
        "git_merge",
        "git_rebase",
        "git_cherry_pick",
        "git_clean",
        "git_set_working_dir",
        "git_clear_working_dir",
    ];

    for tool in tools {
        assert!(response.contains(tool), "Should include {} tool", tool);
    }
}

#[test]
fn test_git_init() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    let mut server = TestServer::new();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "git_init",
            "arguments": {
                "path": repo_path,
                "initial_branch": "main"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_status() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "git_status",
            "arguments": {}
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_add() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "git_add",
            "arguments": {
                "files": ["test.txt"]
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_commit() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    std::fs::write(temp_dir.path().join("test2.txt"), "world").expect("Failed to write file");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "git_commit",
            "arguments": {
                "message": "Second commit",
                "files_to_stage": ["test2.txt"]
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_log() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "git_log",
            "arguments": {
                "max_count": 10
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_branch() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "git_branch",
            "arguments": {
                "operation": "list"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_diff() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello\n").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    std::fs::write(temp_dir.path().join("test.txt"), "hello\nworld\n")
        .expect("Failed to write file");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "git_diff",
            "arguments": {}
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_tag() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "git_tag",
            "arguments": {
                "mode": "list"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_stash() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    std::fs::write(temp_dir.path().join("test.txt"), "modified").expect("Failed to modify file");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "git_stash",
            "arguments": {
                "mode": "push",
                "message": "test stash"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_remote() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "tools/call",
        "params": {
            "name": "git_remote",
            "arguments": {
                "mode": "list"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_show() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 13,
        "method": "tools/call",
        "params": {
            "name": "git_show",
            "arguments": {}
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_reflog() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 14,
        "method": "tools/call",
        "params": {
            "name": "git_reflog",
            "arguments": {}
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_worktree() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 15,
        "method": "tools/call",
        "params": {
            "name": "git_worktree",
            "arguments": {
                "mode": "list"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_set_working_dir() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    let mut server = TestServer::new();

    let response = server.set_working_dir(&repo_path);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_checkout() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 16,
        "method": "tools/call",
        "params": {
            "name": "git_branch",
            "arguments": {
                "operation": "create",
                "name": "feature"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 17,
        "method": "tools/call",
        "params": {
            "name": "git_checkout",
            "arguments": {
                "target": "feature"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_clean() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    std::fs::write(temp_dir.path().join("untracked.txt"), "hello").expect("Failed to write file");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 18,
        "method": "tools/call",
        "params": {
            "name": "git_clean",
            "arguments": {
                "force": false,
                "dry_run": true
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}

#[test]
fn test_git_reset() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to init repo");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to config name");

    std::fs::write(temp_dir.path().join("test.txt"), "hello").expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    std::fs::write(temp_dir.path().join("test.txt"), "hello\nworld\n")
        .expect("Failed to modify file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Second commit"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 19,
        "method": "tools/call",
        "params": {
            "name": "git_reset",
            "arguments": {
                "mode": "soft",
                "target": "HEAD~1",
                "confirmed": false
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("success"),
        "Response should contain success: {}",
        response
    );
}
