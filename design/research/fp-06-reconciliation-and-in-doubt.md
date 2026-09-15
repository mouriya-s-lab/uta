# FP 调查 06：对账与 in-doubt——成熟系统如何建模"已发出、结果不可判定"的外部写

> 本报告只做一手案例调查与横向对比，为核心设计 §8（写与 unknown）提供先例证据，**不做 UTA 设计**。所有论断来自本次实际打开的一手来源（官方文档页、标准/RFC/draft 原文、论文 PDF、公开仓库 clone 到 `/tmp/` 并 pin commit）；术语、类型签名、状态枚举、表名、字段名一律保留英文原文。每条论断标 `[OBSERVED]`（紧邻原文短引与定位）或 `[INFERENCE]`（基于已列证据的分析，不冒充产品契约）。本仓库源码一律未读；每个案例的**逐条完整证据**在同目录 `local://` 研究底稿（见来源清单末的底稿索引），本文是其忠实压缩。

## 摘要（≤10 行）

成熟系统把 in-doubt 建模为**一个显式的、持久的一等状态或 error label**（MongoDB `UnknownTransactionCommitResult`、Oracle `in-doubt` + `DBA_2PC_PENDING.STATE`、PostgreSQL/MySQL `prepared`、Kafka `PREPARE_COMMIT`、Seata `PhaseOne_Timeout`/`STATUS_TRIED`），而不是一个 `Option`、错误字符串或"尚未终结"的 `pending`——它特指"发起方无法凭当前持久证据判定这次有外部副作用的尝试未发生/已发生/仍可能发生"。两阶段协议的作用范围结论有两层：**物理上**，prepare 记录只驻留在真正拥有可提交写状态的资源管理器与协调者日志里（这一窄义支持维护者"两阶段只在写边界"的工作假设）；但**状态上**，本次纳入的每一个 prepare/in-doubt 先例都必然把该状态泄漏到写边界之外的四个接触面——串行化/阻塞、写边界外的对账发起方、审计/投影视图、上游调用方的超时语义——无一例外。收敛从来不是"超时=失败"，而是稳定身份 + 权威查询/日志恢复/独立事件流/安全重放/显式人工裁决；`[INFERENCE]` 因此"两阶段只在写边界"应改写为条件化设计推论，而非普遍定理。核心设计 §8 的"无成熟案例直接建模"这一句被 MongoDB/Oracle 直接反驳，须修正。

## 范围与方法

### 判定口径（贯穿全文，用于区分真 in-doubt 与普通 pending）

- **in-doubt**：发起方无法依据当前持有的持久证据判断一次"有外部副作用的尝试"究竟未发生、已发生、还是仍可能发生。它 ≠ 普通 `pending`/`queued`/`processing`。若某状态只是"尚未终结"但发起方已知对方接受，不计为 in-doubt。
- **写边界**：第一次可能使外部权威系统状态发生持久变化的调用及其本地持久化交界，不是任意 service/API boundary。
- **收敛**：获得权威结果 / 对同一逻辑尝试安全重放 / 由独立证据流补齐 / 显式进入 heuristic 或 manual resolution。**"超时"本身不是收敛**。
- **防重身份**：需说明它标识的是逻辑操作、一次网络尝试、事务分支还是订单；并记录有效期/保留期。无保留期证据不得声称可无限重放。

### 两阶段的四种分类（不得统称"两阶段"）

`[INFERENCE]` 本文严格区分四类，避免把它们冒充为同一个"prepare"：①**原子提交协议** `prepare`/`commit`（PostgreSQL / Oracle / MySQL XA / Kafka / MongoDB 跨 shard）；②**TCC** `Try`/`Confirm`/`Cancel`（Seata）；③**业务状态机**（Stripe PaymentIntent）；④**查询/重放/对账收敛协议**（Binance REST + FIX、Temporal Activity）。只有 ① 与 ② 有真正的 prepare 记录；③④ 靠稳定身份 + 查询/事件/重放收敛，**不暴露** prepare 记录。

### 案例与支撑材料的区分

**完整系统案例（逐项五问）**：PostgreSQL prepared transaction、Oracle in-doubt distributed transaction、MySQL XA、Stripe PaymentIntent、Binance Spot order、Temporal Activity、Apache Kafka（KIP-98）、Apache Seata TCC、MongoDB `UnknownTransactionCommitResult`（9 个）。**支撑材料（解释协议/边界，不套五问）**：X/Open XA 语义（经 Oracle/MySQL 页转述）、IETF Idempotency-Key draft、FIX Session/Application 协议、Nasdaq FIX DROP、卡组织 auth/clearing/settlement、Garcia-Molina & Salem《SAGAS》、Pat Helland 两篇。

---

## 案例 1：PostgreSQL prepared transaction（分类①原子提交）

**① 系统/状态名/边界。** `[OBSERVED]` `PREPARE TRANSACTION` 定义为 two-phase commit 的 prepare；状态名是 `prepared transaction`。原文："After this command, the transaction is no longer associated with the current session; instead, its state is fully stored on disk, and there is a very high probability that it can be committed successfully, even if a database crash occurs before the commit is requested."（sql-prepare-transaction.html, Description）。`[INFERENCE]` 写边界是 `EndPrepare` 里 `XLOG_XACT_PREPARE` 的 WAL flush 交界：源码 `twophase.c:1226` 原文注释 "If we crash now, we have prepared: WAL replay will fix things"（pin `862092932c9479b79732f3b441da05453ea5e06d`）。

**② 类型/状态定义。** `[OBSERVED]` 视图 `pg_prepared_xacts` "contains one row per prepared transaction. An entry is removed when the transaction is committed or rolled back."，字段 `transaction xid` / `gid text` / `prepared timestamptz` / `owner name` / `database name`（view-pg-prepared-xacts.html）。源码 `GlobalTransactionData`（`twophase.c:150-173`）字段含 `prepare_start_lsn`/`prepare_end_lsn`/`fxid`/`locking_backend`/`valid`/`ondisk`/`inredo`/`gid[GIDSIZE]`；共享索引 `TwoPhaseStateData`（`twophase.c:175-191`），落盘头 `TwoPhaseFileHeader = xl_xact_prepare`（`twophase.c:963-984`，字段在 `xact.h:353-371`）。

**③ 收敛协议。**
- *resolver/发起方*：`[OBSERVED]` 参与者 resolver 是 `COMMIT PREPARED`/`ROLLBACK PREPARED`，"can be issued from any session, not only the one that executed the original transaction."；文档指定 external transaction manager 为使用者——"Unless you're writing a transaction manager, you probably shouldn't be using PREPARE TRANSACTION."
- *authoritative evidence owner*：`[INFERENCE]` PostgreSQL 实例的本地 TM（`TwoPhaseState`+WAL+`pg_xact`）是本地 prepared 与最终结果的权威；全局多资源决策归 external TM，本次来源未规定其日志格式。
- *evidence channel*：`[OBSERVED]` (i) 状态查询 = 读 `pg_prepared_xacts`；(ii) 日志回放 = `RecoverPreparedTransactions` "reload the state for each prepared transaction (reacquire locks, etc)."（`twophase.c:2074-2180`）+ `PrepareRedoAdd`（`twophase.c:2506-2613`）；(iii) 显式 `COMMIT`/`ROLLBACK PREPARED`。**无** webhook/drop-copy/自动 heuristic。
- *timeout*：`[OBSERVED]` 文档无 TTL、无自动 resolver、无"超时即失败"；反而警告不要久留。
- *stable identity*：`[OBSERVED]` 调用者提供的 `gid`（`< 200 bytes`，"must not be the same as the identifier used for any currently prepared transaction"，冲突报 `transaction identifier "%s" is already in use`，`twophase.c:391-404`）；无时间保留承诺。
- *crash window*：`[OBSERVED]` `XLogFlush` 之前无"已持久 prepared"保证；之后即使原 session 未返回，WAL redo 也恢复为 prepared（`twophase.c:1215-1257`）。
- *终态*：`[OBSERVED]` `COMMIT PREPARED`→committed、`ROLLBACK PREPARED`→rolled back，两者都使 `pg_prepared_xacts` 行移除。未发现 heuristic/manual 状态枚举。

**④ prepare 作用范围与写边界外泄漏。** `[OBSERVED]` prepare state 落在参与写的 WAL + 共享 `TwoPhaseState` + 需要时 `pg_twophase` state file；prepared txn "continues to hold whatever locks it held."；被排除操作有 temp table / `WITH HOLD` cursor / `LISTEN`/`NOTIFY`。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 持锁直到最终 commit/rollback（Caution 原文），仍在 `ProcArray` 中。
- **(b) 恢复发起方**：`[OBSERVED]` external TM + 任意 session + 启动期 recovery code，均在原 session 之外。
- **(c) 审计投影**：`[OBSERVED]` `pg_prepared_xacts`——"the internal transaction manager data structures are momentarily locked, and a copy is made for the view"。
- **(d) 上游超时**：`[OBSERVED]` PREPARE 后原 session 已无 active transaction、效果不可见，无 timeout→failure 契约。

