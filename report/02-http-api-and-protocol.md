# 02 — UTA 对外 HTTP 接口与协议契约（`@traderalice/uta-protocol`）

> 摘要（≤10 行）
> 本区域覆盖 UTA 服务对外的全部 HTTP 面与两侧共享的协议包。运行时端点共 **49 条**：trading 39 条（`services/uta/src/http/routes-trading.ts`，38 条注册语句，其中 `:332` 的 for 循环执行两次）、simulator 9 条（`routes-simulator.ts`）、health 1 条（`main.ts:148`），与下文 49 张端点表逐一对应。服务自身**无鉴权、无中间件、无 `app.use`、无 `onError`**，只监听 127.0.0.1:47333；权限门全部在 Alice BFF（`src/webui/routes/trading-proxy.ts`）侧实现。错误映射存在六种以上互不相同的响应体形状，`{error,code,transient}` 是事实主线，但协议包里为此声明的 `WireBrokerError` 零引用。`packages/uta-protocol/src/schemas/index.ts` 是空的 `export {}`，注释宣称的“每端点 Request/Response schema + zValidator”并不存在；协议包实际只贡献类型、`UTAClient`、broker preset/search 规则与两个真实使用的 zod schema（option research / order book）。版本策略为 0.1.0 + `workspace:*` 引用，无 changelog、无版本协商字段、无发布路径。

---

## 1. 概览与职责边界

### 1.1 本区域负责什么

- UTA 服务的 HTTP 面：路由分组、路径、方法、请求/响应体、状态码、错误体形状、幂等性与副作用。
- 两侧共享的协议包 `packages/uta-protocol`：导出清单、契约类型、schema、client SDK、版本与兼容策略、构建/发布形态。
- 路由实现与协议契约之间的漂移清单（附证据）。

### 1.2 不负责什么

- 各端点在 UTA 领域层的语义（账户/订单/持仓建模、staging 与 ledger 细节、快照存储、broker pack 内部）：参见 `03-account-orders-positions.md`、`04-market-data-contracts-fx.md`、`05-staging-approval-ledger.md`、`06-snapshots-and-guards.md`、`07-brokers-and-packs.md`。
- Alice 侧 SDK 消费方行为、UI 页面与工具层调用策略：参见 `08-alice-consumers-and-ui.md`。
- 进程启动/关闭/重启与持久化文件：参见 `01-process-lifecycle.md`、`09-persisted-state-tests-docs-issues.md`。

### 1.3 上下游

| 方向 | 对方 | 契约 |
|---|---|---|
| 上游 | Alice BFF（`src/webui/plugin.ts:254-266`） | 把 `/api/trading/*`、`/api/simulator/*` 原样转发到 UTA；`/api/trading/config/*` 是 Alice 自有路由，只有 test-connection 直连 UTA |
| 上游 | Alice SDK（`src/services/uta-client/`） | 服务端内进程消费 HTTP，方法名尽量对齐原 in-process API |
| 上游 | Guardian / supervisor | `GET /__uta/health` 作为 readiness 探测（`src/services/uta-supervisor/health.ts:29`） |
| 上游 | UI | 经 BFF 代理直调；不 import 协议包，手抄 wire 类型 |
| 下游 | broker 包（`packages/uta-broker-*`）、`packages/ibkr` | 协议包 re-export `IBroker`、`Contract` 类型；broker 包依赖协议包做类型与 preset |

### 1.4 计数核对（本报告自证无遗漏）

| 来源 | 方法 | 数字 |
|---|---|---|
| `routes-trading.ts` 顶层注册语句 | `grep -cE '^  app\.(get\|post\|delete\|put\|patch)\('` | 37 |
| 其中 `:332` 的 for 循环 | 循环体 1 条语句 × 2 个 route 名 | 2 |
| trading 运行时路由（tsx 枚举 `app.routes`） | `TRADING_COUNT=39` | 39 |
| `routes-simulator.ts` 注册语句 / 运行时 | `SIM_COUNT=9` | 9 |
| `main.ts:148` health | `app.get('/__uta/health')` | 1 |
| **合计** | | **49** |

本报告第 2 章共 **49 张端点表**（trading 39 + simulator 9 + health 1），与上表一致。

---

## 2. 功能清单

### 2.0 横向通用规则（先读这节，端点表只写差异）

**路由挂载与中间件。** `main.ts:163` 挂 `/api/trading` → `createTradingRoutes(tradingCtx)`；`main.ts:168` 挂 `/api/simulator` → `createSimulatorRoutes(tradingCtx)`。`main.ts` 中不存在任何 `app.use`，`services/uta/src` 全目录 `app.use(` 命中数为 0，也没有 `onError`/`notFound` 注册。**路由层没有任何鉴权、日志、CORS、限流中间件**；`UTAEngineContext`（`services/uta/src/types.ts`）只注入 `utaManager` / `fxService` / `snapshotService` 三个字段。

**错误映射的六种形状。**

| 形状 | 产生位置 | 语义 |
|---|---|---|
| `{ error, health }` 503 | `routes-trading.ts:108-113` | UTA 离线（`account.health === 'offline'`），同时 `nudgeRecovery()` |
| `{ error, code, transient }` 500/503 | `routes-trading.ts:117-123` | BrokerError：`permanent`（CONFIG/AUTH）=500，其余=503；分类逻辑见 `packages/uta-protocol/src/types/broker.ts:41-78` |
| `{ error }` 4xx/500 | 各 handler 自己的 try/catch（如 `:325`、`:372`、`:392`、`:563`） | 直传 `err.message`，不经 BrokerError 分类 |
| `{ error, phase }` 400/500 | `routes-trading.ts:70-78` + `PHASE_STATUS:63` | 一次性下单管线的阶段失败：stage/commit=400，push=500 |
| `{ error, code }` 409 | `routes-trading.ts:487-490`、`:517-520`、`:531-535` | `PENDING_HASH_REQUIRED` / `PENDING_HASH_CONFLICT` |
| `{ error, issues }` 400 | `routes-simulator.ts:98-100` | simulator 的 zod 失败（`issues` 为 zod issue 数组）；JSON 解析失败另有 `{error:'Invalid JSON'}` |

此外：未捕获异常交给 Hono 默认错误处理，返回 **500 `text/plain` "Internal Server Error"**（实测：把 `utaManager.listUTAs` 造成抛错的 ctx 跑 `GET /uta`、`/equity`、`/contracts/search` 均为 `500 text/plain`）。因为 `UTAClient.safeJSON` 对非 JSON 文本返回原字符串，SDK 侧的 `UTAHttpError.message` 会退化为 `UTA GET <path> returned 500`（`packages/uta-protocol/src/client/UTAClient.ts:78-85`）。

**账户解析与 404 文案。** `resolveAccount`（`:92-96`）按 `:id` 查 `utaManager.get`，失败统一 `404 {error:'Account not found'}`；只有三条一次性下单路由写作 `{error:'UTA not found'}`（`:596`、`:611`、`:627`）。

**数值线格式。** 订单相关数值一律字符串上/下线，避免 IEEE-754 精度损失（注释见 `routes-trading.ts:16-20`，`packages/uta-protocol/src/types/git.ts:250-255`）。`Decimal` 实例经 JSON 序列化后是字符串（decimal.js 定义 `toJSON = toString`，实测 `new Decimal('1.5')` → `"1.5"`），因此 broker 返回的 `Contract`/`Order` 中的 Decimal 字段天然是字符串。

**鉴权/权限门（全部在 Alice 侧，不在 UTA 侧）。** UTA 绑定 `127.0.0.1`（`main.ts:172-175`），信任边界是主机。Alice BFF 的 `app.all('*')`（`trading-proxy.ts:115`）在 `readonly` 模式下用 `isVenueMutation`（`:193-206`）拦截 `wallet/push`、`wallet/place-order`、`wallet/close-position`、`wallet/cancel-order`、`simulate-price` 以及整个 `/api/simulator`，返回 403；`lite` 模式返回 503；UTA 不可达时 502；总超时 30s（`:24`）。Alice 全局 auth 中间件（`src/webui/middleware/auth.ts`）负责 cookie/loopback/CSRF，对 `/api/trading/*` 与 `/api/simulator/*` 生效。

