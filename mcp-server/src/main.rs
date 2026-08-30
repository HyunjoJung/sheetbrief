use sheetbrief_mcp::{build_router_with_config, ServerConfig};
use std::{io, net::SocketAddr, str::FromStr};
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
    let config = ServerConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let cancellation_token = CancellationToken::new();
    let app = build_router_with_config(cancellation_token.child_token(), token, config);
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(address = %listener.local_addr()?, "SheetBrief MCP listening");
    axum::serve(listener, app)
        .with_graceful_shutdown({
            let cancellation_token = cancellation_token.clone();
            async move {
                shutdown_signal().await;
                cancellation_token.cancel();
            }
        })
        .await?;
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