`[INFERENCE]` prepare 状态确实泄漏到边界外，是为跨 session/跨 crash 的原子 prepare/commit 服务，但 PostgreSQL 不持有 external TM 的全局决策日志。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` "It is unwise to leave transactions in the prepared state for a long time. This will interfere with the ability of VACUUM to reclaim storage, and in extreme cases could cause the database to shut down to prevent transaction ID wraparound ..."；未配置 external TM 时建议 `max_prepared_transactions = 0`；freelist 耗尽报 "maximum number of prepared transactions reached"。未发现 heuristic 分支。

## 案例 2：Oracle in-doubt distributed transaction（分类①原子提交）

**① 系统/状态名/边界。** `[OBSERVED]` 直接命名 `in-doubt`："After the nodes are prepared, the distributed transaction is said to be **in-doubt**"；一般定义 "A transaction becomes in-doubt if the two-phase commit mechanism fails"（server crash / network disconnect / unhandled software error）。最清晰的 in-doubt 实例是 commit point site 已本地 commit 而其他节点仍 `PREPARED`（Distributed Transactions Concepts §34.3–§34.4）。

**② 类型/状态定义。** `[OBSERVED]` `DBA_2PC_PENDING` "describes distributed transactions awaiting recovery"，字段含 `LOCAL_TRAN_ID`/`GLOBAL_TRAN_ID`/`STATE`/`MIXED`/`ADVICE`/`TRAN_COMMENT`/`FAIL_TIME`/`FORCE_TIME`/`RETRY_TIME`/`COMMIT#` 等；`STATE ∈ {collecting, prepared, committed, forced commit, forced rollback}`（19c Reference；9i 历史术语为 `forced abort (rollback)`）。配套 `DBA_2PC_NEIGHBORS`（`IN_OUT`/`INTERFACE`/`BRANCH`）。

**③ 收敛协议。**
- *resolver/发起方*：`[OBSERVED]` 自动 = `RECO` background process（"exponentially growing time intervals"，恢复连接后自动 resolve 并删除已 resolve 行）；手工 = DBA 的 `COMMIT FORCE`/`ROLLBACK FORCE`（可用 `LOCAL_TRAN_ID` 或 `GLOBAL_TRAN_ID`）。
- *authoritative evidence owner*：`[OBSERVED]` commit point site 的 redo commit record——"the transaction is considered committed even though some participating nodes may still be only in the prepared state"。
- *evidence channel*：`[OBSERVED]` RECO 重连原 2PC 通道 / 查 `DBA_2PC_PENDING`+`DBA_2PC_NEIGHBORS` / 人工 FORCE。无 webhook/drop-copy。
- *timeout*：`[OBSERVED]` `RETRY_TIME` 只记录上次 RECO 尝试；官方要求 "always allow the automatic recovery features"，只有 extended outage 才 override。`[INFERENCE]` timeout/重试只触发下一次查询或人工升级，非失败终态。
- *stable identity*：`[OBSERVED]` `GLOBAL_TRAN_ID` 跨节点相同、`LOCAL_TRAN_ID` 本地 `n.n.n`；无 ID 生成算法/保留天数。
- *crash window*：`[OBSERVED]` prepare 时写 redo + 加 distributed lock；`FAIL_TIME`=row inserted 的 `SYSDATE`。`[INFERENCE]` unknown 窗口从 prepared/pending 持久建立到 commit point site 决定可重取并传播之间。
- *终态*：`[OBSERVED]` 正常 = 全 commit 或全 rollback，RECO 删除已 resolve 行；手工 = `FORCED COMMIT`/`FORCED ROLLBACK`，不一致则 `MIXED=YES` 留存需 `PURGE_MIXED`；remote db 重建须 `PURGE_LOST_DB_ENTRY`。

**④ prepare 作用范围与写边界外泄漏。** `[OBSERVED]` prepare 以 pending transaction table/`DBA_2PC_PENDING` 可见并由 redo 支撑；commit point site "never enters the prepared state" 且先 commit，read-only 节点回 read-only message 不参加 commit phase。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` in-doubt 期 "the data is locked for both reads and writes"，本地查询可 indefinitely blocked 返回 `ORA-01591`。
- **(b) 恢复发起方**：`[OBSERVED]` RECO 独立 daemon + DBA 均在原 session 之外。
- **(c) 审计投影**：`[OBSERVED]` `DBA_2PC_PENDING`/`DBA_2PC_NEIGHBORS` 系统视图。
- **(d) 上游超时**：`[OBSERVED]` application 直接收到 `ORA-02050`（rolled back, some remote in-doubt）/`ORA-02053`（committed, some remote in-doubt）/`ORA-02054`（in-doubt），官方要求 robust application 保存 transaction information。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` 手工 force "can generate consistency problems"，"A wrong decision can lead to database inconsistencies that can be difficult to trace and that you must manually correct"；不一致标记 `MIXED=YES`（"part of the transaction was committed on one node and rolled back on another node"），需 `PURGE_MIXED`；remote db 重建得新 database ID 时须 `PURGE_LOST_DB_ENTRY`。

## 案例 3：MySQL 8.0 XA（分类①原子提交）

**① 系统/状态/边界。** `[OBSERVED]` external XA 中 "a MySQL server acts as a Resource Manager and client programs act as Transaction Managers"，仅 `InnoDB` 支持。状态机 `XA START→ACTIVE`、`XA END→IDLE`、`XA PREPARE→PREPARED`；"an XA transaction in `PREPARED` state is now persistent until an explicit `XA COMMIT` or `XA ROLLBACK` statement is issued"。`[INFERENCE]` 页面未直呼 in-doubt，但按口径 `PREPARED` branch + TM/网络第二阶段前后失联即 in-doubt。

**② 类型/身份。** `[OBSERVED]` `xid: gtrid [, bqual [, formatID ]]`（各 ≤ 64 bytes，`formatID` 默认 1）；XID 通常由 TM 生成、须彼此不同，同一 global transaction 的 branches 共享 `gtrid`、`bqual` 须不同（后者是 MySQL limitation 非 XA spec）。`XA RECOVER` "lists all XA transactions that are in the `PREPARED` state"，输出 `formatID, gtrid_length, bqual_length, data`，需 `XA_RECOVER_ADMIN` 看他人 XID。`[INFERENCE]` `XA RECOVER` 是 prepared-branch inventory/recovery **读取接口**，不是 prepare 第二阶段，也不返回 commit/rollback 事实。

**③ 收敛协议。**
- *resolver/发起方*：`[OBSERVED]` external XA 中 client program/TM 负责第二阶段；**无** Oracle RECO 式自动 resolver，恢复入口是 TM 用 `XA RECOVER` 找 branch 再 `XA COMMIT`/`XA ROLLBACK`。
- *authoritative evidence owner*：`[INFERENCE]` 全局决定归 external TM；MySQL server 只拥有自身 RM branch 的 `PREPARED` 状态与执行结果；来源未规定 TM durable decision log。
- *evidence channel*：`[OBSERVED]` `XA RECOVER` 读取；显式 `XA COMMIT`/`XA ROLLBACK`；binlog 分两段（`XA_prepare_log_event` + 第二 GTID，可交错），但历史一致性受限不可单独当权威。
- *timeout*：`[OBSERVED]` prepared "persistent until an explicit `XA COMMIT` or `XA ROLLBACK`"，无超时自动判定。
- *stable identity*：`[OBSERVED]` `gtrid`/`bqual`/`formatID`；`XA START` 若 XID 已用会失败；PREPARED 保留到显式终结后不再被 `XA RECOVER` 列出；无历史保留天数。
- *crash window*：`[OBSERVED]` 8.0.30 起 `XA PREPARE` 改为 two-phase 以维护 storage engine/binlog 顺序；`[INFERENCE]` unknown 起点是 `PREPARED` 持久化后、TM 第二阶段结果未被确认的窗口。
- *终态*：`[OBSERVED]` `XA COMMIT`/`XA ROLLBACK` 终止 PREPARED；`XA COMMIT ... ONE PHASE` 直接 prepare+commit。未定义 Oracle `MIXED`/`FORCE`/server-side manual override。