下表所有端点默认满足：UTA 层鉴权=无；权限门=Alice BFF 的 mode/auth 规则；因此逐表只在有额外差异时重复说明。

### 2.1 全局（顶层）端点

**GET /api/trading/uta — 列出已注册 UTA**

| 项 | 内容 |
|---|---|
| 注册 | `services/uta/src/http/routes-trading.ts:134` |
| 用途/触发 | UTA 列表、账户选择器；SDK `listUTAs` |
| 请求 | 无参数 |
| 成功响应 | `{ utas: UTASummary[] }`（`id`、`label`、`asVendor`、`capabilities`、`health`，字段见 `packages/uta-protocol/src/types/manager.ts:12-19`） |
| 错误 | 无显式分支；领域层抛错 → Hono 默认 500 非 JSON |
| 副作用/幂等 | 无副作用；幂等 |
| 调用方 | `UTAManagerSDK.listUTAs`（`src/services/uta-client/UTAManagerSDK.ts:85`）；UI 经代理 `ui/src/api/trading.ts:58` |

**GET /api/trading/equity — 跨账户聚合权益（美元）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:140` |
| 用途/触发 | Portfolio 汇总、AI portfolio 工具 |
| 请求 | 无参数 |
| 成功响应 | `AggregatedEquity`（`totalEquity`/`totalCash`/`totalUnrealizedPnL`/`totalRealizedPnL` 均为字符串，可选 `fxWarnings`、`accounts[]`；`manager.ts:21-`） |
| 错误 | 无显式分支；单账户失败被吞并降级为 `info:null`（`uta-manager.ts:205-217`） |
| 副作用/幂等 | 会 `nudgeRecovery()` 触发离线账户恢复；读幂等 |
| 调用方 | `UTAManagerSDK.getAggregatedEquity:162`；UI `trading.ts:66` |

**GET /api/trading/contracts/search — 跨账户可交易合约启发式搜索**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:150` |
| 用途/触发 | 数据源 symbol → 可下单 aliceId 的桥接；Market workbench、手下单入口 |
| 请求 query | `pattern`（也接受 `query`；空 → 直接返回空集）— 必填语义但可空；`assetClass` ∈ equity/crypto/currency/commodity/unknown（非法值静默降级为 unknown，`:160-162`）；`source`（兼容 `accountId`）— 限定单个账户 |
| 成功响应 | `{ results: ContractSearchHit[], count, utasConfigured }`；`utasConfigured` 仅在存在 UTA 时出现，无 UTA 时形态为 `{results:[],count:0,utasConfigured:0}`，空 pattern 时是 `{results:[],count:0}`（`:151`、`:154-157`） |
| 错误 | 单账户搜索失败被 `Promise.allSettled` 跳过（`contract-search.ts:48-52`），无错误返回 |
| 副作用/幂等 | 只读；幂等 |
| 调用方 | `UTAManagerSDK.searchContracts:212`（只传 pattern）、`UTAAccountSDK.searchContracts:219`（传 pattern+source 后本地过滤）；UI `trading.ts:291` |

**GET /api/trading/fx-rates — 当前在用货币的美元汇率表**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:170` |
| 用途/触发 | 跨币种换算；AI portfolio 工具 |
| 请求 | 无参数 |
| 成功响应 | `{ rates: [{currency, rate(number), source, updatedAt}] }`；USD 被排除在收集之外 |
| 错误 | 逐账户 try/catch 静默跳过（`:182`），无错误返回；异常表现为货币缺失 |
| 副作用/幂等 | 触发 `fxService.getRate` 的缓存与外部拉取；读幂等 |
| 调用方 | `UTAManagerSDK.getFxRates:170`；UI `trading.ts:72` |

**POST /api/trading/test-connection — 临时实例化 broker 验证凭据**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:199` |
| 用途/触发 | 配置向导“测试连接”；Alice BFF `trading-config.ts:343` 直连转发（不经代理的 mode 门，只判 lite） |
| 请求体 | 完整 UTA 配置对象；经 `utaConfigSchema.parse({...body, id: body.id ?? '__test__'})`（schema 在 Alice 侧 `src/core/config.ts:448`） |
| 成功响应 | `{ success: true, account, positions }` |
| 错误 | `{ success: false, error }` 400（任何解析/连接失败）；无 404/503 分支 |
| 副作用/幂等 | 创建→`init`→读账户/持仓→`finally` 关闭，不注册进 UTAManager；幂等（每次新建连接） |
| 调用方 | 无 SDK 方法；Alice BFF `trading-config.ts:343-360`；UI `trading.ts:309`（打 `/api/trading/config/test-connection`） |

### 2.2 账户生命周期与价格模拟

**POST /api/trading/uta/:id/reconnect — 重连单个 UTA**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:221` |
| 用途/触发 | 凭据轮换后恢复连接 |
| 请求 | 无体；`:id` 路径参数 |
| 成功响应 | `ReconnectResult`（`{success:true,message}`）200 |
| 错误 | `reconnectUTA` 返回 `success:false` → 同体 500（`:224`）；并发重连返回 `{success:false,error:'Reconnect already in progress'}`（`uta-manager.ts:84-86`） |
| 副作用/幂等 | 重读 accounts.json、关闭旧连接、重建 broker、重注册 CCXT 工具；非幂等（并发有 in-flight 保护） |
| 调用方 | UI `trading.ts:78` 直调；**SDK `reconnectUTA` 不经此路由**，改走 Guardian 重启（`UTAManagerSDK.ts:184-190`） |
| 备注 | 未找到 UTA 时不返回 404，而是 `{success:false,message:'not found in config'}` 200/500 |

**POST /api/trading/uta/:id/sync — 强制 broker 状态同步**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:229` |
| 用途/触发 | AI 怀疑内存订单状态过期时主动同步；可带延迟等待成交回填 |
| 请求体 | `{ delayMs?: number }`（非 number 时忽略；JSON 解析失败按 `{}`） |
| 成功响应 | `SyncResult`（`{hash, updatedCount, updates[]}`） |
| 错误 | `404 {error:'Account not found'}`；异常 → `500 {error}` |
| 副作用/幂等 | 写 git 日志（同步类 commit）；重复调用可能追加新 commit，非幂等 |
| 调用方 | `UTAAccountSDK.sync:351`；工具层经 SDK 调用 |

