use crate::structs::{PositionTradeEvent, TradeAction};
use alloy::primitives::Address;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription, UserData};
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

/// Errors specific to the HyperliquidMonitor.
#[derive(Error, Debug)]
pub enum HyperliquidMonitorError {
    #[error("Failed to parse wallet address: {0}")]
    AddressParseError(String),
}

/// Monitor for Hyperliquid wallet position openings.
pub struct HyperliquidMonitor {
    wallet_address: Address,
    is_testnet: bool,
}

impl HyperliquidMonitor {
    /// Creates a new HyperliquidMonitor instance.
    pub fn new(wallet_address: Address, is_testnet: bool) -> Self {
        Self {
            wallet_address,
            is_testnet,
        }
    }

    /// Runs the monitor WebSocket subscription in a loop, reconnecting if disconnected.
    pub async fn run(
        &self,
        event_tx: UnboundedSender<PositionTradeEvent>,
    ) -> Result<(), HyperliquidMonitorError> {
        let base_url = if self.is_testnet {
            BaseUrl::Testnet
        } else {
            BaseUrl::Mainnet
        };

        info!(
            wallet = %self.wallet_address,
            is_testnet = self.is_testnet,
            "Starting Hyperliquid monitor"
        );

        loop {
            info!("Connecting to Hyperliquid InfoClient...");
            let mut info_client = match InfoClient::with_reconnect(None, Some(base_url)).await {
                Ok(client) => client,
                Err(e) => {
                    error!(
                        error = ?e,
                        "Failed to connect to Hyperliquid InfoClient, retrying in 5 seconds..."
                    );
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            let (ws_sender, mut ws_receiver) = tokio::sync::mpsc::unbounded_channel();

            info!(
                "Subscribing to UserEvents for wallet: {}",
                self.wallet_address
            );
            if let Err(e) = info_client
                .subscribe(
                    Subscription::UserEvents {
                        user: self.wallet_address,
                    },
                    ws_sender,
                )
                .await
            {
                error!(
                    error = ?e,
                    "Failed to subscribe to UserEvents, retrying in 5 seconds..."
                );
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            info!("Successfully subscribed to UserEvents.");

            // Quiet window after the last fill of an order before flushing the
            // aggregate. Fills of one order can arrive several seconds apart
            // (observed gaps up to ~1.5s), so the timer is re-armed per fill
            // (debounce) instead of running once from the first fill.
            const AGGREGATION_QUIET_WINDOW: Duration = Duration::from_secs(3);

            let (timeout_tx, mut timeout_rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64)>();

            struct AggregationState {
                coin: String,
                side: String,
                accumulated_sz: f64,
                accumulated_px_sz: f64,
                time: u64,
                tid: u64,
                crossed: bool,
                start_position: f64,
                // Bumped on every fill; a flush timer only fires the flush if
                // its generation still matches (i.e. no newer fill arrived).
                generation: u64,
            }

            let mut active_opening_orders =
                std::collections::HashMap::<u64, AggregationState>::new();

            loop {
                tokio::select! {
                    msg = ws_receiver.recv() => {
                        match msg {
                            Some(Message::User(user_msg)) => {
                                debug!(user_msg = ?user_msg, "Received user event message");
                                if let UserData::Fills(fills) = user_msg.data {
                                    for fill in fills {
                                        let start_pos: f64 = fill.start_position.parse().unwrap_or(0.0);
                                        let sz: f64 = fill.sz.parse().unwrap_or(0.0);
                                        let px: f64 = fill.px.parse().unwrap_or(0.0);

                                        // Capture fills that open or increase a position. Hyperliquid
                                        // labels these dir = "Open Long" / "Open Short"; gating on
                                        // start_pos == 0.0 (the old check) silently dropped adds to an
                                        // existing position.
                                        if fill.dir.starts_with("Open") || active_opening_orders.contains_key(&fill.oid) {
                                            let generation = match active_opening_orders.entry(fill.oid) {
                                                std::collections::hash_map::Entry::Vacant(e) => {
                                                    // First fill of the opening order: create entry
                                                    e.insert(AggregationState {
                                                        coin: fill.coin.clone(),
                                                        side: fill.side.clone(),
                                                        accumulated_sz: sz,
                                                        accumulated_px_sz: px * sz,
                                                        time: fill.time,
                                                        tid: fill.tid,
                                                        crossed: fill.crossed,
                                                        start_position: start_pos,
                                                        generation: 0,
                                                    });
                                                    info!(
                                                        coin = %fill.coin,
                                                        side = %fill.side,
                                                        px = %fill.px,
                                                        sz = %fill.sz,
                                                        oid = fill.oid,
                                                        dir = %fill.dir,
                                                        start_pos = start_pos,
                                                        "New opening order detected, starting aggregation..."
                                                    );
                                                    0
                                                }
                                                std::collections::hash_map::Entry::Occupied(mut e) => {
                                                    // Subsequent fill of the active opening order: aggregate it
                                                    let state = e.get_mut();
                                                    state.accumulated_sz += sz;
                                                    state.accumulated_px_sz += px * sz;
                                                    state.generation += 1;
                                                    info!(
                                                        coin = %fill.coin,
                                                        side = %fill.side,
                                                        px = %fill.px,
                                                        sz = %fill.sz,
                                                        oid = fill.oid,
                                                        accumulated_sz = %state.accumulated_sz,
                                                        "Aggregated additional fill for active opening order"
                                                    );
                                                    state.generation
                                                }
                                            };

                                            // (Re-)arm the flush timer for this order; older timers
                                            // become no-ops because their generation no longer matches.
                                            let tx = timeout_tx.clone();
                                            let oid = fill.oid;
                                            tokio::spawn(async move {
                                                sleep(AGGREGATION_QUIET_WINDOW).await;
                                                let _ = tx.send((oid, generation));
                                            });
                                        } else {
                                            debug!(
                                                coin = %fill.coin,
                                                dir = %fill.dir,
                                                start_pos = start_pos,
                                                oid = fill.oid,
                                                "Ignoring non-opening trade fill"
                                            );
                                        }
                                    }
                                }
                            }
                            Some(Message::HyperliquidError(err_msg)) => {
                                error!(error = %err_msg, "Received error message from Hyperliquid WS");
                            }
                            Some(Message::Pong) => {
                                debug!("Received Pong from Hyperliquid WS");
                            }
                            Some(Message::NoData) => {
                                // Server idle-disconnect or dropped connection. Because the
                                // client is created with InfoClient::with_reconnect, the SDK
                                // auto-reconnects (~1s) and resubscribes UserEvents on the same
                                // channel, so we keep reading instead of tearing down here.
                                // NOTE: a short application-level recv() timeout was intentionally
                                // NOT added: the SDK does not forward Pong frames to the
                                // subscription channel, so a quiet market (no fills) is
                                // indistinguishable from a dead connection and any finite timeout
                                // would cause spurious reconnects and missed-fill blind windows.
                                warn!("Hyperliquid WS disconnected; SDK is auto-reconnecting...");
                            }
                            Some(other) => {
                                debug!(msg = ?other, "Received other message from Hyperliquid WS");
                            }
                            None => {
                                warn!("Hyperliquid WS receiver closed");
                                break;
                            }
                        }
                    }
                    flush = timeout_rx.recv() => {
                        match flush {
                            Some((oid, generation)) => {
                                // Only flush if no newer fill re-armed the timer since this
                                // one was spawned; otherwise a later timer will handle it.
                                let matches_generation = active_opening_orders
                                    .get(&oid)
                                    .is_some_and(|s| s.generation == generation);
                                if !matches_generation {
                                    continue;
                                }
                                if let Some(state) = active_opening_orders.remove(&oid).filter(|s| s.accumulated_sz > 0.0) {
                                    let avg_px = state.accumulated_px_sz / state.accumulated_sz;
                                    info!(
                                        coin = %state.coin,
                                        side = %state.side,
                                        avg_px = %avg_px,
                                        total_sz = %state.accumulated_sz,
                                        oid = oid,
                                        "Aggregated opening order completed! Sending event..."
                                    );

                                    // Position delta is negative for sells (short opens/adds)
                                    let signed_sz = if state.side == "B" {
                                        state.accumulated_sz
                                    } else {
                                        -state.accumulated_sz
                                    };
                                    let action = if state.start_position == 0.0 {
                                        TradeAction::Open
                                    } else {
                                        TradeAction::Increase
                                    };

                                    let event = PositionTradeEvent {
                                        coin: state.coin,
                                        side: state.side.clone(),
                                        px: format!("{:.5}", avg_px),
                                        sz: format!("{:.5}", state.accumulated_sz),
                                        time: state.time,
                                        tid: state.tid,
                                        oid,
                                        action,
                                        start_pos: format!("{:.5}", state.start_position),
                                        end_pos: format!("{:.5}", state.start_position + signed_sz),
                                        crossed: state.crossed,
                                    };

                                    if let Err(e) = event_tx.send(event) {
                                        error!(
                                            error = ?e,
                                            "Failed to send PositionTradeEvent through channel"
                                        );
                                    }
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }

            warn!("Hyperliquid WebSocket connection closed. Reconnecting in 5 seconds...");
            sleep(Duration::from_secs(5)).await;
        }
    }
}
