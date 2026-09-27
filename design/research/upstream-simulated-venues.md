# 上游调查：免开户的模拟交易场所

> 本报告为集成（`design/integration/design.md`）选择上游提供候选证据，**不做 UTA 设计**。要找的是任何人不开户、不做 KYC、不入金即可取得的模拟交易场所，覆盖加密货币、外汇、股票三类资产，运行平台只要求 macOS 或 Linux 之一。调查于 2026-09-27 进行，只读项目仓库（pin commit 或 tag）、官方文档与官方帮助页；**未注册账户、未安装或运行任何候选、未调用任何交易 API**，所以以下是文档核验结论，不是运行验证。凭推断的内容标 `[推断]`，查不到的写“未查到”。

## 1. 加密货币

|候选|获取/门槛|交易接口|调用方订单键|查询|推送|行情|维护/许可|
|---|---|---|---|---|---|---|---|
|**1 Binance Spot Testnet（推荐）**|托管；需 GitHub OAuth/API key；免 Binance 账户、KYC、入金，自动给虚拟金；约月度重置。|REST+WS；LIMIT/MARKET/STOP*/LIMIT_MAKER；撤单。改单仅减量保优先级；cancel-replace 可部分成功，非原子。|`newClientOrderId`；open order 内唯一；文档未给长度/字符集；可按此查询/撤。|按 id/client id 查单、列未结/历史、`myTrades` 有 trade id、查 Spot 余额；无持仓概念。|`executionReport` 有 execution/trade id；未见续接游标，重连后 REST 对账属推断。|测试网 WS；另有免 key 真实行情 REST/WS。|`binance/binance-spot-api-docs@828ca74`，2026-09-18；根 LICENSE 未查到，托管服务非开源。|
|**2 Slick-Sim（自托管，受限）**|MIT，Linux CI；C++23/CMake/依赖；macOS 未验证。无注册；Coinbase WS gateway 会调用真实 Coinbase 认证 fee API。|Coinbase REST+WS 或 Hyperliquid REST；Coinbase 限 4 类单；HL 仅 GTC/IOC（ALO 变 GTC、trigger 变 limit）。|Coinbase `client_order_id` 无文档限制，但改撤靠模拟器 UUID；HL `c` 可 cancelByCloid，数值 oid 坏。|Coinbase 历史查询挂起；HL `/info` 查到的是生产状态；无模拟余额/持仓。|Coinbase WS 推送，无续接；HL 无成交推送，按 cloid 撤单是 fire-and-forget 且未知单也回 success。|可选 Coinbase/HL 公共实时 WS；无历史回放。|`SlickQuant/slick-sim@25660dd`，v0.2.0，2026-08-24，MIT；官方称 early-stage。|