**POST /api/trading/uta/:id/simulate-price — 假想价格变动下的 PnL 推演**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:246` |
| 用途/触发 | AI 的“如果涨 10% 会怎样”探索 |
| 请求体 | `{ changes: PriceChangeInput[] }`，元素 `{symbol, change}`；`change` 形如 `@88000` / `+10%` / `-5%`，`symbol='all'` 表示全仓等比例；**无 schema 校验**，非数组时按空数组（`:252`） |
| 成功响应 | `SimulatePriceChangeResult`（`success` + `currentState`/`simulatedState`/`summary`，均可含失败变体 `success:false,error`） |
| 错误 | `404 {error:'Account not found'}`；捕获后按消息是否含 simulate/mock 映射 400/500（`:255-259`）——**路由注释自相矛盾**（`:244-245` 称“非 mock 抛错”，`:255` 又称映射给非 mock） |
| 副作用/幂等 | 实际实现是 git 层的通用纯计算（`TradingGit.ts:753`），对所有 broker 生效；只读幂等 |
| 调用方 | `UTAAccountSDK.simulatePriceChange:359`（先过 `assertVenueWritable`）、工具层 `src/tool/trading.ts:651` |

### 2.3 账户读取（数据查询，统一走 `queryAccount`）

以下 6 条共用同一处理骨架 `queryAccount`（`:104-123`）：离线→503 `{error,health}` 并触发恢复；失败→按 BrokerError 分类 500/503。逐表只记请求与响应差异。

**GET /api/trading/uta/:id/subaccounts — 钱包（子账户）列表**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:264` |
| 请求 | `:id`；无 query |
| 成功响应 | `{ subAccounts: SubAccountRef[] }` |
| 错误 | 404 `Account not found`；503 离线；500/503 BrokerError |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.listSubAccounts:133`；UI `trading.ts:97` |

**GET /api/trading/uta/:id/account — broker 侧账户信息**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:271` |
| 请求 | 可选 `?subAccountId=`（省略=聚合） |
| 成功响应 | `AccountInfo`（直接作为顶层 JSON，无包装字段） |
| 错误 | 同上骨架 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getAccount:139`；UI `trading.ts:85`；AI account 工具 |

**GET /api/trading/uta/:id/positions — 持仓**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:278` |
| 请求 | 可选 `?subAccountId=` |
| 成功响应 | `{ positions: Position[] }` |
| 错误 | 同骨架 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getPositions:145`；UI `trading.ts:90` |

**GET /api/trading/uta/:id/orders — 挂单**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:285` |
| 请求 | 可选 `?ids=a,b,c`（逗号分隔）；省略时取 `getPendingOrderIds()` |
| 成功响应 | `{ orders: OpenOrder[] }` |
| 错误 | 同骨架 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getOrders:152`；UI `trading.ts:101`（不透传 ids） |

**GET /api/trading/uta/:id/market-clock — 市场开闭时间**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:297` |
| 请求 | 无 |
| 成功响应 | `MarketClock`（`isOpen` + 可选 `nextOpen`/`nextClose`；24/7 场所可缺省） |
| 错误 | 同骨架 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getMarketClock:179`；UI `trading.ts:106` |

**GET /api/trading/uta/:id/quote/:symbol — 按 symbol 取报价（legacy 路径形态）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:307` |
| 用途 | 注释称“保留给 legacy UI 调用方”（`:303-306`） |
| 请求 | 路径参数 `:symbol`；服务端动态 `import('@traderalice/ibkr')` 造 `Contract` 并只设 `symbol`（`:309-312`） |
| 成功响应 | `Quote` |
| 错误 | 同骨架 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | **仓库内零调用方**（grep `/quote/` 仅命中本路由与 `safe/knowledge/endpoints.md:60`） |

### 2.4 合约研究（option / depth / expand / bar / details）

**POST /api/trading/uta/:id/quote — 按 Contract 或 aliceId 取报价**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:318` |
| 请求体 | 完整 `Contract` 子集或 `{aliceId}` 提示；服务端 `Object.assign(new Contract(), body)`（`:321-322`），aliceId 展开在领域层 `_expandAliceIdIfNeeded`（`UnifiedTradingAccount.ts:1210`） |
| 成功响应 | `Quote`（`quote.contract` 回填 aliceId） |
| 错误 | 404；其余一律 `500 {error}`（**不走 queryAccount**，`:324`） |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getQuote:161`；AI 工具 `src/tool/trading.ts:533`（注释指明用 POST 形态） |

**POST /api/trading/uta/:id/contract/option-contracts — 期权合约页**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:332`（`:331` for 循环第 1 项） |
| 请求体 | `optionResearchSchema`（`packages/uta-protocol/src/broker-research.ts:5-13`）：`aliceId` 必填；`expiration`(YYYY-MM-DD)、`expirationFrom/To`、`right`(call/put)、`strikeMin/Max`(≥0)、`limit`(1..1000)、`pageToken`、`feed`(indicative/opra) |
| 成功响应 | broker 原始页对象（含 `snapshots`、`nextPageToken`、`metadata`，无包装） |
| 错误 | 404；schema 失败 → `400 {error: zod message}`；非 STK 标的/不支持该能力 → 经 queryAccount 归为 UNKNOWN → 503 `{error,code,transient:true}` |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getOptionContracts:167`；AI 工具 `src/tool/trading.ts:474` |

**POST /api/trading/uta/:id/contract/option-chain — 期权链（同 schema，同循环）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:332`（循环第 2 项，方法名 `getOptionChain`） |
| 请求/响应/错误 | 同上，唯 broker 方法不同；不支持时抛 “getOptionChain is not supported by this broker pack.” |
| 调用方 | `UTAAccountSDK.getOptionChain:171`；AI 工具 `:486` |

**POST /api/trading/uta/:id/contract/order-book — 盘口深度**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:348` |
| 请求体 | `orderBookSchema`（`broker-research.ts:14-16`）：`aliceId` 必填，`limit` 1..100（缺省 20） |
| 成功响应 | broker 原始深度对象，无包装 |
| 错误 | 404；schema 失败 400；不支持 → 503（UNKNOWN 分类） |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getOrderBook:175`；AI 工具 `:498` |
| 测试 | `services/uta/src/http/broker-research.spec.ts`（4 例，见第 7 章） |

**POST /api/trading/uta/:id/contract/expand — 合约展开（债券发行方/期权链/期货月份）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:363` |
| 请求体 | `{ aliceId: string, filters?: ExpandContractFilters }`；**无 schema 校验**，`String(body.aliceId ?? '')` |
| 成功响应 | `ContractExpansion`（`contracts[]` 已回填 aliceId） |
| 错误 | 404；`500 {error}`（非 aliceId 归属错误、broker 无能力均落 500，虽然领域层用的是 `BrokerError('CONFIG')`） |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.expandContract:185`；AI 工具 `:568` |

**POST /api/trading/uta/:id/historical — 历史 OHLCV**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:377` |
| 请求体 | `{ contract: <Contract|{aliceId}>, params: BarParams }`；`params.start/end` 以 ISO 字符串到达，路由转 `Date`（`:386-387`） |
| 成功响应 | `{ bars: Bar[] }` |
| 错误 | 404；无能力或参数错 → `500 {error}`（`BrokerError('CONFIG')` 未映射 500/503 规则） |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getHistorical:204`（SDK 注释仍称“Phase 1 前会 404”，与实际实现不符，属过期注释） |

**POST /api/trading/uta/:id/contracts/details — 合约明细下钻**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:397` |
| 请求体 | `Contract` 子集；含 `aliceId` 时由领域层展开 |
| 成功响应 | `ContractDetails` 或 `null`（路由对 null 也返回 200 与 `null`） |
| 错误 | 404；其余 `500 {error}` |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.getContractDetails:234`；AI 工具 `:302` |

### 2.5 Wallet / Trading-as-Git

**GET /api/trading/uta/:id/wallet/log — 提交日志（摘要投影）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:413` |
| 请求 query | `limit`（`Number()` 后默认 20，非法值回落默认）、`symbol`（过滤） |
| 成功响应 | `{ commits: CommitLogEntry[] }`（`OperationSummary[]` 内嵌，`git.ts:147-167`） |
| 错误 | 404 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.log:243`；UI `trading.ts:120` |

**GET /api/trading/uta/:id/order-history — 订单维度历史（生命周期折叠）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:425` |
| 请求 query | `limit`（默认 50） |
| 成功响应 | `{ orders: OrderHistoryEntry[] }`（`types/history.ts`，含 `source: alice/external`、`commitHash`） |
| 错误 | 404 |
| 副作用/幂等 | 只读（内部 `exportGitState()` 复制投影）；幂等 |
| 调用方 | `UTAAccountSDK.orderHistory:262`；UI `trading.ts:111` |

**GET /api/trading/uta/:id/trade-history — 成交维度历史**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:432` |
| 请求 query | `limit`（默认 50） |
| 成功响应 | `{ trades: TradeHistoryEntry[] }`（`source: order/external/reconcile`） |
| 错误 | 404 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.tradeHistory:270`；UI `trading.ts:116` |

**GET /api/trading/uta/:id/wallet/show/:hash — 单条提交完整内容**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:439` |
| 请求 | 路径参数 `:hash`（8 位短哈希） |
| 成功响应 | `GitCommit`（`parentHash`/`operations`/`results`/`stateAfter`） |
| 错误 | 404 `{error:'Account not found'}` / 404 `{error:'Commit not found'}` |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.show:248`（把 “Commit not found” 转成 null）；UI `trading.ts:126` |

