# 04 — 市场数据、合约契约与 FX

> 摘要（≤10 行）：本区域是 UTA 的"标的是谁 + 值多少钱"层。合约身份由 `aliceId = {utaId}|{brokerNativeKey}` 定义，`nativeKey` 由各 broker 自定义（IBKR=conId、CCXT=统一线格式 symbol、Alpaca=ticker）。`contract-discipline.ts` 用 IBKR SecType 分类法做结构性校验，是唯一的"什么是合规合约"权威。合约搜索是**数据 vendored symbol → broker 搜索模式**的启发式桥接（剥离报价后缀）+ 各 broker 自己的目录检索 + 共享模糊打分（6 档，上限 50），**没有跨 broker 去重**。keyless 数据源（binance/okx/bybit）是不落盘、只读、只提供行情/K 线的虚拟 UTA。FX 是"最终永不失败"的六级回退链，保证任何币种都能换算成 USD，代价是可能静默用硬编码汇率。行情/报价本身**不缓存**，缓存的是目录（6h 刷新）与 FX（分层 TTL）。

---

## 1. 概览与职责边界

### 1.1 本区域负责

- **合约标识模型**：`aliceId` 的构造、解析、跨账户校验、反向重建（`UnifiedTradingAccount` 的 aliceId 管理段）。
- **合约结构分类法**：SecType 枚举、每种 secType 的必填字段、校验与硬失败。
- **合约搜索**：模式归一化 → 目标账户筛选 → 并发 fan-out → broker 侧候选检索 → 模糊排序 → 拍平返回。
- **合约翻译**：把统一 `Contract` 翻译成各 broker 的原生表示（反之亦然），以及各 broker 的搜索实现与资产类别标注。
- **keyless 数据源**：无凭据公共行情源的构造、注入、能力声明与拒绝面。
- **FX**：汇率来源优先级、缓存 TTL、USD 换算、缺失汇率行为及所有调用点。
- **行情/报价获取路径与刷新策略**（quote / bars 的取值与刷新语义）。

### 1.2 本区域不负责

- HTTP 端点总表、状态码、鉴权与 BFF 代理转发 → 见 `02-http-api-and-protocol.md`（本文件只在行为差异处引用具体端点）。
- broker 连接生命周期、注册、preset、健康恢复、catalog 刷新调度机制本身 → 见 `07-brokers-and-packs.md`（本文件写"catalog 缓存对搜索的影响"这一侧）。
- 订单/持仓的生命周期、成本基准、市场价值的最终记账口径 → 见 `03-account-orders-positions.md`（本文件只写 `multiplier` 与 FX 换算的输入侧）。
- 快照与守卫 → 见 `06-snapshots-and-guards.md`。
- Alice 侧 UI/工具消费面的完整行为 → 见 `08-alice-consumers-and-ui.md`（本文件写"消费者对合约/FX 契约的假设"这一侧）。
- K 线联邦、BarService 质量/新鲜度体系 → 见 `02`/`08`；本文件只写 broker 侧 `getHistorical` 的合约解析与拒绝行为。

### 1.3 上下游

- **上游**：Alice 主进程（AI 工具、UI 经 BFF 代理）、UTA HTTP 路由层、catalog 定时刷新（`services/uta/src/main.ts:39,134-141`）。
- **下游**：TWS/IBKR bridge、CCXT exchange 实例、Alpaca REST、Longbridge SDK、LeverUp relayer + Pyth、TraderHub（FX 表 + 参考数据）、market-data currency client（yfinance）。

### 1.4 一句话数据流

Alice 的分析页/LLM 给出**数据 vendor 的 symbol**（`AAPL` / `BTCUSD` / `700.HK`），本区域把它变成**可下单的 broker 合约身份**（`ibkr-tws|265598` / `bybit-main|BTC/USDT:USDT` / `alpaca-paper|AAPL`），下游一切操作都以该身份为键。

---

## 2. 功能清单

### 2.1 aliceId 构造（stamp）

- **触发方式**：任何 broker 返回合约的路径（搜索/持仓/挂单/报价/合约详情/展开）。
- **输入**：broker 返回的 `Contract` 实例。
- **处理步骤/规则**：调用 `broker.getNativeKey(contract)` 取原生键，拼接 `{utaId}|{nativeKey}` 写回 `contract.aliceId`。
- **输出/副作用**：原地修改传入的 Contract（调用方持同一对象引用）。
- **证据**：`services/uta/src/domain/trading/UnifiedTradingAccount.ts:510-513`。
- **调用点**（共 9 处，覆盖所有对外返回合约的出口）：`UnifiedTradingAccount.ts:168,169`（状态聚合）、`:973`（外部观测到的未知订单）、`:1028`（持仓）、`:1128`（挂单）、`:1135`（报价）、`:1176`（合约展开结果）、`:1182`（搜索结果）、`:1201`（合约详情）。

### 2.2 aliceId 解析与反向重建

- **触发方式**：HTTP 路由或 AI 工具传入 `aliceId`（可能只有 aliceId、没有 symbol）。
- **输入**：字符串 aliceId。
- **处理步骤/规则**：
  1. `parseAliceId` 用**首个** `|` 切分（`indexOf`，故 nativeKey 自身可含 `|`）；无 `|` 返回 null。
  2. 校验 `utaId` 是否属于本账户实例；跨账户直接抛错（不静默 no-op）。
  3. 调 `broker.resolveNativeKey(nativeKey)` 重建完整 Contract。
  4. 回填 `contract.aliceId`。
- **输出/副作用**：完整可路由的 Contract；失败抛普通 `Error`（不是 BrokerError）。
- **错误与边界**：格式非法 → 提示"使用 searchContracts 获取合法标识"；跨 UTA → 指明实际归属账户；IBKR 的 `issuer:` 目录键**故意**在这里被 `resolveNativeKey` 拒绝（目录不可交易），必须走 `expandContract`。
- **证据**：`UnifiedTradingAccount.ts:516-521`（parse）、`:534-545`（rebuild）、`brokers/ibkr/IbkrBroker.ts:898-905`（issuer 拒绝）、`:1160-1163`（expand 注释说明 hub 键绕过 resolveNativeKey）。

### 2.3 aliceId 存根展开（overlay）

- **触发方式**：调用方传 `{ aliceId, ...若干显式字段 }`，但 `symbol`/`localSymbol` 为空。
- **输入**：部分填充的 Contract。
- **处理步骤/规则**：先按 aliceId 展开完整合约，再**逐字段 overlay 非默认值**；跳过 `aliceId`、`undefined`、`''`、`null`、数值 `0` 与 `UNSET_INTEGER`/`UNSET_DOUBLE`。
- **输出/副作用**：展开后的 Contract（新对象）。
- **错误与边界**：这是一段被注释记录过的历史 bug 修复区——早期"无差别拷贝"会把 `conId` 覆盖回 0，导致 TWS 报 error 321（by-conId 报价路径在线上静默失效）。因此**数值 0 与哨兵值一律视为"未设置"**。
- **证据**：`UnifiedTradingAccount.ts:1205-1235`（注释 `:1216-1231`）。

### 2.4 合约结构校验（contract discipline）

- **触发方式**：所有 broker 的合约构造都会经过 `buildContract`，其末尾调用 `assertContract`。
- **输入**：IBKR `Contract` 实例。
- **处理步骤/规则**：见 §3.2 的规则表。
- **输出/副作用**：`{ok:true}` 或 `{ok:false, errors:string[]}`；`assertContract` 把 errors 用 `; ` 连接后抛 `Invalid contract: ...`。
- **错误与边界**：校验**只在 broker 输出边界**生效（`buildContract` 返回前）。`getContractDetails` 传入的查询 Contract、`expandContract` 的手工拼装 Contract、`resolveNativeKey` 的兜底 Contract **都不经过**该校验（见 §8.2）。
- **证据**：`services/uta/src/domain/trading/contract-discipline.ts:72-100,107-112`；`brokers/contract-builder.ts:57-74`。

### 2.5 合约搜索（聚合层）

- **触发方式**：`GET /api/trading/contracts/search`（路由层，`services/uta/src/http/routes-trading.ts:150-165`）或 AI 工具 `searchContracts`（Alice 侧自有实现，`src/tool/trading.ts:220-280`）。
- **输入**：`pattern`（必填）、`assetClass`（可选，缺省 `unknown`，路由侧用 `ALLOWED_ASSET_CLASSES` 白名单过滤非法值，`routes-trading.ts:81-83`）、`source`/`accountId`（可选，指定单一账户）。
- **处理步骤/规则**：
  1. 归一化：`normalizeBrokerSearchPattern(pattern, assetClass)`；结果为空串 → 直接返回 `[]`。
  2. 目标选择：给了 `source` 就用指定账户（**绕过 `asVendor` 过滤**）；否则取全部账户并过滤 `asVendor !== false`。目标为空 → `[]`。
  3. 并发 fan-out：`Promise.allSettled`，每个目标调 `uta.searchContracts(brokerPattern)`。
  4. 失败处理：**静默跳过** `rejected` 的分支（不记录、不上抛）。
  5. 拍平：每条 hit 携带 `source`（账户 id）、`contract`、`derivativeSecTypes`、`assetClass`（来自 `broker.assetClassFor?.(contract)`，未实现者为 `undefined`）。
