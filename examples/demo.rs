use alloy::primitives::address;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use tradesync::HyperliquidMonitor;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new("info"))
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| anyhow::anyhow!("failed to set global tracing subscriber: {e}"))?;

    info!("--- Starting Hyperliquid monitor demo ---");

    // Standard wallet address for demo
    let wallet = address!("0xc64cc00b46101bd40aa1c3121195e85c0b0918d8");
    let monitor = HyperliquidMonitor::new(wallet, true);

    let (tx, mut rx) = mpsc::unbounded_channel();

    // Spawn the monitor in a background task
    tokio::spawn(async move {
        if let Err(e) = monitor.run(tx).await {
            tracing::error!("Monitor error: {:?}", e);
        }
    });

    info!("Listening for position trade events. Close with Ctrl+C...");
    while let Some(event) = rx.recv().await {
        info!(
            "DEMO RECEIVED POSITION TRADE: coin={}, side={}, px={}, sz={}, action={:?}, start_pos={}, end_pos={}, time={}, tid={}",
            event.coin,
            event.side,
            event.px,
            event.sz,
            event.action,
            event.start_pos,
            event.end_pos,
            event.time,
            event.tid
        );
    }

    Ok(())
}
