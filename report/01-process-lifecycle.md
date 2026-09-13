# 01 — UTA 进程生命周期（UTA 重构调查报告）

> 摘要（≤10 行）
> 本区域覆盖 UTA 作为一个独立 OS 进程的启动、监听、健康、崩溃、重启与关闭，以及它的三个启动宿主：dev Guardian（tsx watch）、构建产物宿主（prod Guardian / Electron / Bun internal role）、Docker 与远程 Runtime。
> UTA 进程自身只有 `services/uta/src/main.ts`（200 行）一个入口，启动顺序固定：loadConfig → eventLog/ToolCenter/UTAManager → 账户初始化（单账户失败只 warn）→ FX → 快照 → order-sync poller → catalog refresh → Hono app → `serve()` → 注册 SIGINT/SIGTERM。
> 关键脆弱点：端口来自 `Number(env ?? 47333)`，空字符串会变成 `0`（随机端口）、非数字会变成 `NaN`（同步 RangeError → fatal 退出）；`serve()` 之后立即打印 "listening"，没有 bind 失败处理，EADDRINUSE 会以未处理 'error' 事件崩溃（已实测）；没有 `uncaughtException`/`unhandledRejection` 处理器。
> 健康面 `/__uta/health` 是浅探测（恒 `ok:true`），只报 `startedAt` 与账户数量，不反映任何账户健康；账户级健康在 `/api/trading/uta` 里由 `UTAManager` 提供。
> 重启协议是跨进程文件旗标：Alice 原子写 `data/control/restart-uta.flag`，Guardian 的 fs.watch 去抖 100ms 后 SIGTERM + 重新 spawn；Alice 侧再按 `startedAt` 变化轮询确认（默认 20s）。
> 崩溃容忍度按宿主而异：dev 的 tsx watch 子进程崩溃后 **不会** 自动重启（已实测）；prod/Electron 的 UTA 崩溃只打日志、标记 offline，不重启；只有 `restart-uta.flag` 会触发重启。
> Docker 与打包宿主都用 `services/uta/dist/uta.js`（tsup，ESM，es2023），Bun standalone 则用 `--internal-role uta` 重入自身可执行文件。

---

## 1. 概览与职责边界

### 1.1 本区域负责什么

- UTA 进程的**进程级**行为：入口、启动顺序、监听端口、信号处理、退出码、健康端点、崩溃表现。
- UTA 的**生命周期管理宿主**：dev Guardian（`scripts/guardian/dev.ts`）、构建产物 Guardian（`scripts/guardian/prod.mjs`）、Electron 桌面（`apps/desktop/src/main.ts`）、Bun standalone（`packages/cli/bin/openalice-bun.ts` + `packages/cli/src/internal-role.ts`）、Docker（`Dockerfile` + `docker-compose.yml`）。
- **重启路径**：Alice 侧的 `triggerUTARestart()`（`src/services/uta-supervisor/restart-trigger.ts`）、Guardian 侧的 flag watcher 与 `OptionalServiceController.restart()`、以及进程内 `UTAManager.reconnectUTA()` 与二者的关系。
- **账户连接的生命周期状态机**：`UnifiedTradingAccount` 的 connect / health / recovery / close（`services/uta/src/domain/trading/UnifiedTradingAccount.ts`），以及 `UTAManager` 作为注册表与工厂的角色（`uta-manager.ts`）。
- UTA 的构建与打包产物流（`services/uta/package.json`、`tsup.config.ts`、`tsconfig.json`、runtime bundle 清单）。

### 1.2 不负责什么

- UTA 的 HTTP 路由细节与协议 schema：参见 `02-http-api-and-protocol.md`。
- 账户/订单/持仓的业务语义、staging/commit/push 账本：参见 `03-account-orders-positions.md`、`05-staging-approval-ledger.md`。
- broker 适配器与 Broker Pack 加载：参见 `07-brokers-and-packs.md`。
- 快照调度内容与 guard 语义：参见 `06-snapshots-and-guards.md`（本文件只写其进程级启动与 stop）。
- 持久化文件 schema 与迁移：参见 `09-persisted-state-tests-docs-issues.md`。
- Alice BFF 代理的路由行为与 UI 呈现：参见 `08-alice-consumers-and-ui.md`。
- 市场数据/FX 的契约：参见 `04-market-data-contracts-fx.md`。

### 1.3 上下游

| 方向 | 对方 | 契约 |
|---|---|---|
| 上游（宿主） | dev Guardian（`scripts/guardian/dev.ts`） | 用 `tsx watch` spawn；注入 `OPENALICE_UTA_PORT`、`OPENALICE_HOME`、`NODE_OPTIONS=--conditions=openalice-source`；轮询 `/__uta/health` 15s |
| 上游（宿主） | prod Guardian（`scripts/guardian/prod.mjs`） | `node services/uta/dist/uta.js`；注入 `OPENALICE_UTA_PORT`/`OPENALICE_HOME`/`OPENALICE_LAUNCHER`/`OPENALICE_GUARDIAN_PID` |
| 上游（宿主） | Electron（`apps/desktop/src/main.ts`） | `process.execPath + ELECTRON_RUN_AS_NODE=1` 执行 `services/uta/dist/uta.js`；`stdio: 'inherit'` |
| 上游（宿主） | Bun standalone（`packages/cli/bin/openalice-bun.ts`） | `--internal-role uta` 重入；`globalThis.__OPENALICE_INTERNAL_ROLE_DISPATCH__` 抑制直接入口 |
| 下游（被消费） | Alice（`src/main.ts`、`src/services/uta-client/*`、`src/webui/routes/trading-proxy.ts`） | HTTP `OPENALICE_UTA_URL`；健康探测 `/__uta/health`；账户列表 `/api/trading/uta` |
| 下游（被消费） | Guardian 状态面 | `runtime.status` 的 `components.uta` / `componentDetail.uta`（`state` + `pid`，`required: false`） |
| 下游（被消费） | Workspace CLI `alice-uta`、AI trading tools、Connector review | 经 Alice 的 SDK/代理（细节见 08） |
| 下游（数据） | 用户数据目录 | `data/event-log/events.jsonl`、`data/trading/<id>/`、`data/control/restart-uta.flag` |

```mermaid
flowchart TD
  CLI[openalice CLI / Docker / pnpm dev] --> G[Guardian: dev.ts | prod.mjs | Electron main]
  G -->|spawn| U[UTA process<br/>main.ts startUTAService]
  G -->|env OPENALICE_UTA_URL| A[Alice process]
  A -->|HTTP /api/trading/*| U
  A -->|原子写 restart-uta.flag| F[(data/control/restart-uta.flag)]
  F -->|fs.watch 100ms 去抖| G
  G -->|SIGTERM + respawn| U
  A -->|轮询 startedAt 变化| U
```

---

## 2. 功能清单

### 2.1 UTA 进程启动（`startUTAService` 有序步骤）

触发方式：进程被宿主 spawn，`services/uta/src/main.ts:195-200` 的直接入口守卫在 `globalThis.__OPENALICE_INTERNAL_ROLE_DISPATCH__` 为假时调用 `startUTAService()`；为真（Bun internal role、Workspace CLI 等）则由 dispatcher 显式 `await startUTAService()`（`packages/cli/bin/openalice-bun.ts:47-50`）。

启动顺序（含失败容忍）：

