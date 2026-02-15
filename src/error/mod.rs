use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitMcpError {
    #[error("Git command failed: {0}")]
    GitCommandFailed(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Invalid repository state: {0}")]
    InvalidState(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    #[error("Merge conflict detected")]
    MergeConflict,

    #[error("Uncommitted changes exist")]
    UncommittedChanges,
}

pub type Result<T> = std::result::Result<T, GitMcpError>;
