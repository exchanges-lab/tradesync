use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use dotenvy::dotenv;
use std::env;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use tradesync::{HyperliquidMonitor, NotionRowData, NotionWriter, SymbolRedirects};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env if present
    let _ = dotenv();

    // Initialize tracing subscriber
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| anyhow::anyhow!("failed to set global tracing subscriber: {e}"))?;

    info!("Starting Hyperliquid to Notion Sync Service...");

    // Retrieve WALLET_ADDRESS and IS_TESTNET from environment
    let wallet_str =
        env::var("WALLET_ADDRESS").context("Missing WALLET_ADDRESS in environment variables")?;
    let wallet = wallet_str
        .parse::<alloy::primitives::Address>()
        .map_err(|e| anyhow::anyhow!("Invalid WALLET_ADDRESS: {e}"))?;

    let is_testnet_str = env::var("IS_TESTNET").unwrap_or_else(|_| "true".to_string());
    let is_testnet = is_testnet_str.parse::<bool>().unwrap_or(true);

    // Retrieve Notion configurations from environment
    let notion_token =
        env::var("NOTION_API_KEY").context("Missing NOTION_API_KEY in environment variables")?;
    let notion_database_id = env::var("NOTION_DATABASE_ID")
        .context("Missing NOTION_DATABASE_ID in environment variables")?;

    // Retrieve TradeSnap configurations from environment
    let enable_screenshot_str =
        env::var("ENABLE_SCREENSHOT").unwrap_or_else(|_| "false".to_string());
    let enable_screenshot = enable_screenshot_str.parse::<bool>().unwrap_or(false);
    let tradesnap_url = env::var("TRADESNAP_URL").ok();

    let btcusdt_snapshot = env::var("BTCUSDT_SNAPSHOT")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let redirect_path = env::var("REDIRECT_PATH").ok().map(PathBuf::from);
    let symbol_redirects = SymbolRedirects::load(redirect_path.as_deref())
        .context("Failed to load symbol redirects")?;
    if let Some(path) = &redirect_path {
        info!(path = %path.display(), "Loaded symbol redirects");
    }

    let snapshot_15m = env::var("SYMBOL_15M_SNAPSHOT")
        .or_else(|_| env::var("SYMBOL_15m_SNAPSHOT"))
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let snapshot_1h = env::var("SYMBOL_1H_SNAPSHOT")
        .or_else(|_| env::var("SYMBOL_1h_SNAPSHOT"))
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let snapshot_4h = env::var("SYMBOL_4H_SNAPSHOT")
        .or_else(|_| env::var("SYMBOL_4h_SNAPSHOT"))
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let snapshot_1d = env::var("SYMBOL_1D_SNAPSHOT")
        .or_else(|_| env::var("SYMBOL_1d_SNAPSHOT"))
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    // Initialize monitor and writer
    let monitor = HyperliquidMonitor::new(wallet, is_testnet);
    let writer = NotionWriter::new(
        notion_token,
        notion_database_id,
        enable_screenshot,
        tradesnap_url,
        snapshot_15m,
        snapshot_1h,
        snapshot_4h,
        snapshot_1d,
    )
    .context("Failed to initialize NotionWriter")?;

    let (tx, mut rx) = mpsc::unbounded_channel();

    // Spawn the monitor loop in the background
    tokio::spawn(async move {
        if let Err(e) = monitor.run(tx).await {
            error!("Monitor runtime error: {:?}", e);
        }
    });

    info!("Service running. Monitoring position openings and writing to Notion...");
    while let Some(event) = rx.recv().await {
        // Format Date/Time to "YYYY/MM/DD HH:MM"
        let dt = Utc
            .timestamp_millis_opt(event.time as i64)
            .single()
            .unwrap_or_else(Utc::now);
        let date_time_str = dt.format("%Y/%m/%d %H:%M").to_string();

        let resolved_symbol = symbol_redirects.resolve(&event.coin, btcusdt_snapshot);

        // Map Direction: B -> Long, S -> Short
        let direction_str = if event.side == "B" {
            "Long".to_string()
        } else {
            "Short".to_string()
        };

        // Parse quantity and price to calculate USD value
        let sz: f64 = event.sz.parse().unwrap_or(0.0);
        let px: f64 = event.px.parse().unwrap_or(0.0);
        let usd_val = sz * px;

        // Determine Order Type using NotionRowData helper
        let order_type_str = NotionRowData::determine_order_type(event.crossed);

        info!(
            coin = %event.coin,
            side = %event.side,
            px = %event.px,
            sz = %event.sz,
            usd_val = %format!("{:.2}", usd_val),
            order_type = %order_type_str,
            oid = event.oid,
            "Captured position open event, writing to Notion..."
        );

        // Construct Notion Row Data
        let row = NotionRowData {
            symbol: resolved_symbol.notion_symbol,
            quantity: event.sz,
            filled_price: event.px,
            direction: direction_str,
            exchange: "Hyperliquid".to_string(), // Can be dynamically set or config-driven in the future
            date_time: date_time_str,
            time: event.time,
            order_id: event.oid,
            check: false,
            order_type: order_type_str,
            snapshot_ticker: resolved_symbol.snapshot_ticker,
        };

        // Write row to Notion
        if let Err(e) = writer.write_row(row).await {
            error!("Failed to write row to Notion: {:?}", e);
        }
    }

    Ok(())
}