**公开免账户行情：** Binance REST `data-api.binance.vision`（klines/trades/depth）+ WS `data-stream.binance.vision`；Coinbase `wss://advanced-trade-ws.coinbase.com`（level2/ticker/trades/candles，无 JWT）。[Binance](https://github.com/binance/binance-spot-api-docs/blob/828ca74b809cfedbd5602df328b5f706368d483b/faqs/market_data_only.md) · [Coinbase](https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/websocket/websocket-endpoints)

**排序/结论：** 1) Binance 测试网，若 GitHub 登录可接受，是订单与账户查询最完整的免券商账户方案；2) Slick-Sim 是自托管近似方案，但查询缺失/行为缺口使其不是可直接依赖的完整 broker。Hyperliquid Testnet 排除：官方 faucet 要求同地址曾 mainnet 入金。[官方 faucet](https://hyperliquid.gitbook.io/hyperliquid-docs/onboarding/testnet-faucet.md)。

依据：[Binance Testnet/API](https://github.com/binance/binance-spot-api-docs/blob/828ca74b809cfedbd5602df328b5f706368d483b/testnet/general-info.md)；[Slick-Sim v0.2.0](https://github.com/SlickQuant/slick-sim/tree/v0.2.0) · [Coinbase adapter](https://slickquant.github.io/slick-sim/integration-coinbase/) · [Hyperliquid adapter](https://slickquant.github.io/slick-sim/integration-hyperliquid/) · [known gaps](https://slickquant.github.io/slick-sim/known-gaps/)。

## 2. 外汇

|候选|获取 / 平台 / 维护许可|接口 / 订单|调用方订单键|查询|推送 / 重连|行情|
|---|---|---|---|---|---|---|
|**QuantReplay**|自托管 Docker Compose，无账户；Linux/amd64 明确，Apple Silicon 辅助镜像支持 arm64 但示例模拟器仍 linux/amd64。v12 `a58c7c6`（2026-08-12），repo 页示更新 2026-09-19；Apache-2.0。|FIXT.1.1/FIX50SP2 下单、撤单、改单；REST 管理。支持 FXSPOT 等工具类型，但当前 Matching-only；quote-driven/RFQ 在 roadmap。Market/Limit；Day/GTC/IOC/FOK/GTD。改单用单个 35=G；拒绝时原单不变，未明示原子性保证。|ClOrdID(tag11) 须唯一；请求长度/字符限制未查到。撤改用 OrderID 或 OrigClOrdID 定位；没有按 ClOrdID 查询。|有 ExecutionReport；未查到 OrderStatusRequest、未结/成交列表、持仓或余额 API。|FIX 回报及行情 snapshot/incremental；ExecID 格式 OrderID-Counter。FIX session 有 MsgSeqNum/ResendRequest，但应用回报重放保证未查到。行情订阅断线失效需重订；断线订单保留或取消可配置。|随机订单发生器；CSV/PostgreSQL/TimescaleDB 历史多层 bid/ask 回放。直连实时外部 feed 尚非文档现有功能。最接近完整 FX venue，但非完整账户模拟 broker。|
|**QuickFIX C++ 样例**（`ordermatch`；`executor` 更简单）|自托管、无账户；官方文档支持 Linux/macOS。v1.16.0 `2ce8a60`（2026-05-09），repo 页示更新 2026-09-25；QuickFIX Software License 1.0（BSD-like，带署名/商标条款）。|FIX 4.2 acceptor。ordermatch 只接 Limit、DAY，撮合交叉簿；支持新单、撤单，无改单处理；executor 自动填充 limit 单。|ClOrdID 以字符串作键，无长度约束文档；撤单用 OrigClOrdID+Symbol+Side。样例不强制唯一；无按键查询。|仅 CLI 可显示 symbol/现存簿；无 FIX 查单、历史成交、持仓/余额 API。|新单、成交、撤单发 ExecutionReport；ExecID 为进程内递增数。MarketDataRequest 只处理 snapshot 请求且源码未回送行情；无行情推送。订单状态仅内存。|无外部 FX 行情或真正 FIX 行情服务；样例不足以作为现成 FX venue。|

**免费、免账户 FX 行情：**

- **实时：** [biquote](https://biquote.io/docs/) 文档称 read REST 和 SignalR 无注册/API key；`/hubs/tick` 推 EURUSD 等 bid/ask，历史 tick 最多 1000 条、OHLC 最多 1000 根。源只标“MT5 / Broker 1”，未识别券商；FX 成交量/last 恒为 0。无事件序号/断线补缺、延迟/SLA 说明，亦未找到数据许可/再分发条款或维护记录；仅适合谨慎的测试行情，不视为官方统一外汇市场。
- **历史/日频：** [ECB SDMX](https://data.ecb.europa.eu/help/api/data-examples) 公开 REST 示例（无 key/登录要求说明），可查 EUR 基准日参考汇率历史；工作日公布，非实时且非可成交报价。比如 `EXR/D.USD.EUR.SP00.A` 为每欧元美元数。不能直接构成模拟订单簿。

**排序/建议：** 1) QuantReplay；2) QuickFIX 样例（仅 FIX 协议/撮合起点）。推荐 QuantReplay 配合自生成订单簿；ECB 适合作历史参考，biquote 可试作实时外部输入，但需自行接入且授权/连续性不明。严格要求订单查询和账户余额/持仓时，未找到完整免账户 FX 模拟 broker。

一手来源：[QuantReplay v12](https://github.com/Quod-Financial/quantreplay/releases/tag/v12) · [README](https://github.com/Quod-Financial/quantreplay/blob/main/README.md) · [FIX ROE](https://quod-financial.github.io/quantreplay/FIXRulesOfEngagement/FIXRulesOfEngagement.html) · [REST API](https://quod-financial.github.io/quantreplay/RESTAPI/RESTAPI.html) · [license](https://github.com/Quod-Financial/quantreplay/blob/main/LICENSE)；[QuickFIX examples docs](https://github.com/quickfix/quickfix/blob/master/doc/html/examples.html) · [ordermatch handler](https://github.com/quickfix/quickfix/blob/master/examples/ordermatch/Application.cpp) · [matcher](https://github.com/quickfix/quickfix/blob/master/examples/ordermatch/Market.cpp) · [v1.16.0](https://github.com/quickfix/quickfix/releases/tag/v1.16.0) · [license](https://github.com/quickfix/quickfix/blob/master/LICENSE)；[ECB API examples](https://data.ecb.europa.eu/help/api/data-examples) · [reference-rate methodology](https://data.ecb.europa.eu/methodology/exchange-rates)。

## 3. 股票

|候选|获取方式与门槛|交易接口 / 订单类型 / 改单|调用方订单键|查询|推送|行情|维护与许可|
|---|---|---|---|---|---|---|---|
|[Quod-Financial/quantreplay@v12](https://github.com/Quod-Financial/quantreplay/releases/tag/v12)|自托管 Docker Compose、Postgres、Liquibase；免券商账户。模拟器镜像为 linux/amd64；macOS/Linux 可走 Docker，ARM 模拟器未确认。|[FIXT.1.1/FIX50SP2](https://quod-financial.github.io/quantreplay/FIXRulesOfEngagement/FIXRulesOfEngagement.html)，REST 仅管理；D/F/G，Market/Limit，Day/GTC/IOC/FOK/GTD。G 为单条改单请求，拒绝时原单不变。|ClOrdID(11) 调用方唯一；长度/字符限制未查到。取消/改单用 OrderID(37)/OrigClOrdID(41)。ExecID(17) 唯一于 listing/order。|未查到 OrderStatusRequest、订单/成交列表、持仓或余额 API；REST 查配置、venue/listing/status。|FIX ExecReport、MsgSeqNum(34)；订单会话可持久化并重发；行情断线取消订阅，重连须重订阅/取快照。|可随机生成流动性，或回放 CSV/DB 多档订单簿；无实时公开源直连。|v12，2026-08-12，commit a58c7c6；[Apache-2.0](https://github.com/Quod-Financial/quantreplay/blob/v12/LICENSE)。|
|[koralkulacoglu/fix-exchange@v1.12.13](https://github.com/koralkulacoglu/fix-exchange/releases/tag/v1.12.13)|自托管；Ubuntu 文档：C++14、CMake、QuickFIX、SQLite、OpenSSL、Abseil；macOS 构建未确认。|FIX4.2/TCP，D/F/G，Market/Limit；改单支持，原子语义未查到。管理为纯文本 TCP；行情 UDP。|FIX ClOrdID(11)；字符/长度限制及撤改单 ID 规则未查到。|未查到订单查询、持仓、余额接口。|FIX 重连可重放挂单/成交/撤单；序号可配，默认 logon/logout 重置。UDP 自带递增 seq，可发现缺口、不能续传。|仅广播本地撮合事件；无自带/外接免费行情或随机做市，需对手订单提供流动性。|v1.12.13，2026-05-14，commit 8f42eb2；[MIT](https://github.com/koralkulacoglu/fix-exchange/blob/v1.12.13/LICENSE)。|
|[philipodonnell/paperbroker@3b4124e](https://github.com/philipodonnell/paperbroker/commit/3b4124ee79532d4b56b5fd5e864ed2ca1f4f857e)|自托管 Python/Flask REST/JSON；不需外部券商账户，但需创建本地模拟账户；README 验证 Ubuntu。|有买卖/平仓 JSON 路由；非撮合交易所。改单/撤单能力未查到。|未查到调用方订单键。|可查账户（含现金/持仓）；未查到订单/成交查询。|未查到推送。|默认 Google Finance quote adapter，含 2017 测试数据；行情依赖陈旧。|最近 master commit 2018-04-08；[MIT](https://github.com/philipodonnell/paperbroker/blob/master/LICENSE)，仅低优先级。|

**免费无账户行情：**

- **Stooq**：[公开日线页](https://stooq.com/q/d/?s=aapl.us) 给出无 key 的 CSV URL；页面免登录可读，但官方未给 API 稳定性/延迟承诺，CSV 直读遇 JS anti-bot challenge；[条款](https://stooq.com/terms.html) 禁止未获同意再分发。不是已确认实时源。
- **IEX HIST**：[官方说明](https://www.iex.io/products/equities/market-data-connectivity)：免费、T+1、最近 12 个月，历史文件为 IEX-TP PCAP，仅 IEX 非全市场；下载是否必须登录未明示。实时 TOPS/DEEP 为订阅产品。条款见[此处](https://www.iex.io/legal/hist-data-terms)。
- **结论：** 未查到官方保证、免账户免费的股票实时 API。Yahoo 官方称历史 CSV 需 Gold，且未查到官方公开行情 API：[说明](https://help.yahoo.com/kb/SLN2311.html)。

**排序结论：** 推荐 **QuantReplay**：最完整的免账户本地股票撮合器，有 FIX 生命周期、合成对手流动性和行情推送；缺订单/成交查询及账户持仓/余额。轻量 FIX 备选为 fix-exchange，但需自行供流动性。若要真实历史，IEX PCAP 须转换成 QuantReplay 多档簿 CSV；Stooq 日 OHLC 只能粗略定价，不能直接回放深度簿。

## 4. 不限资产类别的模拟器

|候选|获取/资产/接口|调用方键、查询、推送|行情/维护/许可|
|---|---|---|---|
|[Open Outcry@29bc14e](https://github.com/tolyo/open-outcry/tree/29bc14ec42c5b88f95572b184532281ec8e76ede)|自托管 Go+PG；mac/Linux [推断]。REST Limit/Market/StopLoss/StopLimit；下/查/撤，无改。种子 BTC_EUR、SPX；可混代码，股票语义有限。|无客户键（服务生 UUID）；按 UUID 查/撤；查订单/挂单/持仓/币账；成交仅 id；无推送/续接。|无行情。AGPL-3.0；提交 2026-03-21，无 release。|
|[QuickFIX ordermatch@386ce46](https://github.com/quickfix/quickfix/tree/386ce46e917ae494ab6e90b1be90fd421cdbe3f9/examples/ordermatch)|自托管 FIX4.2/C++17；官方支持 mac/Linux，symbol 分簿。仅 Limit+DAY；下/撤；ExecReport 推状态/部分及全成，无改/查单。|ClOrdID 可撤（限制未声明）；生成 ExecID；无历史订单/成交、持仓、余额查询。|无行情，MD 请求无回包。QuickFIX 自定义许可 1.0；提交 2026-05-20，v1.16.0(05-09)。|
|[OrderBookMatchingEngine@b5320bf](https://github.com/khrapovs/OrderBookMatchingEngine/tree/b5320bf4bfd2b711830ec0cb38b38f36728de8f2)|自托管 Python≥3.11/FastAPI；单簿无 symbol，不能混类。REST 下单/撮合/撤，Limit/Market。|字符串 order_id 可查/撤；查挂单、成交 id/价量；无改、持仓/余额、推送。|内置噪声交易者，无外部行情。MIT；提交 2026-09-14，v0.12.0(07-25)。|

免账户行情（候选均未接）：加密 [Coinbase 历史 K 线](https://docs.cdp.coinbase.com/exchange/reference/exchangerestapi_getproductcandles)+[公有 WS](https://docs.cdp.coinbase.com/exchange/websocket-feed/overview)，无认证，仅上架品种；FX [ECB SDMX](https://data.ecb.europa.eu/help/api/data) 日线，约 16:00 CET 发布 14:15 参考价，非实时；股票 [Nasdaq 匿名历史接口](https://api.nasdaq.com/api/quote/AAPL/historical) 可读日 OHLCV，但无官方 API 契约，免费自动使用稳定性未确认。

排序/推荐：Open Outcry > QuickFIX > 单簿 OrderBookMatchingEngine。Open Outcry 最接近自建多类原型，但无客户键、成交明细、推送，不能严谨核对 UTA 状态。未查到完整符合者；近似方案是 Open Outcry + 外部公开行情。macOS/Linux 对 Open Outcry、OrderBookMatchingEngine 未找到官方声明，已标 [推断]。

## 附录：需要开户的券商外汇模拟盘（准入门槛核验）

这一节核验的是券商 demo / paper 账户取得**模拟交易 API 凭据**的门槛。它们都需要在券商处注册账户（部分要求已获批的真实账户），不属于本报告要找的免开户场所，保留在此作为排除依据与将来按券商接入时的参考。

准入分档：①无需注册；②只需邮箱或第三方登录；③需注册平台/开发者/模拟账户，但官方流程未要求 KYC、真实经纪账户或入金；④需 KYC、开立/获批真实经纪账户或入金。

|候选|模拟 API 凭据门槛|中国大陆 / 香港官方资料结论|
|---|---|---|
|OANDA v20 Practice|③：fxTrade Practice 账户注册后，在账户管理页生成个人 API token；官方美区 demo 注册页列出居住地、姓名、邮箱、电话、密码，没有列出入金或身份文件要求。|v20 API 明确不向 OANDA Global Markets（OGM）与 TMS Brokers S.A. 开放。香港居民在 OGM 合格地区列表内，但 OGM 客户又不能用 v20 API；大陆是否能从其他实体取得 v20 Practice 账户，所查官方页未确认。([API 入门](https://developer.oanda.com/rest-live-v20/introduction/)；[美区 demo 要求](https://help.oanda.com/us/en/faqs/open-demo-account.htm)；[OGM 合格地区](https://help.oanda.com/bvi/en/faqs/eligible-ogm-countries-bvi.htm)；[OGM API 限制](https://help.oanda.com/bvi/en/faqs/oanda-log-in-support-bvi.htm))|
|FXCM demo / ForexConnect / FCLite|③：FXCM demo 页面写明填邮箱与居住地并提交；API 登录凭据由 FXCM 提供给 demo 账户。|FXCM 官方 FAQ 称可对不同 FXCM 分支开 demo，但 FXCM Markets 的 Open Demo 页面明确写该页面不面向香港居民；大陆是否可通过指定实体取得该 API demo 未明确。([Open Demo](https://www.fxcm.com/markets/open-demo/)；[跨地区 demo FAQ](https://www.fxcm.com/markets/help/new-accounts-how-can-i-open-an-fxcm-demo-practice-account-for-a-different-country/))|
|IG demo API|④（官方 API 凭据路径）：IG Labs 的 demo API key 流程要求先以 live account 登录，切换到 demo 再创建 demo key。|API session 文档只说有地区性登录限制；未查到能明确裁定大陆或香港居民可注册“demo-only API”的 IG 官方地区清单。([IG API Getting Started](https://labs.ig.com/gettingstarted)；[API session](https://labs.ig.com/reference/session.html)；[独立 demo 页面](https://www.ig.com/en/demo-account))|
|Interactive Brokers Paper（FX）|④：普通 paper account 需开立且获批 live account；IBKR 说明 live account 不一定要立即入金。|IBKR 可申请国家/地区名单明确列出 China 与 Hong Kong SAR；仍需经过真实账户申请/批准流程。([开户 Paper 账户说明](https://www.interactivebrokers.com/campus/trading-lessons/how-to-open-an-ibkr-paper-trading-account/)；[可申请地区](https://www.interactivebrokers.com/en/accounts/open-account-country-list.php))|
|Saxo SIM|③：注册 Saxo Developer Account，表单要求姓名、主联系电话、邮箱、国家；developer portal 可取 24 小时 SIM token。|Saxo 官方公告称香港业务于 2024-09-30 停止接收新的香港 brokerage clients；developer SIM 是独立流程，公告没有明确说 SIM portal 对香港封锁。大陆未得到明确结论。([SIM developer 注册](https://www.developer.saxo/accounts/sim/signup)；[香港停接新客户公告](https://www.home.saxo/en-hk/content/commentaries/pr/press-release/saxo-closes-hong-kong-office-30092024))|
|cTrader Open API demo|③（附加应用审查）：需要 cTrader ID、cTrader-affiliated broker 的 demo 交易账户，并提交 Open API 应用给 Spotware review。|Open API 文档没有大陆/香港国家名单；broker 的地域与账户政策决定可否取得 demo。([Open API 入门](https://help.ctrader.com/open-api/)；[应用注册/审查](https://help.ctrader.com/open-api/api-application/))|
|MetaTrader 5 demo + 官方 Python 包|③（broker-dependent）：MT5 终端可向选定 broker/server 申请 demo，输入个人资料并设虚拟 deposit；不需要入金。Python 包经 IPC 连本地 MT5 terminal，官方安装说明以 Windows Python 为要求。|MetaQuotes 的平台说明没有跨 broker 的地区资格名单；大陆/香港可用性必须看具体 broker/server。([MT5 开户说明](https://www.metatrader5.com/en/terminal/help/startworking/acc_open)；[Python integration](https://www.mql5.com/en/docs/python_metatrader5))|

各候选的接口细节（下单/撤单/改单、调用方订单键、查询、推送与恢复、行情、限流、重置）：

- **OANDA v20**：Practice/live 同一套 v20 REST 与 streaming API，只换 host（`api-fxpractice.oanda.com`、`stream-fxpractice.oanda.com`）。`PUT /orders/{orderSpecifier}` 是单次请求取消旧单并创建替代单。调用方键 `clientExtensions.id`（`ClientID`，string，长度/字符集未列），可用 `@<clientID>` 查单、replace、cancel；官方只禁止与另一个 pending order 重复，保留期未查到。交易流水带 transaction ID 与 `lastTransactionID`，断线后可用 `transactions/sinceid` 补齐；价格流每 instrument 最多 4 次/秒。REST 120 req/s/IP，最多 20 active streams/IP。官方帮助页给出手动 Add virtual funds，未见重置 endpoint。([Order endpoints](https://developer.oanda.com/rest-live-v20/order-ep/)；[Transaction endpoints](https://developer.oanda.com/rest-live-v20/transaction-ep/)；[Development Guide](https://developer.oanda.com/rest-live-v20/development-guide/))
- **FXCM FCLite / ForexConnect**：FCLite 用 REST 风格 IAM 取 JWT（access token 有效 1 分钟），下单走 SDK session/managers；ForexConnect 是独立专有 SDK。`CustomId`（订单表 `RequestTXT`）的长度、唯一性、按键查撤均未查到；后续操作用 `OrderID`。未查到事件序号或重放 cursor、通用请求限流。demo 默认 $20,000，Trading Station demo 30 天不活动过期。([FCLite Getting Access](https://www.fxcm.com/public-docs/fclite/gettingstarted/authentication/getting_access.html)；[Orders](https://www.fxcm.com/public-docs/fclite/trading/orders/index.html)；[Orders table](https://www.fxcm.com/public-docs/fclite/commonresources/TradingTables/TTOrders.html))
- **IG**：demo 与 live 同一 REST/Lightstreamer API（`demo-api.ig.com` / `api.ig.com`）。无调用方订单键，使用 IG 生成的 `dealReference`，确认后得 `dealId`；`GET /confirms/{dealReference}` 只保留约 1 分钟。无序号回放保证。限额：app non-trading 60/min、account trading 100/min、历史价格 10,000 points/week、40 并发订阅。([REST Reference](https://labs.ig.com/rest-trading-api-reference.html)；[Streaming Guide](https://labs.ig.com/streaming-api-guide.html)；[FAQ](https://labs.ig.com/faq.html))
- **IBKR paper**：TWS/IB Gateway socket API，FX 用 `secType="CASH"`、`IDEALPRO`。订单键是 API client 递增整数 `orderId`（与 `clientId` 共同界定），`orderRef` 标注面向机构客户；`reqExecutions` 每笔部分成交有 `execId`，修正时 execId 最后点号后缀变化。无跨断线续接 cursor；出站消息上限 50/s。paper 余额可在 Client Portal 重置。([Order reference](https://www.interactivebrokers.com/docs/tws-api/ref/order)；[Execution](https://www.interactivebrokers.com/docs/tws-api/ref/execution)；[changelog](https://www.interactivebrokers.com/docs/tws-api/changelog))
- **Saxo SIM**：同一 OpenAPI、不同 host；`POST/PATCH/DELETE /trade/v2/orders`。`ExternalReference` 最长 50 字符、官方明示不必唯一，不能按它查撤。WebSocket 断线可带最后的 `MessageId` 续接短缓存，但官方说明它不是序号。每 session/service group 120 req/min，下单 1/s，15 秒内相同 POST/PATCH 返回 409。`PUT /port/v1/accounts/{AccountKey}/reset` 可重置余额并清空订单与持仓。([Environments](https://www.developer.saxo/openapi/learn/environments)；[Place order](https://www.developer.saxo/openapi/referencedocs/trade/v2/orders/post__trade)；[Plain WebSocket streaming](https://www.developer.saxo/openapi/learn/plain-websocket-streaming)；[Rate limiting](https://www.developer.saxo/openapi/learn/rate-limiting))
- **cTrader Open API**：`clientOrderId` 最长 50 字符，改撤都要求 `orderId`；`ProtoOAReconcileReq` 拉未结与持仓，`dealId` 为成交唯一 ID（范围未写明）。无通用事件序号；每 connection 每秒 50 个非历史、5 个历史请求。([Messages](https://help.ctrader.com/open-api/messages/)；[Model messages](https://help.ctrader.com/open-api/model-messages/))
- **MT5 Python 包**：经本地 terminal 的 IPC，不是远程 API；无调用方订单键（只有 `magic`/`comment`），按 server ticket 查询；Python 侧不支持 `OnTick`/`OnTradeTransaction` 回调，只能轮询；限流与重置由 broker/server 决定。([Python reference](https://www.mql5.com/en/docs/python_metatrader5)；[Python integration details](https://www.mql5.com/en/book/advanced/python))