| # | 步骤 | 证据 | 失败时的行为 |
|---|---|---|---|
| 0 | 打印 `[uta] bootstrap @ <ISO>` | `main.ts:42-43` | — |
| 1 | 检测并打印出站代理（`HTTPS_PROXY`/`https_proxy`/`HTTP_PROXY`/`http_proxy`/`ALL_PROXY`/`all_proxy`），URL 中 `//user:pass@` 被脱敏为 `//***@` | `main.ts:44-53` | 无代理则静默跳过 |
| 2 | `await loadConfig()`（读 `data/config/*.json`，含 `accounts` 之外的 trading/marketData/snapshot） | `main.ts:56` | **抛出 → 整个 bootstrap reject → 直接入口的 `.catch` 打印 `[uta] fatal:` 并 `exit(1)`**（`main.ts:196-199`） |
| 3 | `createEventLog()` | `main.ts:63` | 失败同上（fatal） |
| 4 | `new ToolCenter()` | `main.ts:64` | — |
| 5 | `new UTAManager({ eventLog, toolCenter })` | `main.ts:65` | — |
| 6 | `purgeEphemeralUTAs(await readUTAsConfig())`：删除 `ephemeral === true` 的账户目录并回写 `accounts.json`；返回 survivors | `main.ts:69`，`src/core/config.ts:811-822` | 失败同上（fatal） |
| 7 | `buildKeylessDataUTAs(config.trading.keylessDataSources, userIds)`；若结果非空打印 `[uta] keyless data sources enabled: <ids>` | `main.ts:71-74`，`keyless-data-sources.ts` | 纯函数；id 冲突时跳过（不 shadow 用户账户） |
| 8 | 逐账户 `await utaManager.initUTA(accCfg)`，跳过 `enabled === false`；**每个账户在 try/catch 内，失败只 `console.warn` 并继续** | `main.ts:76-87` | 单账户失败不影响其它账户与服务启动（有 spec 证明，见 7.1） |
| 9 | `utaManager.registerCcxtToolsIfNeeded()` | `main.ts:88` | 无 CCXT 账户则 no-op |
| 10 | FX 装配：`getSDKExecutor()` / `buildRouteMap()` / `buildSDKCredentials(config.marketData.providerKeys)` → `new SDKCurrencyClient(...)` → `new FxService(currencyClient, undefined, config.marketData.hub)` → `utaManager.setFxService` | `main.ts:93-100` | 无 try/catch；构造期抛错即 fatal |
| 11 | 快照：`createSnapshotService({ utaManager, eventLog })`；`setSnapshotHooks` 挂 `post-push`/`post-reject` 触发器；`createSnapshotScheduler({...config.snapshot})` 并 `await snapshotScheduler.start()`；`enabled` 时打印一行 | `main.ts:104-114` | 无 try/catch；抛错即 fatal |
| 12 | order-sync poller：读 `config.trading.observeExternalOrdersEvery`；`'off'` → 0；其余 `parseDuration`；解析失败（`null`）只 warn 并回退 15m；`startOrderSyncPoller(() => utaManager.resolve(), { observeIntervalMs })`（**返回值被丢弃，永远停不下来**）；打印一行说明两条 lane | `main.ts:122-128` | 解析失败降级为 15m |
| 13 | catalog refresh：`setInterval(..., 6h)` 遍历 `utaManager.resolve()` 调 `refreshCatalog()`，单个失败只 warn；`timer.unref()` | `main.ts:134-141`（`CATALOG_REFRESH_MS = 6h` 在 `main.ts:39`） | 不阻塞退出（unref） |
| 14 | 组装 Hono app：`GET /__uta/health`、`app.route('/api/trading', ...)`、`app.route('/api/simulator', ...)` | `main.ts:145-168` | — |
| 15 | `serve({ fetch, port: UTA_PORT, hostname: '127.0.0.1' })` 并立刻打印 `[uta] listening on http://127.0.0.1:<port>` | `main.ts:172-177` | 见下 2.2；日志在 bind 成功之前打印 |
| 16 | 注册 `SIGINT`/`SIGTERM` → `shutdown(signal)` | `main.ts:179-192` | 不注册 `SIGHUP`、`uncaughtException`、`unhandledRejection` |
| 17 | 返回（函数 resolve）。**注意：`process` 靠 HTTP server 与定时器保活** | `main.ts:193` | 若 `serve` 失败且进程未退出，函数会 resolve 到 dispatcher |

### 2.2 端口解析与 bind（脆弱点集中区）

- `const UTA_PORT = Number(process.env['OPENALICE_UTA_PORT'] ?? 47333)`（`main.ts:38`）。`??` 只在 `undefined`/`null` 时回退，**空字符串不回退**。
- 实测（`node -e`）：`Number('') === 0`、`Number('abc') === NaN`、未设置 → 47333。
  - 环境变量为空串 → 端口 `0` → `listen(0)` 绑定**随机临时端口**（实测绑定到 52243），而所有宿主都按固定端口探测健康，于是健康探针永远失败但进程"看起来正常"。
  - 环境变量为非数字 → `NaN` → `serve()` **同步抛出** `RangeError [ERR_SOCKET_BAD_PORT]` → 被 `main.ts:196-199` 的 `.catch` 接住 → `[uta] fatal:` + `exit(1)`。
- bind 失败（EADDRINUSE）行为实测：Hono 的 `serve()` 不抛同步错误、返回的 server 没有任何 'error' 监听者，Node 抛 `Unhandled 'error' event` → 进程崩溃退出码 1。崩溃发生在 `[uta] listening on ...` 日志之后（该日志先于 bind 结果打印），所以 Guardian 侧会看到"打印了 listening 然后进程消失"。
- 绑定地址硬编码 `'127.0.0.1'`（`main.ts:175`），与文档"UTA 永不对外暴露"一致（`docs/local-runtime.md:105-112`、`docs/docker-deployment.md:35-38`）。端口本身可被 `OPENALICE_UTA_PORT` 改，Docker 镜像默认 `OPENALICE_UTA_PORT=47333`（`Dockerfile:155`）且 compose 只发布 47331（`docker-compose.yml`）。

### 2.3 健康端点 `/__uta/health`

- 实现为纯静态 JSON：`{ ok: true, startedAt: <ISO 字符串，进程级> , utas: utaManager.listUTAs().length }`（`main.ts:148-152`）。
- 语义：**只要进程还在 listen 就恒为 `ok: true`**，不检查任何账户、不检查 broker、不检查数据库/event log。账户级真实健康在 `GET /api/trading/uta` 的 `utas[].health`（`services/uta/src/http/routes-trading.ts:134-136` → `UTAManager.listUTAs()` → `uta.getHealthInfo()`）。
- `startedAt` 是重启确认协议的唯一标识：Alice 触发重启后轮询该字段是否变化（见 2.7）。
- 消费方：dev Guardian readiness（15s，`dev.ts:277-280`）、prod Guardian（15s，`prod.mjs:388-399`）、Electron（15s，`apps/desktop/src/main.ts:310-322` 与 `UTA_READY_TIMEOUT_MS = 15_000` 于 `:86`）、Alice 启动（**750ms**，`src/main.ts:146-152`）、Alice BFF `/status`（`STATUS_TIMEOUT_MS = 1_000`，`src/webui/routes/trading-proxy.ts:79-96`）、`triggerUTARestart` 轮询、supervisor smoke（`scripts/guardian/smoke.ts:93-96`）、Bun feasibility（`scripts/build-bun-runtime-feasibility.ts:114`）。

### 2.4 关闭路径（SIGINT / SIGTERM）

`main.ts:179-192`：

1. `stopping` 幂等守卫：第二次信号直接返回（`main.ts:181-182`）。
2. 打印 `[uta] <SIG> → shutdown`。
3. `clearInterval(catalogRefreshTimer)`。
4. `snapshotScheduler.stop()`。
5. `server.close()` —— **不 await**，也不检查在途请求；HTTP 连接可被立即切断。
6. `await utaManager.closeAll().catch(() => {})` —— 逐账户 `uta.close()`：清 recovery timer、解绑 broker connection listener、`broker.close()`；**任何错误被吞掉**（`uta-manager.ts` closeAll 段；`UnifiedTradingAccount.ts:1253-1261`）。
7. `await eventLog.close().catch(() => {})` —— 同样吞错。
8. `process.exit(0)` —— 硬退出，不等 order-sync poller（其 timer 已 unref 且无 stop 调用）、不等未完成的 `refreshCatalog()`、不等 FX 或在途 snapshot。