- **输出/副作用**：`ContractSearchHit[]`；**不做跨 broker 去重**，同一合约在多账户下会出现多次（连 `aliceId` 都不同）。
- **错误与边界**：搜索本身不会因单个 broker 故障而报错，代价是"部分失败"与"真的没匹配"在响应中不可区分。UI 侧有 `utasConfigured` 字段区分"0 个账户"这一种特殊情况。
- **证据**：`services/uta/src/domain/trading/contract-search.ts:27-65`；协议类型 `packages/uta-protocol/src/types/manager.ts:50-58`。

### 2.6 合约搜索（模式归一化）

- **触发方式**：聚合层与 Alice AI 工具各调用一次（两处实现同一函数，来自协议包）。
- **输入**：vendor symbol + assetClass hint。
- **处理步骤/规则**：见 §3.3。
- **输出/副作用**：纯函数，无副作用。
- **错误与边界**：`crypto`/`currency` 分支下若匹配到报价后缀，返回值会被**强制大写**；未匹配时不改大小写（`trim` 后原样返回）。**幂等性不成立**（见 §6.3）。
- **证据**：`packages/uta-protocol/src/brokers/search-rules.ts:36-55`；UTA 侧兼容 shim `services/uta/src/domain/trading/contract-search-rules.ts`（re-export 协议包）。

### 2.7 合约搜索（各 broker 实现）

| Broker | 检索方式 | 候选过滤 | 特殊行为 | 证据 |
|---|---|---|---|---|
| IBKR | TWS `reqMatchingSymbols`（服务端模糊） | 无（信任 TWS 排序） | `.` 后缀拆分为 base + pairCurrency；`CASH` 且无 conId 的 hub 会**内联展开**成具体货币对 | `IbkrBroker.ts:242-268`、`expandCashHub 270-287` |
| CCXT | 本地 markets 表 + 共享模糊排序 | `active !== false`、base/quote 齐全、quote ∈ {USDT,USD,USDC} | 先按 swap>future>spot>option、USDT>USD>USDC 预排序（稳定排序的 tiebreak 来源）；坏市场 try/catch 跳过并 `console.warn` | `CcxtBroker.ts:434-503`（候选 442-449、预排序 456-464、跳过 480-486） |
| Alpaca | 本地 catalog（`/v2/assets`）+ 模糊排序 | catalog 已加载则全量；**未加载时回显单条** | STK 行统一标注 `derivativeSecTypes=['OPT']` | `AlpacaBroker.ts:241-266`（回显 247-251、标注 263） |
| Longbridge | 无模糊端点 | — | 恒回显单条：`makeContract(pattern)`，裸 ticker 默认 `.US` | `LongbridgeBroker.ts:237-244`；`longbridge-contracts.ts:147-151` |
| LeverUp | 本地 pair 列表 `includes` 匹配 | base 或 symbol 包含查询串 | 全部映射为 `CRYPTO_PERP`/`LEVERUP` | `LeverupBroker.ts:169-181`、`pairToContract 192-203` |
| Mock | 无 | — | **忽略 pattern**，恒返回一条 AAPL 桩 | `MockBroker.ts:261-266` |

### 2.8 模糊排序（fuzzy-rank）

- **触发方式**：缓存型 broker（Alpaca / CCXT / Mock，以及 LeverUp 之外的目录型实现）在拿到候选集后调用。
- **输入**：候选数组（至少要有 `contract.symbol`；可选 `base`/`quote`/`name`/`localSymbol`/`description`）、查询串、可选 `limit`。
- **处理步骤/规则**：逐条打分（见 §3.4 表），丢弃 0 分项，按 `(-score, 原始索引)` 稳定排序，截断到 `limit`（默认 50）。
- **输出/副作用**：`ContractDescription[]`，每条 `contract` 是原候选的**浅克隆**（`Object.assign(new Contract(), …)`）。
- **错误与边界**：空/纯空白查询 → `[]`；查询串里的正则元字符会被转义（`BRK.B` 安全）；`limit` 之外的候选被丢弃且**无 total 字段**告知调用方被截断。
- **证据**：`services/uta/src/domain/trading/brokers/fuzzy-rank.ts:49-73`（打分）、`:75-99`（排序与 limit）。

### 2.9 合约翻译（统一 → broker 原生）

| 目标 | 入口 | 关键规则 | 证据 |
|---|---|---|---|
| IBKR | `ibkr-contracts.makeContract` | 默认 STK + SMART + USD；**无翻译层**（统一 Contract 就是 IBKR 原生类型） | `ibkr-contracts.ts:15-22` |
| Alpaca | `alpaca-contracts.makeContract` | OCC 期权正则识别（8 位数字串反推到期/行权价/权利，multiplier 固定 `'100'`）；`assetClass==='crypto'` 或含 `/` → CRYPTO 对；否则 STK/SMART/USD | `alpaca-contracts.ts:20-38`（正则 22、crypto 30-36） |
| CCXT | `ccxt-contracts.marketToContract` | `spot→CRYPTO`、`swap→CRYPTO_PERP`、`future→FUT`、`option→OPT`；`localSymbol` = CCXT 统一线格式 symbol（唯一性原语）；FUT/OPT 由 `market.expiry`(ms) 推导 `YYYYMMDD`、`contractSize` 推导 multiplier、`strike`/`optionType` 填充期权字段 | `ccxt-contracts.ts:26-34`、`76-111` |
| Longbridge | `longbridge-contracts.makeContract` | 后缀表 HK→SEHK/HKD、US→SMART/USD、SH→SSE/CNY、SZ→SZSE/CNY、SG→SGX/SGD；**一律 STK** | `longbridge-contracts.ts:19-25`、`:33-43` |
| LeverUp | `LeverupBroker.pairToContract` | 合成资产统一 `CRYPTO_PERP` + `LEVERUP` | `LeverupBroker.ts:192-203` |

**反向（broker 原生 → 统一）**：

- Alpaca：`resolveSymbol` 对 OPT 取 `localSymbol`，对非 STK/CRYPTO 返回 null，其余取大写 symbol（`alpaca-contracts.ts:40-44`）。
- CCXT：`contractToCcxt` 三级回退——`localSymbol` 直接查 markets 表 → `symbol` 本身是线格式 → 按 base+secType(+currency) 搜索；**命中多于一条时返回 null**（歧义不猜）（`ccxt-contracts.ts:118-144`）。
- IBKR：`getNativeKey` 三级——`conId` → `issuer:{issuerId}`（BOND 目录）→ 裸 symbol 兜底（`IbkrBroker.ts:891-896`）。
- CCXT nativeKey：`localSymbol || symbol`（`CcxtBroker.ts:1337-1339`）；解析用 markets 表直查，未命中返回**骨架合约**（只有 localSymbol/symbol），让后续操作在下游大声失败（`CcxtBroker.ts:1341-1356`）。

### 2.10 合约展开（hub → leaves）

- **触发方式**：`POST /uta/:id/contract/expand`（`routes-trading.ts:363-372`）或 AI 工具 `expandContract`。
- **输入**：aliceId + filters（`expiry`/`right`/`strikeMin`/`strikeMax`/`secType`(默认 OPT)/`limit`(默认 60，上限 200)）。
- **处理步骤/规则**：校验 aliceId 属于本账户 → 调 broker 展开 → 对返回的每一条合约重新 stamp aliceId。
- **覆盖范围**：**仅 IBKR 与 Alpaca 实现**（`IbkrBroker.ts:391`、`AlpacaBroker.ts:702`）；其余 broker 走 UTA 侧 loud-refuse（`CONFIG` 错误：不支持合约展开）。
- **错误与边界**：
  - IBKR：`issuer:` 前缀 → 返回该发行人的债券列表（按到期日排序、附加 coupon/maturity 描述）；纯数字 nativeKey → 先解析 underlying 再展开期权链/期货月份；非法键抛 `EXCHANGE`。
  - Alpaca：只支持股票期权、**必须**提供 `YYYYMMDD` 到期日；分页 token 重复会被判定为错误。展开结果显式标注"期权只读"。
  - 数量截断：`total` 报告全量，`limit` 之外的部分附 `hint` 说明。
