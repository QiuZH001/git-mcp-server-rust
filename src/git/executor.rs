use crate::config::Config;
use crate::error::{GitMcpError, Result};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

pub struct GitExecutor {
    config: Arc<Config>,
    working_dir: Option<PathBuf>,
}

impl GitExecutor {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            working_dir: None,
        }
    }

    pub fn set_working_dir(&mut self, path: PathBuf) -> Result<()> {
        let validated = self.config.validate_path(&path)?;
        self.working_dir = Some(validated);
        Ok(())
    }

    pub fn get_working_dir(&self) -> Option<&PathBuf> {
        self.working_dir.as_ref()
    }

    pub fn clear_working_dir(&mut self) {
        self.working_dir = None;
    }

    fn build_command_in_dir(&self, args: &[&str], dir: Option<&Path>) -> Result<Command> {
        let mut cmd = Command::new("git");

        for arg in args {
            cmd.arg(arg);
        }

        if let Some(dir) = dir {
            let validated = self.config.validate_path(dir)?;
            cmd.current_dir(validated);
        } else if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        } else if let Some(base) = &self.config.git_base_dir {
            cmd.current_dir(base);
        }

        if let Some(name) = &self.config.git_username {
            cmd.env("GIT_AUTHOR_NAME", name);
            cmd.env("GIT_COMMITTER_NAME", name);
        }

        if let Some(email) = &self.config.git_email {
            cmd.env("GIT_AUTHOR_EMAIL", email);
            cmd.env("GIT_COMMITTER_EMAIL", email);
        }

        cmd.env("GIT_TERMINAL_PROMPT", "0");

        Ok(cmd)
    }

    pub fn execute(&self, args: &[&str]) -> Result<GitOutput> {
        let mut cmd = self.build_command_in_dir(args, None)?;

        let output = cmd
            .output()
            .map_err(|e| GitMcpError::GitCommandFailed(format!("Failed to execute git: {}", e)))?;

        let stdout = output_to_string(output.stdout);
        let stderr = output_to_string(output.stderr);

        if !output.status.success() {
            return Err(GitMcpError::GitCommandFailed(format!(
                "Git command failed with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(GitOutput {
            stdout,
            stderr,
            status: output.status.code().unwrap_or(-1),
        })
    }

    pub fn execute_in_dir(&self, dir: &Path, args: &[&str]) -> Result<GitOutput> {
        let mut cmd = self.build_command_in_dir(args, Some(dir))?;

        let output = cmd
            .output()
            .map_err(|e| GitMcpError::GitCommandFailed(format!("Failed to execute git: {}", e)))?;

        let stdout = output_to_string(output.stdout);
        let stderr = output_to_string(output.stderr);

        if !output.status.success() {
            return Err(GitMcpError::GitCommandFailed(format!(
                "Git command failed with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(GitOutput {
            stdout,
            stderr,
            status: output.status.code().unwrap_or(-1),
        })
    }

    pub fn execute_with_stdin(&self, args: &[&str], stdin_data: &str) -> Result<GitOutput> {
        let mut cmd = self.build_command_in_dir(args, None)?;
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| GitMcpError::GitCommandFailed(format!("Failed to spawn git: {}", e)))?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_data.as_bytes()).map_err(|e| {
                GitMcpError::GitCommandFailed(format!("Failed to write stdin: {}", e))
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| GitMcpError::GitCommandFailed(format!("Failed to wait for git: {}", e)))?;

        let stdout = output_to_string(output.stdout);
        let stderr = output_to_string(output.stderr);

        if !output.status.success() {
            return Err(GitMcpError::GitCommandFailed(format!(
                "Git command failed with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(GitOutput {
            stdout,
            stderr,
            status: output.status.code().unwrap_or(-1),
        })
    }

    pub fn execute_with_stdin_in_dir(
        &self,
        dir: &Path,
        args: &[&str],
        stdin_data: &str,
    ) -> Result<GitOutput> {
        let mut cmd = self.build_command_in_dir(args, Some(dir))?;
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| GitMcpError::GitCommandFailed(format!("Failed to spawn git: {}", e)))?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_data.as_bytes()).map_err(|e| {
                GitMcpError::GitCommandFailed(format!("Failed to write stdin: {}", e))
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| GitMcpError::GitCommandFailed(format!("Failed to wait for git: {}", e)))?;

        let stdout = output_to_string(output.stdout);
        let stderr = output_to_string(output.stderr);

        if !output.status.success() {
            return Err(GitMcpError::GitCommandFailed(format!(
                "Git command failed with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(GitOutput {
            stdout,
            stderr,
            status: output.status.code().unwrap_or(-1),
        })
    }
}

fn output_to_string(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(&e.into_bytes()).into_owned(),
    }
}

#[derive(Debug, Clone)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

impl GitOutput {
    pub fn lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    pub fn trim(&self) -> &str {
        self.stdout.trim()
    }
}
