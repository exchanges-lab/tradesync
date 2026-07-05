use crate::structs::NotionRowData;
use chrono::{TimeZone, Utc};
use notion_client::endpoints::Client as NotionClient;
use notion_client::endpoints::blocks::append::request::AppendBlockChildrenRequest;
use notion_client::endpoints::pages::create::request::CreateAPageRequestBuilder;
use notion_client::objects::block::{Block, BlockType, ImageValue, ParagraphValue};
use notion_client::objects::file::{ExternalFile, File};
use notion_client::objects::page::{DatePropertyValue, PageProperty, SelectPropertyValue};
use notion_client::objects::parent::Parent;
use notion_client::objects::property::DateOrDateTime;
use notion_client::objects::rich_text::{RichText, Text};
use reqwest::ClientBuilder;
use serde_json::Number;
use std::collections::BTreeMap;
use thiserror::Error;
use tracing::{error, info};

/// Errors specific to the NotionWriter.
#[derive(Error, Debug)]
pub enum NotionWriterError {
    #[error("Notion client error: {0}")]
    ClientError(String),
    #[error("Notion API request failed: {0}")]
    RequestError(#[from] notion_client::NotionClientError),
}

/// Writer for logging trade records to the Notion Database.
pub struct NotionWriter {
    client: NotionClient,
    database_id: String,
    enable_screenshot: bool,
    tradesnap_url: Option<String>,
    btcusdt_snapshot: bool,
    snapshot_15m: bool,
    snapshot_1h: bool,
    snapshot_4h: bool,
    snapshot_1d: bool,
}

impl NotionWriter {
    /// Creates a new NotionWriter instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        token: String,
        database_id: String,
        enable_screenshot: bool,
        tradesnap_url: Option<String>,
        btcusdt_snapshot: bool,
        snapshot_15m: bool,
        snapshot_1h: bool,
        snapshot_4h: bool,
        snapshot_1d: bool,
    ) -> Result<Self, NotionWriterError> {
        let client = NotionClient::new(token, Some(ClientBuilder::new()))
            .map_err(|e| NotionWriterError::ClientError(format!("{:?}", e)))?;
        Ok(Self {
            client,
            database_id,
            enable_screenshot,
            tradesnap_url,
            btcusdt_snapshot,
            snapshot_15m,
            snapshot_1h,
            snapshot_4h,
            snapshot_1d,
        })
    }