未处理：`SIGHUP`（dev Guardian 的 cascade 会发它，见 `scripts/guardian/shared.ts:408-410`——但那是给 Guardian 自己的信号，UTA 只收到级联的 SIGTERM）；`uncaughtException` / `unhandledRejection`（无全局处理器，任何未捕获异常走 Node 默认退出码 1）。

### 2.5 账户初始化与注册（`UTAManager`）

| 功能 | 行为 | 证据 |
|---|---|---|
| `initUTA(cfg)` | `createBroker(cfg, { fxService })`（异步解析 engine，可能 dynamic import Broker Pack）→ `loadGitState(cfg.id)` → `new UnifiedTradingAccount(broker, {...})`（构造函数内 fire-and-forget `_connect()`）→ `add(uta)` | `uta-manager.ts:62-83` |
| 健康回调 | `onHealthChange` → `eventLog?.append('account.health', { accountId, ...health })`；**未 await、无 catch**，即 event-log 写失败会变成未处理的 rejection | `uta-manager.ts:73-75` |
| `add(uta)` | 重复 id 抛 `UTA "<id>" already registered` | `uta-manager.ts:144-149` |
| `reconnectUTA(id)`（进程内） | 重入守卫 `reconnecting` Set → `readUTAsConfig()` → `removeUTA(id)` → 在配置里找 cfg（找不到算成功，返回 "removed or disabled"）→ `initUTA` → `await uta.waitForConnect()` → 若引擎是 ccxt 则重新注册 CCXT tools → 成功/失败都返回结构化 `ReconnectResult`，**不抛** | `uta-manager.ts:85-125` |
| `removeUTA(id)` | 从 Map 删除后 `uta.close()`，close 异常被吞 | `uta-manager.ts:127-132` |
| `registerCcxtToolsIfNeeded()` | 任一账户 `broker.brokerEngine === 'ccxt'` 时向 `ToolCenter` 注册 `trading-ccxt` 工具集 | `uta-manager.ts:134-141` |
| `resolve(source?)` | 无参返回全部；有参按 id 精确匹配（返回数组） | `uta-manager.ts:181-188` |
| `closeAll()` | 逐账户 close，吞错 | `uta-manager.ts:309+` |

已知时序缺口：`initUTA` 中 `createBroker` 已构造并开始连接后，若 `add()` 抛（重复 id），该 broker 不会被 close，成为孤儿连接；调用方（bootstrap）只在 `main.ts:82-86` 捕获并 warn。

### 2.6 账户级健康与自动恢复（`UnifiedTradingAccount`）

这是"UTA 进程活着但账户不健康"的完整语义。

常量（`UnifiedTradingAccount.ts:100-109`）：`DEGRADED_THRESHOLD = 3`、`OFFLINE_THRESHOLD = 6`、`RECOVERY_BASE_MS = 5_000`、`RECOVERY_MAX_MS = 60_000`、`CONNECT_GRACE_MS = 1_500`。

状态与判定：

- tier（静态）：`keyless → 'data'`；`readOnly → 'account'`；否则 `'trading'`（`UnifiedTradingAccount.ts:233-236`）。
- targetReach：`data → 'connected'`；其余 → `'readable'`（`:238-240`）。
- reach 阶梯排序 `down < connected < readable`；`_reachedTarget()` 比较 rank（`:242-249`）。
- `health` 派生规则（`:251-260`，按序短路）：
  1. `_disabled`（永久性配置错误）→ `offline`；
  2. `reach === 'down'` → `offline`；
  3. `consecutiveFailures >= 6` → `offline`；
  4. 未达 target → `degraded`（"能连上但账户读失败"，明确为避免一次 getAccount 抖动打挂整条连接）；
  5. `consecutiveFailures >= 3` → `degraded`；
  6. 否则 `healthy`。
- `getHealthInfo()`（`:262-282`）输出 `status`、`reach`、`tier`、`consecutiveFailures`、`lastError`、`lastSuccessAt`、`lastFailureAt`、`recovering`、`connecting`、`disabled`（类型定义 `packages/uta-protocol/src/types/broker.ts:381-405`）。

状态转换：

- 初始：构造函数发起 `_connect()`（fire-and-forget，`_connectPromise` 被 `.catch(() => {})` 吸收，`waitForConnect()` 返回原始 promise 供显式等待，`:220-229`）。
- `_attemptReach()`（`:285-330`）：L1 `broker.init()` → `connected`；funded 账户再 L2 `broker.getAccount()` → `readable`；keyless data 账户停在 L1、永不调用 getAccount。捕获到永久错误时置 `_disabled`。
- `_connect()`（`:354-390`）：计时并打印 `UTA[<id>]: connecting (target <reach>)…`；settle 后**先清 `_connecting` 再 emit**；warm `_ensureSubAccounts()`（best-effort）；disabled → warn + emit + 抛 `BrokerError('CONFIG')`；达 target → `_onSuccess()` + emit；否则把 failures 顶到 `OFFLINE_THRESHOLD`、`_startRecovery()` + emit，`down` 时再抛 `BrokerError('NETWORK')`。
- `_callBroker()`（`:409-440`）：`_disabled` → 直接抛 CONFIG；初次连接在途 → 先 `_awaitConnectOrGrace()`（race 1.5s grace），仍 connecting 则抛 `BrokerError('CONNECTING')`（**不计数、不降级、不触发恢复**，背景连接继续）；offline 且 recovering → 抛 CONNECTING；其余走 try：成功 `_onSuccess()`，失败 `BrokerError.from(err)` + `_onFailure()` 后重抛。
- recovery 循环：`_startRecovery()` 置 `_recovering` 并 emit + 日志，立刻排第一次尝试；`_scheduleRecoveryAttempt(attempt, delayOverride?)` 延迟 `min(5000 * 2^attempt, 60000)`；每次尝试重新 `_attemptReach()`，disabled 则退出循环，达 target 则 `_onSuccess()`（清 timer/`_recovering`/失败计数）并打印 `auto-recovery succeeded`，否则 warn + emit + 递归下一次（`:470-510`）。
- `_onBrokerConnectionState`（`:334-352`）：broker 主动报 `dead` → 直接把 reach 置 `down`、failures 顶到 6、记录 lastError，初次连接期间交给 `_connect` 收尾，否则启动/推进 recovery；`restored` → `nudgeRecovery()`（清 timer 并立即尝试）；`alive` → no-op。`nudgeRecovery` 只在 `_recovering` 时生效。
- `close()`（`:1253-1261`）：解绑 connection listener、清 recovery timer 与 `_recovering`，再 `broker.close()`。

### 2.7 进程级重启协议（flag 文件 → Guardian）

协议三方（注释见 `restart-trigger.ts:1-11`）：

1. **Alice 写入**：`triggerUTARestart(opts)`（`src/services/uta-supervisor/restart-trigger.ts:47-82`）。
   - 若 `isUTADisabled()`（`OPENALICE_LITE_MODE` 或 `OPENALICE_UTA_DISABLED` 为真值）→ 立即返回 `{ triggered: false, ready: false, error: 'UTA disabled by OPENALICE_LITE_MODE' }`（`:54-56`）。
   - 先探测一次健康取 `oldStartedAt`（失败为 `undefined`）。
   - `mkdir -p` 后**原子写**：写 `restart-uta.flag.tmp`（内容为 ISO 时间戳），再 `rename` 覆盖目标（`:67-71`）。
   - 循环轮询 `${url}/__uta/health`，间隔 200ms，总预算 20s；只要 `startedAt` 非空且与旧值不同即返回 `{ triggered: true, ready: true, oldStartedAt, newStartedAt }`；超时返回 `{ triggered: true, ready: false, error: 'UTA did not come back within timeout' }`。
   - 注意：`triggered: true` 只表示"旗标已写"，即使旧 UTA 从未启动（`oldStartedAt === undefined`）也会在第一次成功探测时判 ready。