**④ prepare 作用范围与泄漏。** `[OBSERVED]` `PREPARED` 记录在 server/storage-engine 与 binlog 两部分协作下持久；8.0.29+ `xa_detach_on_prepare=ON` 时脱离 session 可另 connection 终结。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 页面未量化锁集合，不外推。
- **(b) 恢复发起方**：`[OBSERVED]` TM/`XA_RECOVER_ADMIN` 恢复方在 server 外；另一 connection 可 detached 终结。
- **(c) 审计投影**：`[OBSERVED]` `XA RECOVER` server-wide；replica `events_transactions_current` 可能 stale `PREPARED`——官方要求 "use `XA RECOVER` rather than the Performance Schema transaction tables"。
- **(d) 上游超时**：`[OBSERVED]` 未定义 timeout→业务结论或自动 rollback；`[INFERENCE]` 调用方须持 XID 走 `XA RECOVER`/TM evidence。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` 8.0.30 前 `XA PREPARE`/`COMMIT`/`ROLLBACK` 中 unexpected halt 会使 server 与 binary log 不一致（"contain extra XA transactions not applied" 或 "miss XA transactions that are applied"）；同 XID sequential reuse + `ONE PHASE` 中断 "This is a known issue"；XA 与 replication/binlog filter "is not supported"，empty XA 会停 replica。

## 案例 4：Stripe PaymentIntent + Idempotency-Key（分类③业务状态机 + ④收敛）

**① 系统/状态/边界。** `[OBSERVED]` "The API supports idempotency for safely retrying requests"；`processing` = "required actions are complete and the payment uses an asynchronous payment method"。`[INFERENCE]` Stripe **无** `in_doubt` status——真正的 in-doubt 是客户端发出 `POST` 后、收到响应或写稳本地记录前遇连接错误那一刻；`processing` 多数是"已被 Stripe 接受的异步处理"，不是 in-doubt。IETF draft §1 同一边界："the client sent a POST request to the server, but the request timed out ... does not know if the resource was created or updated"。

**② 类型/状态。** `[OBSERVED]` PaymentIntent `status ∈ {requires_payment_method, requires_confirmation, requires_action, processing, succeeded, requires_capture, canceled}`；`canceled` 不可撤销；confirm 有 "variable upper limit"，超限转 `canceled`。in-doubt 类型不是枚举值，而是 transport/本地证据层的 `unknown` 条件。

**③ 收敛协议（分类④）。**
- *resolver/发起方*：`[INFERENCE]` 商户侧服务/对账 worker。
- *authoritative evidence owner*：`[OBSERVED]` Stripe——"Use the API or Workbench Inspector to get the authoritative state"；BalanceTransactions 是 immutable ledger。
- *evidence channel*：`[OBSERVED]` (i) 同 key 重放——"the resulting status code and body of the first request ... regardless of whether it succeeds or fails"，"including 500 errors"；(ii) `retrievePaymentIntent` 查询；(iii) webhook（不保证顺序、可重复、须按 `event IDs` 去重、live 自动重试最多三天）；(iv) BalanceTransactions/Reporting（`payout.reconciliation_completed`）。
- *timeout*：`[OBSERVED]` connection error 后 "safely repeat the request"；polling "much less reliable and might trigger rate limits"。`[INFERENCE]` timeout 只触发重放/查询/等待，不判失败。
- *stable identity*：`[OBSERVED]` client 生成（V4 UUID，≤255），"You can remove keys from the system automatically after they're at least 24 hours old. We generate a new request if a key is reused after the original is pruned."，参数不一致报错。
- *crash window*：`[OBSERVED]` "We save results only after the execution of an endpoint begins."——validation/并发冲突未启动 endpoint 时不保存幂等结果，可重试。
- *终态*：`[OBSERVED]` `succeeded`（资金在账户）；`canceled`（不可撤销）；`requires_payment_method`（可换 method 重试）；`requires_action`/`processing`/`requires_capture` 仍需动作/异步/capture。

**④ prepare 作用范围与泄漏。** `[INFERENCE]` **无** prepare 记录；`requires_capture` 是授权后延迟 capture 的业务状态，不是 2PC prepare；授权 hold "place a hold ... to reserve funds" 保留的是支付方式资金，非跨库 participant lock。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 并发同 key 冲突在写边界内（draft 建议 `409`/`422`）。
- **(b) 恢复发起方**：`[OBSERVED]` webhook consumer / report worker 在原请求外发起对账。
- **(c) 审计投影**：`[OBSERVED]` PaymentIntent `status` / webhook / BalanceTransactions 是边界外投影。
- **(d) 上游超时**：`[OBSERVED]` 上游直接看到 network error/timeout 而非 `in_doubt` 字段，靠同 key 重放收敛。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` 不要跨请求复用 key；key 至少 24h 后可 prune，prune 后复用生成 new request；首次 500 也会被重放（同 key retry 不自动修复）；3DS `Failure`→`requires_payment_method`。未发现网络层 in-doubt 的 heuristic/manual 终态。

## 案例 5：Binance Spot order + FIX（分类④查询/重放收敛）

**① 系统/状态/边界。** `[OBSERVED]` `POST /api/v3/order`（Data Source `Matching Engine`）；in-doubt 名为 client-side `-1007 TIMEOUT`——"Timeout waiting for response from backend server. Send status unknown; execution status unknown."，且 "This does not always mean that the request failed in the Matching Engine"；WebSocket 5xx："the request might have actually succeeded. Please use query methods to confirm the status"。

**② 类型/状态。** `[OBSERVED]` 响应级 unknown 不是 `status` 枚举；订单 `status ∈ {NEW, PENDING_NEW, PARTIALLY_FILLED, FILLED, CANCELED, PENDING_CANCEL(Currently unused), REJECTED, EXPIRED, EXPIRED_IN_MATCH}`。`GET /api/v3/order`（Data Source `Memory => Database`）用 `origClientOrderId` 回读。

**③ 收敛协议（分类④）。**
- *resolver/发起方*：`[OBSERVED]` 官方指令 "please perform an API query for its status"；`[INFERENCE]` 发起方是 `POST` 的客户端/后续 reconciliation worker。
- *authoritative evidence owner*：`[INFERENCE]` Matching Engine / order record（查询路径落到 `Memory => Database`）；公共文档未用 "authoritative" 也未保证无延迟。
- *evidence channel*：`[OBSERVED]` 状态查询 `GET`（主）+ User Data Stream `executionReport`（`c`/`x`/`X`/`i`，"pushed in real-time"，独立于同步响应）；**原 `POST` 重放不是有来源支持的收敛通道**（官方只给 query）；无 drop-copy/heuristic。
- *timeout*：`[OBSERVED]` API 处理 10 秒，超时返回 `-1007` unknown 文本；`[INFERENCE]` timeout 触发取证而非失败。
- *stable identity*：`[OBSERVED]` `newClientOrderId`（`STRING`，"A unique id among open orders"；REST："accepted only when the previous one is filled"，WebSocket："filled or expired"；冲突 = "Duplicate order sent" / "clOrdId is already in use"）；**无数值保留期**。
- *crash window*：`[INFERENCE]` 最危险窗口是 `POST` 已可能离开进程但 client id 未持久化——丢掉自选 id 后无法从公开契约查回订单。
- *终态*：`[OBSERVED]` `FILLED`/`CANCELED`/`REJECTED`/`EXPIRED` 是可回读终态；`NEW`/`PARTIALLY_FILLED` 仍可能变化；`PENDING_CANCEL` `Currently unused`。查询无结果时只能诚实保持 unresolved，"超时"不得收敛成失败。

**④ prepare 作用范围与泄漏。** `[INFERENCE]` **不暴露** prepare/commit；写入口只泄漏 unknown/id/status。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 同 `newClientOrderId` 在 open-order 冲突/reject（非可见 prepare lock）。
- **(b) 恢复发起方**：`[OBSERVED]` resolver 是写边界外客户端；`POST` 不带客户可见 prepare/recovery handle。
- **(c) 审计投影**：`[OBSERVED]` User Data Stream 可做本地投影（未承诺完整/不丢失）。
- **(d) 上游超时**：`[OBSERVED]` 上游看到 `-1007`/5xx unknown，不见中间 prepare。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` timeout/5xx 可能已成功不得判失败；API 异步 "some delay in the response is normal and expected"；部分 historical order `cummulativeQuoteQty < 0` "data is not available at this time"；client id 只约束 open orders。未发现 heuristic 或"查询空即失败"原话；本次未找到逐字 "rather than resubmit"。

**FIX 支撑材料。** `[OBSERVED]` 官方 Session Layer 明确分两通道：**session recovery**（`MsgSeqNum(34)`/`ResendRequest(2)`/`SequenceReset(4)`/`PossDupFlag(43)`/`PossResend(97)`，修消息流缺口与重复投递）与 **business-state recovery**（`Order Status Request(H)` → `Execution Report(8)` 带 `ExecType(150)=I` + `OrdStatus(39)`，及 `Order Mass Status Request(AF)`）。原文："the FIX session layer does not initiate a resend of an application message nor does the FIX session layer detect duplication application messages"——session 重同步**不**自动恢复订单业务状态。possible duplicate 用同一 seqnum + `PossDupFlag=Y`；application 级重发消耗**新** seqnum + `PossResend=Y`，可用 "globally unique persistent identifier" 强制幂等。官方警告不慎 reset 可致 "order overfills"。Nasdaq FIX DROP 是与 order-entry 分开的独立 outbound 报告流（"does not provide the ability to enter orders"），但未承诺完整/无限回放。

## 案例 6：Temporal Activity（分类④查询/重放收敛）

**① 系统/状态/边界。** `[OBSERVED]` "Temporal guarantees that an Activity Task either runs or timeouts"；idempotency 说明给出 crash window：Worker 完成 Activity function 后 "crashes just before it notifies the Temporal Service" → "the Event History won't reflect the successful completion of the Task, so the Activity will be retried"。retry-enabled 下 "will be observed as completed exactly once" 但 "the Activity may be executed multiple times and may even partially complete more than once"。`[INFERENCE]` in-doubt 不是命名状态，是"外部调用已可能成功、完成报告尚未进 Event History"的区间。

**② 类型/状态。** `[OBSERVED]` server `ActivityExecutionStatus ∈ {SCHEDULED, STARTED, COMPLETED, FAILED, CANCELED, TERMINATED, TIMED_OUT, PAUSED, ...}`——**无** `UNKNOWN`/`IN_DOUBT`（`activity_state.pb.go:35-80`, pin `8e653fa74ae70cf21208aef33d5ef0b02d72ff14`）。持久对象 `ActivityState`（4 类 timeout + `RetryPolicy` + `Status`）、`ActivityAttemptState`（`Count`/`StartRequestId`）、`ActivityHeartbeatState`（`Details`/`TotalHeartbeatCount`）。完成持久点 = `AddActivityTaskCompletedEvent`（`respondactivitytaskcompleted/api.go:50-115`）。

**③ 收敛协议（分类④）。**
- *resolver/发起方*：`[OBSERVED]` Temporal Service 的 timeout/retry 编排自动把新 Activity Task 放回 Task Queue。
- *authoritative evidence owner*：`[INFERENCE]` 被 Activity 调用的外部服务；Temporal 不掌握外部写是否成功。
- *evidence channel*：`[OBSERVED]` 外部三通道 "external system completes ... via Async Completion" / 完成后外部 Signal / subsequent Activity "polls the external system"；Heartbeat 只传进度/存活/取消，不作外部提交证明。
- *timeout*：`[OBSERVED]` 四类——`Schedule-To-Start`（"does not trigger any retries"）、`Start-To-Close`、`Schedule-To-Close`、`Heartbeat`；`[INFERENCE]` 只使编排进 retry/失败/超时路径，非外部结果判定。
- *stable identity*：`[OBSERVED]` 官方建议 `Workflow Run ID + Activity ID` 作外部幂等键，且 "idempotency keys ... are enforced by the service you are calling from your Activity, not by the Activity itself"；`Task Token` 是单次 attempt 标识、retry 后可失效；无外部 TTL 承诺。
- *crash window*：`[OBSERVED]` 外部写成功、Worker 通知前崩溃 → Event History 无 completion → retry；heartbeat progress 只在送达 Service 后才给下一 attempt。
- *终态*：`[OBSERVED]` 成功 / `FAILED` / `TIMED_OUT` / `CANCELED` / `TERMINATED`；无 heuristic/manual outcome。

**④ prepare 作用范围与泄漏。** `[INFERENCE]` 属分类④，**无** prepare participant；Temporal 持有编排状态与恢复责任但**不掌握外部写是否成功**——正是"编排层不应假装拥有外部 prepare"的直接支撑。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 是任务编排等待（result 到 close 才交回），非外部资源锁。
- **(b) 恢复发起方**：`[OBSERVED]` Temporal Service 重派 + 外部 poll/Signal/Async Completion；恢复责任分裂在编排层与外部权威服务。
- **(c) 审计投影**：`[OBSERVED]` `Visibility`/`LastHeartbeat`，但 "caller ... cannot directly read heartbeat progress"。
- **(d) 上游超时**：`[OBSERVED]` 上游看到完成/exhausted-retry/timeout error，无 `IN_DOUBT` result。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` Activity 必须 idempotent，否则 Worker 完成后崩溃会致 "duplicate charges in a payment processing scenario"；强烈建议设 `Start-To-Close Timeout`（Server 不直接检测 Worker crash）；heartbeat 可被 throttled 致 cancellation 延迟、progress 在 Worker crash 前丢失；不要在 Activity 内手写 retry。无 heuristic/manual outcome。