**GET /api/trading/uta/:id/wallet/status — staging 区状态**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:447` |
| 请求 | 无 |
| 成功响应 | `GitStatus`（`staged`、`pendingMessage`、`pendingHash`、`head`、`commitCount`） |
| 错误 | 404 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `UTAAccountSDK.status:256`；UI `trading.ts:132`；`pendingHash` 是人工审批的 CAS 令牌 |

**POST /api/trading/uta/:id/wallet/commit — 提交 staging（不推送）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:458` |
| 请求体 | `{ message }`（非字符串或 trim 后为空 → 400 `{error:'Commit message is required'}`；注意此处是手写校验，非 zod） |
| 成功响应 | `CommitPrepareResult`（`{prepared:true, hash, message, operationCount}`） |
| 错误 | 404；400 空 message；500 `{error}` |
| 副作用/幂等 | 生成 pending 提交并发布 `pendingHash`；空 staging 提交行为见 05 文档；重复调用非幂等 |
| 调用方 | `UTAAccountSDK.commit:344`；AI 工具层 commit |

**POST /api/trading/uta/:id/wallet/reject — 拒绝待推送提交**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:475` |
| 请求体 | `{ reason?: string, expectedPendingHash: string }`；hash 经 `readExpectedPendingHash`（`:85-90`）读取，要求非空字符串（trim 后） |
| 成功响应 | `RejectResult`（`{hash, message, operationCount}`） |
| 错误 | 404；`400 {error:'Nothing to reject'}`（无 pendingMessage 时先判，`:477`）；`409 {error,code:'PENDING_HASH_REQUIRED'}`；`409 {..., 'PENDING_HASH_CONFLICT'}`；其余 500 `{error: String(err)}` |
| 副作用/幂等 | 清 staging + 记录拒绝；同一 hash 重放会因 staging 已清 → 400；非幂等 |
| 调用方 | `UTAAccountSDK.reject:297`；UI `trading.ts:136`（PushApprovalPanel.tsx:443 传 undefined reason） |

**POST /api/trading/uta/:id/wallet/push — 人工审批后推送（真实下单）**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:503` |
| 请求体 | `{ expectedPendingHash: string }` |
| 成功响应 | `PushResult`（`{hash,message,operationCount,submitted[],rejected[]}`） |
| 错误 | 404；400 `Nothing to push`；409 `PENDING_HASH_REQUIRED`/`PENDING_HASH_CONFLICT`（冲突由 `PendingHashConflictError` → `isPendingHashConflict` 判定）；其余 500 |
| 副作用/幂等 | 向 broker 真实提交订单；**非幂等**，靠 expectedPendingHash 做乐观并发保护 |
| 调用方 | `UTAAccountSDK.push:290`；UI `PushApprovalPanel.tsx:423`；被 `safe/knowledge/endpoints.md` 标记为最高价值攻击目标；Alice readonly 模式在此路径 403 |

### 2.6 Stage-only 下单（AI 工具层用）

四条路由共同点：**无 zod 校验**，body 直接交给领域层；任何抛出都映射 `400 {error}`（`:543`、`:555`、`:567`、`:583`）；成功返回 `AddResult`（`git.ts:105-109`，`{staged:true,index,operation}`）。

**POST /api/trading/uta/:id/wallet/stage-place-order**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:537` |
| 请求体 | `StagePlaceOrderParams`（`git.ts:256-283`）：`aliceId`、`action`、`orderType` 必填；`totalQuantity`/`cashQty` 至少一个由**领域层**校验；`subAccountId` 对多钱包场所必填；数值全为字符串 |
| 错误 | 404 / 400 一律 `{error}` |
| 副作用/幂等 | 追加 staging（同 session 可多次）；非幂等 |
| 调用方 | `UTAAccountSDK.stagePlaceOrder:314`；AI 工具 |

**POST /api/trading/uta/:id/wallet/stage-modify-order**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:549` |
| 请求体 | `StageModifyOrderParams`：`orderId` + 可选 `totalQuantity`/`lmtPrice`/`auxPrice`/`trailStopPrice`/`trailingPercent`/`orderType`/`tif`/`goodTillDate` |
| 错误 | 404 / 400 |
| 副作用/幂等 | 追加 staging；非幂等 |
| 调用方 | `UTAAccountSDK.stageModifyOrder:321` |

**POST /api/trading/uta/:id/wallet/stage-close-position**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:561` |
| 请求体 | `StageClosePositionParams`：`aliceId` 必填，`qty` 空/缺省=全平，`subAccountId` 语义同上 |
| 错误 | 404 / 400 |
| 副作用/幂等 | 追加 staging；非幂等 |
| 调用方 | `UTAAccountSDK.stageClosePosition:328` |

**POST /api/trading/uta/:id/wallet/stage-cancel-order**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:573` |
| 请求体 | `{orderId}`；路由显式要求非空字符串（`:578-580`），其余字段忽略 |
| 错误 | 404 / 400 `{error:'orderId is required'}` / 400 |
| 副作用/幂等 | 追加 staging；非幂等 |
| 调用方 | `UTAAccountSDK.stageCancelOrder:335` |

### 2.7 One-shot 下单（stage→commit→push，UI 手单用）

共用 `runOneShot`（`:70-78`）与 `executeOneShotOrder`（`services/uta/src/domain/trading/order-entry.ts:42-63`）；`PHASE_STATUS`（`:63-67`）把 phase 映射为 stage/commit→400、push→500。body 先过 zod，失败返回 `400 {error, phase:'validate'}`。

**POST /api/trading/uta/:id/wallet/place-order**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:594` |
| 请求体 | `placeOrderSchema`（`:24-47`）：`aliceId`、`action(BUY/SELL)`、`orderType`、`message` 必填；`totalQuantity`/`cashQty` 至少一个（`.refine`）；其余可选 `symbol/ lmtPrice/ auxPrice/ trailStopPrice/ trailingPercent/ tif/ goodTillDate/ outsideRth/ parentId/ ocaGroup/ takeProfit/ stopLoss/ subAccountId`；数值为 `z.string().min(1)` |
| 成功响应 | `PushResult` |
| 错误 | 404 `{error:'UTA not found'}`；400 `{error, phase:'validate'\|'stage'\|'commit'}`；500 `{error, phase:'push'}` |
| 副作用/幂等 | 立即经 broker 下单；非幂等；用户填表即视为人工审批（`order-entry.ts` 头注释） |
| 调用方 | UI `trading.ts:175`；**无 SDK 方法** |

**POST /api/trading/uta/:id/wallet/close-position**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:609` |
| 请求体 | `closePositionSchema`（`:49-55`）：`aliceId`+`message` 必填，`symbol`/`qty`/`subAccountId` 可选；qty 保持字符串到 Decimal |
| 成功响应/错误 | 同 place-order |
| 副作用/幂等 | 平仓；非幂等 |
| 调用方 | UI `trading.ts:179` |

**POST /api/trading/uta/:id/wallet/cancel-order**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:625` |
| 请求体 | `cancelOrderSchema`（`:57-61`）：`orderId`+`message` 必填 |
| 成功响应/错误 | 同 place-order |
| 副作用/幂等 | 撤单；重复撤销第二次会因订单状态/`orderId` 不存在报错，非幂等 |
| 调用方 | UI `trading.ts:183` |

### 2.8 快照

**GET /api/trading/uta/:id/snapshots — 账户快照列表**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:642` |
| 请求 query | `limit`（默认 100）；UI 还发送 `startTime`/`endTime`，**路由不读取** |
| 成功响应 | `{ snapshots: UTASnapshotSummary[] }` |
| 错误 | 无：`snapshotService` 缺失 → `{snapshots:[]}`；store 抛错也被吞 → `{snapshots:[]}`（`:643`、`:650-652`）。无法区分“无数据”与“存储故障” |
| 副作用/幂等 | 只读幂等 |
| 调用方 | UI `trading.ts:262`；无 SDK 方法（`UTAManagerSDK.setSnapshotHooks` 是 no-op） |