    /// Inserts a new row into the target Notion database.
    pub async fn write_row(&self, data: NotionRowData) -> Result<(), NotionWriterError> {
        info!(
            symbol = %data.symbol,
            order_id = data.order_id,
            "Writing row to Notion database"
        );

        let mut properties = BTreeMap::new();

        // 1. Symbol (Title column)
        properties.insert(
            "Symbol".to_string(),
            PageProperty::Title {
                id: None,
                title: vec![RichText::Text {
                    text: Text {
                        content: data.symbol.clone(),
                        link: None,
                    },
                    annotations: None,
                    plain_text: None,
                    href: None,
                }],
            },
        );

        // 2. Quantity (RichText column)
        properties.insert(
            "Quantity".to_string(),
            PageProperty::RichText {
                id: None,
                rich_text: vec![RichText::Text {
                    text: Text {
                        content: data.quantity,
                        link: None,
                    },
                    annotations: None,
                    plain_text: None,
                    href: None,
                }],
            },
        );

        // 3. Filled Price (RichText column)
        properties.insert(
            "Filled Price".to_string(),
            PageProperty::RichText {
                id: None,
                rich_text: vec![RichText::Text {
                    text: Text {
                        content: data.filled_price,
                        link: None,
                    },
                    annotations: None,
                    plain_text: None,
                    href: None,
                }],
            },
        );

        // 4. Direction (Select column)
        properties.insert(
            "Direction".to_string(),
            PageProperty::Select {
                id: None,
                select: Some(SelectPropertyValue {
                    id: None,
                    name: Some(data.direction),
                    color: None,
                }),
            },
        );

        // 5. Exchange (MultiSelect column)
        properties.insert(
            "Exchange".to_string(),
            PageProperty::MultiSelect {
                id: None,
                multi_select: vec![SelectPropertyValue {
                    id: None,
                    name: Some(data.exchange),
                    color: None,
                }],
            },
        );

        // 6. DataTime (Date column)
        let dt = Utc
            .timestamp_millis_opt(data.time as i64)
            .single()
            .unwrap_or_else(Utc::now);

        properties.insert(
            "DataTime".to_string(),
            PageProperty::Date {
                id: None,
                date: Some(DatePropertyValue {
                    start: Some(DateOrDateTime::DateTime(dt)),
                    end: None,
                    time_zone: None,
                }),
            },
        );

        // 7. Order ID (Number column)
        properties.insert(
            "Order ID".to_string(),
            PageProperty::Number {
                id: None,
                number: Some(Number::from(data.order_id)),
            },
        );

        // 8. Check (Checkbox column)
        properties.insert(
            "Check".to_string(),
            PageProperty::Checkbox {
                id: None,
                checkbox: data.check,
            },
        );

        // 9. Order Type (Select column)
        properties.insert(
            "Order Type".to_string(),
            PageProperty::Select {
                id: None,
                select: Some(SelectPropertyValue {
                    id: None,
                    name: Some(data.order_type),
                    color: None,
                }),
            },
        );

        // Build CreateAPageRequest and create page
        let request = CreateAPageRequestBuilder::default()
            .parent(Parent::DatabaseId {
                database_id: self.database_id.clone(),
            })
            .properties(properties)
            .build()
            .map_err(|e| {
                NotionWriterError::ClientError(format!("Failed to build page request: {:?}", e))
            })?;

        let page = self.client.pages.create_a_page(request).await?;
        info!(page_id = %page.id, "Successfully wrote row to Notion Database");

        if let (true, Some(url)) = (self.enable_screenshot, &self.tradesnap_url) {
            let coin = if data.symbol.ends_with("USDC") {
                data.symbol.trim_end_matches("USDC").to_string()
            } else if data.symbol.ends_with("USDT") {
                data.symbol.trim_end_matches("USDT").to_string()
            } else {
                data.symbol.clone()
            };

            let ticker = if self.btcusdt_snapshot {
                format!("BINANCE:{}USDT.P", coin.to_uppercase())
            } else {
                format!("BINANCE:{}USDC.P", coin.to_uppercase())
            };
            let tradesnap_url = url.trim_end_matches('/');
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let mut children = Vec::new();

            let mut timeframes = Vec::new();
            if self.snapshot_15m {
                timeframes.push("15m");
            }
            if self.snapshot_1h {
                timeframes.push("1h");
            }
            if self.snapshot_4h {
                timeframes.push("4h");
            }
            if self.snapshot_1d {
                timeframes.push("1D");
            }

            for timeframe in &timeframes {
                let request_url = format!(
                    "{}/chart?ticker={}&interval={}",
                    tradesnap_url, ticker, timeframe
                );

                info!(
                    symbol = %data.symbol,
                    ticker = %ticker,
                    timeframe = %timeframe,
                    request_url = %request_url,
                    "Requesting screenshot from TradeSnap..."
                );

                match http_client.get(&request_url).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            #[derive(serde::Deserialize)]
                            struct TradeSnapResponse {
                                png_url: String,
                            }
                            match response.json::<TradeSnapResponse>().await {
                                Ok(json_res) => {
                                    let png_url = json_res.png_url;
                                    info!(
                                        symbol = %data.symbol,
                                        timeframe = %timeframe,
                                        png_url = %png_url,
                                        "Successfully got screenshot URL from TradeSnap."
                                    );

                                    let text_block = Block {
                                        block_type: BlockType::Paragraph {
                                            paragraph: ParagraphValue {
                                                rich_text: vec![RichText::Text {
                                                    text: Text {
                                                        content: format!(
                                                            "{}_{} Snapshot",
                                                            data.symbol, timeframe
                                                        ),
                                                        link: None,
                                                    },
                                                    annotations: None,
                                                    plain_text: None,
                                                    href: None,
                                                }],
                                                color: None,
                                                children: None,
                                            },
                                        },
                                        ..Default::default()
                                    };

                                    let image_block = Block {
                                        block_type: BlockType::Image {
                                            image: ImageValue {
                                                file_type: File::External {
                                                    external: ExternalFile { url: png_url },
                                                },
                                            },
                                        },
                                        ..Default::default()
                                    };

                                    let empty_block = Block {
                                        block_type: BlockType::Paragraph {
                                            paragraph: ParagraphValue {
                                                rich_text: vec![],
                                                color: None,
                                                children: None,
                                            },
                                        },
                                        ..Default::default()
                                    };

                                    children.push(text_block);
                                    children.push(image_block);
                                    children.push(empty_block);
                                }
                                Err(err) => {
                                    error!(
                                        "Failed to deserialize TradeSnap response for {}: {:?}",
                                        timeframe, err
                                    );
                                }
                            }
                        } else {
                            error!(
                                "TradeSnap returned error status for {}: {:?}",
                                timeframe,
                                response.status()
                            );
                        }
                    }
                    Err(err) => {
                        error!("Failed to request TradeSnap for {}: {:?}", timeframe, err);
                    }
                }
            }

            if !children.is_empty() {
                info!(
                    page_id = %page.id,
                    block_count = children.len(),
                    "Appending all screenshot blocks to Notion page..."
                );
                let append_request = AppendBlockChildrenRequest {
                    children,
                    position: None,
                };
                if let Err(err) = self
                    .client
                    .blocks
                    .append_block_children(&page.id, append_request)
                    .await
                {
                    error!("Failed to append blocks to Notion page: {:?}", err);
                } else {
                    info!(
                        "Successfully appended all screenshot blocks to Notion page {}",
                        page.id
                    );
                }
            }
        }

        Ok(())
    }
}