2. **Guardian 接收**：dev 用 `startFlagWatcher`（`scripts/guardian/shared.ts:558+`，`fs.watch` + 每次改名事件去抖 100ms + 1s 轮询兜底处理 boot 期文件尚未存在的情况）；prod 自己实现同样语义但**只依赖 `fs.watch`**（`prod.mjs:569-590`），flag 名用 `FLAG_PATH.slice(lastIndexOf('/') + 1)` 取（POSIX 分隔符假设）。
3. **Guardian 重启**：
   - dev：`OptionalServiceController.restart()`（`shared.ts:491-523`）：重入守卫 → `cascade.expectExit(old)`（避免被当成崩溃）→ 若子树存活则 `killTree(SIGTERM)`，8s 未退出再 `SIGKILL` 等 5s → `spawnChild(spec)` 新子进程 → `cascade.trackReplacement` → `waitForHttp(health, 15s)`；失败打印"failed to come back up after restart"并返回 false。
   - prod：`restartUTA()`（`prod.mjs:483-535`）：先重解析 trading mode 与 product（nano/lite 则停 UTA 并置 `disabled` 直接返回）→ 重入守卫 → 若无子进程则直接 spawn；否则 `SIGTERM` 等 8s、必要时 `SIGKILL`，再 spawn，`waitForUTA()` 15s。
   - Electron：`reconcileUTA()`（`apps/desktop/src/main.ts:1197-1235`）+ `planUTATransition`（`apps/desktop/src/uta-lifecycle.ts:5-11`，纯函数：lite → stop/none；否则 running ? restart : start）；`stopManagedProcess` 用 `UTA_RESTART_GRACE_MS = 8_000`（`:88`）。
   - 三个宿主都会在重启后检查 `utaStatus`/`utaStatus` 并打印结果。

### 2.8 触发重启的调用点

| 调用点 | 效果 | 证据 |
|---|---|---|
| `notifyUTAReload()`（fire-and-forget） | 仅写旗标并后台等结果，日志 warn；不阻塞 HTTP 响应 | `src/webui/routes/trading-config.ts:29-39` |
| `POST /api/trading/config/broker-packs/:engine/install` | `notifyUTAReload()` | `trading-config.ts:163-171` |
| 创建/更新/删除账户的 config 路由 | `notifyUTAReload()`，其中更新路径**还额外调用** `ctx.utaManager.reconnectUTA(id)`（进程内重连，与进程重启叠加） | `trading-config.ts:244-246, 287-299, 322` |
| Broker Pack 自动更新器 | `notifyUTAReload` 同类触发（`src/services/broker-packs/auto-updater.ts`） | 见 07 报告 |
| `UTAManagerSDK.reconnectUTA(id)`（Alice SDK） | **不是**进程内重连：调 `triggerUTARestart()`（整进程重启），成功返回 `{success:true,message:'UTA restarted'}` | `src/services/uta-client/UTAManagerSDK.ts` 生命周期段（注释在 `:176-186`） |
| `UTAManagerSDK.removeUTA(id)` | 同样触发整进程重启，失败只 warn（best-effort） | 同上 |
| `POST /api/trading/uta/:id/reconnect`（UTA 进程内路由） | `ctx.utaManager.reconnectUTA(id)`（**进程内**、只重连单个账户，返回 200/500） | `services/uta/src/http/routes-trading.ts:221-225` |
| UI `ReconnectButton` | `api.trading.reconnectUTA(id)` → Alice BFF 的 `/api/trading/uta/:id/reconnect`（当前实现是 SDK 的整进程重启） | `ui/src/components/ReconnectButton.tsx:18-32`、`ui/src/api/trading.ts` |

即：同名 `reconnectUTA` 在 UTA 进程内是"单账户重连"，在 Alice SDK 是"整进程重启"，UI 走的是后者。

### 2.9 宿主启动路径三种形态

**A. dev（源码 + 热重载）**

- 入口：根 `package.json` 的 `dev` = `tsx scripts/guardian/dev.ts`。
- UTA spec：`command: 'tsx'`、`args: buildTsxWatchArgs('services/uta/src/main.ts', UTA_BACKEND_WATCH_INCLUDES, env)`、`prefixLogs: true`（`dev.ts:266-272`）。
- `buildTsxWatchArgs`（`scripts/guardian/dev-hot-reload.ts:27-42`）：默认 `OPENALICE_BACKEND_HOT_RELOAD` 未设置即为真，产 `watch --clear-screen=false --include services/uta/src --include packages --exclude <spec/dist/__test__ 等> <entry>`；显式假值（0/false/no/off）退化为一次性 `[entry]`。
- 子环境：`NODE_OPTIONS` 追加 `--conditions=openalice-source`、`OPENALICE_HOME`、`AQ_LAUNCHER_ROOT`、`OPENALICE_LAUNCHER=dev`、`OPENALICE_GUARDIAN_PID`、`OPENALICE_GUARDIAN_STARTED_AT`，以及 nano 项目时的 `OPENALICE_PROJECT_PRODUCT=nano` + `OPENALICE_UTA_DISABLED=1`（`dev.ts:242-263`）；UTA 额外 `OPENALICE_UTA_PORT`（`:270`）。
- 跳过条件：`projectProduct === 'nano' || initialMode.mode === 'lite'`（`dev.ts:136`）→ 不 spawn，banner 打印 `UTA → disabled (NanoAlice|lite)`（`:192`）。
- 就绪：`waitForHttp(http://127.0.0.1:<port>/__uta/health, 15s)` → `utaStatus` = ready/offline（`:275-282`）。
- 崩溃语义：UTA 被登记为 **nonCritical** 子进程（`dev.ts:368-373`），其退出只打印 `optional service offline, continuing`、不触发级联（`shared.ts:399-405`）；**UTA 侧没有任何自动重启**（只有 Connector 有 `armConnectorRecovery`，`:391-414`）。
- 叠加 tsx 语义：实测子进程 `process.exit(3)` 后 `tsx watch` wrapper **仍然存活且不重新拉起**（14 秒观察，仅 1 次运行），因此 dev 下 UTA 一旦崩溃，端口仍被 wrapper 占用、健康永久失败，且 Guardian 不会发现（除非文件变更触发 watch 重跑）。
- 重启：flag watcher → 若 nano/lite 则 SIGTERM + `utaStatus = 'disabled'`；否则无则新建、有则 `uta.restart()`（`:428-456`）。

**B. 构建产物（prod Guardian / Electron / Bun standalone）**