**DELETE /api/trading/uta/:id/snapshots/:timestamp — 删除单条快照**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:654` |
| 请求 | 路径参数 `:timestamp`（ISO，路由做 `decodeURIComponent`；未做格式校验） |
| 成功响应 | `{ success: true }` |
| 错误 | 503 `{error:'Snapshot service not available'}`（实测）；404 `{error:'Snapshot not found'}` |
| 副作用/幂等 | 删除持久化快照；重复删除第二次 404，非幂等 |
| 调用方 | UI `trading.ts:270`；**与 `safe/knowledge/endpoints.md:70` 记录不符**（该文档写作 `DELETE /api/trading/uta/:id/snapshots`“wipe snapshots”，实现是带时间戳的删除单条） |

**GET /api/trading/snapshots/equity-curve — 跨账户权益曲线**

| 项 | 内容 |
|---|---|
| 注册 | `routes-trading.ts:664` |
| 请求 query | `limit`（默认 200）；同样**忽略 UI 发送的 `startTime`/`endTime`** |
| 成功响应 | `{ points: [{ timestamp, equity(string), accounts: Record<id, netLiquidation> }] }`；按分钟取整分组、按时间排序、缺失账户用 last-known 前向填充（`:683-712`） |
| 错误 | 无：service 缺失或抛错都返回 `{points:[]}`（`:666`、`:718-719`） |
| 副作用/幂等 | 只读幂等；复杂度 O(accounts × limit) |
| 调用方 | UI `trading.ts:275`；无 SDK 方法 |

### 2.9 Health

**GET /__uta/health — 就绪探测**

| 项 | 内容 |
|---|---|
| 注册 | `services/uta/src/main.ts:148` |
| 用途/触发 | Guardian readiness、Alice supervisor 轮询（200ms 间隔，15s 预算，`src/services/uta-supervisor/health.ts:29-39`） |
| 请求 | 无 |
| 成功响应 | `{ ok: true, startedAt: ISO, utas: number }` |
| 错误 | 无显式错误分支；进程未起时是连接失败而非 HTTP 错误 |
| 副作用/幂等 | 只读幂等 |
| 调用方 | `decodeUTAHealth`（严格校验三字段，任一不符抛 “Invalid UTA health response.”）；BFF `GET /api/trading/status`（代理自有，非 UTA 端点）复用它判断 available |

### 2.10 Simulator（9 条，全部 MockBroker-only）

差异总述（对 trading 面）：`resolveMock`（`routes-simulator.ts:82-91`）对未知 id → 404 `{error:UTA <id> not found}`，对非 MockBroker → 400 `{error:UTA <id> is not a simulator (broker=<类名>)}`；`parseBody`（`:93-103`）JSON 解析失败 → 400 `{error:'Invalid JSON'}`，schema 失败 → 400 `{error:'Validation failed', issues:[...]}`；**没有 health 门、没有 queryAccount 分类、所有操作立即生效（不经 git 三阶段）**；数值字段的 `numericString` 与 trading 侧定义不同（`:26` 接受 string|number 再转字符串）。Alice BFF 在 readonly 模式把整个 `/api/simulator` 判为 venue mutation → 403。

**GET /api/simulator/utas — 列出可用模拟账户**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:109` |
| 请求/响应 | 无参 → `{ utas: [{id,label}] }`（仅 `broker instanceof MockBroker`，`:111-113`） |
| 错误/副作用 | 无错误分支；只读幂等 |
| 调用方 | UI `ui/src/api/simulator.ts:71` |

**GET /api/simulator/uta/:id/state — 模拟器全量状态**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:117` |
| 成功响应 | `MockBroker.getSimulatorState()`（`MockBroker.ts:761-800`）：`cash`(string)、`markPrices[]`、`positions[]`（含 `avgCostSource: broker/wallet`、衍生字段）、`pendingOrders[]`（仅 `Submitted`） |
| 错误 | 404 未知 UTA；400 非 Mock |
| 副作用/幂等 | 只读幂等 |
| 调用方 | UI `simulator.ts:73` |

**POST /api/simulator/uta/:id/mark-price — 设置标记价并自动撮合**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:124` |
| 请求体 | `{ nativeKey: string, price: string\|number }` |
| 成功响应 | `{ filled: string[] }`（触达的挂单 id 列表；成交按 markPrice，优于限价语义，`MockBroker.ts:555-563`） |
| 错误 | 404/400（解析、校验、broker 抛出均 400 `{error}`） |
| 副作用/幂等 | 改内存标记价 + 自动成交挂单；非幂等（重复设置可能连续撮合不同单） |
| 调用方 | UI `simulator.ts:76` |

**POST /api/simulator/uta/:id/tick-price — 相对百分比移动标记价**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:138` |
| 请求体 | `{ nativeKey, deltaPercent: number }`（+5 表示 +5%） |
| 成功响应 | `{ filled: string[] }` |
| 错误 | 404；未设过 markPrice → 400 `{error:'...call setMarkPrice first'}` |
| 副作用/幂等 | 同上；非幂等 |
| 调用方 | UI `simulator.ts:79` |

**POST /api/simulator/uta/:id/orders/:orderId/fill — 手动成交挂单**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:152` |
| 请求体 | `{ price?: string\|number, qty?: string\|number }`；缺省价 = 该合约 markPrice → 挂单限价 → 兜底 `100`（`MockBroker.ts:~599`）；缺省量 = 全量 |
| 成功响应 | `{ ok: true }` |
| 错误 | 404；400（订单非 `Submitted`、qty≤0、qty 超总量、未知 orderId） |
| 副作用/幂等 | 改持仓/现金/订单状态；重复成交第二次 400，非幂等 |
| 调用方 | UI `simulator.ts:82` |

**POST /api/simulator/uta/:id/orders/:orderId/cancel — 强制撤单**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:168` |
| 请求体 | 无（仅路径参数） |
| 成功响应 | `{ ok: true }` |
| 错误 | 404；400（未知 orderId） |
| 副作用/幂等 | 状态置 Cancelled；重复调用 400（无幂等保证）；同步 handler，无 await |
| 调用方 | UI `simulator.ts:85` |

**POST /api/simulator/uta/:id/external-deposit — 模拟外部入金/空投**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:182` |
| 请求体 | `{ nativeKey, quantity, contract?: contractSchema }`；contract 的 `secType` 用 `z.enum(SEC_TYPES)` 白名单（含 OpenAlice 扩展 `CRYPTO_PERP`，`contract-discipline.ts:30-36`） |
| 成功响应 | `{ ok: true }` |
| 错误 | 404；400（校验/unknown nativeKey） |
| 副作用/幂等 | 覆盖式写入持仓（`avgCost = markPrice ?? 0`，`avgCostSource:'wallet'`，`MockBroker.ts:~688`）；同名重放=重置而非累加，**注意语义** |
| 调用方 | UI `simulator.ts:88` |

