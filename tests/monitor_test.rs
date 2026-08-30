use alloy::primitives::address;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tradesync::HyperliquidMonitor;

#[tokio::test]
async fn test_monitor_initialization() {
    // Standard test wallet address
    let wallet = address!("0xc64cc00b46101bd40aa1c3121195e85c0b0918d8");
    let monitor = HyperliquidMonitor::new(wallet, true);

    let (tx, _rx) = mpsc::unbounded_channel();

    // Verify it connects and runs within a timeout (since it runs an infinite reconnect loop)
    let run_result = timeout(Duration::from_secs(3), monitor.run(tx)).await;

    // Timeout should occur if the connection is successful and continues running
    assert!(run_result.is_err(), "Monitor should have timed out");
}