- 产物：`services/uta/package.json` 的 `build` = `tsup`（entry `{uta: 'src/main.ts'}` → `dist/uta.js`，ESM、es2023、sourcemap、`clean`、`splitting:false`、`skipNodeModulesBundle:true`、条件 `openalice-source`；`tsup.config.ts`）。`start` = `node dist/uta.js`。`engines.node >= 22.19.0`（与 `packages/cli/src/runtime-bundle.mjs` 的 `RUNTIME_BUNDLE_MINIMUM_NODE_VERSION` 一致）。
- `tsconfig.json` 通过 `paths: {"@/*": ["../../src/*"]}` 复用主仓 `src/`（如 `core/config.js`、`core/event-log.js`、`domain/market-data/*`），并用 `customConditions: ["openalice-source"]` 让 workspace 包解析到源码条件导出。这是 UTA 与 Alice 共享 `src/` 代码的**编译期耦合点**（也是重构必须处理的边界）。
- prod Guardian（`scripts/guardian/prod.mjs`）：`runtimeProcessSpec({ role:'uta', legacyPath:'services/uta/dist/uta.js', ... })`（`:280-300`）→ 非 bun provider 时 `cmd = node binary, args = [legacyPath]`（`scripts/guardian/runtime-process-spec.mjs`）。
- Electron：`process.execPath + ELECTRON_RUN_AS_NODE=1` 执行 `resolve(repoRoot, 'services/uta/dist/uta.js')`（`apps/desktop/src/main.ts:594, 791-813`），`stdio: 'inherit'`；打包后资源根为 `process.resourcesPath/runtime`，但 UTA 入口按 `__dirname/../../` 解析（既有的 asar 内路径假设，`assert-desktop-package.mjs:40-49` 把 `services/uta/dist/uta.js` 列为 ASAR 必需文件）。
- Bun standalone：`runtimeProcessSpec` 的 bun 分支返回 `cmd = executable, args = ['--internal-role', 'uta']`；`openalice-bun.ts:44-52` 设置 `globalThis.__OPENALICE_INTERNAL_ROLE_DISPATCH__ = true` 后动态 import `services/uta/src/main.js` 并 `await startUTAService()`，从而跳过直接入口守卫。
- 就绪与降级：prod 15s 探测失败只 warn "continuing with trading offline"（`prod.mjs:658-666`）；Electron 15s 失败 warn "continuing with trading offline"（`apps/desktop/src/main.ts:954-958`）。
- 崩溃语义：prod 的 `child.once('exit')` 置 `utaStatus='offline'` 并打印 "UTA exited unexpectedly … trading offline, Alice stays up"，**不重启**（`prod.mjs:304-314`）；Electron 同样只打印（`:807-810`）。唯一自动恢复路径是控制旗标（Bun feasibility 脚本正是这么验证的：SIGKILL UTA → 观察 offline → 写旗标 → 观察新 PID ready，`scripts/build-bun-runtime-feasibility.ts:146-165`）。
- 关闭：prod `shutdown()` 对 utaChild 先 SIGTERM，5s 后 SIGKILL，再释放 control server 与 runtime lock 后 `process.exit`（`prod.mjs:537-563`）。Electron `stopChildren()` 用 `SIGTERM_GRACE_MS` 逐子进程 `killTree`，然后释放 runtime lock 并退出（`apps/desktop/src/main.ts:1317-1345`）。
- 状态上报：两者都把 UTA 作为 `components.uta` + `componentDetail.uta`（`state` + 可选 `pid`，`required: false`）写进 runtime status（`prod.mjs:245-266`；`dev.ts:218-236`）；Supervisor CLI 把它渲染为 "● UTA ready / ○ UTA off / ◇ UTA ?"（`packages/cli/src/supervisor-connection-chronicle.ts:281-295, 309-325`）。

**C. Docker / 远程 Runtime**

- 镜像：`Dockerfile` 构建 UTA 并把 `services/uta/dist`、`scripts`、`node_modules` 一起放入；`ENV OPENALICE_UTA_PORT=47333`、`OPENALICE_HOME=/data`、`HOME=/data/home`（`:148-160`）。
- 启动：`ENTRYPOINT ["/usr/bin/tini","--"]` + `CMD ["node","scripts/guardian/prod.mjs"]`（`:169-170`）；tini 负责信号转发与僵尸回收，Guardian 负责 spawn UTA → Alice。
- 健康检查针对 **Alice** 的 `/api/version`（`:164-165`），不针对 UTA；compose 只映射 `47331:47331`，注释明确 MCP 端口不暴露（`docker-compose.yml:24-30`）。
- 远程 Runtime：文档要求 UTA 端口（以及 MCP/control/PTY）永不公开，远程只暴露 Alice 的 web 面（`docs/remote-access.md:106-109`）；`runtime.status` 中 UTA 作为可选组件上报（`docs/remote-access.md:445`、示例 `:509` "uta": "disabled"）。
- 远程/分离部署目前没有"UTA 跑在另一台机器"的产品路径：`OPENALICE_UTA_URL` 虽可指向任意 URL（`src/services/uta-supervisor/url.ts:10-14`），但 Guard 的 spawn、flag 文件、`OPENALICE_HOME` 与 kill 逻辑都假设同机同 home。

### 2.10 Alice 侧引导与降级（与 UTA 进程的耦合）

- `src/main.ts:144-155`：解析 trading mode；lite → warn 并完全跳过探测；否则 `waitForUTAReady({ baseUrl, timeoutMs: 750 })`。**750ms 远小于 Guardian 的 15s 与 UTA 冷启动时间**（CCXT loadMarkets 可能数十秒），因此 Alice 启动日志常出现 "unavailable … continuing in lite mode" 而 UTA 稍后其实健康——功能上无害（BFF 每次请求重新探测），但会让启动日志与真实状态不一致。
- `UTAManagerSDK` 始终被构造（`src/main.ts:156-159`），在不可用/只读时用 `unavailableReason`/`readonlyMutationReason` 返回结构化错误。
- BFF `/api/trading/status` 每次请求实时探测（1s 超时），不可达时返回 `available:false` 与原因，不缓存（`src/webui/routes/trading-proxy.ts:74-116`）。
- Alice 侧对 `/status` 与代理路由的细节见 `08-alice-consumers-and-ui.md`。

---

## 3. 数据与状态

### 3.1 内存状态

| 归属 | 状态 | 生命周期 | 说明 |
|---|---|---|---|
| `main.ts` | `startedAt`（ISO 字符串） | 进程 | 健康端点与重启确认标识 |
| `main.ts` | `stopping` 布尔 | 进程 | 关闭幂等 |
| `main.ts` | `catalogRefreshTimer` | 进程（unref） | 6h |
| `main.ts` | `snapshotScheduler` | 进程 | 由 `config.snapshot` 驱动 |
| `main.ts` | **order-sync poller 句柄（被丢弃）** | 进程（unref） | 无法停止；`main.ts:127` |
| `UTAManager` | `entries: Map<id, UnifiedTradingAccount>` | 进程 | 启动注册 + 运行时增删 |
| `UTAManager` | `reconnecting: Set<id>` | 进程 | 进程内重连重入守卫 |
| `UnifiedTradingAccount` | `_consecutiveFailures`、`_lastError`、`_lastSuccessAt`、`_lastFailureAt`、`_recoveryTimer`、`_recovering`、`_disabled`、`_connecting`、`_currentReach`、`_connectPromise` | 账户对象 | 见 2.6 |
| Guardian（三个宿主） | `utaStatus` / `utaStatus`、子进程句柄、flag watcher、cascade 集合 | 宿主进程 | `starting|ready|offline|disabled|stopping` |

### 3.2 持久化与文件

| 路径 | 生产者 | 消费者 | 格式/语义 |
|---|---|---|---|
| `data/control/restart-uta.flag` | Alice（原子 rename 写；内容为 ISO 时间戳，但**内容不被读取**） | Guardian（fs.watch 事件触发） | 仅"存在/被改动"是信号；文件不被删除（每次触发靠 mtime/rename 事件） |
| `data/config/accounts.json` | Alice 配置路由；UTA 启动时 `purgeEphemeralUTAs` 会删除 `ephemeral` 项并回写 | UTA（每次启动、进程内 reconnect 时重读）、Guardian（`hasPersistedUTAs`、加密封套解封） | schema 见 09 报告 |
| `data/config/trading.json` | 用户/设置 | UTA（`observeExternalOrdersEvery`、`keylessDataSources`）、Guardian（`mode`） | `mode` 可被 env 覆盖 |
| `data/config/snapshot.json` | 用户 | UTA（调度器） | `enabled` 默认 true、`every` 默认 `'15m'`（`src/core/config.ts:373-382`） |
| `data/event-log/events.jsonl` | UTA（`account.health` 等） | 事件系统消费方 | 追加式；内存环形缓冲默认 500（`src/core/event-log.ts:96, 111`） |
| `data/trading/<utaId>/`（git 状态） | UTA（`createGitPersister`） | UTA 启动 `loadGitState` | 见 03/09 报告 |
| `<home>/data/config/alice-project.json` | 安装/首次启动 | Guardian（product = trader/nano） | 缺失视为 `trader`（`packages/guardian-runtime/src/alice-project-product.ts:16-49`） |
| `runtime/broker-packs/` | Alice 安装器 | UTA（动态 import） | 见 07 报告 |

