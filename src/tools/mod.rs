pub mod advanced;
pub mod analysis;
pub mod branching;
pub mod history;
pub mod remote;
pub mod repo;
pub mod staging;

use crate::config::Config;
use crate::git::GitExecutor;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ToolContext {
    pub config: Arc<Config>,
    pub executor: Arc<RwLock<GitExecutor>>,
}

impl ToolContext {
    pub fn new(config: Config) -> Self {
        let config = Arc::new(config);
        let executor = GitExecutor::new(config.clone());
        Self {
            config,
            executor: Arc::new(RwLock::new(executor)),
        }
    }

    pub fn from_shared(config: Arc<Config>) -> Self {
        let executor = GitExecutor::new(config.clone());
        Self {
            config,
            executor: Arc::new(RwLock::new(executor)),
        }
    }
}