## 案例 7：Apache Kafka（KIP-98，分类①原子提交，边界受限）

**① 系统/状态/边界。** `[OBSERVED]` 事务范围 = 跨 `TopicPartitions` + consumer offsets 的原子写（"either all messages are committed, or none of them are"），但 "the transactional guarantees mentioned here are from the point of view of the producer"。`beginTransaction()` 后 "the transaction won't begin from the coordinator's perspective until the first record is sent"。`[INFERENCE]` in-doubt = `EndTxn`/commit RPC 响应丢失或 producer crash 落在 data write 与 final coordinator record 之间。

**② 类型/状态。** `[OBSERVED]` RPC 签名 `InitPidRequest => TransactionalId TransactionTimeoutMs`；`EndTxnRequest => TransactionalId PID Epoch Command`（`Command`: `false(0)=ABORT`, `true(1)=COMMIT`）。coordinator 在 `EndTxn` 后依次写 `PREPARE_COMMIT`/`PREPARE_ABORT` → 各 partition `COMMIT(PID)`/`ABORT(PID)` marker → final `COMMITTED`/`ABORTED`。`Transaction Log` 是 "a persistent and replicated record of every transaction" + coordinator state store。

**③ 收敛协议。**
- *resolver/发起方*：`[OBSERVED]` 同 `transactional.id` 的 replacement producer 经 `initTransactions()`/`InitPidRequest`（bump epoch、fence zombie、"Recovers (rolls forward or rolls back) any transaction left incomplete"，synchronous）。
- *authoritative evidence owner*：`[INFERENCE]` transaction coordinator 与其 replicated transaction log。
- *evidence channel*：`[OBSERVED]` coordinator log 的 `PREPARE_*`/`COMMITTED`/`ABORTED` / control marker / `read_committed` consumer / 同 id recovery / transaction log 对有 Read permission 者 debug 开放。
- *timeout*：`[OBSERVED]` `transaction.timeout.ms` 到时 coordinator "proactively abort the ongoing transaction"（Kafka 内部 abort，非外部判定）。
- *stable identity*：`[OBSERVED]` application 提供的 `transactional.id`（跨 session generation、"Exactly one active producer with a given TransactionalId"、epoch fencing）；`transactional.id.timeout.ms` 默认 `604800000 (7 days)`——**有明确过期**。
- *crash window*：`[OBSERVED]` `EndTxn` 的 `PREPARE_*` → marker → final 序列留下可恢复中间窗口；`[INFERENCE]` producer crash/response loss 落此窗口即 in-doubt。
- *终态*：`[OBSERVED]` `COMMITTED`（对 downstream 可见）或 `ABORTED`（downstream discard）；无 heuristic/manual reconciliation state。

**④ prepare 作用范围（严格限 Kafka）。** `[OBSERVED]` participant = Kafka logs/offsets（`transaction.state.log.replication.factor=3` 等），`PREPARE_*` 在 coordinator log，marker 发往每个 partition leader，offset participant 是 `__consumer-offsets`。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 同 id 单 active producer + epoch fencing；`read_committed` 必须等 marker 才交付；endless transaction 阻止 LSO 前进。
- **(b) 恢复发起方**：`[OBSERVED]` broker coordinator + replacement producer（Kafka 控制面，非 DBA/外部 TM）。
- **(c) 审计投影**：`[OBSERVED]` transaction log debug read + LSO/PID/active transactionalIds metrics。
- **(d) 上游超时**：`[OBSERVED]` `ProducerFencedException`/`InvalidProducerEpoch`/timeout。

`[INFERENCE]` **EOS 不能写成"Kafka 之外的支付/DB/券商副作用 exactly once"**——原子对象与 coordinator/log/marker 全在 Kafka 内。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` consumer 侧 guarantee "a bit weaker"（compaction 覆盖、log segment 删除丢前半、seek 到中间、不读全部 partition）；"endless transaction attack"；恶意 producer 可 hijack PID；初始化 "some ... failures are irrecoverable and will require a new producer instance"。无 heuristic outcome。

## 案例 8：Apache Seata TCC（分类②Try/Confirm/Cancel）

**① 系统/状态/边界。** `[OBSERVED]` TCC 侵入 service layer、独立于底层 database，`Try`=检查并保留资源、`Confirm`=执行实际业务、`Cancel`=取消预留恢复初始态；"The try phase must involve committing local transactions."。`BranchStatus.PhaseOne_Timeout(4)` 原文仅 "Branch logic is NOT reported for a timeout."——`[INFERENCE]` 只说明超时未报告，不证明 Try 无副作用。**无** `InDoubt` 枚举。

**② 类型/状态定义。** `[OBSERVED]` 参与者 `tcc_fence_log`（`PRIMARY KEY (xid, branch_id)`，`status TINYINT`），`CommonFenceConstant`: `STATUS_TRIED(1)`/`STATUS_COMMITTED(2)`/`STATUS_ROLLBACKED(3)`/`STATUS_SUSPENDED(4)`（pin `41694e140b40894aa3aae4cfe6ecffed9b321a36`）。TC `BranchStatus` 16 值（`Registered(1)`/`PhaseOne_Timeout(4)`/`PhaseTwo_Committed(5)`/`PhaseTwo_CommitFailed_Retryable(6)`/`.._Unretryable(7)`/`STOP_RETRY(14)` 等）；`GlobalStatus` 含 `Committing`/`CommitRetrying`/`CommitRetryTimeout(16)` 等，`UnKnown(0)`="an ambiguous transaction state, usually use before begin"。TC 持久化在 `global_table`/`branch_table`。

**③ 收敛协议。**
- *resolver/发起方*：`[OBSERVED]` TM 发全局 commit/rollback；phase-two resolver 是 TC 的 `DefaultCoordinator` retry loop（`handleRetryCommitting`/`handleRetryRollbacking`），非原始业务线程。
- *authoritative evidence owner*：`[OBSERVED]` TC `global_table`/`branch_table`（协调权威）+ 参与者本地 fence row + local transaction（本地权威）。
- *evidence channel*：`[OBSERVED]` 同 branch phase-two 安全重放（`commitFence` 对已 `STATUS_COMMITTED` 记 "idempotency rejected" 返回 true）/ `getStatus(xid)` Seata RPC / 本地 `select ... for update`；无 webhook/drop-copy 作外部权威。
- *timeout*：`[OBSERVED]` 官方 "needs to retry the operation"；默认 `MAX_COMMIT_RETRY_TIMEOUT=-1L` 不自动终止。
- *stable identity*：`[OBSERVED]` TC 生成 `(xid, branchId)`（`UUIDGenerator`），`@TwoPhaseBusinessAction.name` 须唯一（重复报 `RepeatRegistrationException`）；保留期 `cleanPeriod` 默认 1 天、示例 1h，终态 row 会被清理。
- *crash window*：`[OBSERVED]` fence row 与 RM business operation 在同一 `TransactionTemplate`，"succeed or fail together"；commit 后 response 前是 in-doubt 窗口，靠 `STATUS_COMMITTED`/`STATUS_ROLLBACKED` 抑制重复 callback。
- *终态*：`[OBSERVED]` `PhaseTwo_Committed`/`PhaseTwo_Rollbacked`；`STOP_RETRY` 或 retry 超时转 `CommitRetryTimeout`/`RollbackRetryTimeout` 记 "need to be handled it manually."

**④ prepare 作用范围与泄漏。** `[OBSERVED]` prepare = 业务 `Try` 的 reserve（"not true lock ... without the need for blocking and waiting"），`for update` 只锁本地 fence 行。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` 仅同 fence row 被行锁串行化；业务资源是否阻塞取决于 Try/Confirm/Cancel 代码。
- **(b) 恢复发起方**：`[OBSERVED]` 正常模式 = 写边界外 TC scheduler；same-database 优化下 RM 异步线程查 TC global state。
- **(c) 审计投影**：`[OBSERVED]` `global_table`/`branch_table` 泄漏到 console/metrics（`queryByXid`）。
- **(d) 上游超时**：`[OBSERVED]` 上游看到 `GlobalStatus` 或 TM `"RPC timeout"`，**不**直接见参与者 `STATUS_TRIED`/`STATUS_SUSPENDED`。