### 3.3 进程级默认端口与 URL

- UTA 端口默认 47333（`main.ts:38`；`prod-ports.mjs` 的 `PORT_DEFAULTS.uta = 47333`；Dockerfile ENV）。
- Alice 解析 UTA URL：`OPENALICE_UTA_URL` 优先，否则 `http://127.0.0.1:${OPENALICE_UTA_PORT || '47333'}`（`src/services/uta-supervisor/url.ts:10-14`）。
- 端口分工：web 47331 / mcp 47332 / uta 47333 / connector 47334（`scripts/guardian/prod-ports.mjs:8-15`）；dev 与 Electron 会动态 claim 空闲端口并写入 `ports` 文件（`apps/desktop/src/main.ts:759-770`）。

---

## 4. 外部交互

| 方向 | 对方 | 协议 | 契约要点 |
|---|---|---|---|
| UTA ← 宿主 | Guardian dev/prod/Electron/Bun | 进程 spawn + env | 见 2.9；环境变量表见 5 |
| UTA ← Alice | HTTP | `/__uta/health`、`/api/trading/*`、`/api/simulator/*` | 路径与 schema 见 02 |
| UTA → Alice | 无（无回调） | — | UTA 不知道 Alice 存在；重启只能靠 flag 文件 |
| UTA ↔ Broker | SDK / TCP | IBKR TWS socket、CCXT REST/WS、Longbridge、Alpaca | 见 07 |
| UTA → 数据目录 | 文件 | `events.jsonl`、`data/trading/<id>/` | 见 3.2 |
| UTA → 进程外 | CCXT 出站（可经代理） | 依赖 `HTTPS_PROXY` 等 | 启动时打印脱敏后的代理（`main.ts:44-53`） |
| 宿主 → UTA（终止） | 信号 | `SIGTERM`（正常）、`SIGKILL`（超时兜底） | 见 2.4/2.9；宿主不发送自定义信号 |

---

## 5. 配置项、默认值、环境变量、feature 开关

### 5.1 UTA 进程读取的环境变量

| 变量 | 默认 | 作用 | 证据 |
|---|---|---|---|
| `OPENALICE_UTA_PORT` | 47333 | 监听端口（`Number()` 转换，见 2.2） | `main.ts:38` |
| `OPENALICE_HOME` | `~/.openalice`（`src/core/paths.ts:37-39`） | 用户数据根；Docker 为 `/data` | `src/core/paths.ts` |
| `OPENALICE_APP_HOME` | `process.cwd()` | 应用资源根（打包时为 `resources/runtime`） | `src/core/paths.ts:40` |
| `OPENALICE_LAUNCHER` | — | 宿主身份标识（dev/electron/docker/…） | 各宿主注入 |
| `OPENALICE_GUARDIAN_PID` / `OPENALICE_GUARDIAN_STARTED_AT` | — | 宿主存活观测（UTA 本身不读，仅记录） | 各宿主注入 |
| `OPENALICE_LITE_MODE` / `OPENALICE_UTA_DISABLED` | 假 | **UTA 进程自身不读取**；只影响宿主是否 spawn 与 Alice 是否触发重启 | `grep` 无命中；`url.ts:18-20`、`trading-mode.ts:16-18` |
| `OPENALICE_TRADING_MODE` | — | 同上，宿主/Alice 读取 | `packages/guardian-runtime/src/trading-mode.ts:22-33` |
| `OPENALICE_BACKEND_HOT_RELOAD` | 真 | dev 宿主 tsx watch 开关 | `scripts/guardian/dev-hot-reload.ts:17-25` |
| `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`（含小写） | — | 出站代理探测并打印；CCXT 经 `applyEnvProxy` 桥接 | `main.ts:46-49` |
| `NODE_OPTIONS` | — | dev 注入 `--conditions=openalice-source` | `dev.ts:246` |

### 5.2 配置文件中的相关项

| 项 | 默认 | 作用 |
|---|---|---|
| `trading.observeExternalOrdersEvery` | `'15m'`，`'off'` 关闭 | 外部订单观察 lane 周期；非法值 warn 后回退 15m（`src/core/config.ts:394-403`；`main.ts:122-126`） |
| `trading.keylessDataSources` | `[]` | 需要时自动生成 `<ex>-readonly` 的 keyless 数据 UTA（binance/okx/bybit） |
| `trading.mode` | 未设置（auto） | lite/readonly/pro；env 优先 |
| `snapshot.enabled` / `snapshot.every` | `true` / `'15m'` | 快照调度器周期 |
| `accounts[].enabled` | — | `false` 时启动跳过该账户 |
| `accounts[].ephemeral` | — | 启动时清除该账户数据并从配置删除 |

### 5.3 内建常量（不可配置）

`CATALOG_REFRESH_MS = 6h`（`main.ts:39`）、order-sync 快 lane `10_000ms`（`order-sync-poller.ts:43`）、`CONNECT_GRACE_MS = 1_500`、`RECOVERY_BASE_MS = 5_000`、`RECOVERY_MAX_MS = 60_000`、`DEGRADED_THRESHOLD = 3`、`OFFLINE_THRESHOLD = 6`（`UnifiedTradingAccount.ts:100-109`）、Guardian 就绪 15s、flag 去抖 100ms、Alice 重启等待 20s/200ms、Alice 启动探测 750ms、BFF 状态探测 1s。

---

## 6. 不变量、时序与并发假设

### 6.1 不变量

1. UTA 只绑定 `127.0.0.1`，端口由 `OPENALICE_UTA_PORT` 决定（`main.ts:175`）——文档与实现一致。
2. `/__uta/health` 只要进程在监听就返回 `ok:true`；**它不是就绪语义的充分条件**（`main.ts:148-152`）。
3. 单个账户初始化失败不影响其它账户与 HTTP 面（`main.ts:76-87`，并有端到端 spec 支撑）。
4. `startedAt` 在一次进程生命周期内不变；Alice 用它做重启确认（`main.ts:42`、`restart-trigger.ts:66-80`）。
5. 重启的唯一跨进程信号是 `data/control/restart-uta.flag`；UTA 进程内不做热重载（`main.ts:1-11` 顶部注释）。
6. `UTAManager` 中 id 唯一，重复 `add` 抛错（`uta-manager.ts:144-149`）。
7. 一个账户的 `_recovering` 为真时不会重复启动 recovery 循环（`_startRecovery` 幂等）。
8. UTA 不依赖 Alice 存活；Alice 不依赖 UTA 存活（双向可选）。

### 6.2 时序假设