- **证据**：`UnifiedTradingAccount.ts:1164-1179`；`IbkrBroker.ts:391-449`；`AlpacaBroker.ts:702-722`；协议类型 `packages/uta-protocol/src/types/broker.ts:190-217`。

### 2.11 keyless 数据源构造与注入

- **触发方式**：配置 `trading.json` 的 `keylessDataSources` 非空时，UTA 启动阶段注入。
- **输入**：`KeylessDataSource` 数组（枚举 `binance`/`okx`/`bybit`）+ 现有用户账户 id 集合。
- **处理步骤/规则**：去重 → 过滤掉与既有 `${ex}-readonly` 撞名的 → 逐个构造 UTAConfig：`id=${ex}-readonly`、label `<Exchange> (read-only data)`、`presetId='ccxt-custom'`、`keyless/readOnly/asVendor=true`、`editable=false`、`guards=[]`。
- **输出/副作用**：返回虚拟 UTAConfig 数组，**不写入 accounts.json**；由 `main.ts` 与用户账户一起顺序初始化。初始化失败只告警不中断（保护用户真实账户）。
- **错误与边界**：这是刻意的 opt-in——注释明确写过"把公共数据源当默认账户会让全新安装未经同意连接多个交易所"。keyless 账户被排除在权益聚合之外（`uta-manager.ts:209-210`）。
- **证据**：`services/uta/src/domain/trading/keyless-data-sources.ts:1-36`；注入 `services/uta/src/main.ts:70-89`；schema `src/core/config.ts:384-410`。

### 2.12 keyless 已实现的拒绝面

| 行为 | 结果 | 证据 |
|---|---|---|
| 凭据校验 | 跳过（不调 `checkRequiredCredentials`） | `CcxtBroker.ts:331-342` |
| `getAccount` | 返回全 0 的 USD 账户（不调 `fetchBalance`） | `CcxtBroker.ts:949-954` |
| `getPositions` | `[]` | `CcxtBroker.ts:1049-1050` |
| `getOrders(ids)` | `[]` | `CcxtBroker.ts:1115` |
| `getOpenOrders` | `[]` | `CcxtBroker.ts:1194-1195` |
| 下单提案（stage） | loud-refuse（`CONFIG`） | `UnifiedTradingAccount.ts:549-555` |
| 账户级变更（push 等） | loud-refuse（`CONFIG`） | `UnifiedTradingAccount.ts:557-562` |
| `assetClassFor` | 恒 `'crypto'`（交易所的"股票"是合成品） | `CcxtBroker.ts:1313-1316` |

### 2.13 FX 汇率获取（getRate）

- **触发方式**：任何需要 USD 换算的读路径。
- **输入**：货币代码（任意大小写散列→内部大写归一）。
- **处理步骤/规则**：六级优先链，见 §3.5。
- **输出/副作用**：`{rate, source, updatedAt, stale?}`；副作用是写 `liveRates` 缓存、可能的 `console.warn`（每币种仅一次，`defaultWarned` 去重）。
- **错误与边界**：**该函数永不 reject**——所有远端失败都被吞掉并降级。`source` 只有 `'live' | 'cached' | 'default'` 三种取值。
- **证据**：`services/uta/src/domain/trading/fx-service.ts:146-208`。

### 2.14 FX 金额换算（convertToUsd）

- **触发方式**：账户权益聚合、Longbridge 余额折算、Alice 持仓工具。
- **输入**：金额字符串 + 币种。
- **处理步骤/规则**：`new Decimal(amount)` → 若为 0 直接返回 `{usd:'0'}`（**不查汇率、不带警告**）→ 否则取汇率相乘。
- **输出/副作用**：`{usd: string, fxWarning?: string}`；**仅当** `source === 'default'` 才附警告，且警告文本含币种、汇率与 `updatedAt`。
- **错误与边界**：金额字符串非法（空串、空白、`abc`、`1,000`）时 `Decimal` 构造**抛异常**，该异常会穿透到调用方（已实测）。
- **证据**：`fx-service.ts:211-220`。

### 2.15 行情 / 报价获取

- **触发方式**：`POST /uta/:id/quote`（body 可为完整 Contract 或 `{aliceId}` 存根）、`GET /uta/:id/quote/:symbol`（legacy 路径参数形式，构造只有 symbol 的裸 Contract）。
- **输入**：合约 + 账户 id。
- **处理步骤/规则**：存根展开 → broker `getQuote` → 回填 aliceId。
- **输出/副作用**：`Quote`（`contract`/`last`/`bid`/`ask`/`volume`/`high?`/`low?`/`timestamp`，数值一律字符串以保 Decimal 精度）。
- **刷新策略**：**UTA 层没有报价缓存**——每次调用都打穿到 broker。刷新频率由调用方决定：UI 的行情头（走的是另一条 market-data 路径）60s 轮询一次（`ui/src/components/market/QuoteHeader.tsx:29`）。
- **错误与边界**：
  - IBKR：每次快照临时占用一条 TWS 行情线（上限约 100 条），`regulatorySnapshot=false`——**不会**为单次请求把账户 opt-in 到付费的美股监管快照；无权限时可能拿到免费延迟数据。带 conId 的调用会先解析规范合约（含 `reqContractDetails` 缓存），避免用错误的 exchange/currency 去问 conId（曾导致 TWS error 321）。
  - Alpaca：CRYPTO 走 `/v1beta3` 快照并要求 latestTrade + latestQuote 同时存在；OPT **直接拒绝**并指向 option-chain（避免给出没有 feed 标注的价格）。
  - CCXT：`fetchTicker` 后若 markets 表命中则用 `marketToContract` 归一化返回合约。
  - Longbridge：quote + depth 并发（depth 失败降级为空盘口，bid/ask 记 `'0'`）。
- **证据**：`UnifiedTradingAccount.ts:1132-1140`；路由 `routes-trading.ts:303-328`；`IbkrBroker.ts:811-834`、`352-381`、`324-348`；`AlpacaBroker.ts:549-587`；`CcxtBroker.ts:1222-1244`；`LongbridgeBroker.ts:631-655`；协议类型 `packages/uta-protocol/src/types/broker.ts:304-312`。

### 2.16 历史 K 线获取与合约解析

- **触发方式**：UTA 层 `getHistorical(contract, params)`。
- **处理步骤/规则**：存根展开 → 若 broker 无 `getHistorical` 则 loud-refuse（`CONFIG`）→ broker 实现。
- **覆盖**：Alpaca（`AlpacaBroker.ts:597`）、CCXT（`CcxtBroker.ts:1254`）、Mock（`MockBroker.ts:492`）实现；**IBKR 与 Longbridge 无实现**（能力声明里也不含 `historicalBars`）。
- **关键语义**：`BarParams.limit` 的跨 broker 契约是"窗口内**最近** N 根"；CCXT 适配器为此专门反推 `since` 并翻页（上游 API 语义是"since 之后第一页"），并对单页上限（如 OKX 300）做游标推进 + 按时间戳去重（`CcxtBroker.ts:1254-1310`）。
- **证据**：`UnifiedTradingAccount.ts:1141-1152`；协议类型 `broker.ts:328-370`。

### 2.17 资产类别标注（assetClassFor）

- **触发方式**：聚合搜索遍历 hit 时调用（可选方法）。
- **唯一实现**：CcxtBroker 恒返回 `'crypto'`（`CcxtBroker.ts:1316-1318`）。其余 broker 未实现 → hit 的 `assetClass` 为 `undefined`。
- **下游兜底**：BarService 在 `assetClass` 缺失时用 secType 启发式（`src/domain/market-data/bars/bar-service.ts:50-58`、`:377`）。

### 2.18 目录刷新（catalog refresh）

- **触发方式**：UTA 启动后每 6 小时定时（`services/uta/src/main.ts:39,134-141`）逐个账户调用 `uta.refreshCatalog()`；Alpaca 在首次 catalog 加载失败时也会异步补一次。
- **处理步骤/规则**：broker 若有 `refreshCatalog` 就调用，否则 no-op；CCXT 用 `loadMarkets(true)` 强制重拉，Alpaca 拉 `/v2/assets?status=active` 并过滤 `tradable !== false`。
- **输出/副作用**：重建本地目录与索引（Alpaca 还重建 `catalogBySymbol`）。Alpaca 的失败**不覆盖**旧 catalog（保留陈旧可用性）。
- **证据**：`UnifiedTradingAccount.ts:1186-1196`；`CcxtBroker.ts:420-430`；`AlpacaBroker.ts:223-238`。

---

## 3. 数据与状态

### 3.1 统一合约字段表（`Contract`，来自 `packages/ibkr/src/contract.ts`）

