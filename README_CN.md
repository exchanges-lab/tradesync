# tradesync

一个高效、解耦的 Hyperliquid 钱包持仓监控与 Notion 数据库记录工具。

> English version: [README.md](./README.md)

## 1. 简介
`tradesync` 是一个基于 Rust 构建的实时钱包持仓监控与分析工具。它通过 WebSocket 订阅指定的 Hyperliquid 钱包地址，实时捕获开仓交易，将单个订单的多笔分批成交（基于 500ms 防抖聚合机制）合并聚合，并自动将格式化后的交易数据同步到 Notion 数据库。

## 2. 核心特性
* **实时钱包订阅**：使用 `hyperliquid-rust-sdk` 与钱包的 `UserEvents` 建立持久化 WebSocket 连接，以毫秒级延迟感知持仓变化。
* **基于 `oid` 的成交聚合**：将同一订单（`oid`）在 500ms 窗口内的多笔分批成交聚合为一行统一记录，避免拆单产生重复的数据库条目。
* **开仓事件捕获**：通过校验起始仓位（`start_position`）是否为零来识别开仓动作。目前仅捕获开仓（忽略平仓，以规避拆单的复杂性）。
* **订单类型识别**：自动识别订单执行类型（taker 成交 `crossed == true` 记为 `MARKET`，maker 成交 `crossed == false` 记为 `LIMIT`）。
* **健壮的重连机制**：实现了健壮的连接循环，在网络抖动或 WebSocket 断开后自动重连。
* **Notion 集成**：集成 `notion-client` API，将结构化数据行（`Symbol`、`Quantity`、`Filled Price`、`Direction`、`Exchange`、`DataTime`、`Order ID`、`Order Type`、`Check`）直接写入目标 Notion 数据库。

## 3. Notion 模板与效果预览
本项目向 Notion 数据库写入数据，你需要先准备一个字段结构匹配的数据库。可直接复制官方模板（点击右上角 **Duplicate** 即可一键复制到你的工作区）：

> 📋 **Notion 数据库模板**：<https://annafish.notion.site/2026-2e08f81ac37080c6a474f00373d13287>

复制后，将该数据库的 ID 填入 `.env` 的 `NOTION_DATABASE_ID`，并把你的 Notion 集成（integration）连接到该页面即可。

<details>
<summary>🗂️ Notion 数据库示例页面（点击展开）</summary>

<img src="https://img.cathiefish.art/github/tradesync/tradesync1.png" width="720" alt="tradesync 数据库示例" />

</details>

<details>
<summary>📜 单条记录页完整长截图（点击展开）</summary>

<img src="https://img.cathiefish.art/github/tradesync/tradesync2.png" width="420" alt="tradesync 记录页示例" />

</details>


> 🖼️ **关于记录页里的 TradingView 实时截图**
> 记录页中多周期（15m / 1h / 4h / 1D）的 TradingView 实时截图，并非由 `tradesync` 自身生成，而是由配套服务 **TradeSnap**（独立仓库：<https://github.com/exchanges-lab/tradesnap>）负责。`tradesync` 在写入 Notion 后，通过 `TRADESNAP_URL` 调用 TradeSnap 拉取截图并追加到对应页面。
> 该功能为可选项，由 `ENABLE_SCREENSHOT` 控制；关闭时 `tradesync` 仅写入交易数据行，不依赖 TradeSnap。

## 4. 架构与模块
目录结构与模块说明如下：

```
tradesync/
├── src/
│   ├── lib.rs          # 统一模块导出
│   ├── structs.rs      # 数据模型与事件定义（如 NotionRowData）
│   ├── hyperliquid.rs  # Hyperliquid WebSocket 监控实现
│   └── notion.rs       # Notion 数据库写入实现
├── examples/           # 开发示例
│   ├── demo.rs         # 实时钱包监控示例
│   └── fetch_notion_db.rs # 用于获取 Notion 数据库 schema 属性的开发脚本
├── tests/              # 集成测试与单元测试
│   └── monitor_test.rs # 监控连通性与重连循环测试
├── references/         # 本地参考子模块
├── docker-compose.yml  # Docker Compose 部署编排
├── Dockerfile          # 容器镜像构建文件
├── .env                # 本地环境变量配置（已被 git 忽略）
├── .env.example        # 环境变量配置模板
├── Cargo.toml          # Cargo 包文件，含远程 Git 依赖
└── CHANGELOG.md        # 变更日志
```

* **HyperliquidMonitor**（`src/hyperliquid.rs`）：长期运行的服务，维护与 Hyperliquid API 的 WebSocket 连接，并发出聚合后的交易事件。
* **NotionWriter**（`src/notion.rs`）：将捕获到的数据行格式化并写入 Notion 数据库。
* **structs**（`src/structs.rs`）：定义 `PositionTradeEvent`、`NotionRowData` 等数据模型。

## 5. 部署