- 顺序：宿主先 spawn UTA 再 spawn Alice（dev `:286 → :320`；prod `:658 → :670+`；Electron `:954 → :962`），因此 Alice 通常能在 750ms 探测窗口内看到已启动的 UTA——但 UTA 的 `loadConfig` + N 个账户构造会拖长这一窗口，冷启动时 Alice 大概率记 "lite mode"。
- 重启完成判定完全依赖 `startedAt` 变化；若新进程恰好复用了同一 ISO 时间戳（毫秒级碰撞几乎不可能，但 `startedAt` 由宿主时钟产生）或旧进程从未健康过，判定会退化为"第一次成功探测即 ready"。
- Guardian 的 flag watcher 在 prod 只监听 `fs.watch` 事件；卷/文件系统不支持 inotify 或跨容器挂载时有丢事件风险（dev 路径额外有 1s 轮询兜底；prod 没有）。
- 重启窗口内 Alice 的 `/api/trading/*` 大概率得到 502/连接失败（BFF 直连，无重试队列；`trading-proxy.ts` 的 `PROXY_TIMEOUT_MS = 30_000`）；UI 上的表现由 08 报告描述。

### 6.3 并发假设

- `triggerUTARestart` 没有互斥：多个 config 变更并发时可能重复写旗标；Guardian 侧 `restart()`/`restartUTA()` 有重入守卫（dev/prod 都是"already in progress → skip/false"），因此**并发触发中只有第一个会真正重启**，后续被静默丢弃（对调用方而言 `ready:false`）。
- `trading-config.ts` 更新路径同时 `notifyUTAReload()` 与 `ctx.utaManager.reconnectUTA(id)`，二者都会尝试进程重启：一个走旗标、一个直接写旗标，叠加后同样只有一次生效。
- 账户连接与 HTTP 请求并发：`_callBroker` 用 1.5s grace + CONNECTING 快速失败避免阻塞；recovery 定时器与请求路径共享 `_currentReach`/`_consecutiveFailures`，无锁（单线程事件循环串行化，但 await 点之间可交错）。
- order-sync poller 有 `running` 重入守卫，避免慢 broker 叠加上一轮（`order-sync-poller.ts:55`）。
- catalog refresh 与 poller 都是"遍历所有账户"但互不协调，同一账户可能同时被 refreshCatalog 与 sync 调用（单账户内部无并发保护）。

---

## 7. 测试覆盖

### 7.1 有覆盖

| 测试 | 覆盖行为 |
|---|---|
| `services/uta/src/uta-startup-resilience.spec.ts`（165 行，platform contract lane，`scripts/test-lanes.mjs:148`） | 真实 spawn `node --import tsx services/uta/src/main.ts`，用假的 TWS server 在握手期断连，同时配置一个健康的 mock 账户；断言：健康端点 `ok:true` 且 `utas===2`、故障账户 `status==='offline'`、`reach==='down'`、`connecting===false`、`recovering===true`、`lastError` 含 `'closed during handshake'`、健康账户 `healthy/readable`、进程未退出、输出不含 `[uta] fatal:`、再等 100ms 后仍 `ok:true`（`:49-165`）。这是本区域**唯一**的进程级端到端测试。超时 30s/40s。 |
| `services/uta/src/domain/trading/__test__/uta-health.spec.ts`（258 行，fake timers） | 初次连接成功/失败、恢复成功、指数退避重试、运行时断开与恢复、offline 行为、`close()` 清理（6 个 describe 块） |
| `services/uta/src/domain/trading/uta-manager.spec.ts` | add/去重抛错/remove/未知 id、`listUTAs` 摘要、`resolve` 与 `resolveOne` 分支、聚合 equity、合约搜索范围、`closeAll` 容错（失败账户不阻断） |
| `services/uta/src/domain/trading/order-sync-poller.spec.ts` | 完整生命周期（push→pending→价格穿越→记录成交）、跳过 keyless/不健康/无挂单账户、外部订单慢 lane、单账户失败不影响其它账户 |
| `services/uta/src/domain/trading/UnifiedTradingAccount.spec.ts`（约 1103-1205 段） | 健康跟踪、连续 3 次失败降级 |
| `scripts/guardian/shared.spec.ts` | 端口文件解析/优先级、`isLiteModeEnv`、`resolveGuardianTradingMode`、Windows shim、`startFlagWatcher` 检测新旗标（含 fs.watch + 轮询兜底） |
| `scripts/guardian/dev-hot-reload.spec.ts` | 默认开启、关闭后退化、参数拼装包含 `--include services/uta/src` |
| `scripts/guardian/runtime-process-spec.spec.ts` | bun 分支重入私有 role；其它 provider 保持 node 入口 |
| `packages/guardian-runtime/src/trading-mode.spec.ts` | 模式解析优先级（env > config > auto） |
| `packages/cli/src/supervisor-connection-chronicle` 系列 spec | UTA 状态行渲染（ready/off/unknown 映射） |
| `scripts/guardian/smoke.ts`（`pnpm test:system:dev-stack`） | 真实 `pnpm dev` 全栈：解析 banner 端口、等待 UTA 健康 200、**软失败**地验证 UTA 重启（写旗标后 `startedAt` 变化）、检查退出后端口释放 |
| `scripts/build-bun-runtime-feasibility.ts` | Bun 多进程：SIGKILL UTA → 断言 `componentDetail.uta.state === 'offline'` 且 Alice 仍 ready；写旗标 → 断言新 PID 与 `ready` |

### 7.2 无覆盖（重要缺口）

- `src/services/uta-supervisor/restart-trigger.ts` 本身没有 spec：`triggered/ready/超时/disabled` 四个分支、`startedAt` 比较逻辑均无直接测试（只有 `optional-carrier` 的 decode 测试与 `trading-config.spec.ts` 里的 mock）。
- `OptionalServiceController.restart()`（dev）与 `restartUTA()`（prod）没有单测；smoke 里 UTA 重启被显式降级为 **SOFT（永不 fail）**（`smoke.ts:285-306`），所以"dev 重启路径坏了"不会让任何 CI 变红。
- prod Guardian 的 flag watcher（仅 fs.watch、POSIX 分隔符）无测试。
- Electron 的 `reconcileUTA`/`planUTATransition` 只有纯函数级覆盖（`uta-lifecycle.ts` 极小），进程级路径无测试。
- **UTA 崩溃后的行为**在两个宿主里都无测试：prod/Electron 的"只报不重启"、dev 的"tsx wrapper 悬挂"均未被断言。
- 端口解析边界（空串 → 0、非数字 → NaN）无测试。
- `main.ts` 的关闭序列（`server.close()` 不 await、错误吞掉、`process.exit(0)`）无测试。
- `unhandledRejection`（如 `onHealthChange` 里 eventLog append 失败）无测试。

---

## 8. 观察到的问题、技术债、耦合与重构关注点

### 8.1 已实测的缺陷（附探针）