| 字段 | 含义 | 取值/约束 | 行号 |
|---|---|---|---|
| `conId` | IBKR 全局唯一合约 id | number，默认 0；0 视为未设置 | :119 |
| `symbol` | 标的代码（期权=标的，期货=品种根） | string，默认 `''` | :120 |
| `secType` | 证券类型 | `SecType \| ''`；空串 = 未设置（broker 输出边界会被拒绝） | :127 |
| `lastTradeDateOrContractMonth` | 期权=YYYYMMDD，期货=YYYYMM | string，默认 `''`；**无格式校验** | :128 |
| `strike` | 行权价 | number，默认 `UNSET_DOUBLE`（= `Number.MAX_VALUE`） | :130 |
| `right` | 期权方向 | string；校验接受 `C`/`P`/`CALL`/`PUT`（大小写不敏感），存原值 | :131 |
| `multiplier` | 合约乘数 | string；OPT/FOP/FUT 必填；非衍生品缺省由下游补 `'1'` | :132 |
| `exchange` | 执行/路由交易所 | string；universal 必填 | :133 |
| `primaryExchange` | 主交易所（SMART 路由用） | string，可选 | :134 |
| `currency` | 计价货币 | string；universal 必填 | :135 |
| `localSymbol` | broker 本地符号 | string；**各 broker 自定语义，不做跨 broker 归一** | :136 |
| `tradingClass` | 交易类别（如 SPX vs SPXW） | string，可选 | :137 |
| `issuerId` | 债券发行人 id | string；IBKR 用它构造 `issuer:` 目录键 | :142 |
| `description` | 长名称/自由描述 | string；Alpaca 存公司名、CCXT 存 `BASE/QUOTE type (settle settled)` | :141 |
| `aliceId` | 系统级唯一标识 | 声明合并而来，可选；`{utaId}\|{nativeKey}` | 协议包 `types/contract-ext.ts:27-30` |

哨兵常量：`UNSET_INTEGER = 2**31-1`、`UNSET_DOUBLE = Number.MAX_VALUE`、`UNSET_DECIMAL = 2**127-1`（`packages/ibkr/src/const.ts:10-13`）。

`SecType` 联合类型（`contract.ts:98-116`）：`STK, OPT, FUT, FOP, IND, CASH, BOND, CMDTY, WAR, IOPT, FUND, BAG, NEWS, CFD, CRYPTO, CRYPTO_PERP`。头注释写明**不允许新增 secType**（需项目负责人签字），`CRYPTO | CRYPTO_PERP` 是唯一被允许的偏离；`coerceSecType`（`:88-96`）在解码边界把未知值降为 `''` 并告警。

`ContractDescription`（`contract.ts:330-333`）：`{contract, derivativeSecTypes[]}` — 搜索结果的标准外壳。

### 3.2 合规合约规则表（`contract-discipline.ts`）

| 规则 | 内容 | 证据 |
|---|---|---|
| 允许的 secType 集合 | 15 个 IBKR 原生 + `CRYPTO_PERP`（共 16）；大小写敏感 | `:30-42` |
| universal 必填 | `symbol`、`secType`、`exchange`、`currency`（空串/未设置即失败） | `:48,54,76-80` |
| OPT 附加必填 | `lastTradeDateOrContractMonth`、`strike`、`right`、`multiplier` | `:56` |
| FOP 附加必填 | 同 OPT | `:57` |
| FUT 附加必填 | `lastTradeDateOrContractMonth`、`multiplier` | `:58` |
| STK/CASH/BOND/WAR/CRYPTO/CRYPTO_PERP | 只有 universal；multiplier 缺省由下游补 `'1'` | `:59-60` |
| right 值域（仅 OPT/FOP 且非空时） | 归一化后必须是 `C`/`P`/`CALL`/`PUT` | `:90-96` |
| "未设置"判定 | 字符串 `=== ''`；数值 `=== UNSET_DOUBLE`；其余 `!= null` | `:119-124` |
| 硬失败 | `assertContract` 抛 `Invalid contract: <errors 用 ; 连接>` | `:107-112` |
| 网络边界枚举 | 模拟器路由用 `SEC_TYPES` 生成 zod enum | `http/routes-simulator.ts:22,45-50` |

**实测行为（本次调查用脚本直接验证）**：

- `symbol/secType/exchange/currency` 齐全的 STK → `{ok:true}`。
- OPT 缺四个衍生字段 → 一次性返回 4 条错误。
- `right:'c'`（小写）、`right:'CALL'` → 通过。
- `strike: 0` → **通过**（0 不是 `UNSET_DOUBLE`；`hasContractField` 对数值只比对哨兵）。语义上 0 是"非期权"的哨兵，校验器不受此约定约束。
- `multiplier:'0'`、`lastTradeDateOrContractMonth:'garbage'` → **通过**（只判非空，不做格式/语义校验）。
- `BOND`/`CASH`/`IND` 只填 universal → 通过。
- CRYPTO 缺 `exchange` → 失败（`exchange is required`）。

**未被校验器覆盖的"合规"约束**（散落在别处）：

- `buildPosition` 额外拒绝 OPT/FOP 且 multiplier 为 `'1'` 或 `''` 的持仓——判定为上游解码缺乘数的 bug，会让市值/盈亏错约 100 倍（`brokers/contract-builder.ts:120-127`）。
- `buildContract` 把 `localSymbol` 默认为 `symbol`（`:63`）。
- IBKR 路由层对无 conId 的合约用另一套必填检查（`secType/exchange/currency/symbol|localSymbol`），并给出"先通过 search/expand 解析"的指引（`IbkrBroker.ts:369-380`）。

### 3.3 搜索模式归一化规则表

| assetClass | 规则 | 结果示例（实测） |
|---|---|---|
| `equity` / `commodity` / `unknown` / 缺省 | 恒等（仅 trim） | `EURUSD`+equity → `EURUSD`；`BRK.B` → `BRK.B` |
| `crypto` / `currency` | 剥离已知报价后缀，base 长度须 ≥ 2，命中则整体大写 | `BTCUSDT`→`BTC`；`ETHBTC`→`ETHBTC`（BTC 非报价币）；`LUSD`→`LUSD`（base 仅 1 字符）；`1000PEPEUSDT`→`1000PEPE`；`btcusd`→`BTC` |
| 不匹配的后缀形态 | 原样返回（保持大小写） | `BTC-USD`→`BTC-USD`；`BTC/USDT`→`BTC/USDT`；`EUR=X`→`EUR=X` |
| 空/空白 | 返回处理后的空串（调用方据此短路） | `'  '` → `''` |

报价币集合（顺序敏感，长优先）：`USDT, USDC, BUSD, USD, EUR, JPY, GBP, CNY`（`search-rules.ts:18`）；后缀正则 `^([A-Z0-9]{2,})(?:...)$`，大小写不敏感（`:20-23`）。

### 3.4 模糊排序分档表（实测）

| 分 | 条件 | 说明 |
|---|---|---|
| 100 | `symbol`、`base` 或 `name` **完全等于**查询 | 精确匹配优先于一切 |
| 80 | `symbol` 或 `base` 以查询**开头** | 前缀 |
| 70 | `name` 以查询开头，且查询后一个字符不是字母数字（词边界） | 避免 "cor" 命中 "cortex" 这类伪词首 |
| 50 | `name` 中查询构成**整词**（正则 `\b…\b`，元字符已转义） | |
| 30 | `symbol`、`localSymbol` 或 `name` **包含**查询 | |
| 20 | `quote` 等于查询 | 最弱信号，仅在无其他命中时触发 |
| 0 | 无信号 | 条目被丢弃 |

实测示例（查询 `CORN`，三个候选：symbol=`CORN`/name=`Teucrium Commodity Trust`；symbol=`CORNING`/name=`Corning Inc`；symbol=`POPCORN`/base=`POPCORN`/quote=`USDT`）：

- 结果顺序 `CORN`（100 精确 symbol）→ `CORNING`（80 前缀）→ `POPCORN`（30 子串）。
- 查询大小写无关（`corn` 同序）。
- 查询 `USDT` 时，若候选无 `name`，报价档 20 生效并返回全部条目；**若候选带 `name`（CCXT 用 `market.id`，如 `BTCUSDT`），tier 30 会先命中**，第 20 档在 CCXT 场景实际很难触达。

排序与截断：稳定排序 `(-score, 原始索引)`，默认 `limit=50`。CCXT 的预排序（swap→future→spot→option、USDT→USD→USDC）通过稳定排序保留为 tiebreak，因此"BTC"这类精确 base 命中会呈现衍生品在前的顺序。

