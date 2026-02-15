use std::env;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub git_base_dir: Option<PathBuf>,
    pub git_username: Option<String>,
    pub git_email: Option<String>,
    pub git_sign_commits: bool,
    pub git_wrapup_instructions_path: Option<PathBuf>,
    pub log_level: String,
    pub transport_type: TransportType,

    pub http_host: String,
    pub http_port: u16,
    pub http_endpoint_path: String,

    pub session_mode: SessionMode,
    pub response_format: ResponseFormat,
    pub response_verbosity: ResponseVerbosity,

    pub auth_mode: AuthMode,
    pub auth_secret_key: Option<String>,
    pub oauth_issuer_url: Option<String>,
    pub oauth_audience: Option<String>,
    pub oauth_public_key_pem: Option<String>,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportType {
    Stdio,
    Http,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionMode {
    Stateless,
    Stateful,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseFormat {
    Json,
    Markdown,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseVerbosity {
    Minimal,
    Standard,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthMode {
    None,
    Jwt,
    Oauth,
}

impl Default for Config {
    fn default() -> Self {
        let transport_type = match env::var("MCP_TRANSPORT_TYPE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "http" => TransportType::Http,
            _ => TransportType::Stdio,
        };

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
            git_wrapup_instructions_path: env::var("GIT_WRAPUP_INSTRUCTIONS_PATH")
                .ok()
                .map(PathBuf::from),
            log_level: env::var("MCP_LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            transport_type,

            http_host: env::var("MCP_HTTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            http_port: env::var("MCP_HTTP_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(3015),
            http_endpoint_path: env::var("MCP_HTTP_ENDPOINT_PATH")
                .unwrap_or_else(|_| "/mcp".to_string()),

            session_mode: match env::var("MCP_SESSION_MODE")
                .unwrap_or_else(|_| "auto".to_string())
                .to_lowercase()
                .as_str()
            {
                "stateless" => SessionMode::Stateless,
                "stateful" => SessionMode::Stateful,
                _ => SessionMode::Auto,
            },
            response_format: match env::var("MCP_RESPONSE_FORMAT")
                .unwrap_or_else(|_| "json".to_string())
                .to_lowercase()
                .as_str()
            {
                "markdown" => ResponseFormat::Markdown,
                "auto" => ResponseFormat::Auto,
                _ => ResponseFormat::Json,
            },
            response_verbosity: match env::var("MCP_RESPONSE_VERBOSITY")
                .unwrap_or_else(|_| "standard".to_string())
                .to_lowercase()
                .as_str()
            {
                "minimal" => ResponseVerbosity::Minimal,
                "full" => ResponseVerbosity::Full,
                _ => ResponseVerbosity::Standard,
            },

            auth_mode: match env::var("MCP_AUTH_MODE")
                .unwrap_or_else(|_| "none".to_string())
                .to_lowercase()
                .as_str()
            {
                "jwt" => AuthMode::Jwt,
                "oauth" => AuthMode::Oauth,
                _ => AuthMode::None,
            },
            auth_secret_key: env::var("MCP_AUTH_SECRET_KEY").ok(),
            oauth_issuer_url: env::var("OAUTH_ISSUER_URL").ok(),
            oauth_audience: env::var("OAUTH_AUDIENCE").ok(),
            oauth_public_key_pem: env::var("OAUTH_PUBLIC_KEY_PEM").ok(),
            allowed_origins: env::var("MCP_ALLOWED_ORIGINS")
                .ok()
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default(),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn validate_path(&self, path: &Path) -> crate::error::Result<PathBuf> {
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