1. **端口解析把空串当 0、把非数字当 NaN**（`main.ts:38`）：`OPENALICE_UTA_PORT=""` 实测 `Number('') === 0` → 绑定随机端口（探针绑定到 52243），宿主固定端口探测永不成功但进程不报错；`OPENALICE_UTA_PORT="abc"` → `NaN` → `serve()` 同步抛 `RangeError [ERR_SOCKET_BAD_PORT]` → fatal exit 1。宿主侧（Guardian/Electron）大多用 `String(port)` 写入，交互面小，但 `OPENALICE_UTA_PORT` 是文档化的用户可设变量。
2. **bind 失败无处理，且日志先于 bind**（`main.ts:172-177`）：探针证明 `serve()` 在 EADDRINUSE 时不抛同步错误、返回的 server 没有 'error' 监听者，Node 抛 `Unhandled 'error' event` 并以退出码 1 终止；此时 `[uta] listening on ...` 已打印。守卫与 smokes 里有"端口被旧 UTA 占用"的注释（`smoke.ts:288-305`、`shared.ts:283-292`），说明这是已知故障模式，但 UTA 侧没有针对它的诊断输出。
3. **dev 下 UTA 崩溃不会被恢复**：实测 `tsx watch` 子进程 exit 3 后 wrapper 存活 ≥14s 且只运行过 1 次；Guardian dev 把 UTA 标 nonCritical 且没有 UTA 崩溃监听（对比 Connector 有 `armConnectorRecovery`，`dev.ts:391-414`）；`smoke.ts` 又把重启验证降级为 SOFT。合起来意味着"开发环境中 UTA 静默死亡"可以长期无人发现。
4. **prod/Electron 崩溃不重启**：`prod.mjs:304-314`、`apps/desktop/src/main.ts:807-810` 只打日志。用户要恢复必须改一次 broker 配置（触发旗标），或者重启整个 Runtime。Bun feasibility 脚本把这个缺口当作"设计"验证（SIGKILL 后 state 变 offline，再写旗标才恢复，`build-bun-runtime-feasibility.ts:146-165`）。
5. **`startedAt` 作为重启标识的语义弱点**：`restart-trigger.ts` 在旧进程从未健康（`oldStartedAt === undefined`）时，只要新进程健康即判 ready；同时 `triggered: true` 只保证旗标写入，调用方无法区分"Guardian 收到并重启"与"没人监听"。UI 的 `ReconnectButton` 因此可能显示成功而实际什么都没发生（反向：Guardian 未运行时也会报 ready）。
6. **`triggerUTARestart` 无并发合并**：并发触发只有第一次生效，其余调用方拿到 `ready:false` 但错误信息只说超时（`restart-trigger.ts:74-81`）。
7. **更新路径双触发**：`trading-config.ts:244-246` 同时 `notifyUTAReload()`（旗标）与 `ctx.utaManager.reconnectUTA(id)`（SDK → 又一次旗标），语义重叠且第二个调用被重入守卫丢弃，日志会误导排查者。
8. **`notifyUTAReload`/SDK 的"成功"语义与 UTA 进程内 `/uta/:id/reconnect` 重名**：同一用户动作在 UI、SDK、UTA 内部三处含义不同（整进程重启 vs 单账户重连），见 2.8。

### 8.2 结构与耦合关注点

- **`main.ts` 是启动聚合器而非装配层**：协议装配（HTTP）、后台循环（poller/refresh/snapshot）、领域初始化（FX/账户/purge）与进程生命周期（bind/信号）混在同一 200 行函数内，且**没有任何依赖注入或 start/stop 返回值**——`startUTAService()` 返回 `Promise<void>`，调用方无法停止它、无法拿到端口、无法拿到 manager 句柄（`main.ts:41`）。重构若要把 UTA 变成可测试的模块，第一步应是引入显式的 lifecycle 对象（start 返回 `{ port, manager, stop }`）。
- **后台任务没有统一所有权**：`snapshotScheduler` 有 stop，`catalogRefreshTimer` 手工 clear，order-sync poller 的句柄被丢弃（`main.ts:127`）。关闭序列里没有任何"停止并等待在途工作"的语义。
- **健康语义双轨**：`/__uta/health`（进程）与 `/api/trading/uta`（账户）分离，且前者叫 "health" 却完全不反映健康；Alice 的 `waitForUTAReady`（750ms）与 Guardian 的 15s 是两个不同容忍度，同一事实在两个消费者的日志里呈现相反结论。
- **UTA 与 Alice 通过 `@/*` 编译期共享 `src/`**（`tsconfig.json` paths + `tsup` conditions）：`loadConfig`、`event-log`、`paths`、`market-data/credential-map` 等都在 Alice 侧。这既是当前的耦合点，也是"UTA 是独立服务"叙事的反例（进程独立、代码不独立）。相关持久化与配置 schema 见 09 报告。
- **宿主三份实现**：dev/prod/Electron 各自实现"读 flag → kill → respawn → 探活 → 设状态"，彼此有细微差异（去抖、轮询兜底、grace、日志、状态值域），Bun/远程再叠一层。共享部分只有 `OptionalServiceController`（仅 dev 使用）与 `killTree`。
- **文档与实现的偏差**：`docs/local-runtime.md:105-112` 与 `docs/remote-access.md` 把 UTA 描述为可由 Bun 角色启动的独立可选进程（与实现一致）；但 `docs/uta-live-testing.md`、`docs/testing.md` 未描述任何进程生命周期验证路径。`docs/local-runtime.md:193` 声称候选版本会"强制 Connector 与 UTA 故障并证明其恢复且不重启 Alice"——实现上这由 `build-bun-runtime-feasibility.ts` 承担，而**恢复只能通过写旗标**，与"自动恢复"的通常读法有落差。
- **`process.exit(0)` 的硬退出**：在途的 event-log append、git persist、snapshot 写盘都可能被截断（UTA 侧对 `account.health` 的 append 甚至不 await）。若重构关注停机数据完整性，这是第一个要改的点。
- **`ephemeral` 账户 purge 是启动副作用**：`purgeEphemeralUTAs` 在**每次** UTA 启动时写 `accounts.json`（`src/core/config.ts:811-822`），并与 Alice 的配置写入共享同一文件，重启风暴时存在写者竞争。

---

## 9. 未探索区域与开放问题

### 9.1 未探索区域

1. **Electron 打包产物的实际运行验证**：我只读了 `apps/desktop/src/main.ts` 与打包断言（`scripts/assert-desktop-package.mjs` `ASAR_REQUIRED_FILES` 含 `services/uta/dist/uta.js`），没有运行 `desktop-packaged-smoke` 或检查 `electron-builder` 配置（仓库内未找到 `electron-builder.yml`；打包命令的配置文件位置未定位）。
2. **Bun standalone 的 UTA 角色在真实安装态的表现**：只读了 `openalice-bun.ts`、`internal-role.ts`、`runtime-process-spec.mjs` 与 feasibility 脚本；没有运行 `openalice up/status`。
3. **远程 Runtime（SSH/服务器）下的 UTA 行为**：`docs/remote-access.md` 的相关章节只作阅读，未验证 `runtime.status` 的实际输出。
4. **`scripts/guardian/prod-ports.mjs` 的完整端口冲突策略**（`planProdPorts` 在 skipUta 与实际占用冲突时的行为）只读了片段。
5. **`control-server`/`runtime-lock` 与 UTA 无关但同属 Runtime 生命周期**：未深入（属于其他区域）。
6. **事件系统对 `account.health` 的消费端**：`docs/event-system.md` 未读，appender 的失败模式（未 await）没有实证。
7. **order-sync poller / snapshot 调度器在关闭时的实际在途行为**：未做运行时实验。
8. **Windows 路径假设**（prod flag 名用 `/` 切分、`killTree` 的 taskkill 分支）未在 Windows 上验证。

### 9.2 开放问题（供设计阶段决策）

1. UTA 崩溃后应该自动重启吗？如果要，谁来重启、退避多少、以什么为"健康失败"的判据（当前 `/__uta/health` 不足以判断）？
2. 进程健康与账户健康是否应合并为一个契约（例如 `/__uta/health` 返回聚合 `degraded`）？现在两轨并存导致监视器各说各话。
3. "重启"的用户语义应该保留整进程重启，还是提供单账户热重连？当前两个端点同名不同义。
4. `startUTAService()` 是否应改为返回可停止、可观测的 lifecycle 句柄，以便宿主统一停止后台任务与等待在途工作？
5. UTA 是否应继续通过 `@/*` 直接复用 Alice 的 `src/`？若不，`loadConfig`/`event-log`/`paths` 需要哪个包承载？
6. 端口解析是否改成严格校验（拒绝空串/NaN，或改用 `portSchema` 同款 zod 校验）并让 bind 错误带上诊断日志？
7. dev 宿主是否应把 UTA 的崩溃纳入恢复（对齐 Connector 的 `RestartBackoff`），并把 smoke 里的 UTA 重启验证从 SOFT 提升为硬断言？
