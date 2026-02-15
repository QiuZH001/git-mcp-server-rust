use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
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

struct HttpTestServer {
    child: std::process::Child,
    host: String,
    port: u16,
    endpoint_path: String,
}

impl HttpTestServer {
    fn new(session_mode: Option<&str>) -> Self {
        Self::new_with_env(session_mode, &[])
    }

    fn new_with_env(session_mode: Option<&str>, extra_env: &[(&str, &str)]) -> Self {
        let binary = get_binary_path();
        let host = "127.0.0.1".to_string();
        let port = reserve_free_port();
        let endpoint_path = "/mcp".to_string();

        let mut cmd = Command::new(&binary);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("MCP_TRANSPORT_TYPE", "http")
            .env("MCP_HTTP_HOST", &host)
            .env("MCP_HTTP_PORT", port.to_string())
            .env("MCP_HTTP_ENDPOINT_PATH", &endpoint_path);

        if let Some(mode) = session_mode {
            cmd.env("MCP_SESSION_MODE", mode);
        }
        for (k, v) in extra_env {
            cmd.env(k, v);
        }

        let child = cmd.spawn().expect("Failed to start HTTP server");

        let server = Self {
            child,
            host,
            port,
            endpoint_path,
        };
        server.wait_ready();
        server
    }

