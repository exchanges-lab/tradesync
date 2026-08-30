/// The action type of a trade fill relative to the position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeAction {
    /// Opening a position (initial position size was 0)
    Open,
    /// Adding to an existing position (absolute position size increases)
    Increase,
    /// Reducing an existing position (absolute position size decreases but remains non-zero)
    Decrease,
    /// Fully closing an existing position (resulting position size becomes 0)
    Close,
}

/// Represents a detected trade event on Hyperliquid affecting a wallet's position.
#[derive(Debug, Clone)]
pub struct PositionTradeEvent {
    pub coin: String,
    pub side: String, // "B" (Buy) or "S" (Sell)
    pub px: String,   // Price as string
    pub sz: String,   // Trade size as string
    pub time: u64,    // Epoch timestamp in milliseconds
    pub tid: u64,     // Unique trade ID
    pub oid: u64,     // Hyperliquid Order ID
    pub action: TradeAction,
    pub start_pos: String, // Position size before the trade
    pub end_pos: String,   // Position size after the trade
    pub crossed: bool,     // True if taker (market), false if maker (limit)
}

/// Represents the structured row data formatted for Notion database insertion.
#[derive(Debug, Clone)]
pub struct NotionRowData {
    pub symbol: String,                  // e.g., "BTCUSDC"
    pub quantity: String,                // e.g., "0.02484"
    pub filled_price: String,            // e.g., "72543"
    pub direction: String,               // "Long" or "Short"
    pub exchange: String,                // e.g., "Hyperliquid"
    pub date_time: String,               // Format: "YYYY/MM/DD HH:MM"
    pub time: u64,                       // Unix timestamp in milliseconds
    pub order_id: u64,                   // Hyperliquid order ID (oid)
    pub check: bool,                     // false
    pub order_type: String,              // "MARKET" or "LIMIT"
    pub snapshot_ticker: Option<String>, // Exact TradingView ticker, if configured
}

impl NotionRowData {
    /// Determine Order Type from the crossed flag:
    /// crossed == true represents MARKET (taker), crossed == false represents LIMIT (maker)
    pub fn determine_order_type(crossed: bool) -> String {
        if crossed {
            "MARKET".to_string()
        } else {
            "LIMIT".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_order_type() {
        assert_eq!(NotionRowData::determine_order_type(true), "MARKET");
        assert_eq!(NotionRowData::determine_order_type(false), "LIMIT");
    }
}