**POST /api/simulator/uta/:id/external-withdraw — 模拟外部提现**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:196` |
| 请求体 | `{ nativeKey, quantity }`（无 contract 字段） |
| 成功响应 | `{ ok: true }` |
| 错误 | 404；400（无该持仓、校验失败） |
| 副作用/幂等 | 持仓量递减，≤0 时删除该持仓；现金不变，非幂等 |
| 调用方 | UI `simulator.ts:91` |

**POST /api/simulator/uta/:id/external-trade — 模拟用户在交易所手动成交**

| 项 | 内容 |
|---|---|
| 注册 | `routes-simulator.ts:210` |
| 请求体 | `{ nativeKey, side: BUY/SELL, quantity, price, contract? }` |
| 成功响应 | `{ ok: true }` |
| 错误 | 404；400（qty≤0 等） |
| 副作用/幂等 | 更新持仓 + 现金，标记 wallet 来源；无仓位时 BUY 开多、SELL 开空（与 placeOrder 的“禁止裸卖空”不同，`MockBroker.ts:~730-745`）；非幂等 |
| 调用方 | UI `simulator.ts:94` |

---

## 3. 数据与状态

### 3.1 UTA 服务侧（路由相关）

| 状态 | 位置/形态 | 生命周期 |
|---|---|---|
| `UTAEngineContext` | 进程内对象（`services/uta/src/types.ts`），含 `utaManager`/`fxService`/`snapshotService?` | 进程级；`snapshotService` 可选，缺失时快照路由降级 |
| 路由实例 | 每个 `create*Routes()` 各返回一个 Hono 实例，无共享状态 | 进程级 |
| staging / pendingHash | 在 `UnifiedTradingAccount` + `TradingGit`，由 wallet 路由读写 | 进程内存 + git 持久化（详见 05） |
| 快照存储 | 由 `snapshotService` 持有（详见 06） | 磁盘 |
| 内存中无速率限制/会话/审计状态 | — | 路由层无状态 |

### 3.2 协议包侧

| 项 | 值 |
|---|---|
| 包名/版本 | `@traderalice/uta-protocol` 0.1.0（`packages/uta-protocol/package.json`） |
| 导出条件 | `exports['.']` = `openalice-source` → `./src/index.ts`；`import` → `./dist/index.js`；`types` → `./dist/index.d.ts` |
| 构建产物 | `files: ["dist"]`，`dist` 未构建；仓库内消费方通过 `customConditions: ["openalice-source"]`（`services/uta/tsconfig.json`）走源码 |
| 依赖 | `@traderalice/ibkr`(workspace)、`decimal.js`、`zod@^4.3.6` |
| 入口导出 | `src/index.ts`：types/index、schemas/index、client/UTAClient、brokers/{preset-catalog,presets,search-rules}、broker-research |
| 仓库内消费者 | 根 `package.json:113`、`services/uta`、`uta-broker-{ibkr,alpaca,ccxt,leverup,longbridge}` 各自 `workspace:*` |

### 3.3 协议包契约清单（按用途分组）

| 分组 | 主要符号 | 说明 |
|---|---|---|
| 错误 | `WireBrokerError`（`types/errors.ts:10-19`）、`BrokerErrorCode`、`BrokerError` 类（`types/broker.ts:33-79`） | `WireBrokerError` 是**未被使用的设计稿**；`BrokerError` 被 UTA 领域层与 Alice 工具层真实使用 |
| 健康/摘要 | `UTAReach`、`UTATier`、`BrokerHealth`、`BrokerHealthInfo`、`UTASummary`、`AccountCapabilities` | `/uta` 与错误体 `health` 字段的形状 |
| 账户/持仓/订单 | `AccountInfo`、`SubAccountRef`、`Position`、`PositionRisk`、`OpenOrder`、`Quote`、`MarketClock`、`PlaceOrderLeg`、`PlaceOrderResult` | 账户读取端点的响应类型 |
| 合约 | `ContractSearchHit`（扁平）、`ContractSearchResult`（分组，仅 in-process 用）、`ExpandContractFilters`、`OptionGridEntry`、`ContractExpansion`、`optionResearchSchema`、`orderBookSchema`、`BrokerResearch` | 唯一被**双侧真实使用**的 zod schema 是 `optionResearchSchema`/`orderBookSchema`：UTA 路由 `safeParse` + Alice 工具 `inputSchema` |
| Trading-as-Git | `Operation`（8 变体：placeOrder/modifyOrder/closePosition/cancelOrder/syncOrders/observeExternalOrder/reconcileBalance/[推断其余]）、`OperationResult`、`GitState`、`GitCommit`、`AddResult`、`CommitPrepareResult`、`PushResult`、`RejectResult`、`GitStatus`、`OperationSummary`、`CommitLogEntry`、`GitExportState`、`SyncResult`、`OrderStatusUpdate`、`PriceChangeInput`、`SimulatePriceChangeResult`、`Stage*Params`、`getOperationSymbol` | wallet/stage/one-shot/sync/simulate 端点的请求与响应类型 |
| 历史投影 | `HistoryContract`、`OrderHistoryEntry`、`TradeHistoryEntry` | order-history / trade-history 响应 |
| Broker preset | `BROKER_PRESET_CATALOG`、`BUILTIN_BROKER_PRESETS`、`SerializedBrokerPreset`、`BrokerPresetDef`、`getBrokerPreset`、`isPaperPreset`、各 `*_PRESET` 常量、`BrokerEngine`、`BrokerConfigField`、`IBroker`、`deriveUtaId`、`mintInstanceId`、`ModeOption`、`AccountCapabilities` | UTA broker factory、Alice 配置向导、UI 预设目录（`ui/src/theme/semanticColors.spec.ts:189` 还直接读源文件做主题一致性校验） |
| 搜索规则 | `normalizeBrokerSearchPattern`、`AssetClassHint` | 唯一“函数式契约”导出，UTA 与 Alice 两侧都调用 |
| Client | `createUTAClient`、`UTAClientOptions`、`UTAClient`、`RequestOpts`、`UTAHttpError` | 无响应校验、15s 超时、`put`/`delete` 无调用方 |
| 声明合并 | `types/contract-ext.ts` 给 IBKR `Contract` 加 `aliceId?` | 通过 `types/index.ts` 的 side-effect import 生效 |
| Schema 目录 | `src/schemas/index.ts` | 内容为 `export {}`（第 10 行），仅注释宣称端点级 schema + zValidator |

### 3.4 协议包与路由的一致性检查（逐项证据）

| 检查 | 结论 | 证据 |
|---|---|---|
| 每端点 Request/Response schema | **不存在** | `schemas/index.ts:1-10`；全仓 `zValidator` 仅出现在该注释里 |
| Broker 错误线格式 | **不一致** | 声明 `WireBrokerError{code,message,transient,hint?}`（`errors.ts:10`），实际 `{error,code,transient}`（`routes-trading.ts:117-122`）；`WireBrokerError` 全仓零引用（grep 仅命中自身与 UTAClient 的注释）；hint 由 Alice 工具层自造（`src/tool/trading.ts:36-46`） |
| 合约搜索响应形状 | **协议类型与 HTTP 不符** | 协议定义 `ContractSearchResult`（分组，`manager.ts`），HTTP 返回扁平 `ContractSearchHit[]`；SDK 注释记录了这次漂移（`UTAAccountSDK.ts:210-216`） |
| numericString 定义 | **两侧不同** | trading：`z.string().min(1)`（`:21`）；simulator：`z.union([z.string().min(1), z.number()]).transform`（`:26`） |
| stage-* 请求校验 | **缺失** | 4 条 stage 路由 body 直传（`:542`、`:554`、`:566`、`:583`），schema 只写在 one-shot 侧 |
| option/order-book 请求校验 | **一致** | 双侧共用 `packages/uta-protocol/src/broker-research.ts` 的 schema |
| 部分端点请求校验 | **缺失** | `/contract/expand`、`/historical`、POST `/quote`、`/contracts/details`、`/sync`、`/simulate-price`、`/test-connection`（后者借用 Alice 的 `utaConfigSchema`） |
| `queryAccount` 错误分类覆盖 | **不完整** | expand/historical/details/POST quote 用 try/catch→500，绕过 offline/transient 判定（`:325`、`:372`、`:392`、`:404`） |
| SDK 方法指向的端点 | **3 条不存在** | `getState` → `GET /wallet/state`（`UTAAccountSDK.ts:278`）、`exportGitState` → `GET /wallet/export`（`:282`）、`contractFromAliceId` → `GET /contract-by-alice-id`（`:376`） |
| UI 类型来源 | **手抄镜像** | `ui/src/api/types.ts:442` 注释“the UI does not import uta-protocol, so keep these in lockstep”；`ui/package.json` 无该依赖 |
| 文档与现实 | **有陈旧条目** | `safe/knowledge/endpoints.md:70` 的 `DELETE .../snapshots` 与实现 `:654` 不符；`ui/src/api/simulator.ts:3` 指向不存在的 `src/webui/routes/simulator.ts`（实际在 `src/webui/plugin.ts:262` 挂载代理） |

---

## 4. 外部交互

### 4.1 调用方向

| 调用方 | 被调方 | 通道 | 关键约束 |
|---|---|---|---|
| 浏览器 UI | Alice BFF `/api/trading/*`、`/api/simulator/*` | HTTP 同源 | cookie/loopback/CSRF 中间件；mode 门 |
| Alice BFF | UTA `/api/trading/*`、`/api/simulator/*` | HTTP → `resolveUTAUrl()`（`OPENALICE_UTA_URL` 或 `OPENALICE_UTA_PORT`，默认 `http://127.0.0.1:47333`） | 30s 超时、仅转发白名单 header、逐字节透传 body 与状态码 |
| Alice 进程内 SDK | UTA 同上 | HTTP | 15s 超时（`UTAClient`）；无重试；无响应 schema 校验 |
| Guardian | UTA `/__uta/health` | HTTP | 1s/200ms 轮询，仅观测 |
| Alice BFF `/api/trading/config/test-connection` | UTA `POST /api/trading/test-connection` | HTTP 直连（不经代理） | 只在 lite 模式拦截；其余状态码原样透传 |
| UTA 路由 | broker 包 / `packages/ibkr` | 进程内 | 路由动态 `import('@traderalice/ibkr')` 构造 `Contract`（`:310`、`:321`、`:384`、`:402`） |
| UTA 路由 | `fxService` / `snapshotService` | 进程内 | FX 失败静默；快照失败静默 |

### 4.2 契约要点

- **无版本协商**：没有 header、query 或 body 字段表达协议版本；唯一“版本”是包 `0.1.0`。破坏性变更只能同仓同版本一起改（协议包 `src/index.ts:1-13` 注释声称“未来可能异构部署”，但没有配套机制）。
- **无发布路径**：`scripts/preflight-public-cli-authority.mjs:9-12` 的发布清单只含 `openalice*` 二进制包；`packages/uta-protocol` 不在其中，`files:["dist"]` 且 `dist` 未构建，因此若被打包会是空包。无 changeset 目录、无 CHANGELOG。
- **兼容面**：消费方全部用 `workspace:*`（根 `package.json:113`、`services/uta/package.json:18` 及 5 个 broker 包），版本升级不需要协调。
- **协议泄漏进领域层**：`services/uta/src/domain/trading/brokers/types.ts:7` 直接 `export * from '@traderalice/uta-protocol'`；`contract-search-rules.ts:7`、`order-history.ts:22` 等也把协议包当作领域类型源。协议包的边界在实现上并不等于 HTTP 边界。

---

## 5. 配置项、默认值、环境变量

| 配置 | 默认值 | 作用点 |
|---|---|---|
| `OPENALICE_UTA_PORT` | 47333 | `services/uta/src/main.ts:38`（绑定端口与日志） |
| `OPENALICE_UTA_URL` | 未设时 `http://127.0.0.1:${port}` | `src/services/uta-supervisor/url.ts:11-16`（Alice 侧代理目标） |
| `OPENALICE_LITE_MODE` / `OPENALICE_UTA_DISABLED` | 真值之一即禁用 | `url.ts:18-20`；BFF 返回 503 |
| 代理超时 | 30_000ms（`PROXY_TIMEOUT_MS`）、status 1_000ms | `src/webui/routes/trading-proxy.ts:24-25` |
| SDK 超时 | 15_000ms（`timeoutMs` 可覆盖） | `packages/uta-protocol/src/client/UTAClient.ts:56` |
| 路由默认 limit | wallet/log 20、order/trade-history 50、snapshots 100、equity-curve 200、order-book 20、option limit 无默认（broker 兜底） | 见各端点表 |
| `ALLOWED_ASSET_CLASSES` | equity/crypto/currency/commodity/unknown；非法值降级 unknown | `routes-trading.ts:81-83`、`:160-162` |
| `SEC_TYPES` 白名单 | IBKR canonical + `CRYPTO_PERP` | `services/uta/src/domain/trading/contract-discipline.ts:30-36`；simulator 用它做 400 门 |
| `CATALOG_REFRESH_MS` | 6h（非 HTTP 面，但影响路由读到的 catalog） | `main.ts:39` |
| Feature 开关 | 路由面无开关；唯一“开关”是 `ctx.snapshotService` 是否存在及 Alice 的 trading mode（pro/readonly/lite） | `types.ts`、`trading-mode.ts` |

---

## 6. 不变量、时序与并发假设

1. **数值精度**：所有金额/数量走字符串；`Decimal` 经过 JSON 后仍是字符串。破坏该不变量会引入 IEEE-754 误差（`routes-trading.ts:16-20`、`git.ts:250-255`）。
2. **aliceId 归属**：`aliceId = "{utaId}|{nativeKey}"`；`contractFromAliceId` 对格式错误或跨账户都抛普通 `Error`（`UnifiedTradingAccount.ts:534-545`）。在 option/order-book 端点上这是 503（UNKNOWN 分类），在 expand/details 上是 500——都不是 400。
3. **审批 CAS**：push/reject 必须带 `expectedPendingHash`；`PENDING_HASH_REQUIRED`（缺令牌）与 `PENDING_HASH_CONFLICT`（令牌过期）都是 409。并发保护在 `TradingGit`（`inflightWrite` + `PendingHashConflictError`）而不在路由层。
4. **one-shot 的顺序与回滚**：stage 抛错不入库（无清理）；commit 失败的注释说明“没有 pending hash 就没有安全 reject 授权”，因此**不回滚 staging**（`order-entry.ts:52-59`）；push 失败保留已提交的 pending 记录。也就是说失败重试前，staging 可能已含上一次的部分操作。
5. **状态码与副作用不匹配**：`/reconnect` 用 200/500 表达业务成功/失败（HTTP 语义弱）；`/test-connection` 用 400 包裹业务失败；`/sync` 与 `/simulate-price` 的 body 解析失败按空输入继续（可能静默做无用功）。
6. **路由原子性**：单请求内多步（fx-rates 遍历账户、equity-curve 遍历快照）没有事务或快照隔离，读到的是各账户不同时刻的混合数据。
7. **无并发限制**：同一 UTA 的多个写请求可并行进入领域层，只有 git 层做串行化/冲突检测；simulator 的 mark-price/fill 之间无锁。
8. **幂等性汇总**：GET 全部幂等；stage-*/commit/sync/push/reject/one-shot/simulator 全部非幂等；唯一近似幂等的是 `external-deposit`（覆盖式写入，重放等于重置而非累加——语义上更危险）。
9. **健康门只在 `queryAccount` 路径**：走 queryAccount 的 9 条读取路由会 `nudgeRecovery()`；expand/historical/details/quote/search/fx 不受影响，也不触发恢复。
10. **无审计**：UTA 路由不打请求日志；审计信息只存在于 git 日志（谁 push 了什么）与 event-log。

