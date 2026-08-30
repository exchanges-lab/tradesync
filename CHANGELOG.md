# Changelog

本文件记录项目所有值得注意的变更。
格式遵循 Keep a Changelog，版本号遵循 SemVer。

## [Unreleased]
### Added
- 新增 `REDIRECT_PATH` 配置，支持通过扁平 YAML 文件将 HIP-3 原始 symbol 重定向到准确的 TradingView ticker。
- 支持对接 TradeSnap 截图服务，自动抓取新交易对应的 TradingView 图表快照。
- 新增 `ENABLE_SCREENSHOT` 与 `TRADESNAP_URL` 配置，控制截图的开关和请求地址。
- 新增 `BTCUSDT_SNAPSHOT` 配置，控制使用 `BINANCE:{coin}USDT.P` 还是 `BINANCE:{coin}USDC.P` 格式生成图表截图。
- 新增 `SYMBOL_15M_SNAPSHOT`、`SYMBOL_1H_SNAPSHOT`、`SYMBOL_4H_SNAPSHOT`、`SYMBOL_1D_SNAPSHOT` 四个独立的开关配置，精细控制所要插入截图的时间周期。
- 新建页面时，按配置周期顺序在 Notion 页面正文内自动追加文字标题（`SYMBOL_{timeframe} Snapshot`）、截图图片以及空白行间距。
- 添加 `serde` 依赖库（并开启 `derive` feature）以支持接口响应的反序列化。
- 支持根据 `crossed` 属性自动映射 `Order Type` 类型（`MARKET` / `LIMIT`）写入 Notion 数据库。
- 实现 `NotionWriter` 模块，支持将格式化交易记录批量写入 Notion 数据库。
- 实现 `HyperliquidMonitor` 针对同一订单（`oid`）在 500 毫秒内的多成交 tick 聚合功能，防止拆单造成多笔重复写入。
- 定义 `NotionRowData` 与 `TradeAction` 结构，提供结构化的 Notion 写入数据模型。
- 新增 `examples/fetch_notion_db.rs` 用于实时抓取 Notion Database 列配置的开发脚本。
- 实现 `HyperliquidMonitor` 用于实时订阅以太坊/Hyperliquid钱包账户的交易与开仓事件。
- 添加 `alloy`、`chrono` 与 `serde_json` 库依赖。
- 新增 integration test `tests/monitor_test.rs` 与使用示例 `examples/demo.rs`。
- 添加外部参考库作为 Git 子模块：`noc` (notion-client) 与 `hype` (hyperliquid-rust-sdk)。

### Changed
- HIP-3 交易写入 Notion 时自动移除 DEX namespace（如 `xyz:GOLD` 写为 `GOLD`）；未配置截图重定向时安全跳过截图。
- 将项目与 Crate 命名从 `nosync` 统一重命名为 `tradesync`，并同步更新了所有 Rust 源码包导入和 Docker 镜像名称。
- 更新 Docker Compose 配置，移除了 `tradesnap` 和 `tradesync` （原 `nosync`）多余的 volumes 目录挂载，并统一使用 `:latest` 镜像标签以替代固定版本。
- 移除 `Level` 字段的相关计算与 Notion 写入逻辑（交由用户后续手动设置）。
- 重构 `src/main.rs`，实现从 Hyperliquid WebSocket 捕获开仓事件并自动格式化写入 Notion Database 的闭环服务。
- 将 `hyperliquid_rust_sdk` 与 `notion-client` 依赖方式更改为指向官方 Git 仓库 of 远程依赖，移除本地 Path 依赖。

### Removed
- 移除初始化模版中未使用的占位模块 (`ModuleA`, `ModuleB`) 及其对应测试文件。

## [0.1.0] - 2026-05-30
### Added
- 初始化 Rust 工程项目结构与核心配置
- 实现 `ModuleA` 与 `ModuleB` 业务类及接口设计
- 配置 `tokio` 异步运行时、`thiserror` 自定义错误与 `anyhow` 二进制入口错误处理
- 集成 `tracing` 结构化日志与 `dotenvy` 环境配置读取
- 新增集成测试与示例代码