**分档注释与实现不一致**：`fuzzy-rank.ts:36-44` 的分档注释只列到 30，遗漏了 20 档；实现见 `:69-71`。同一段注释还声称与数据侧 `aggregateSymbolSearch` 打分一致——本次未核对数据侧实现（见 §9）。

### 3.5 FX 汇率优先级链（`fx-service.ts:146-208`）

| 序 | 来源 | 命中条件 | 返回 `source` | 副作用 |
|---|---|---|---|---|
| 0 | USD 直通 | 币种为 USD | `live` | 无（不查任何缓存/远端） |
| 1 | 新鲜 live 缓存 | 距 `fetchedAt` < TTL（默认 5 分钟） | `live` | 无 |
| 2 | TraderHub FX 表 | hub 启用且表缓存 < 30 分钟（或退避期内有旧表） | `live` | 写入 per-currency 缓存；整表缓存 30 分钟；失败后**冷却 60 秒** |
| 3 | vendor 客户端 | 存在 currency client；按币种单查（`counter_currencies:'USD'`，硬编码 `provider:'yfinance'`） | `live` | 写缓存；要求 `last_rate > 0` |
| 4 | 过期 live 缓存 | 有缓存但已过期 | `cached` | 附 `stale: true` |
| 5 | 硬编码默认表 | 币种在表内 | `default` | 首次使用时 `console.warn` 一次 |
| 6 | 1:1 兜底 | 未知币种 | `default` | 告警一次，`updatedAt` 固定 `'1970-01-01'` |

**Hub 交互细节**：`GET {baseUrl}/api/data/fx-rates`，5 秒超时，要求响应 `counter === 'USD'` 且 `rates` 存在，逐条只接受有限正数；整表为空视为失败。`config.marketData.hub` 默认 `{enabled:true, baseUrl:'https://traderhub.openalice.ai'}`（`src/core/config.ts:345-352`）。UTA 启动时把该配置注入 FxService（`services/uta/src/main.ts:94-100`）。

**硬编码默认表**（`fx-service.ts:49-71`）：21 个币种（HKD/EUR/GBP/JPY/CNY/CNH/CAD/AUD/NZD/SGD/CHF/KRW/SEK/NOK/DKK/INR/TWD/MXN/ZAR/BRL），全部 `updatedAt: '2026-04-08'`。注释要求"发版时更新"——**没有任何机制保证它被更新**。

**缺失汇率行为（实测）**：

- `HKD`（无 client、无 hub）→ `0.128` / `default` / 警告 `HKD: using default rate 0.128 (last updated 2026-04-08)`。
- `ZZZ`（未知）→ `1` / `default` / `updatedAt '1970-01-01'` / 警告"defaulting to 1:1"。
- 小写 `hkd` 正常归一化命中默认表。
- `convertToUsd('0', …)` → `{usd:'0'}`，**不查汇率、不告警**。
- 金额串非法（`''`/`'  '`/`'abc'`/`'1,000'`）→ Decimal 抛错穿透。

### 3.6 内存状态与持久化

| 状态 | 位置 | 生命周期 | 说明 |
|---|---|---|---|
| `liveRates` (per-currency) | FxService 实例字段 | 进程生命周期 | `{rate, updatedAt, fetchedAt}`；**无容量上限、无清理** |
| `hubTable` | FxService | 进程生命周期 | 整表 + `fetchedAt`；30 分钟 TTL |
| `hubDownUntil` | FxService | 进程生命周期 | 失败冷却时间戳（60s） |
| `defaultWarned` | FxService | 进程生命周期 | 去重告警集合，只增不减 |
| markets 表 | CcxtBroker | 进程生命周期 | `loadMarkets` 结果；6h 强制刷新；搜索的热路径 |
| catalog / catalogBySymbol | AlpacaBroker | 进程生命周期 | `/v2/assets?status=active` 过滤 `tradable!==false` |
| `conIdContracts` | IbkrBroker | 进程生命周期 | conId → 规范合约的 promise 缓存；失败时**只删除本次观测到的 promise**，避免瞬时故障永久毒化缓存（`IbkrBroker.ts:324-348`） |
| aliceId | Contract 实例字段 | 随对象 | 无独立注册表；由 broker 的 nativeKey 解析规则保证可逆 |

**持久化**：本区域**不落盘任何文件**。keyless UTA 明确不写 accounts.json；搜索结果、汇率、目录全部是进程内状态。UTA 重启即全部重建。

---

## 4. 外部交互

### 4.1 调用方向

- **UTA → broker SDK/REST**：IBKR TWS（`reqMatchingSymbols`、`reqContractDetails`、`reqMktData`）、CCXT exchange 实例（`loadMarkets`/`fetchTicker`/`fetchOHLCV`）、Alpaca REST（`/v2/assets`、`/v1beta3/crypto/*`、`/v2/orders` 等）、Longbridge SDK（`staticInfo`/`quote`/`depth`）、LeverUp（Pyth 价格 + relayer）。
- **UTA → TraderHub**：`GET /api/data/fx-rates`（只读公共数据，无用户数据附加）。
- **UTA → market-data currency client**：进程内 SDK executor 调用，`getSnapshots({base, counter_currencies:'USD', provider:'yfinance'})`。
- **Alice → UTA**：经 `/api/trading/*`（BFF 代理），契约由 `packages/uta-protocol` 定义。

### 4.2 关键契约（接口形状，不重复端点表）

| 契约 | 形状要点 | 证据 |
|---|---|---|
| `ContractSearchHit` | `{source, contract, derivativeSecTypes[], assetClass?}`；`assetClass` 为 venue 权威，缺失时消费方用 secType 兜底 | 协议 `types/manager.ts:44-58` |
| `ContractSearchResult` | `{accountId, results[]}` 分组形状；**仅 UTA 服务内 `UTAManager.searchContracts` 使用**（生产调用方只有 BarService 通过 SDK 的扁平版本） | `types/manager.ts:37-42` |
| `ContractDescription` | `{contract, derivativeSecTypes}` | `packages/ibkr/src/contract.ts:330-333` |
| `Quote` | 数值一律字符串（Decimal 精度）；`high`/`low` 可选 | `types/broker.ts:304-312` |
| `MarketClock` | `{isOpen, nextOpen?, nextClose?, timestamp?}` | `types/broker.ts:315-320` |
| `BarParams` | `interval` 为 8 档枚举；`limit` 语义为"窗口内最近 N 根" | `types/broker.ts:328-353` |
| `HistoricalBarsCapability` | `{supported, quality?, qualityBySecType?, supportedBarSizes?}` | `types/broker.ts:438-445` |
| `AccountCapabilities` | `{supportedSecTypes[], supportedOrderTypes[], historicalBars?}`；缺 `historicalBars` 即 loud-refuse | `types/broker.ts:447-452` |
| `ExpandContractFilters` / `ContractExpansion` | filters 默认 `limit=60` 上限 200；expansion 的 `total` 报全量、`kind` 区分 contracts/optionGrid | `types/broker.ts:190-217` |
| `BrokerResearch` | 可选 `getOptionContracts`/`getOptionChain`/`getOrderBook`；老 pack 可继续加载 | `packages/uta-protocol/src/broker-research.ts:20-25` |
| 归一化函数 | `normalizeBrokerSearchPattern` + `AssetClassHint` 常量与方法 | `packages/uta-protocol/src/brokers/search-rules.ts:14-55` |

### 4.3 Alice 侧消费面（本文件只写契约假设；完整行为见 08）

- **AI 工具自建 fan-out**：`src/tool/trading.ts:220-280` 复制了同一套"归一化 → 过滤 asVendor → allSettled"逻辑（**未复用** UTA 侧聚合函数），额外做了两件事：用 `compactContract` 精简字段（`src/tool/trading-compact.ts:72-100`），并把 aliceId 含 `|issuer:` 的行标为 `expandable: true`（`src/tool/trading.ts:271`）。工具描述里明确区分 LEAVES 与 DIRECTORIES。
- **SDK**：`UTAAccountSDK.searchContracts` 从扁平结果里按 `source` 过滤（`src/services/uta-client/UTAAccountSDK.ts:210-222`，注释记录了早期按分组形状解析导致的"永远返回空"事故）；`UTAManagerSDK.searchContracts` **忽略 `assetClass` 参数**，不把它带进查询串（`src/services/uta-client/UTAManagerSDK.ts:205-217`）。
- **UI**：`TradeableContractsPanel` 调 `tradingApi.searchContracts(symbol, assetClass)`，拿到结果后**自行重排**——按 secType 分层 STK(0) < CRYPTO(1) < CRYPTO_PERP(2) < FUT(3) < OPT(4)，同层保持 broker 原序（`ui/src/components/market/TradeableContractsPanel.tsx:104-127`），并默认折叠到 `COLLAPSED_LIMIT`。
- **UI 类型缺口**：`ui/src/api/trading.ts:13-27` 的 `ContractSearchHit` **缺 `assetClass`** 字段（协议包有），面板也无法据此额外排序/标注。
- **BarService**：`src/domain/market-data/bars/bar-service.ts:320-380` 调 `utaManager.searchContracts(query)`，要求 hit 必须有 `aliceId`（否则跳过），用 `barId` 与账户能力表过滤不可供 K 线的账户，并用 `assetClass ?? secTypeToAssetClass(secType)` 标注。barId 格式与 aliceId 同为 `{id}|{native}`，但**语义不同**：barId 允许 vendor 前缀（`yfinance|AAPL`）且 nativeSymbol 允许含 `:` 表示衍生品（`src/domain/market-data/bars/types.ts:39-54`）。

