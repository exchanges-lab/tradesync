use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedirectError {
    #[error("failed to read redirect file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse redirect file {path}: {source}")]
    Parse {
        path: String,
        source: serde_yaml::Error,
    },
}

/// Exact Hyperliquid symbol to TradingView ticker redirects.
#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct SymbolRedirects(HashMap<String, String>);

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedSymbol {
    pub notion_symbol: String,
    pub snapshot_ticker: Option<String>,
}

impl SymbolRedirects {
    pub fn load(path: Option<&Path>) -> Result<Self, RedirectError> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        let path_display = path.display().to_string();
        let contents = fs::read_to_string(path).map_err(|source| RedirectError::Read {
            path: path_display.clone(),
            source,
        })?;

        serde_yaml::from_str(&contents).map_err(|source| RedirectError::Parse {
            path: path_display,
            source,
        })
    }

    pub fn resolve(&self, raw_symbol: &str, use_usdt_snapshot: bool) -> ResolvedSymbol {
        if let Some((_, symbol)) = raw_symbol.split_once(':') {
            return ResolvedSymbol {
                notion_symbol: symbol.to_string(),
                snapshot_ticker: self.0.get(raw_symbol).cloned(),
            };
        }

        let snapshot_quote = if use_usdt_snapshot { "USDT" } else { "USDC" };
        ResolvedSymbol {
            notion_symbol: format!("{raw_symbol}USDC"),
            snapshot_ticker: Some(format!(
                "BINANCE:{}{}.P",
                raw_symbol.to_uppercase(),
                snapshot_quote
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redirects() -> SymbolRedirects {
        serde_yaml::from_str(
            r#"
"xyz:GOLD": "TVC:GOLD"
"xyz:NVDA": "NASDAQ:NVDA"
"#,
        )
        .unwrap()
    }

    #[test]
    fn resolves_standard_hyperliquid_symbol() {
        assert_eq!(
            redirects().resolve("BTC", true),
            ResolvedSymbol {
                notion_symbol: "BTCUSDC".to_string(),
                snapshot_ticker: Some("BINANCE:BTCUSDT.P".to_string()),
            }
        );
    }

    #[test]
    fn strips_hip3_prefix_and_applies_redirect() {
        assert_eq!(
            redirects().resolve("xyz:GOLD", true),
            ResolvedSymbol {
                notion_symbol: "GOLD".to_string(),
                snapshot_ticker: Some("TVC:GOLD".to_string()),
            }
        );
    }

    #[test]
    fn skips_snapshot_for_unmapped_hip3_symbol() {
        assert_eq!(
            redirects().resolve("xyz:SPCX", true),
            ResolvedSymbol {
                notion_symbol: "SPCX".to_string(),
                snapshot_ticker: None,
            }
        );
    }
}