### 方式一：Docker Compose（推荐）
项目已提供 `docker-compose.yml`，直接拉取预构建镜像 `ghcr.io/exchanges-lab/tradesync:latest` 运行，无需本地安装 Rust 工具链。

```bash
# 克隆仓库
git clone https://github.com/cathiefish/tradesync.git
cd tradesync

# 复制并配置环境变量
cp .env.example .env
# 编辑 .env，填入 WALLET_ADDRESS、NOTION_API_KEY、NOTION_DATABASE_ID 等

# compose 使用了一个名为 cycle 的外部网络，首次部署需先创建
docker network create cycle

# 拉取最新镜像并后台启动
docker compose pull
docker compose up -d

# 查看实时日志
docker compose logs -f

# 停止并移除容器
docker compose down
```

> **说明**
> - `docker-compose.yml` 中的服务接入外部网络 `cycle`，便于与同网络下的其他服务（如截图服务 `tradesnap`）通信。若启用截图功能，请将 `TRADESNAP_URL` 设为该网络内可达地址（默认 `http://tradesnap:8003`）。
> - 容器配置了 `restart: unless-stopped`，宿主机重启后会自动拉起。

### 方式二：本地构建运行
适合开发调试或自行构建二进制：

```bash
# 克隆仓库
git clone https://github.com/cathiefish/tradesync.git
cd tradesync

# 复制并配置环境变量
cp .env.example .env
# 编辑 .env 文件，填入你的 WALLET_ADDRESS、Notion 集成 token 与数据库 ID

# 构建项目
cargo build --release

# 运行监控同步服务
cargo run

# 运行本地监控示例
cargo run --example demo
```

## 6. 环境变量
| 变量 | 说明 | 是否必填 | 默认值 / 示例值 |
| :--- | :--- | :--- | :--- |
| `WALLET_ADDRESS` | 要监控的 Ethereum/Hyperliquid 钱包地址 | **是** | `0xc64cc00b46101bd40aa1c3121195e85c0b0918d8` |
| `IS_TESTNET` | 连接 Hyperliquid 测试网（`true`/`false`） | 否 | `true` |
| `NOTION_API_KEY` | Notion 集成 token（内部密钥） | **是** | `secret_xxxxxx...` |
| `NOTION_DATABASE_ID` | Notion 数据库 ID | **是** | `2b08f81ac37083389c5c01242f3c1557` |
| `RUST_LOG` | 日志详细级别（error、warn、info、debug） | 否 | `info` |
| `ENABLE_SCREENSHOT` | 在 Notion 页面中启用 TradingView 图表截图（`true`/`false`） | 否 | `false` |
| `TRADESNAP_URL` | TradeSnap 服务的 API 端点 URL | 否 | `http://tradesnap:8003` |
| `REDIRECT_PATH` | 可选的 YAML 文件路径，将 HIP-3 原始 symbol 映射到准确的 TradingView ticker | 否 | `./redirect.yml` |
| `BTCUSDT_SNAPSHOT` | 截图使用币安 USDT 合约（`true`）而非 USDC 合约（`false`） | 否 | `false` |
| `SYMBOL_15M_SNAPSHOT` | 捕获并插入 15m 周期截图（`true`/`false`） | 否 | `false` |
| `SYMBOL_1H_SNAPSHOT` | 捕获并插入 1h 周期截图（`true`/`false`） | 否 | `false` |
| `SYMBOL_4H_SNAPSHOT` | 捕获并插入 4h 周期截图（`true`/`false`） | 否 | `false` |
| `SYMBOL_1D_SNAPSHOT` | 捕获并插入 1D 周期截图（`true`/`false`） | 否 | `false` |

HIP-3 symbol 写入 Notion 时会去掉 DEX 前缀（例如 `xyz:GOLD` 写为 `GOLD`）。如需截图，请创建扁平的 redirect 文件并设置 `REDIRECT_PATH`：

```yaml
"xyz:GOLD": "TVC:GOLD"
"xyz:NVDA": "NASDAQ:NVDA"
```

未配置映射的 HIP-3 symbol 仍会写入 Notion，但会跳过截图，不会猜测数据源。
使用仓库自带的 Docker Compose 时，`REDIRECT_PATH` 填宿主机上的 YAML 路径；Compose 会将它只读挂载到容器内的 `/app/redirect.yml`。


## 7. 开发与测试
### Git 分支策略
* `dev`：活跃开发分支。新功能与修复先合并到此分支。
* `main`：稳定的生产可用分支。

### 本地质量检查
在提交 Pull Request 之前，你**必须**运行：
```bash
# 自动格式化代码库
cargo fmt --all

# 运行 lint 检查
cargo clippy --all-targets --all-features -- -D warnings

# 执行测试套件
cargo test
```

## 8. 变更日志
有关项目更新的详细记录，请参阅 [CHANGELOG.md](./CHANGELOG.md)。
