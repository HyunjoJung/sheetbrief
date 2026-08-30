use sheetbrief_mcp::build_router;
use std::{net::SocketAddr, str::FromStr};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sheetbrief_mcp=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind = std::env::var("SHEETBRIEF_BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let address = SocketAddr::from_str(&bind)
        .map_err(|error| format!("invalid SHEETBRIEF_BIND {bind:?}: {error}"))?;
    let token = std::env::var("SHEETBRIEF_API_TOKEN").ok();
    if token.as_ref().is_some_and(|token| token.len() < 32) {
        return Err("SHEETBRIEF_API_TOKEN must contain at least 32 characters".into());
    }
    if !address.ip().is_loopback() && token.is_none() {
        return Err("a non-loopback bind requires SHEETBRIEF_API_TOKEN".into());
    }

    let cancellation_token = CancellationToken::new();
    let app = build_router(cancellation_token.child_token(), token);
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(address = %listener.local_addr()?, "SheetBrief MCP listening");
    axum::serve(listener, app)
        .with_graceful_shutdown({
            let cancellation_token = cancellation_token.clone();
            async move {
                let _ = tokio::signal::ctrl_c().await;
                cancellation_token.cancel();
            }
        })
        .await?;
    Ok(())
}