`[INFERENCE]` callback 若在 local transaction 外另写 HTTP/远程 DB/设备，**不被** `tcc_fence_log` 原子包住——这正是写边界外仍可能 in-doubt 处。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` TCC 三大问题须业务自处理：空回滚（empty rollback）、悬挂（hanging）、幂等；`STOP_RETRY(14)`="user operate to stop retry"，retry 超时置 `CommitRetryTimeout`/`RollbackRetryTimeout` 并记 "need to be handled it manually."

**Saga/Helland 支撑材料。** `[OBSERVED]` **SAGAS**：补偿 `Ci` "undoes, from a semantic point of view ... but does not necessarily return the database to the state that existed"——**补偿≠回滚**；"sagas may view the partial results of other sagas"，补偿 "no effort is made to notify or abort transactions that might have seen the results"；补偿失败则 "the system is stuck ... manual intervention"；不可补偿例 "if a transaction fires a missile, it may not be possible to undo this action"。**Helland**："Atomic transactions cannot span entities"，跨实体 "must accept uncertainty"，"The uncertainty of the outcome is held in the business semantics rather than in the record lock. This is simply workflow."；"Every tentative operation eventually confirms or cancels."；**Idempotence**："After you send but before you receive an answer. This is the point of confusion. You have absolutely no idea if the other guy has done anything."；"The first message sent to a service must be idempotent"；ACK "says nothing about the actual delivery ... Service A must not act on the ACK."

## 案例 9：MongoDB `UnknownTransactionCommitResult`（分类①原子提交；与 §8 最直接同名先例）

**① 系统/状态/边界。** `[OBSERVED]` MongoDB replica set / sharded 上的 distributed transaction 原子（"Transactions either apply all data changes or roll back the changes."）。`commitTransaction` 失败或带 `writeConcernError` 时 session 仍进 `"transaction committed"` state（"even" 返回 error），再调用须 "re-run the previous `commitTransaction`"。`UnknownTransactionCommitResult` 近似含义 "We don't know if your commit has satisfied the provided write concern."；commit 重试仍失败时 "both the driver and the application do not know the state of the transaction"（Transactions Spec, pin `908c58e48788ff34e92a3bea38dcac79ec7ddeea`）。`[INFERENCE]` 写边界 = `commitTransaction` 网络调用与 driver 本地返回/状态转换交界。

**② 类型/四态辨析。** `[OBSERVED]` 两个一等 error label：`TransientTransactionError`（非 commit 命令遇 network/server-selection error → "the transaction as a whole can be retried"）与 `UnknownTransactionCommitResult`（commit 遇 server-selection/network/retryable/`MaxTimeMSExpired`/write-concern failed|timeout → "can safely call `commitTransaction` again"）。四态辨析：**retry whole transaction**（前者恢复动作）vs **retry commit only**（后者恢复动作）——这正是 §8 `Undeterminable` 的直接类比。commit command 格式含 `lsid`/`txnNumber`/`recoveryToken`；body write 即使 `retryWrites=true` 也不 individually retryable，commit/abort 才可 retry。

**③ 收敛协议。**
- *resolver/发起方*：`[OBSERVED]` 两层——driver 对 commit retryable error **自动重试一次**（不论 `retryWrites`）；application 层显式处理两个 label（core API 不自动）。
- *authoritative evidence owner*：`[INFERENCE]` `mongod`/参与 shard + `mongos` coordinator。
- *evidence channel*：`[OBSERVED]` `commitTransaction` replay（保持同 `lsid`/`txnNumber`）；sharded `recoveryToken` 使新 mongos 恢复 outcome（新 mongos "waits for transaction outcome" 但自身不触发 commit）；**无**独立 GET/webhook/heuristic 契约。
- *timeout*：`[OBSERVED]` write concern timeout "represents ... command succeeded 但未在时限内满足 writeConcern"，driver 不自动重试（免加倍 `wtimeout`）但仍标 label；显式 retry 时 driver 须提升为 `w: majority` + `wtimeout: 10000`。
- *stable identity*：`[OBSERVED]` `lsid` + 单调 `txnNumber`（"enforce at-most-once semantics"，非 exactly-once），array command 另有 server 生成的 `stmtIds`；防重记录在 `config.transactions`，"MongoDB does not re-execute the committed writes"；session 有过期（`localLogicalSessionTimeoutMinutes`）。
- *crash window*：`[OBSERVED]` commit 的 network error **不得**标 `TransientTransactionError`（"it is not known whether the transaction committed or not"）；server-selection error 有时确知未 commit 仍统一暴露 label。
- *终态*：`[OBSERVED]` retry commit 成功 = "the transaction has committed with the provided write concern"；再次失败仍 unknown；无 heuristic success/failure。

**④ prepare 作用范围与泄漏（正面例子）。** `[OBSERVED]` 跨 shard 时内部 two-phase commit（`serverStatus.transactions.commitTypes.twoPhaseCommit`；`$currentOp.twoPhaseCommitCoordinator.state ∈ {writingParticipantList, waitingForVotes, writingDecision, waitingForDecisionAck, deletingCoordinatorDoc}`；`currentPrepared`/`totalPreparedThenCommitted`）。四接触面：
- **(a) 串行化/阻塞**：`[OBSERVED]` in-progress txn 持锁，事务外 write "wait until the transaction ends"，DDL block 到 `maxTimeMS`。
- **(b) 恢复发起方**：`[OBSERVED]` server coordinator + new mongos + driver replay（非 DBA）。
- **(c) 审计投影**：`[OBSERVED]` `serverStatus.transactions`/`$currentOp`/`config.transactions`/TXN logs。
- **(d) 上游超时（最强正面例子）**：`[OBSERVED]` error label 对 upstream driver 客户端直接可见并驱动其重试语义（`has_error_label("UnknownTransactionCommitResult")` → "tried to commit, don't know the outcome"），core API 明确不自动处理、要求 application 显式 retry。

**⑤ 已知问题（官方原文）。** `[OBSERVED]` commit retry "may again fail with a retryable error"，重试本身不是收敛；`w:1` 场景 A 已 commit 但回复丢失 → retry 以 `w:1` 得 `NoSuchTransaction`+`TransientTransactionError` → 重跑整事务 → **两次完整执行都可能永久 commit**（缓解 = `w: majority`，但初始 `w:1` 用户仍可能 rollback）；默认 driver 只重试一次；client unresponsive 超 `localLogicalSessionTimeoutMinutes` 后 write "might retry and apply again"。无 heuristic commit 规则。

## 横向对比表

| 案例 | in-doubt 状态定义 | 证据渠道 | 发起方 | 超时/重试 | 防重机制 | 两阶段作用范围 | heuristic 风险 |
|---|---|---|---|---|---|---|---|
| PostgreSQL | `prepared`（`pg_prepared_xacts` 一行；WAL+`gxact`） | SQL 查询视图 / WAL recovery / 显式 COMMIT-ROLLBACK PREPARED | external TM + 任意 session + 启动 recovery | 无 TTL，无超时判定 | 调用者 `gid`，重复报错，无保留期 | prepare 记录在 WAL+共享内存；持锁泄漏、视图可读、跨 session | 官方无 heuristic；久留伤 VACUUM/wraparound |
| Oracle | `in-doubt`（`DBA_2PC_PENDING.STATE`） | RECO 重连 / 系统视图查询 / DBA FORCE | RECO daemon（自动）+ DBA（人工） | `RETRY_TIME` 指数退避，不判失败 | `GLOBAL_TRAN_ID` 跨节点相同，无保留天数 | prepare 在 pending table+redo；持锁 `ORA-01591`、视图、`ORA-0205x` | FORCE "can generate consistency problems"，`MIXED=YES` |
| MySQL XA | `PREPARED`（`XA RECOVER` 列出） | `XA RECOVER` 读取 / 显式 XA COMMIT-ROLLBACK | client program/TM（server 外） | 无自动 resolver，无超时判定 | `xid=gtrid,bqual,formatID`，无保留期 | prepare 在 InnoDB+binlog；`XA RECOVER` server-wide、Perf Schema stale | 无 `MIXED`/FORCE；8.0.30 前 binlog 不一致 known issue |
| Stripe | transport `unknown`（非 status；`processing`≠in-doubt） | 同 key 重放 / GET retrieve / webhook / BalanceTx ledger | 商户服务/对账 worker | 只触发重放/查询，不判失败 | client `Idempotency-Key`，≥24h 后可 prune | **无 prepare**；hold 保留资金；status/webhook 边界外投影 | 无网络 in-doubt heuristic；key 复用/过期风险 |
| Binance | client-side `-1007 TIMEOUT`（execution status unknown） | GET `origClientOrderId` / User Data Stream event | 客户端/reconciliation worker | timeout 触发查询，明言可能已成功 | `newClientOrderId`（仅 open-order 唯一），无保留期 | **无 prepare**；只泄漏 unknown/id/status | 无 heuristic；复用风险 [INFERENCE] |
| Temporal | 完成未进 Event History 的区间（无命名状态） | 外部 poll / Async Completion / 外部 Signal | Temporal Service 编排（重派 Task） | 4 类 timeout；不掌握外部结果 | `Workflow Run ID + Activity ID`（外部服务执行去重） | **无外部 prepare**；编排层不拥有外部提交事实 | 无 heuristic；非幂等致 duplicate charges |
| Kafka | commit RPC 未定；coordinator `PREPARE_COMMIT` | coordinator log / marker / `read_committed` / 同 id recovery | replacement producer + broker coordinator | `transaction.timeout.ms` 内部 abort | `transactional.id`+epoch fencing，7 天过期 | 严格限 Kafka logs/offsets，不含外部副作用 | 无 heuristic；consumer guarantee weaker |
| Seata TCC | `PhaseOne_Timeout`/`STATUS_TRIED` 组合（无 `InDoubt`） | 同 branch phase-two 安全重放 / `getStatus(xid)` RPC | TC coordinator retry loop（写边界外） | "needs to retry"；默认不超时终止 | `(xid, branchId)` + fence 表，默认清理 1 天 | `Try` 业务 reserve；fence 表本地；global_table 投影 | `STOP_RETRY` + "handled manually"；空回滚/悬挂/幂等须业务处理 |
| MongoDB | `UnknownTransactionCommitResult`（一等 error label） | commit-only replay / sharded `recoveryToken` 新 mongos | driver（自动一次）+ application（显式） | write-concern timeout 标 label 不自动重试 | `lsid`+`txnNumber`（at-most-once），有 session 过期 | 跨 shard two-phase 在 coordinator；label 直达上游驱动重试 | 无 heuristic；`w:1` 可致两次永久 commit |

## 可迁移命题（案例 X 在条件 Y 下用机制 Z 解决 W；UTA 满足 Y 才可迁移）

- **命题 P1（in-doubt 建成一等类型）。** `[OBSERVED+INFERENCE]` MongoDB 在"commit 结果不可判定"条件下，用一等 error label `UnknownTransactionCommitResult` + `TransientTransactionError` 两分（Z）解决了"未知 vs 可整体重试"的混淆（W）。**UTA 满足 Y**（写结果本就要建成 sum type）**即可迁移**：把 `Undeterminable` 与"可安全整体重放"分成不同 variant，而不是一个笼统 unknown。这直接支持 §9 表 "外部写结果 = `Reserved | Sent | Undeterminable` + 证据记录，非 `Option`/字符串"。
- **命题 P2（收敛靠稳定身份 + 独立证据，超时不判失败）。** `[OBSERVED]` PostgreSQL/Oracle/MySQL/MongoDB/Kafka/Binance/Stripe 全体：在"发起方超时"条件下，用"稳定身份查权威状态 / 日志恢复 / 独立事件流 / 同身份安全重放"（Z）解决"不得盲重试"（W）；**无一系统把 timeout 当收敛终态**。UTA 的 C1/C2 已满足 Y（意图先持久化、按 (venue,操作种类) 声明证据渠道），可迁移"超时只触发取证，不产生失败或新尝试"。
- **命题 P3（防重身份必须带作用域与保留期）。** `[OBSERVED]` Kafka `transactional.id` 7 天过期、Seata fence 默认清理 1 天、Stripe key ≥24h prune、Binance 仅 open-order 唯一、PostgreSQL `gid` 无保留承诺、MongoDB session 有过期。**共同教训**：防重键都有有限作用域/保留期。UTA 迁移条件 Y = venue 能力矩阵（§1.4.1，F6）已逐 venue 声明键与保留期；**满足才可把该键当收敛依据**，IBKR 类无键者只能靠 listing+订单身份或人工（正与 C2 一致）。
- **命题 P4（补偿≠回滚，跨边界靠幂等/业务语义）。** `[OBSERVED]` Saga/Helland：跨实体在"无分布式事务"条件下，把不确定性放进业务语义 + 幂等消息（Z）解决跨边界一致性（W），且补偿是语义反向、可见中间结果不撤销。UTA 满足 Y（venue 是权威、本地记录是解释，F1）即可迁移"不宣称跨 venue 原子回滚；撤回旧信号不抹发送历史"（§8.1 已如此）。
- **命题 P5（prepare 记录物理驻留写资源）。** `[OBSERVED]` PostgreSQL/Oracle/MySQL/Kafka/MongoDB/Seata 的 prepare 记录都只存在真正拥有可提交写状态的资源管理器/协调者日志里，从不放进纯读/编排/对账服务（Temporal 明确不拥有外部 prepare）。UTA 迁移条件 Y = 只有 IO 壳（写边界）持久 `Reserved`/`Sent`/`Undeterminable`；读/派生/订阅侧不得持有 prepare 态。**这支持 §8 typestate 只到 `Reserved`、发出后才是运行期 enum。**
- **命题 P6（EOS/事务边界不外溢）。** `[OBSERVED]` Kafka 明言 EOS 只在 Kafka 内原子、不覆盖外部副作用；Temporal 明言编排层不掌握外部写是否成功。UTA 天然满足 Y（venue 异构、无跨 venue 事务），可迁移"lane 内两阶段不假装原子化 venue 之外的世界"。

## 对核心设计 §8 现有表述的修正建议

> 逐句对应 §8 原文；格式 = 原命题 → 支持/反驳案例证据 → `[INFERENCE]` 适用条件 → 建议精确表述。

**修正 1 —— "研究最大缺口：没有成熟案例直接建模'已发出、既非成功也非失败、不得盲重试'……可迁移的只有三条"以及 §8 末"不宣称'unknown 的可组合代数'——无先例"。**
- 反驳证据：`[OBSERVED]` MongoDB `UnknownTransactionCommitResult` 是一等 error label，语义几乎与 §8 `Undeterminable` 同名同义（"don't know ... tried to commit, don't know the outcome"），并区分 retry-commit-only vs retry-whole；Oracle 直接把该状态命名为 `in-doubt` 并配 `DBA_2PC_PENDING` 表结构；PostgreSQL/MySQL/Kafka/Seata 各有持久 prepare/timeout 状态。
- `[INFERENCE]` 适用条件：这些先例建模的是"提交/单次外部写结果不可判定"，与 UTA 的 venue 写高度同构；但没有一个先例给出"unknown 的可组合代数"（把多个 unknown 组合成新 unknown 的运算）。
- 建议表述：把"没有成熟案例直接建模"改为"**在本次纳入并实际打开的 9 个一手案例中，`Undeterminable` 有直接先例（尤以 MongoDB `UnknownTransactionCommitResult`、Oracle `in-doubt` 为最）；缺的只是'unknown 的可组合代数'这一更强主张**"。可迁移的不止三条（见本报告可迁移命题 P1–P6）。保留"不宣称可组合代数"，但理由改为"本次 N 个一手案例未见此运算"，不写成"行业无先例"。

**修正 2 —— "发 IO 之前用 typestate：`Reserved` 记录已持久化才允许调用投放"。**
- 支持证据：`[OBSERVED]` PostgreSQL `EndPrepare` 先 `XLogFlush` 再 `MarkAsPrepared`（持久在前）；MongoDB "We save results only after the execution of an endpoint begins"（Stripe 同）；命题 P5 全体 prepare 记录只驻留写资源。
- `[INFERENCE]` 适用条件：typestate 到 `Reserved` 是安全的；但先例显示写边界不是"任意 API boundary"，而是"第一次可能持久改变外部状态的调用 + 本地持久化"的**交界**。
- 建议表述：维持该句，补一句"**写边界特指第一次可能使 venue 持久变化的投放调用及其本地持久化交界，不是任意接口边界**"（避免把授权/校验等前置步骤误当写边界）。

**修正 3 —— "发出之后是持久的运行期 enum + 证据 gate；IO 壳写回 `Sent | Undeterminable`"。**
- 证据/风险：`[OBSERVED]` Helland 明确 ACK "says nothing about the actual delivery ... even less about any processing"、"Service A must not act on the ACK."；FIX 区分 session 送达与 application 业务确认；Kafka `beginTransaction` 本地 begin ≠ coordinator 视角开始。**没有一个成熟协议让单个"已发送"标志同时表达"字节交给 socket / 到达对端 / 对端接受 / 收到业务 ACK"这几件事。**
- `[INFERENCE]` 适用条件：`Sent` 若不精确定义，会把"字节离开进程"误当"venue 已接受"。
- 建议表述：为 `Sent` 增加精确定义或直接说明这是待定建模点——"**`Sent` 必须限定其含义（本报告证据只支持'投放调用已返回且 IO 壳据 venue 回执判定为已受理'一层；传输 ACK 不足以进入 `Sent`）；无 venue 业务回执时应停在 `Undeterminable` 而非 `Sent`**"。此点宜标为 spike，不在缺 UTA 语义证据时替它拍板。

**修正 4 —— "`Undeterminable` 的唯一后继是携带 P1 声明渠道证据的 Reconciliation 记录"。**
- 证据：`[OBSERVED]` 成熟案例的收敛终态可由**多种直接事件**得到——MongoDB commit-only replay 成功、Stripe 同 key 重放拿回原响应、Binance/Oracle/PostgreSQL 状态查询、Oracle RECO 自动 resolve、Kafka coordinator log recovery、Seata phase-two 重放、Oracle DBA heuristic FORCE。它们是不同机制，不是单一状态迁移。
- `[INFERENCE]` 适用条件："唯一后继"若理解为"唯一状态迁移"会与案例冲突；若理解为"唯一的恢复责任/处理通道（把上述机制收进一个 sum type/phase）"则成立。
- 建议表述：把"唯一后继是 Reconciliation 记录"改为"**`Undeterminable` 的唯一处理责任是对账通道；`Reconciliation` 应是一个容纳 {按键回读 / listing+订单身份 / 成交持仓对账 / 安全重放 / 人工} 的 sum type/phase，其结论 found/absent/inconclusive 才是终态**"——保留"唯一"的是恢复责任的归口，不是单一迁移边。

**修正 5 —— "恢复协议：重启时任何 `Reserved` 且无 `Sent`/`Undeterminable` 后继的尝试一律视为 `Undeterminable`"。**
- 证据两面：`[OBSERVED]` 支持面——PostgreSQL `XLogFlush` 之前"源码没有给出已持久 prepared 的保证"，即持久点与发送之间的崩溃窗口确实不可区分，保守是对的。**反例/限定面**——MongoDB 对 commit 的 server-selection error 承认 "driver knows transaction definitely not committed"（有时确知未发生）却仍统一暴露 label；PostgreSQL WAL flush 之后则确知已 prepared。即：先例在**持久化粒度足够细时能区分"确未发出"与"可能已发出"**。
- `[INFERENCE]` 适用条件：`Undeterminable` 不应仅因"重启后无后继记录"就推出；只有在"持久化粒度无法区分该尝试是否已越过发送屏障"时，保守视为 `Undeterminable` 才正确。若 UTA 记录了更细的 pre-send marker，某些无后继的 `Reserved` 是**确未发出**、可安全重发或仍为 `Reserved`。
- 建议表述："**当 `Reserved` 的持久化粒度无法区分'投放调用是否已发出'时，重启后无后继一律保守视为 `Undeterminable`；若 IO 壳另记了更细的'已发出前'标记，则据该标记把确未发出者保留为可安全发送的 `Reserved`，不误升为 `Undeterminable`**"。这修正了原句的"一律"。

**修正 6（新增，回应维护者把"两阶段只在写边界"降为工作假设）—— prepare/in-doubt 状态是否必须被写边界之外的层可见。**
- 结论：`[OBSERVED]` **本次 9 个一手案例中，凡有真 prepare/in-doubt 状态者（PostgreSQL/Oracle/MySQL/Kafka/MongoDB/Seata），该状态无一例外泄漏到写边界之外的四个接触面**；无 prepare 者（Stripe/Binance/Temporal）也把 unknown 泄漏到上游超时语义与审计投影。逐接触面出处：
  1. **串行化/阻塞**：PostgreSQL prepared txn "continues to hold whatever locks it held"；Oracle in-doubt "data is locked for both reads and writes" + `ORA-01591`；MySQL `PREPARED` 持久占用；Kafka endless transaction 阻止 LSO 前进、`read_committed` 必须等 marker；MongoDB in-progress txn 使事务外 write "wait until the transaction ends"；Seata fence 行 `for update`。
  2. **对账/恢复发起方在写边界外**：Oracle RECO daemon + DBA FORCE；PostgreSQL external TM + 启动 recovery；MySQL `XA_RECOVER_ADMIN` 恢复方；Kafka broker coordinator + replacement producer；MongoDB server coordinator + new mongos + driver replay；Seata TC scheduler；Stripe/Binance 客户端 reconciliation worker。
  3. **审计/投影/监控**：`DBA_2PC_PENDING`/`DBA_2PC_NEIGHBORS`、`pg_prepared_xacts`、`XA RECOVER`、Kafka transaction log debug read + LSO/PID metrics、MongoDB `serverStatus.transactions`/`$currentOp.twoPhaseCommitCoordinator`/`config.transactions`、Seata `global_table`/`branch_table` console、Stripe PaymentIntent `status`/webhook/BalanceTransactions。
  4. **上游调用方超时语义直接看见**：Oracle `ORA-02050`/`ORA-02053`/`ORA-02054`；**MongoDB error label 直达 driver 客户端并驱动其 retry-commit-only 语义（最强正面例子）**；Stripe network error/timeout（靠同 key 重放而非直接看 in-doubt 字段）；Binance `-1007`/5xx unknown；Kafka `ProducerFencedException`/timeout；Temporal exhausted-retry/timeout error。
- `[INFERENCE]` 适用条件与建议表述：维护者的工作假设"两阶段只应存在于写操作的边界"**只在窄义成立**——prepare **记录**物理驻留写资源（命题 P5）；但其**状态**必然对上述四层可见，因为串行化(§4 lane)、对账发起(C2)、审计投影(S8/P10)、上游超时(P9/P13) 本就要读它。因此 §8 应显式承认：**"两阶段只在写边界"= prepare 记录的归属与持久化只在写边界；`Undeterminable` 的可见性不封闭在写边界内——lane 阻塞（§4）、对账发起方（可以是写边界外的规则/人工）、帐票投影与上游会话超时都合法地读取它。** 与 §4 "阻塞什么不进键、进规则" 一致：这些接触面是把 `Undeterminable` 读出去的合法通道，不是把 prepare 逻辑复制进读/编排/对账层。

**修正 7（新增，回应"in-doubt 决议谁自动发起"，决定 UTA 的 IO 壳能否自驱对账）—— 自动守护进程取证 vs 人工，及自动化边界/停在人工的前置条件。**
- 证据（按"自动发起方 / 它能自动做到哪一步 / 何时必须停下等人工"三段读）：
  1. **Oracle（自动 daemon + 人工兜底，边界最清晰）**：`[OBSERVED]` `RECO` background process 自动发起——按 "exponentially growing time intervals" 重连其他节点、依 commit point site 的 commit record 自动 resolve 并删除 pending 行；官方要求 "always allow the automatic recovery features"。**停在人工的前置条件**：只有 extended outage、必须释放 locks/undo，或 remote db 永久丢失/重建得新 database ID（自动 recovery "cannot identify"）时才由 DBA `COMMIT FORCE`/`ROLLBACK FORCE` 或 `PURGE_LOST_DB_ENTRY`；而 heuristic FORCE "can generate consistency problems"，`MIXED=YES` 需人工先解不一致再 `PURGE_MIXED`。`[INFERENCE]` 自动化边界 = **有权威 commit record 可查时全自动；权威源不可达或已被 heuristic 污染时停在人工**。
  2. **Kafka（纯自动，但仅 Kafka 内）**：`[OBSERVED]` 恢复由 broker transaction coordinator + 同 `transactional.id` 的 replacement producer 经 `initTransactions()`/`InitPidRequest` 自动 "Recovers (rolls forward or rolls back)"，synchronous；`transaction.timeout.ms` 到时 coordinator 自动 abort。**无常规人工分支**，但 "some ... failures are irrecoverable and will require a new producer instance"。`[INFERENCE]` 自动化边界 = Kafka 拥有全部 participant（logs/offsets）故可全自动；一旦越出 Kafka（外部副作用）则不在其恢复范围。
  3. **MongoDB（driver 自动一次 + application 显式）**：`[OBSERVED]` driver 对 commit 的 retryable error **自动重试一次**（不论 `retryWrites`），sharded 靠 `recoveryToken` 让 new mongos 自动恢复 outcome；但 core API **不自动**处理 label，"application should ... explicitly retry"。**停在人工/上层的前置条件**：driver 自动上限是"one retry"，之后把 `UnknownTransactionCommitResult` 抛给 application 决定；write-concern timeout driver 主动不自动重试（免加倍 `wtimeout`）。`[INFERENCE]` 自动化边界 = **传输层一次安全重放归 driver；策略性重试/放弃归上层**。
  4. **Seata（TC 自动重试 loop + 停试转人工）**：`[OBSERVED]` phase-two 由 TC `DefaultCoordinator` 的 `handleRetryCommitting`/`handleRetryRollbacking` 定时自动重发（默认 `MAX_*_RETRY_TIMEOUT=-1L` 不自动终止）；`commitFence`/`rollbackFence` 用 fence 状态自动去重。**停在人工的前置条件**：`BranchStatus.STOP_RETRY(14)`="user operate to stop retry"，或配置了非负 retry-timeout 超时后转 `CommitRetryTimeout`/`RollbackRetryTimeout` 并记 "need to be handled it manually."；空回滚/悬挂/幂等三类须业务代码自处理。
  5. **PostgreSQL / MySQL XA（无自带自动 resolver）**：`[OBSERVED]` 二者都不提供 Oracle RECO 式后台自动 resolver——恢复入口是 **external transaction manager**（PostgreSQL "Unless you're writing a transaction manager ..."；MySQL client program/TM 用 `XA RECOVER` 读 prepared branch 再显式终结）；PostgreSQL 仅在启动 recovery 阶段自动重建 prepared 状态与锁，但不自动决定 commit/rollback。`[INFERENCE]` 自动化边界 = **只自动持久化/重建 prepare 态；最终决议一律交外部 TM（可自动可人工）**。
  6. **Stripe / Binance / Temporal（收敛发起方是外部/客户端，无 venue 自动 resolver）**：`[OBSERVED]` Stripe 由商户对账 worker、Binance 由客户端按 `origClientOrderId` 查询、Temporal 由 Service 自动重派 Task（但外部结果需 subsequent Activity poll / 外部 Signal / Async Completion）——venue/平台**不**替调用方自动对账外部写；Temporal Service 只自动重试、不掌握外部结果。
- `[INFERENCE]` 适用条件与建议表述：把上述归纳为一条谱系——**"守护进程能全自动收敛"当且仅当该系统同时拥有 in-doubt 尝试的权威结果源与防重身份（Oracle 有 commit point site record、Kafka 有 coordinator log、Seata 有 TC state、MongoDB driver 有 `lsid`+`txnNumber` 的一次安全重放）；一旦权威源在系统之外（PostgreSQL/MySQL 的 external TM、Stripe/Binance/Temporal 的外部 venue），自动化就止于'重建状态/重放一次/触发查询'，最终决议交给外部发起方或人工。** 对 UTA：F1（venue 是权威、本地是解释）意味着**权威结果源恒在 UTA 之外**，故 IO 壳的自动化边界应对齐 PostgreSQL/Temporal 一侧——**IO 壳可以自动按 P1 声明的证据渠道取证（回读键/listing/成交对账）并在渠道给出 found/absent 时自动收敛，但不得自动做 heuristic 决议**；C2 已规定"渠道 inconclusive 或无渠道 → 停在带 principal 的人工"，这正对应 Oracle/Seata 停在人工的前置条件。建议 §8 显式写明：**`Undeterminable` 的自动收敛以"证据渠道返回确定性结论"为前置；证据不确定即停在人工，IO 壳不得在缺权威证据时自行 heuristic commit/rollback**——与 C1（unknown 不得自动产生新尝试）、C12（fail-closed）一致。

## 未覆盖 / 证据缺口

- `[OBSERVED]` X/Open XA **原始规范**未直接打开（经 Oracle/MySQL 官方页转述 RM/TM/branch/XID 职责）；ACM Queue 网页版 Helland "Idempotence" 返回 403，已用 CACM 期刊 PDF 镜像原文替代。
- `[OBSERVED]` CME iLink drop copy FAQ 打开失败（HTTP/2 INTERNAL_ERROR），drop copy 独立证据流性质改用 Nasdaq FIX DROP 一手 PDF；FIXimate 旧根路径重定向到 Orchimate/FIX.Latest，已改用当前 FIX 4.4 renderer 并记录。
- **未建模**：unknown 的可组合代数（本次 9 案例均无）；`Sent` 的精确多阶段语义（各协议都不用单一标志表达）；跨外部系统的统一 exactly-once（Kafka/Temporal 明确排除）；venue 侧 client-order-id 的**数值保留期**（Binance/多数交易所官方未文档化，F6 已记）。
- **未做**：其他支付商（Adyen/Braintree 仅作卡组织三阶段支撑）、性能对比、UTA 实现方案。

## 来源清单

> 打开状态：**OK** = 原文经 curl/clone/read 实际打开并引用；**PARTIAL** = 部分打开或经镜像；**FAIL** = 未打开。仓库均给完整 commit SHA + 本地路径。逐条完整证据在对应 `local://` 研究底稿。