### 4.4 FX 调用点全景

| 调用方 | 用法 | 证据 |
|---|---|---|
| 账户权益聚合 | 每账户 4 个字段（netLiquidation/totalCashValue/unrealizedPnL/realizedPnL）换算；`fxWarnings` 去重收集；非 USD 账户才走 FX | `services/uta/src/domain/trading/uta-manager.ts:233-256` |
| `/fx-rates` 路由 | 遍历非 offline 账户的 positions + baseCurrency，收集非 USD 币种，逐币 `getRate` | `services/uta/src/http/routes-trading.ts:170-191` |
| Longbridge 余额折算 | `foldBalancesToBase` 把多币种余额折算到 HKD；无 FxService 时退化为"取最大单桶"（明确注释"不谎报单位"） | `LongbridgeBroker.ts:381-437` |
| Longbridge 交叉汇率 | `fxRate(from,to)` 通过两次 `convertToUsd('1', …)` 相除实现 | `LongbridgeBroker.ts:442-448` |
| IBKR 账户 | **不用 FxService**——用 TWS 下发值里的 `ExchangeRate:{currency}`；缺失则把该账户的 position 汇总置为 null 走 broker 的 NetLiquidation | `IbkrBroker.ts:600-604,616-645` |
| Alice AI 工具 getPortfolio | 拉 `/fx-rates` 建 Map；**缺失汇率静默按 1.0**；把 `source==='default'`（以及一条永不命中的 `'fallback'`）计入 fxWarnings | `src/tool/trading.ts:345-364`（死分支 `:352-354`） |
| UI 持仓表 | `rateMap[currency] ?? 1` 换算展示 | `ui/src/pages/PortfolioPage.tsx:733-739` |
| FxService 注入 | broker factory 对实现了 `setFxService` 的 broker 注入（目前仅 Longbridge） | `brokers/factory.ts:42-46` |

---

## 5. 配置项、默认值、环境变量、feature 开关

| 配置 | 位置 | 默认 | 作用 |
|---|---|---|---|
| `trading.keylessDataSources` | `trading.json`（`src/core/config.ts:384-410`） | `[]` | 启用哪些 keyless 公共数据源；空即完全关闭该功能 |
| `marketData.hub.enabled` / `hub.baseUrl` | market-data.json（`config.ts:345-352`） | `true` / `https://traderhub.openalice.ai` | FX 表来源；关掉即跳过优先级第 2 层 |
| `marketData.providers.currency` | market-data.json（`config.ts:312-319`） | `yfinance` | currency client 的供应商选择（FxService 内部还硬编码 `provider:'yfinance'`，两者可能不一致——见 §8.5） |
| FxService TTL | 构造参数（`fx-service.ts:99`） | 5 分钟 | live 缓存有效期；UTA 启动时传 `undefined` 即用默认 |
| FxService hub 参数 | 构造第 3 参 | 由 `marketData.hub` 注入 | 缺省 undefined 则完全跳过 hub 层 |
| hub 整表 TTL | 硬编码常量 | 30 分钟 | `fx-service.ts:113` |
| hub 失败冷却 | 硬编码常量 | 60 秒 | `fx-service.ts:135` |
| hub 请求超时 | 硬编码常量 | 5 秒 | `fx-service.ts:119` |
| 目录刷新周期 | 硬编码常量 | 6 小时 | `services/uta/src/main.ts:39` |
| 模糊搜索 limit | 函数参数 | 50 | `fuzzy-rank.ts:91` |
| 展开 limit | `fuzzy`? 展开 filters | 60（上限 200） | `IbkrBroker.ts:392`、`AlpacaBroker.ts:717` |
| `OPENALICE_UTA_PORT` | env | 47333 | UTA 监听端口（本文件仅旁证） |
| 出站代理 | `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`（含小写） | 无 | 启动时探测并桥接进 CCXT exchange（URL 凭据会脱敏输出） |

**无 rate-limit 配置**：全仓未出现 `enableRateLimit`（已全仓 grep 确认，非本文件区域外遗漏）。CCXT 实例是否限速完全取决于上游库默认与 exchange 自身节流；keyless 账户共用同一 exchange 实例语义。

---

## 6. 不变量、时序与并发假设

### 6.1 不变量

1. **aliceId 一旦生成即不可变**：没有重新解析/重写路径；同一合约在同一账户下多次搜索得到**相同** aliceId（前提是 broker 的 nativeKey 规则稳定）。
2. **aliceId 的账户前缀是权威**：`contractFromAliceId` 与 `expandContract` 都强制校验归属，跨账户立刻抛错。
3. **broker 输出边界的合约必须通过 `validateContract`**：`buildContract` 是唯一出口漏斗（当前所有 broker 的合约构造都经过它，唯一例外见 §8.2）。
4. **`Position.multiplier` 恒非空**：由 `buildPosition` 保证（contract.multiplier → `'1'`），OPT/FOP 的 `'1'` 被直接拒绝。
5. **`convertToUsd` 恒成功**（除金额串非法）：永不因缺汇率而失败。
6. **搜索的 `limit` 是硬截断**：超出部分不返回、不报告总数。
7. **`derivativeSecTypes` 语义是"该标的可展开的族"**，不是"该行是什么"：Alpaca 的 STK 行统一标 `['OPT']`；CCXT 则是把**全体候选**出现过的衍生类型取并集后赋给**每一行**（`CcxtBroker.ts:493-501`）——即同一结果列表里所有行共享同一份 `derivativeSecTypes`。

### 6.2 时序假设

- FX 的三层 TTL（per-currency 5 分钟 / hub 整表 30 分钟 / 失败冷却 60 秒）是**独立计时**的：hub 命中后仍会写 per-currency 缓存，因此短时间内重复查询同一币种不会重复打 hub。
- `getRate` 无并发去重：同一币种的并发调用会各自发起远端请求（`liveRates` 只在成功后写入）。UTA 的 `/fx-rates` 路由是**串行 for 循环**逐币调用（`routes-trading.ts:186-190`），因此同一次请求内不会放大；但多账户并发（getAggregatedEquity 的 `Promise.all`）可能对同一币种重复打 hub。
- `hubDownUntil` 只在请求失败时写入；冷却期内若已有旧表则**继续用旧表**（不返回 null），保证降级平滑。
- IBKR 的 conId 解析缓存以 promise 为粒度，消除同一 conId 的并发重复解析。

### 6.3 已实测的反例 / 边界

- **归一化不幂等**：`BTCUSDEUR` 一次得 `BTCUSD`，二次得 `BTC`（`USD` 也是报价币）。若某条链路对结果二次归一化，语义会继续漂移。当前只有一处调用（无二次应用），因此是潜在而非现实缺陷。
- **tier 20 在 CCXT 场景近乎不可达**：CCXT 传入的 `name` 是 `market.id`（如 `BTCUSDT`），查询 `USDT` 时先被 tier 30 命中，全部候选同分后靠原序排列——结果是"所有含 USDT 的市场"都返回，而非只有报价匹配者。实测已验证此行为。
- **MockBroker 忽略 pattern**：任何查询都返回一条 AAPL 桩，测试与模拟器场景下"搜索成功"不代表匹配正确。

---

## 7. 测试覆盖

### 7.1 已有覆盖（本次实跑：`vitest run` 定向跑本区域 6 个 spec，72 tests，全部通过）

