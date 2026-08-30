# Notion 写入失败核对

核对时间：2026-08-30（UTC）  
日志范围：当前 `tradesync` 容器自 2026-08-20 17:15:26 UTC 启动以来的全部日志

## 需要手工补写的交易

**无。**

日志中共捕获 3 笔开仓，3 笔均已成功创建 Notion 页面，没有出现 `Failed to write row to Notion`。请勿重复补写以下订单：

| 时间（UTC） | Symbol | Quantity | Filled Price | Direction | Exchange | Order ID | Order Type | Notion Page ID |
|---|---:|---:|---:|---|---|---:|---|---|
| 2026-08-23 08:22 | BTCUSDC | 0.00573 | 76269.00000 | Short | Hyperliquid | 523895897445 | LIMIT | 3c58f81a-c370-81ca-b394-dd30d2d7293d |
| 2026-08-25 05:05 | xyz:GOLDUSDC | 0.60690 | 4631.00000 | Short | Hyperliquid | 525904500090 | LIMIT | 3c78f81a-c370-8111-96da-f88f3ff0ea1e |
| 2026-08-27 16:18 | BTCUSDC | 0.00527 | 80463.00000 | Long | Hyperliquid | 528618351412 | LIMIT | 3c98f81a-c370-819d-8284-c314429c8e2a |

## 实际失败项

失败的是创建交易页面后的 TradeSnap 截图步骤，不是交易行写入：

- 订单 `525904500090`（xyz:GOLDUSDC）：15m、1h、4h、1D 截图请求全部超时。
- 订单 `528618351412`（BTCUSDC）：15m、1h、4h、1D 截图请求全部超时。
- 订单 `523895897445`（BTCUSDC）：四个周期截图均成功并已追加到 Notion 页面。

统计：捕获 3 笔；Notion 成功 3 笔；Notion 失败 0 笔；截图成功 4 次；截图超时 8 次。
