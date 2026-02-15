use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub git_base_dir: Option<PathBuf>,
    pub git_username: Option<String>,
    pub git_email: Option<String>,
    pub git_sign_commits: bool,
    pub log_level: String,
    pub transport_type: TransportType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportType {
    Stdio,
    Http,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            git_base_dir: env::var("GIT_BASE_DIR").ok().map(PathBuf::from),
            git_username: env::var("GIT_USERNAME")
                .or_else(|_| env::var("GIT_AUTHOR_NAME"))
                .or_else(|_| env::var("GIT_USER"))
                .ok(),
            git_email: env::var("GIT_EMAIL")
                .or_else(|_| env::var("GIT_AUTHOR_EMAIL"))
                .or_else(|_| env::var("GIT_USER_EMAIL"))
                .ok(),
            git_sign_commits: env::var("GIT_SIGN_COMMITS")
                .unwrap_or_default()
                .to_lowercase()
                == "true",
            log_level: env::var("MCP_LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            transport_type: match env::var("MCP_TRANSPORT_TYPE")
                .unwrap_or_default()
                .to_lowercase()
                .as_str()
            {
                "http" => TransportType::Http,
                _ => TransportType::Stdio,
            },
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn validate_path(&self, path: &PathBuf) -> crate::error::Result<PathBuf> {
        let canonical = path
            .canonicalize()
            .map_err(|_| crate::error::GitMcpError::InvalidPath(path.display().to_string()))?;

        if let Some(base) = &self.git_base_dir {
            let base_canonical = base
                .canonicalize()
                .map_err(|_| crate::error::GitMcpError::InvalidPath(base.display().to_string()))?;

            if !canonical.starts_with(&base_canonical) {
                return Err(crate::error::GitMcpError::InvalidPath(format!(
                    "Path {} is outside allowed directory {}",
                    path.display(),
                    base.display()
                )));
            }
        }

        Ok(canonical)
    }
}