| Spec | 测试数 | 覆盖行为 |
|---|---|---|
| `contract-search-rules.spec.ts` | 15 | crypto/currency 剥离（USD/USDT/USDC/最长优先）、小写输入、无匹配保持、`LUSD` 短 base 保护、裸 base、equity/commodity 恒等、unknown 直通、空串、trim |
| `contract-discipline.spec.ts` | 15 | SEC_TYPES 集合、`isSecType` 收窄、universal 各项缺失、未知 secType、OPT/FOP 四字段、坏 right、FUT 只要求 expiry+multiplier、STK/CRYPTO/CRYPTO_PERP 只需 universal、`assertContract` 抛错与静默 |
| `fuzzy-rank.spec.ts` | 13 | 空查询、0 分丢弃、精确 vs 子串、CCXT 式 exact base、name 精确、symbol 前缀优于子串、name 词边界优于子串、正则元字符转义、limit 与默认 50、tie 保序、报价兜底、字段缺失 |
| `fx-service.spec.ts` | 19 | USD 直通、live 获取、缓存命中、TTL 过期重取、刷新失败退陈旧、默认表回退、counter 不匹配回退、无 client 纯默认表、未知币 1:1、告警去重、换算告警只在 default、stale 不告警、零额不告警、大写归一、hub 优先于 vendor、hub 整表缓存、hub 失败回落 vendor、非法值（0/负）在形状边界被拒 |
| `contract-search.spec.ts` | 2 | `asVendor` 过滤（默认跳过、显式 source 仍可搜） |
| `keyless-data-uta.spec.ts` | 8 | 默认空、按选择构造、不遮蔽用户同名 id、三种交易所构造成功且 keyless 标志贯通、能力声明含 historicalBars、全资产类别报 crypto、账户读全 0 且不 init |
| `brokers/contract-builder.spec.ts` | 14 | `buildContract` 默认与校验（含未知 secType 拒绝）、position pass-through vs derive、multiplier 继承与覆盖、`avgCostSource`、OPT/FOP multiplier=1 拒绝、STK multiplier=1 允许 |
| `brokers/ccxt/ccxt-contracts.spec.ts` | 10 | spot/perp/future 的 localSymbol 线格式与 expiry 推导、USDC/ USDT perp 区分、`assertContract` 通过、线格式直查、base+secType 回退、类型映射 |
| `brokers/ccxt/CcxtBroker.spec.ts:175-215` | 5 | 搜索：空模式、大小写不敏感 base 过滤、只返回 USDT/USD/USDC 报价、排除 inactive、衍生品优先排序 |
| `brokers/alpaca/AlpacaBroker.spec.ts:81-94` | 2 | 空模式、大写回显 |
| `brokers/others/leverup/LeverupBroker.spec.ts:191-217` | 4 | base 匹配、未知返回空、空模式、CRYPTO_PERP/LEVERUP 标注 |
| `uta-manager.spec.ts:142-230` | 4 | 分组搜索：默认全账户、跳过非 vendor、按 accountId、非 healthy 账户被跳过 |
| `ui/src/api/trading.contract-search.spec.ts` | 1 | UI 客户端查询串构造（含 `source`） |
| `src/__tests__/trading-tools.spec.ts:103-118` | 1 | Alice 工具聚合多账户结果 |

### 7.2 无测试覆盖的行为

- **`expandContract` 全路径**：全仓 `*.spec.ts` 中 `expandContract` **零命中**——IBKR 的 `issuer:` 目录展开、期权链网格、期货月份、Alpaca 的期权分页展开全部无测试。
- **HTTP 路由层**：`/contracts/search`、`/fx-rates`、`/uta/:id/quote`、`/uta/:id/contract/expand`、`/uta/:id/historical` 均无路由级测试（`services/uta/src/http/` 下只有 wallet / simulator / broker-research / order-entry 四类 spec）。
- **IBKR 搜索**：`reqMatchingSymbols` 的 base/pairCurrency 拆分、CASH hub 内联展开、`issuer:` nativeKey 拒绝均无单测。
- **Longbridge 搜索**：恒回显行为无测试。
- **MockBroker 搜索**：无测试（尽管它被大量用作其它测试的替身）。
- **`getQuote` 各 broker 实现**：除 Mock 外无单测（IBKR 的付费快照规避、Alpaca 的 OPT 拒绝、Longbridge 的 depth 降级都无覆盖）。
- **`assetClassFor` 只有 keyless 场景被测**（`okx-readonly` 恒 crypto），普通 CCXT 账户的同一行为未单独覆盖。
- **catalog 刷新**：无测试覆盖 6h 定时器或 `refreshCatalog` 的失败保留语义。
- **归一化的非幂等输入**（`BTCUSDEUR`）、**tier 20 的实际触达条件**均无测试。
- **Alice 侧 `src/tool/trading.ts` 的 fan-out 复制**：只有"两账户聚合"一条测试；`expandable` 标记、`compactContract` 字段裁剪、vendor 过滤失败回退（`filterVendorSearchTargets` 的 catch 分支）均无测试。
- **`UTAManagerSDK.searchContracts` 忽略 assetClass**：无测试，也因此没有红灯提示该参数已失效。

---

## 8. 观察到的问题、技术债、耦合与重构关注点

### 8.1 文档与代码冲突（已在正文按代码为准）

| 文档 | 声称 | 实际 | 证据 |
|---|---|---|---|
| `contract-search-rules.md:10` | 规则代码位于 `contract-search-rules.ts` | canonical 实现已迁到 `packages/uta-protocol/src/brokers/search-rules.ts`，服务内文件只是 re-export shim | `services/uta/src/domain/trading/contract-search-rules.ts:1-8` |
| `contract-search-rules.md:20` | HTTP 路由位于 `src/connectors/web/routes/trading.ts` | 该路径已不存在；路由在 `services/uta/src/http/routes-trading.ts:150` | 同上 |
| `services/uta/src/domain/trading/README.md:31` | aliceId 格式为 `{exchange}-{market.id}`（如 `bybit-ETHUSDT`） | 实际为 `{utaId}\|{nativeKey}`（如 `bybit-main\|BTC/USDT:USDT`） | `UnifiedTradingAccount.ts:510-513` |
| UI i18n（英/中/繁/日 4 份） | aliceId 格式为 `alias:broker:exchange-id` | 同上，格式描述错误 | `ui/src/i18n/locales/en.ts:1922`（余见 zh:1889、zh-Hant:1898、ja:1891） |
| `fuzzy-rank.ts:36-44` 注释 | 分档只列到 30 | 实现含第 20 档（报价匹配） | `fuzzy-rank.ts:69-71` |

这些不一致对重构是有利信号：格式字符串被四处复述且四处都错，说明该契约缺少单一可引用来源。

### 8.2 校验纪律的漏点

- `buildContract` 是唯一的强制校验点，但以下合约构造**绕过**它：
  - `MockBroker.makeContract`（`MockBroker.ts:113-120`）——虽然字段齐全，但它同时也是大量测试的工厂，一旦默认值变化不会触发校验。
  - `MockBroker.resolveNativeKey` 的兜底合约（`MockBroker.ts:539-544`）只填 symbol/localSymbol/secType，**缺 exchange/currency**，按纪律标准是非法合约。
  - `LeverupBroker.resolveNativeKey` 的兜底合约（`LeverupBroker.ts:541-547`）缺 currency。
  - `IbkrBroker.resolveNativeKey` 的 symbol 分支（`IbkrBroker.ts:905-915`）缺 `multiplier`（对 STK 可接受）且**把任何非数字 nativeKey 当 STK**（注释自承"其他 secType 应使用 conId"）。
  - `resolveRoutableContract`/`cloneContract`（`IbkrBroker.ts:65,352-381`）是查询期的临时构造，不进入校验。
- 结论：**"合规"在输出边界被强制，在输入/兜底路径不被强制**。重构时应明确这是设计意图还是漏洞——目前代码注释把它描述为"Phase 2 才强制"，但 grep 显示 Phase 1 遗留的描述未清理（`contract-discipline.ts:16-18` 仍在讲 "Phase 1 (this commit) just makes the machinery available"，与现状不符）。

### 8.3 搜索层的结构性重复与遗漏

- **两套 fan-out 实现**：UTA 服务内的 `searchTradeableContracts`（`contract-search.ts:27-65`）与 Alice 工具内的内联实现（`src/tool/trading.ts:242-280`）逻辑同源但独立演化。二者已出现行为差异：Alice 版额外做 `compactContract` 裁剪与 `expandable` 判定，UTA 版额外输出 `assetClass`。`contract-search.ts:1-14` 的头注释明确要求"两个表面必须返回同一形状"，但实际未共享代码。
- **`UTAManager.searchContracts`（分组形状）疑似死代码**：生产路径上 UTA 服务无调用方，只有 spec 与 BarService（后者走的是 Alice 侧 SDK 的扁平版本，通过 `UtaBarGateway` 结构类型满足）。协议包里 `ContractSearchResult` 也随之成为无消费方的类型。**推断**：这是 UTA-split 迁移的遗留。
- **无跨 broker 去重**：同一 `AAPL` 在 alpaca-paper 与 ibkr-tws 下会出现两条 hit、两个 aliceId。UI 用 secType 分层但不合并；LLM 侧需要自行判断。对多账户用户是显著噪声。
- **`assetClass` 参数链路断裂**：协议类型有（`manager.ts:57`）、UTA 聚合层会产出（`contract-search.ts:60`）、UI 类型缺（`ui/src/api/trading.ts:13-27`）、SDK 丢弃（`UTAManagerSDK.ts:205-217`）。只有 CcxtBroker 实现了 `assetClassFor`，其余 broker 下该字段恒 `undefined`，下游只能靠 secType 启发式——而 secType 启发式对"crypto 交易所上的 tokenized 股票/期货"是**已知错误**的来源，这也正是 `assetClassFor` 存在的原因。
- **失败静默**：`Promise.allSettled` 的 rejected 分支无日志（`contract-search.ts:52`）。在只有一个账户且该账户 broker 抛错时，用户看到的是"没有匹配"，而真实原因是故障。UI 只能用 `utasConfigured` 区分部分情形。