    fn wait_ready(&self) {
        for _ in 0..100 {
            if TcpStream::connect((self.host.as_str(), self.port)).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("HTTP server did not become ready in time");
    }

    fn send(
        &self,
        body: &str,
        extra_headers: &[(&str, &str)],
    ) -> (u16, HashMap<String, String>, String) {
        let mut raw = String::new();
        for attempt in 0..5 {
            let connect = TcpStream::connect((self.host.as_str(), self.port));
            let mut stream = match connect {
                Ok(s) => s,
                Err(e) => {
                    if attempt == 4 {
                        panic!("Failed to connect HTTP server: {}", e);
                    }
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };

            let mut req = format!(
                "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nConnection: close\r\nContent-Length: {}\r\n",
                self.endpoint_path,
                self.host,
                self.port,
                body.len()
            );
            for (k, v) in extra_headers {
                req.push_str(&format!("{}: {}\r\n", k, v));
            }
            req.push_str("\r\n");
            req.push_str(body);

            if let Err(e) = stream.write_all(req.as_bytes()) {
                if attempt == 4 {
                    panic!("Failed to write HTTP request: {}", e);
                }
                thread::sleep(Duration::from_millis(20));
                continue;
            }
            if let Err(e) = stream.flush() {
                if attempt == 4 {
                    panic!("Failed to flush HTTP request: {}", e);
                }
                thread::sleep(Duration::from_millis(20));
                continue;
            }

            raw.clear();
            match stream.read_to_string(&mut raw) {
                Ok(_) if !raw.is_empty() => break,
                Ok(_) => {
                    if attempt == 4 {
                        panic!("Received empty HTTP response after retries");
                    }
                }
                Err(e) => {
                    if attempt == 4 {
                        panic!("Failed to read HTTP response: {}", e);
                    }
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        let (head, body) = raw.split_once("\r\n\r\n").expect("Malformed HTTP response");
        let mut lines = head.lines();
        let status_line = lines.next().unwrap_or("HTTP/1.1 500 Internal Server Error");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(500);

        let mut headers = HashMap::new();
        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }

        (status, headers, body.to_string())
    }
}

#[derive(serde::Serialize)]
struct JwtClaims {
    sub: String,
    exp: usize,
}

impl Drop for HttpTestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn reserve_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    let port = listener
        .local_addr()
        .expect("Failed to get local addr")
        .port();
    drop(listener);
    port
}

fn get_binary_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest_dir)
        .join("target")
        .join("debug")
        .join("git-mcp-server")
}

fn init_repo(path: &std::path::Path) {
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(path)
        .output()
        .expect("Failed to init repo");
}

fn config_user(path: &std::path::Path) {
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(path)
        .output()
        .expect("Failed to config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(path)
        .output()
        .expect("Failed to config name");
}

fn commit_file(path: &std::path::Path, file: &str, content: &str, message: &str) {
    std::fs::write(path.join(file), content).expect("Failed to write file");

    Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(path)
        .output()
        .expect("Failed to commit");
}

fn tag_repo(path: &std::path::Path, tag_name: &str) {
    Command::new("git")
        .args(["tag", "-a", tag_name, "-m", "tag"])
        .current_dir(path)
        .output()
        .expect("Failed to create tag");
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

#[test]
fn test_tools_list_schema_no_definitions_ref() {
    let mut server = TestServer::new();
    let request = r#"{"jsonrpc":"2.0","id":20,"method":"tools/list","params":{}}"#;
    let response = server.send(request);
    assert!(
        !response.contains("#/definitions/"),
        "tools/list inputSchema should not contain unresolved $ref: {}",
        response
    );
}

#[test]
fn test_git_status_with_path_argument() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    init_repo(temp_dir.path());

    let mut server = TestServer::new();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "tools/call",
        "params": {
            "name": "git_status",
            "arguments": {
                "path": repo_path
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
fn test_git_clone() {
    let src_dir = TempDir::new().unwrap();
    init_repo(src_dir.path());
    config_user(src_dir.path());
    commit_file(src_dir.path(), "test.txt", "hello", "Initial commit");

    let dst_parent = TempDir::new().unwrap();
    let clone_path = dst_parent.path().join("cloned");
    let clone_path_str = clone_path.to_string_lossy().to_string();

    let mut server = TestServer::new();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "tools/call",
        "params": {
            "name": "git_clone",
            "arguments": {
                "url": src_dir.path().to_string_lossy().to_string(),
                "local_path": clone_path_str
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
    assert!(
        clone_path.join(".git").exists(),
        "Cloned repo should have .git"
    );
}

#[test]
fn test_git_push_fetch_pull() {
    let remote_dir = TempDir::new().unwrap();
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote_dir.path())
        .output()
        .expect("Failed to init bare repo");

    let local_dir = TempDir::new().unwrap();
    init_repo(local_dir.path());
    config_user(local_dir.path());
    commit_file(local_dir.path(), "test.txt", "hello", "Initial commit");

    let local_repo_path = local_dir.path().to_string_lossy().to_string();
    let remote_path = remote_dir.path().to_string_lossy().to_string();

    let mut server = TestServer::new();
    server.set_working_dir(&local_repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 23,
        "method": "tools/call",
        "params": {
            "name": "git_remote",
            "arguments": {
                "mode": "add",
                "name": "origin",
                "url": remote_path
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
        "id": 24,
        "method": "tools/call",
        "params": {
            "name": "git_push",
            "arguments": {
                "remote": "origin",
                "branch": "main",
                "set_upstream": true
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

    let other_dir = TempDir::new().unwrap();
    Command::new("git")
        .args(["clone", remote_dir.path().to_string_lossy().as_ref(), "."])
        .current_dir(other_dir.path())
        .output()
        .expect("Failed to clone remote");
    config_user(other_dir.path());
    commit_file(other_dir.path(), "test2.txt", "world", "Second commit");
    Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(other_dir.path())
        .output()
        .expect("Failed to push from other clone");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 25,
        "method": "tools/call",
        "params": {
            "name": "git_fetch",
            "arguments": {
                "remote": "origin"
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
        "id": 26,
        "method": "tools/call",
        "params": {
            "name": "git_pull",
            "arguments": {
                "remote": "origin",
                "branch": "main"
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
fn test_git_merge() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    config_user(temp_dir.path());
    commit_file(temp_dir.path(), "test.txt", "hello", "Initial commit");

    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create branch");
    commit_file(temp_dir.path(), "feature.txt", "feat", "Feature commit");

    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to checkout main");

    let repo_path = temp_dir.path().to_string_lossy().to_string();
    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 27,
        "method": "tools/call",
        "params": {
            "name": "git_merge",
            "arguments": {
                "branch": "feature",
                "no_fast_forward": true
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
fn test_git_rebase() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    config_user(temp_dir.path());
    commit_file(temp_dir.path(), "test.txt", "hello", "Initial commit");

    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create branch");
    commit_file(temp_dir.path(), "feature.txt", "feat", "Feature commit");

    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to checkout main");
    commit_file(temp_dir.path(), "main.txt", "main", "Main commit");

    let repo_path = temp_dir.path().to_string_lossy().to_string();
    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 28,
        "method": "tools/call",
        "params": {
            "name": "git_rebase",
            "arguments": {
                "upstream": "main",
                "branch": "feature"
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
fn test_git_cherry_pick() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    config_user(temp_dir.path());
    commit_file(temp_dir.path(), "test.txt", "hello", "Initial commit");

    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create branch");
    commit_file(temp_dir.path(), "feature.txt", "feat", "Feature commit");

    let feature_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to get commit hash");
    let feature_hash = String::from_utf8_lossy(&feature_hash.stdout)
        .trim()
        .to_string();

    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to checkout main");

    let repo_path = temp_dir.path().to_string_lossy().to_string();
    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 29,
        "method": "tools/call",
        "params": {
            "name": "git_cherry_pick",
            "arguments": {
                "commits": [feature_hash]
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
fn test_git_blame() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    config_user(temp_dir.path());
    commit_file(
        temp_dir.path(),
        "test.txt",
        "hello\nworld\n",
        "Initial commit",
    );

    let repo_path = temp_dir.path().to_string_lossy().to_string();
    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "tools/call",
        "params": {
            "name": "git_blame",
            "arguments": {
                "file": "test.txt",
                "start_line": 1,
                "end_line": 2
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
fn test_git_clear_working_dir() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    init_repo(temp_dir.path());

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 31,
        "method": "tools/call",
        "params": {
            "name": "git_clear_working_dir",
            "arguments": {
                "confirm": "Y"
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
fn test_resources_list_and_read_working_directory() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = r#"{"jsonrpc":"2.0","id":40,"method":"resources/list","params":{}}"#;
    let response = server.send(request);
    assert!(
        response.contains("git://working-directory"),
        "resources/list should include working dir resource: {}",
        response
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 41,
        "method": "resources/read",
        "params": {
            "uri": "git://working-directory"
        }
    })
    .to_string();
    let response = server.send(&request);
    assert!(
        response.contains(&repo_path),
        "resources/read should include current working dir: {}",
        response
    );
}

#[test]
fn test_prompts_list_and_get_git_wrapup() {
    let mut server = TestServer::new();
    let request = r#"{"jsonrpc":"2.0","id":42,"method":"prompts/list","params":{}}"#;
    let response = server.send(request);
    assert!(
        response.contains("git_wrapup"),
        "prompts/list should include git_wrapup: {}",
        response
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 43,
        "method": "prompts/get",
        "params": {
            "name": "git_wrapup",
            "arguments": {
                "changelogPath": "CHANGELOG.md",
                "skipDocumentation": true,
                "createTag": true,
                "updateAgentFiles": true
            }
        }
    })
    .to_string();
    let response = server.send(&request);
    assert!(
        response.contains("wrap-up"),
        "prompts/get should return wrap-up content: {}",
        response
    );
}

#[test]
fn test_git_wrapup_instructions_tool() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    config_user(temp_dir.path());
    commit_file(temp_dir.path(), "test.txt", "hello", "Initial commit");
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    let mut server = TestServer::new();
    server.set_working_dir(&repo_path);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 44,
        "method": "tools/call",
        "params": {
            "name": "git_wrapup_instructions",
            "arguments": {
                "acknowledgement": "Y",
                "updateAgentMetaFiles": "Yes",
                "createTag": true
            }
        }
    })
    .to_string();
    let response = server.send(&request);
    assert!(
        response.contains("instructions"),
        "wrapup tool should return instructions: {}",
        response
    );
}

#[test]
fn test_git_changelog_analyze_tool() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    config_user(temp_dir.path());
    commit_file(temp_dir.path(), "a.txt", "a", "Initial commit");
    tag_repo(temp_dir.path(), "v0.1.0");
    commit_file(temp_dir.path(), "b.txt", "b", "Second commit");

    let repo_path = temp_dir.path().to_string_lossy().to_string();

    let mut server = TestServer::new();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 45,
        "method": "tools/call",
        "params": {
            "name": "git_changelog_analyze",
            "arguments": {
                "path": repo_path,
                "reviewTypes": ["gaps", "quality"],
                "maxCommits": 10,
                "sinceTag": "v0.1.0"
            }
        }
    })
    .to_string();

    let response = server.send(&request);
    assert!(
        response.contains("gitContext"),
        "changelog analyze should return gitContext: {}",
        response
    );
}

#[test]
fn test_http_initialize_and_resource_read_with_session() {
    let temp_dir = TempDir::new().unwrap();
    init_repo(temp_dir.path());
    let repo_path = temp_dir.path().to_string_lossy().to_string();

    let server = HttpTestServer::new(Some("stateful"));

    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    })
    .to_string();

    let (status, headers, body) = server.send(&initialize, &[]);
    assert_eq!(status, 200, "initialize HTTP status should be 200");
    assert!(
        body.contains("protocolVersion"),
        "initialize should return protocolVersion: {}",
        body
    );
    assert_eq!(
        headers.get("content-type").map(|s| s.as_str()),
        Some("application/json"),
        "initialize should return application/json content-type"
    );
    assert_eq!(
        headers.get("mcp-protocol-version").map(|s| s.as_str()),
        Some("2025-11-25"),
        "initialize should return MCP-Protocol-Version"
    );

    let session_id = headers
        .get("mcp-session-id")
        .cloned()
        .expect("initialize should include MCP-Session-Id header");

    let set_wd = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "git_set_working_dir",
            "arguments": {
                "path": repo_path
            }
        }
    })
    .to_string();
    let (_, _, set_body) = server.send(&set_wd, &[("MCP-Session-Id", &session_id)]);
    assert!(
        set_body.contains("success"),
        "git_set_working_dir over HTTP should succeed: {}",
        set_body
    );

    let read_resource = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "resources/read",
        "params": {
            "uri": "git://working-directory"
        }
    })
    .to_string();
    let (_, _, read_body) = server.send(&read_resource, &[("MCP-Session-Id", &session_id)]);
    assert!(
        read_body.contains(&temp_dir.path().to_string_lossy().to_string()),
        "resources/read should return session working directory: {}",
        read_body
    );
}

#[test]
fn test_http_auth_jwt_mode() {
    let secret = "test-secret-key-123456789";
    let server = HttpTestServer::new_with_env(
        Some("stateful"),
        &[("MCP_AUTH_MODE", "jwt"), ("MCP_AUTH_SECRET_KEY", secret)],
    );

    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    })
    .to_string();

    let (status_no_auth, _, body_no_auth) = server.send(&initialize, &[]);
    assert_eq!(status_no_auth, 401, "JWT mode without auth should be 401");
    assert!(
        body_no_auth.contains("Missing Authorization header"),
        "JWT mode should explain missing auth header: {}",
        body_no_auth
    );

    let claims = JwtClaims {
        sub: "test-client".to_string(),
        exp: 4_102_444_800,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("Failed to create jwt token");
    let auth = format!("Bearer {}", token);

    let (status_with_auth, _, body_with_auth) =
        server.send(&initialize, &[("Authorization", &auth)]);
    assert_eq!(
        status_with_auth, 200,
        "JWT mode with valid auth should be 200"
    );
    assert!(
        body_with_auth.contains("protocolVersion"),
        "initialize should succeed with valid jwt auth: {}",
        body_with_auth
    );
}

#[test]
fn test_http_origin_allowlist() {
    let server = HttpTestServer::new_with_env(
        Some("stateless"),
        &[("MCP_ALLOWED_ORIGINS", "https://allowed.example")],
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/list",
        "params": {}
    })
    .to_string();

    let (status_forbidden, _, body_forbidden) =
        server.send(&request, &[("Origin", "https://blocked.example")]);
    assert_eq!(status_forbidden, 403, "blocked origin should be 403");
    assert!(
        body_forbidden.contains("Forbidden origin"),
        "blocked origin should return forbidden message: {}",
        body_forbidden
    );

    let (status_allowed, _, body_allowed) =
        server.send(&request, &[("Origin", "https://allowed.example")]);
    assert_eq!(status_allowed, 200, "allowed origin should pass");
    assert!(
        body_allowed.contains("tools"),
        "allowed origin should get tools/list response: {}",
        body_allowed
    );
}