---

## 7. 测试覆盖

**HTTP 层 spec（`services/uta/src/http/`，共 4 个文件 644 行）**

| 文件 | 行数 | 覆盖行为 |
|---|---|---|
| `broker-research.spec.ts` | 42 | option-chain 过滤/分页/feed 透传不丢 metadata；`limit=1001` 在触达 broker 前 400；跨账户 aliceId 与非 STK 标的不返回 200；order-book 经账户解析合约并限制 levels |
| `routes-trading-wallet.spec.ts` | 79 | push/reject 缺 `expectedPendingHash` → 409 且**不产生副作用**（断言 mock 未被调用）；hash 变化 → 409 `PENDING_HASH_CONFLICT` 且不当作成功；带 hash 时正常 push |
| `trading-order-entry.spec.ts` | 315 | one-shot 三条路由：happy path、缺 message 的 zod 400、totalQuantity/cashQty 二选一、stage/commit/push 各自的 `phase` 与状态码、404、字符串精度（`'10'` 不以 float 到达 staging）、message 透传到 commit |
| `simulator.spec.ts` | 208 | 用**真实 MockBroker** 驱动：`/utas` 只列 Mock；state 全量；未知 UTA 404；非 Mock 400；mark-price 设置价并自动成交触达挂单；畸形 body 400；external-deposit 绕过订单管线；external-trade 更新持仓+现金并打 wallet 标签；fill 默认价与未知 orderId 400 |