### 案例 1 PostgreSQL（底稿 `local://fp06-A.md`）
1. PostgreSQL 18.6 官方《PREPARE TRANSACTION》 <https://www.postgresql.org/docs/current/sql-prepare-transaction.html>（curl **OK**）。
2. 官方《pg_prepared_xacts》 <https://www.postgresql.org/docs/current/view-pg-prepared-xacts.html>（**OK**）。
3. 官方《COMMIT PREPARED》/《ROLLBACK PREPARED》 <https://www.postgresql.org/docs/current/sql-commit-prepared.html>、<https://www.postgresql.org/docs/current/sql-rollback-prepared.html>（**OK**）。
4. 源码 `github.com/postgres/postgres`，commit `862092932c9479b79732f3b441da05453ea5e06d`，`/tmp/fp06-A/postgres`：`src/backend/access/transam/twophase.c`（150-191, 391-412, 531-553, 703-797, 963-1009, 1147-1270, 1501-1693, 2074-2180, 2506-2613）、`src/include/access/xact.h:353-371`（**OK**）。

### 案例 2/3 Oracle + MySQL（底稿 `local://fp06-B.md`）
5. Oracle Database Administrator's Guide Release 19《Distributed Transactions Concepts》 <https://docs.oracle.com/en/database/oracle/oracle-database/19/admin/distributed-transactions-concepts.html>（curl **OK**）。
6. 同 Release 19《Managing Distributed Transactions》 <https://docs.oracle.com/en/database/oracle/oracle-database/19/admin/managing-distributed-transactions.html>（**OK**）。
7. Oracle Database Reference Release 19《DBA_2PC_PENDING》《DBA_2PC_NEIGHBORS》（`#REFRN23002`/`#REFRN23001`）（**OK**）。
8. Oracle9i Administrator's Guide ch.32 Table 32-2（历史术语核对）（**OK**）。
9. MySQL 8.0 Reference Manual §15.3.8 XA Transactions / §15.3.8.1-.3 <https://dev.mysql.com/doc/refman/8.0/en/xa.html> 及 `xa-statements`/`xa-states`/`xa-restrictions`（**OK**）。

