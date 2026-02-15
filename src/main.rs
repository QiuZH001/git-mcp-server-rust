pub mod error;
pub mod config;
pub mod git;
pub mod tools;

use config::Config;

mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .with_writer(std::io::stderr)
        .init();

    let config = Config::from_env();
    
    tracing::info!("Starting Git MCP Server");
    tracing::debug!("Config: {:?}", config);

    server::run_server(config).await?;

    Ok(())
}