**Alice 工具层 spec（`services/uta/src/__tests__/trading-tools.spec.ts`，387 行）**

覆盖 `UTAManager.resolve`（含 #390 tier 过滤相关行为，经真实 `UTAManager`）、`resolveOne`、`createTradingTools` 的 listUTAs/searchContracts/getOrders 摘要化/getQuote/getContractDetails、`placeOrder.inputSchema` 对空字符串可选数值的宽容与对 `'0'` 的拒绝。注意其手法是把 in-process `UTAManager` **强转**成 `UTAManagerSDK`（第 35 行 `asSDK`），因此它测的是工具层逻辑，**不经过 HTTP**，无法发现 wire 形状漂移；文件头注释也承认 `getFxRates` 缺失被 try/catch 掩盖。

**协议包测试**：仅 `packages/uta-protocol/src/types/broker.spec.ts`（`BrokerError.from` 两个用例，含跨包 name/code 保留）。

**无 HTTP 测试的端点（38/49）**：`GET /uta`、`/equity`、`/contracts/search`、`/fx-rates`、`POST /test-connection`、`/reconnect`、`/sync`、`/simulate-price`、`GET /subaccounts`、`/account`、`/positions`、`/orders`、`/market-clock`、`/quote/:symbol`、`POST /quote`、`/contract/expand`、`/historical`、`/contracts/details`、`GET /wallet/log`、`/order-history`、`/trade-history`、`/wallet/show/:hash`、`/wallet/status`、`POST /wallet/commit`、四条 stage-*、`GET /uta/:id/snapshots`、`DELETE /snapshots/:timestamp`、`GET /snapshots/equity-curve`、`GET /__uta/health`，以及 stage-* 的“无 schema 校验”行为本身。

**未被任何测试断言的路由层行为**：Hono 默认 500 非 JSON 路径、`{error,health}` 离线 503、`transient/permanent` 到 500/503 的映射、`Nothing to push/reject` 400、`utasConfigured` 条件字段、快照降级空数组、equity-curve 前向填充算法。

---

## 8. 观察到的问题、技术债、耦合与重构关注点

1. **协议包的空壳 schema 层是最主要的认知陷阱**：`schemas/index.ts` 的注释描述了一套完整的“每端点 schema + zValidator + 双侧共享解析”体系，但函数体是 `export {}`（`:1-10`），`zValidator` 全仓零使用。任何新读者（含 AI）都会先相信这段注释。
2. **错误契约事实上未定义**：6 种响应体形状并存（见 2.0），`WireBrokerError` 未被使用，`hint` 字段在协议里声明却在实现侧被工具层自造（`src/tool/trading.ts:36-46`）。UI 的手写 `OrderErrorResponse`（`ui/src/api/types.ts:666-671`）只覆盖了其中一种。
3. **同一语义两种状态码**：离线 503 vs 跨账户 aliceId 503 vs 不支持的 broker 能力 503 vs CONFIG 错误 500 vs 400——调用方无法区分“该重试”“该改配置”“该改请求”（`routes-trading.ts:104-123` 对比 `:372`、`:392`）。
4. **校验强度按端点分布极不均匀**：2 个 zod schema（option/order-book）严格、3 个 one-shot schema 严格、simulator 7 个 schema 严格，而 4 条 stage 路由与 expand/historical/details/quote/sync/simulate 完全无校验；`expand` 甚至把 `aliceId` 缺失降级成 `String('')` 再交给领域层报错。
5. **`numericString` 双定义 + 数值宽松度不一致**：trading 只接受非空字符串（`'10'` 可以，`10` 不行），simulator 接受两者并转字符串。手写 UI 时容易踩坑。
6. **隐藏的能力缺失**：`NotImplementedInSDK` 引用的 3 条路由并不存在（`UTAAccountSDK.ts:278/282/376`），SDK 注释里还有已经过期的“Phase 1 前会 404”（`getHistorical`）与已修复的历史 bug 叙述（`searchContracts`）。
7. **快照端点吞异常**（`:643`、`:650-652`、`:718-719`）使“无数据”与“存储损坏”不可区分；同时 `startTime`/`endTime` 被 UI 发送但被服务端忽略，属静默的参数丢失。
8. **legacy 死路由**：`GET /uta/:id/quote/:symbol` 无任何调用方，且每次请求动态 import `@traderalice/ibkr` 构造只带 symbol 的 `Contract`（丢失 secType/exchange/currency，对衍生品语义不完整）。
9. **注释与实现矛盾**：`simulate-price` 的两段注释互相否定（`:244-245` vs `:255-256`）；`MockBroker` 文档说 `setMarkPrice` 由模拟器调用，而实现是通用 git 计算。文档 `safe/knowledge/endpoints.md:70`、`ui/src/api/simulator.ts:3` 也与实现不符。
10. **协议包被当作领域类型桶使用**：`brokers/types.ts:7` 的 `export *`、`contract-search-rules.ts:7` 的 `export *` 让 HTTP 契约包同时成为 UTA 内部类型源；拆分 UTA 时这会变成双向耦合。
11. **无版本/发布机制**：0.1.0 + `workspace:*` + 未构建 `dist` + 不在发布清单（`scripts/preflight-public-cli-authority.mjs:9-12`）。协议包 `src/index.ts` 注释称未来可能异构部署，但没有版本协商字段、没有兼容测试、没有跨版本 fixture。
12. **UI 手抄类型**（`ui/src/api/types.ts:442` 注释明说）使协议演进需要三处同步：协议包、SDK、UI 镜像；`PlaceOrderRequest` 等已与 schema 逐字段对齐但无机制保证。
13. **测试的“形状盲区”**：`trading-tools.spec.ts` 用 `asSDK` 强转绕开 HTTP，因此协议契约（字段名、包装层、错误体）在测试中从不被验证；HTTP spec 只覆盖 3 组端点。
14. **BFF 仅按路径子串做安全判断**：`isVenueMutation`（`trading-proxy.ts:193-206`）用 `includes('/wallet/push')` 等子串匹配，新增写路由（例如未来加 `wallet/stage-push` 或别名路径）不会被 readonly 模式拦截，需要人工记得同步这份列表。
15. **`app.all('*')` 转发时丢弃绝大多数请求头**（白名单仅 8 个），任何基于 header 的能力协商/追踪（除 `x-request-id`）都无法穿透到 UTA。

---

## 9. 未探索区域与开放问题

- 快照的持久化格式、保留策略与 equity-curve 的取数边界：见 `06-snapshots-and-guards.md`。
- staging/commit/push 的领域语义、`pendingHash` 的生成规则与 `TradingGit` 的并发实现细节：见 `05-staging-approval-ledger.md`。
- `Operation` 联合的完整变体清单（本报告只逐字核对了 6 个）与各变体在 push 时的行为：见 `05`。
- broker 包如何实现 `BrokerResearch` / `expandContract` / `getHistorical`，以及哪些 broker 缺哪个能力：见 `07-brokers-and-packs.md`。
- 协议类型中 `AccountCapabilities`、`PositionRisk`、`OptionGridEntry` 的字段级语义：见 `03`、`04`。
- Alice 侧对每个端点的调用策略（重试、降级、UI 展示）与 `alice-uta` CLI 的映射：见 `08-alice-consumers-and-ui.md`。
- 开放问题：UTA 是否有意长期保持“零鉴权 + 127.0.0.1 信任边界”？若未来把 UTA 拆到独立主机（协议包注释所设想），现有 BFF 的 mode 门与 auth 中间件都要重新设计。
- 开放问题：`schemas/index.ts` 是计划实现还是已废弃的设计残留？这决定了协议包重构是“补全”还是“删除该层并明确以类型为契约”。
- 开放问题：`test-connection` 让 UTA 引用 Alice 的 `utaConfigSchema`（`@/core/config.js:448`，`services/uta/tsconfig.json` 的 `@/*` 别名指向 `../../src/*`），这条反向依赖在拆分后如何切断，未找到设计说明。