### 案例 4 Stripe（底稿 `local://fp06-C.md`）
10. Stripe《Idempotent requests》 <https://docs.stripe.com/api/idempotent_requests>（fetch **OK**）。
11. Stripe《PaymentIntents lifecycle》 <https://docs.stripe.com/payments/paymentintents/lifecycle>（**OK**）。
12. Stripe《Payment status updates》《Reporting and reconciliation》《Webhooks》《Place a hold》《Balances and settlement time》《How disputes work》（**OK**）。
13. Adyen《Payments lifecycle》《Capture》《Dispute flow》（卡组织三阶段支撑，**OK**）。
14. IETF `draft-ietf-httpapi-idempotency-key-header-07`（Datatracker 状态页 + 归档纯文本 <https://www.ietf.org/archive/id/draft-ietf-httpapi-idempotency-key-header-07.txt>，**Expired Internet-Draft，非 RFC**，**OK**）。

### 案例 5 Binance + FIX（底稿 `local://fp06-D.md`）
15. Binance 官方 `binance/binance-spot-api-docs` clone，commit `b8a0f61e088c65d18a157f2e11a8e273826b6c08`，`/tmp/fp06-binance-full`：`rest-api.md`/`errors.md`/`enums.md`/`web-socket-api.md`/`user-data-stream.md`（**OK**；开发者门户 REST 入口 HTTP 202 空 body → 用 pinned GitHub 快照）。
16. FIX Protocol Ltd.《FIX Session Layer Technical Specification》June 2020（PDF→文本，**OK**）。
17. FIX 4.4 current renderer（FIXimate 兼容 Orchimate）`OrderStatusRequest(H)`/`ExecutionReport(8)`/`OrdStatus(39)`/`ExecType(150)`/`OrderMassStatusRequest(AF)`（**PARTIAL**：旧 FIXimate 根路径重定向，已记录）。
18. Nasdaq《FIX DROP RASH Format》v1.00（2024-10，**OK**）。CME Drop Copy FAQ（**FAIL**：HTTP/2 INTERNAL_ERROR）。