### 8.4 FX 的脆弱点

- **硬编码汇率表会静默过期**：21 个币种全部标 `2026-04-08`，注释要求发版时手工更新；无 CI 检查、无过期告警（`fx-service.ts:42-48,49-71`）。一旦 hub 与 vendor 同时不可用，非 USD 账户的权益会以数月前的汇率聚合，且只给一条一次性 `console.warn`。
- **警告语义分裂**：`fxWarning` 只在 `source === 'default'` 时产生，所以硬编码表与"未知币 1:1"共用同一档，调用方无法区分"用了过期的真实汇率"与"根本不知道这个币"。未知币的 1:1 换算在数值上等价于"把 JPY 当 USD"，风险等级明显更高。
- **Alice 工具侧的 `'fallback'` 分支永远不会命中**：`src/tool/trading.ts:352` 判断 `r.source === 'default' || r.source === 'fallback'`，而 `FxRate.source` 只有 `live|cached|default`（`fx-service.ts:26-27`）——僵尸分支，也让警告文案与真实来源脱节。
- **两处"缺汇率按 1.0"**：Alice 工具（`src/tool/trading.ts:363`）与 UI 持仓表（`PortfolioPage.tsx:737`）都在 Map 未命中时兜 1.0，而 UTA 的 `/fx-rates` **几乎不可能**漏币（它遍历所有持仓币种），所以这条兜底通常只在"路由不可达"时生效——但那时把外币当美元展示是误导性的。
- **`provider:'yfinance'` 硬编码进 FxService**（`fx-service.ts:168-172`），忽略 `marketData.providers.currency` 配置。配置项与实际取值可能不一致。
- **hub 形状校验只做浅层**：只检查 `counter === 'USD'` 与 rates 存在 + 逐条有限正数，不校验 `updatedAt` 的新鲜度；一个内容陈旧但格式合法的 hub 响应会无条件覆盖本地缓存。
- **`defaultWarned` 只增不减**：长跑进程里币种集合通常有界，风险低；但它是"每进程一次"的告警策略，运维重启后重放。

### 8.5 合约身份层的耦合点

- **nativeKey 语义分散在 5 个 broker**，没有集中契约测试。协议文档（`contract-ext.ts:8-22`）列了 4 家的规则但未覆盖 Longbridge/LeverUp。新增 broker 时很容易写出"`getNativeKey` 和 `resolveNativeKey` 不对称"的实现（二者必须互为逆运算才能保证 aliceId 可往返）。
- **CCXT 的兜底合约是"半成品"**（只有 localSymbol/symbol，无 secType/exchange/currency），注释说"让下游大声失败"——但下游是 `contractToCcxt`，它可能在 markets 表里**恰好**命中同名字符串从而静默成功，掩盖 aliceId 已陈旧的事实。
- **`_expandAliceIdIfNeeded` 的默认值哨兵逻辑与 `hasContractField` 语义重复**（一处用于 overlay 决策，一处用于校验），两处的"未设置"判定标准已漂移：overlay 额外把数值 `0` 当作默认，校验器只比对 `UNSET_DOUBLE`。这是历史 bug 修复留下的双标准。
- **IBKR 的 `issuer:` 前缀是隐式字符串协议**（含 `:` 的 aliceId 表示目录），Alice 工具用 `includes('|issuer:')` 判定 expandable（`src/tool/trading.ts:271`）——字符串嗅探而非结构化字段。重构时值得引入显式的 `kind: 'leaf' | 'directory'`。

### 8.6 搜索质量与可发现性

- **`limit` 截断无提示**：`fuzzyRankContracts` 静默截断到 50；`ContractSearchHit` 无 `total`，UI 的"显示更多"只能展开已有结果，无法知道是否还有更多。与 `expandContract` 的 `total`+`hint` 设计明确不一致。
- **CCXT 的报价过滤（USDT/USD/USDC）是为"搜索"拍的策略，同时影响"结算"**：以 EUR 计价的加密对在搜索里不可见，用户的抱怨会是"我的交易所明明有 BTC/EUR"。
- **搜索不区分"目录行"与"叶子行"**（除 Alice 工具的 `|issuer:` 嗅探外）：UTA 聚合层原样透传 IBKR 的 BOND issuer hub 行，HTTP/UI 消费者拿到的是不可交易合约却看起来与叶子一样。

---

## 9. 未探索区域与开放问题

### 9.1 未探索（本次未读/未验证）

- **数据侧的 `aggregateSymbolSearch` 打分实现**：`fuzzy-rank.ts:46-47` 声称与之"保持一致"，本文件未核对该函数（属于 `src/domain/market-data/aggregate-search.ts`，与 02/08 交界）。
- **IBKR live 行为**：`reqMatchingSymbols` 的真实返回形状、CASH hub 展开的实测结果、付费快照规避在 entitlement 缺失时的实际降级路径，均只依据代码与 `docs/uta-live-testing.md` 的描述，未跑真实 TWS。
- **CCXT 各交易所的市场元数据差异**：`marketToContract` 对 dated FUT/OPT 的 `expiry`/`contractSize` 依赖上游质量，被 try/catch 跳过的市场比例未统计。
- **Alpaca 期权展开的分页上限与 `pageToken` 循环保护的实战表现**（上限 1000/页、token 重复检测）未验证。
- **Longbridge 的 `staticInfo`/`optionQuote`/`warrantQuote` 富化细节**（属于持仓侧，03 区域）只读了 FX 折算部分。
- **LeverUp 的 Pyth 价格时间戳语义与 `network` 切换对 pair 列表的影响**未细读。
- **`/api/data/fx-rates`（TraderHub）的真实响应契约**：只依据 UTA 侧的校验逻辑反推；hub 的刷新节奏（代码注释称"hourly"）未独立核实。
- **UI 侧 `assetClass` 在面板排序中的潜在用途**、`OrderEntryDialog` 的 aliceId 直填路径（`ui/src/components/uta/OrderEntryDialog.tsx:387`）完整交互未探索（属 08）。
- **`uta-live-testing.md` 中与本区域相关的既有验收清单**（如 `:341-345` 的 hub/leaf 往返要求、`:389-403` 的 conId 相关事故记录）只做了关键词检索，未逐条映射到测试覆盖表。

### 9.2 开放问题（需要设计决策）

1. **跨 broker 去重责任归属**：应在聚合层（按"同一经济标的"归一，但 crypto 交易所的合成资产与真实股票不可等价）还是留给消费者？当前明确不做，但 UI 与 LLM 都在各自处理。
2. **`localSymbol` 的定位**：注释说它是"各 broker 自定的原生符号，不做跨 broker 归一"，同时 CCXT 用它承载统一线格式、Alpaca 用它承载 OCC 符号。是否需要一层显式的"broker 原生符号"类型而非共用 string？
3. **`contract-discipline` 的强制边界**：是否应把 `resolveNativeKey` 兜底路径也纳入校验（代价是骨架合约会直接抛错，而当前设计刻意让它存活到下游失败）？
4. **FX 的"过期"语义**：是否需要把 `source` 从三值扩展为能区分"硬编码新鲜/硬编码过期/未知币 1:1"，并让 `fxWarning` 分级？当前三档信息量不足以支撑 UI 分级告警。
5. **归一化幂等性**：是否应对 `normalizeBrokerSearchPattern` 施加"一次通过"契约（当前 `BTCUSDEUR` 会两级剥落），还是保持现状并在调用点用注释固化"只调用一次"的假设？
6. **`ContractSearchResult` / `UTAManager.searchContracts` 的处置**：确认是否可删（疑似 UTA-split 遗留），或保留给未来的分组式消费方。
7. **搜索失败的可观测性**：`allSettled` 静默吞错是否应至少聚合一条 warning 到响应里（消费方目前无法区分"无匹配"与"broker 挂了"）。