### 案例 6/7 Temporal + Kafka（底稿 `local://fp06-E.md`）
19. Temporal 官方 docs：`/activities`、`/activity-definition`、`/activity-execution`、`/encyclopedia/detecting-activity-failures`、`/encyclopedia/retry-policies`、`/develop/typescript/activities/timeouts`（`.md` 原文，**OK**）；`/encyclopedia/activities`（**FAIL** 404，已降级）。
20. Temporal server clone，commit `8e653fa74ae70cf21208aef33d5ef0b02d72ff14`，`/tmp/fp06-E/temporal`：`chasm/lib/activity/activity.go`、`.../gen/activitypb/v1/activity_state.pb.go`、`.../tasks.go`、`.../statemachine.go`、`service/history/api/recordactivitytaskheartbeat/api.go`、`.../respondactivitytaskcompleted/api.go`（**OK**）。
21. Apache KIP-98 <https://cwiki.apache.org/confluence/display/KAFKA/KIP-98+-+Exactly+Once+Delivery+and+Transactional+Messaging>（page version 75, Adopted）+ Confluence REST body.storage（**OK**）。

### 案例 8 Saga/Helland/Seata（底稿 `local://fp06-F.md`）
22. Garcia-Molina & Salem《SAGAS》ACM 1987（PDF <https://www.cs.cornell.edu/andru/cs711/2002fa/reading/sagas.pdf>，**OK**）。
23. Pat Helland《Life beyond Distributed Transactions》CIDR 2007（PDF，**OK**）。
24. Pat Helland《Idempotence Is Not a Medical Condition》CACM 2012-05（CACM 期刊 PDF 镜像，**OK**；queue.acm.org 版 **FAIL** 403 Cloudflare）。
25. Seata 官方 TCC 文档 <https://seata.apache.org/docs/user/mode/tcc/>、`/dev/mode/tcc-mode`、`/blog/seata-tcc-fence/`、`/blog/tcc-mode-design-principle/`（**OK**）。
26. Seata 源码 `github.com/apache/incubator-seata`，commit `41694e140b40894aa3aae4cfe6ecffed9b321a36`，`/tmp/fp06-F/seata`：`BranchStatus.java`、`GlobalStatus.java`、`CommonFenceConstant.java`、`CommonFenceDO.java`、`SpringFenceHandler.java`、`ActionInterceptorHandler.java`、`TCCResourceManager.java`、`AbstractCore.java`、`DefaultCore.java`、`DefaultCoordinator.java`、`script/server/db/mysql.sql`、`script/client/tcc/db/mysql.sql`（**OK**）。

### 案例 9 MongoDB（底稿 `local://fp06-G.md`）
27. MongoDB Manual 8.3：《Handle Transactions in Applications》《Retryable Writes》《Server Sessions》《Transactions》《Production Considerations》《$currentOp》《serverStatus》（curl **OK**）。
28. MongoDB `github.com/mongodb/specifications`，commit `908c58e48788ff34e92a3bea38dcac79ec7ddeea`，`/tmp/fp06-G/specifications`：`source/transactions/transactions.md`（322-367, 431-432, 576-580, 609-623, 700-714, 722-785, 1031-1109, 1183-1186）、`source/retryable-writes/retryable-writes.md`（14-19, 140-146, 221-312, 567-583）（**OK**）。
