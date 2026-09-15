# UTA 问题域（原 uta-design.md §1 + 附录 B）

> 来源：本文逐字迁移自已退休的 `uta-design.md` 的 §1（问题域）与附录 B（维护者原话）。依据 `uta-core-design.md` §15，这两部分继续有效，作为核心设计的域事实来源；原 uta-design.md 的 §3–§6（旧设计中心）已被 `uta-core-design.md` 取代并随该文件删除。
> 说明：下文保留原 §1 行文，其中对 §2–§7、附录 A 等原 uta-design.md 其它章节的交叉引用指向已退休文档，仅存于 git 历史；域事实证据链见同目录 `investigation/*.md` 与 `research/fp-00..fp-05`。

---

## 1. 问题域

方法：先域后机器。§1.1 把事实分四类；§1.2 列出机器与域共享的现象（带名字与方向的记录类别，不是动词）；§1.3 每条需求引用现象；§1.4 调查结论；§1.5 需求的两簇。设计义务：`机器规格 + 外部域事实 + 假设 ⇒ 需求`。

### 1.1 事实分类

**F 外部域事实**（不能设计）

|#|事实|证据|
|---|---|---|
|F1|venue 是账户、持仓、订单、成交、价格的权威来源；本地任何记录都是对它的解释|`docs/uta-live-testing.md:138` "Never trust the ledger over the venue"|
|F2|instrument 身份只在其 venue 内有意义；同一"东西"在不同 venue 是不同 instrument|`packages/uta-protocol/src/types/broker.ts:615-621`（conId / CCXT symbol / ticker）；`:600-606`（CCXT 的 AAPL 是合成代币）|
|F3|instrument 之间有 venue 给出的派生关系（期权→标的、期货月份→品种）|`broker.ts:507-513,190-223`|
|F4|instrument 会上市、退市、到期|域常识；既有目录刷新是对此的响应（O5）|
|F5|对外部写的结果可能不可判定：请求可能到达 venue 而调用方拿不到回执|gRPC/tonic 明言 deadline 错误时写可能已完成（`investigation/rust-feasibility.md:175`）；venue 侧无回音是网络固有性质|
|F6|venue 的能力、限额、幂等/回读/续传语义按 venue × 操作种类变化，且不少条目官方未文档化|`investigation/venue-capabilities.md:148-178` 矩阵与 unknown 列表|
|F7|venue 会限流；历史数据有 pacing|`ccxt/exchanges/hyperliquid.spec.ts:95-99`（429）；IBKR pacing、Longbridge 500 symbol 上限（`venue-capabilities.md` §2、§3）|
|F8|子账户 / 钱包是某些 venue 的结构|`broker.ts:535-553`|
|F9|用户可在 venue 自己的 app 里下单、出入金；我们只能事后观察|`broker.ts:578-586`|
|F10|订单从 listing 消失不等于终态（listing 可能滞后或不完整）|`UnifiedTradingAccount.ts:853-877` 二次确认；Longbridge `client_request_id` 仅 10 分钟缓存（`venue-capabilities.md:156`）|
|F11|推送流有自己的时钟；venue 事件时间与本地收到时间不同源|域常识；`src/domain/market-data/bars/types.ts:92` 明言 freshness "neither establishes measured feed latency"|

**O 既有机器事实**（现在的实现；决定"新设计不能假设什么"，不是需求）

|#|事实|证据|
|---|---|---|
|O1|六个 adapter 无一使用推送式市场数据；无一向 venue 发调用方幂等键|`brokers/` grep `WebSocket|subscribe|watch*` 无匹配；`IbkrBroker.ts:837` snapshot；`ibkr/README.md:40-45`；grep `client_order_id|clientOrderId|client_request_id|clientOid|orderLinkId` 无匹配|
|O2|写结果类型只有 `submitted|filled|rejected|cancelled|user-rejected`，无 unknown、无 principal|`packages/uta-protocol/src/types/git.ts:56-77,93-102`|
|O3|push 先调 venue，再读状态、再追加提交、再清 staging；任何异常记为 `rejected`|`TradingGit.ts:141-155,157-184`|
|O4|一个 `UTAConfig` = presetId + 凭据 + 可选子账户 + guards + keyless/readOnly/asVendor/editable/ephemeral 标志；tier 由 keyless/readOnly 推导|`src/core/config.ts:442-487`；`UnifiedTradingAccount.ts:233-243`|
|O5|目录每 6h 刷新；快照按调度捕获；event log 只记 health/snapshot，不记 ledger commit|`services/uta/src/main.ts:134-141`；`uta-manager.ts:61-76`；`src/core/event-log.ts:127-163`|
|O6|同账户写互斥是单个 `TradingGit` 实例内的布尔量；跨客户端（UI / AI / connector）无序列化定义|`TradingGit.ts:64-80,242-248`|
|O7|Alice→UTA 是 HTTP JSON、无认证；"信任边界是 host"；只读拦截靠路径字符串匹配且漏 `/wallet/commit|reject|stage-*`|`src/webui/routes/trading-proxy.ts:1-8,115-215,193-206`；`routes-trading.ts:453-535`|
|O8|改配置 / 换凭据 = Alice 写 flag → 整个 UTA 进程重启|`UTAManagerSDK.ts:174-200`；`main.ts:8-10`|
|O9|持久化：ledger 是每账户一个 pretty JSON（每次提交整写）；快照 50 条一 chunk 的 JSONL + index（`version:1`）；event log JSONL；无 retention/compaction；配置有 migration 与 `_meta.json`；两个 Alpaca legacy id 共用一条回退路径|`git-persistence.ts:12-47`；`snapshot/store.ts:1-85`；`src/core/config.ts:545-548`；`git-persistence.ts:18-39`|
|O10|消费方全部是请求/响应轮询：健康 1s、BFF 30s、connector 1.5s、UI 3–300s|`investigation/alice-consumers.md:206`|
|O11|20 条已观察缺陷，其中 E1 所有权绕过、E2 数量不校验、E3 子账户竞态、E5 unknown→rejected、E6 投放后落盘窗口、E7 成交字段丢失、E8 状态压成 rejected、E18 guard 绕过健康记账、E19 冷却先记后发、E20 重启丢 staging|`investigation/existing-capabilities.md:254-277`|

**S 利益相关者要求**

|#|要求|来源|
|---|---|---|
|S1–S7|= B1–B7（见 §1.3 R-B）|维护者原话，附录 B|
|S8|每个决定（批准 / 否决 / 自动放行）事后能回答"谁、何时、依据什么"|维护者 K8 "谁和谁能关联"；既有缺口 O2|
|S9|运维者能改审核规则、装 / 卸本机程序、换一个集成的凭据、重启一个连接、看状态|D7 角色；既有 O8 是当前实现方式|
|S10|Alice 现有消费面（附录 A 的 A01–A51）在新边界上仍可实现|`investigation/alice-consumers.md:192-207`|
|S11|只读 / 策略门在多处边缘执行，需要 UTA 暴露调用方身份与账户用途|`alice-consumers.md:204`|

**H 假设 / 威胁模型**（每条带证伪条件；被证伪则回到 §1 重分类）

|#|假设|依据|证伪 / 关闭事件|
|---|---|---|---|
|H1|对同一外部效果的重复投放不可接受（资金后果）|域常识；维护者 K8|维护者明确接受"宁重复不漏单"的账户类别|
|H2|程序（AI 写的决策代码）不可信：会死循环、吃内存、超产意图、输出非法|维护者 "ai写不好scala"、K8|—（威胁模型，不证伪）|
|H3|程序不得接触 venue 凭据；凭据 owner 是 Alice 侧的封存配置（`accounts.json` + `sealing.key`）|`src/core/config.ts:704-787` 封存；`alice-consumers.md:213`|维护者把凭据 owner 移到 UTA 或 OS secret broker|
|H4|同一 (账户, 子账户) 上的写需要一个全序；不同账户可并行|F1 + F8：venue 侧顺序只能由我们发出的顺序决定|某 venue 文档保证并发写的确定性排序（未见）|
|H5|订阅的 owner 是 UTA 而非消费方：消费方断开后订阅仍有业务价值（程序在跑）|S1/S3：程序在 UTA 内运行|维护者决定程序随消费方连接存亡|
|H6|决定有时效，过期未决 = 否决，不做补偿|connector `ttlMs`（`delivery-manager.ts:271`）、Telegram 过期（`telegram-uta.ts:196-199`）是现有唯一实例|维护者要求人类决定无过期，或过期后自动执行|
|H7|信任单位是 **OS 用户**：本机非本用户的进程不可信；本用户的进程视为用户本人（它们本就能读该用户的文件与进程环境）；同机进程能连到 UTA 不等于有权发写——写权限来自认证得到的 principal × 策略 scope，不来自连接|O7 现状无认证 + K1 独立进程使 IPC 成为正式边界；UDS 对端凭据 / 命名管道 ACL 的粒度都是用户|维护者要求同用户进程之间也互相隔离（则需能力令牌 / 管道句柄传递，X4 升级路径）；或接受"host 即信任边界"|
|H8|Windows 是必须支持的运行目标|§0 已确认|—|
|H9|程序状态需要跨 UTA 重启恢复（而非重启后从零重建）|S3 alert/决策连续性；若不恢复则每次重启丢失指标暖机与程序 state|维护者接受重启后程序冷启动 + 历史回填|
|H10|同一 `UTA_HOME` 上同时只能有一个 UTA 核心实例持有帐票与订阅表；孤儿实例（父进程死而子进程活）是真实威胁|`scripts/guardian/shared.ts:271-276`：Windows 上 kill 只到 wrapper，孤儿 UTA 继续占端口使新实例无法绑定|—（威胁模型）|

### 1.2 机器与域共享的现象

每个现象是一类**带名字、方向、语义字段**的记录。字段写语义不写类型；协议类型在 §4。`→机` = 域给机器；`机→` = 机器给域。所有记录共享一个不可变信封：记录身份、记录时间（本地墙钟 + 单调计数）、记录者 principal 或来源；下表不重复列。**观察引用** `ObservationRef = (来源, 代数, 持久序号)` 是跨现象引用观察的唯一方式；裸序号不出现在任何其他现象里。

|#|现象|方向|语义字段|被哪些需求引用|
|---|---|---|---|---|
|P1 能力声明|集成→机|集成身份、venue、账户、按 (操作种类 / 流种类) 的 supported / unsupported、配额、每种写操作的 unknown 证据渠道（完备枚举：按调用方键回读 / open-order listing + venue 订单身份（含"缺席需二次确认"的间隔） / 成交或持仓对账 / 无），历史 bar 有无与 pacing、子账户有无、续传游标有无|C2、C8、A18、A19、Q13、Q15、Q6|
|P2 观察|集成→机；程序→机（派生）|来源（账户、公共 feed、或**程序**——程序输出的指标 / alert 是派生观察，与外部观察同一形状）、主体（instrument / 账户 / 子账户 / feed）、种类（quote / book / bar / clock / 余额 / 持仓 / 订单状态 / 成交 / 目录 / 连接状态 / 健康 / 汇率 / 派生）、venue 事件时间（可无）、venue 序号（可无）、venue 续传游标（可无）、本地收到时间、**持久序号**（UTA 在"来源 × 流 × 代数"这个有序范围内赋的单调序号）、原始负载、质量标记；订单状态 / 成交观察带归因（某尝试 / 外部 / 未定）|B1–B5、A11–A17、Q1–Q3、Q16、Q18|
|P3 gap / 控制|机→程序/消费方|**来源 gap**（某范围断代：新代数首条记录，含前一范围与其最后序号、原因 ∈ {start, disconnect, quota, ingress_overflow, program_upgrade}）；**投递 gap**（某订阅对某范围的损失：gap id、from/to 序号、原因 ⊆ {slow_consumer, compacted}，需显式确认）；**程序 gap**（程序滞后被跳过的区间）；**状态通知**（readiness backfilling/live、订阅挂起 / 恢复；非损失，不需确认，由订阅状态派生）|C6、Q10、Q14、Q23、Q27|
|P4 订阅需求 / 订阅状态|消费方/程序→机；机→集成|owner、选择器（来源 × instrument × 种类）、代数、状态（待接纳 / 活 / 被拒 / 挂起）、拒绝原因、**消费游标**（该订阅已确认的最后 ObservationRef；"确认"指投递、消费方持久化还是处理完成，是 §3 必须决定并写明重复可见性的点）|B1、C5、Q9、Q13、Q15、Q23|
|P5 回填|机→集成；集成→机|范围、页 / 游标、结果、与实时流的边界、readiness|A19、Q22|
|P6 意图|程序/消费方→机|意图身份、principal、目标（账户、子账户、instrument）、操作种类（下单 / 改单 / 撤单 / 平仓）、参数、依据（ObservationRef 集合）、**生产者**（程序制品 hash + 版本 + 调用身份，或消费方会话）、过期|B7、C1、C3、A29–A35、Q5、Q7、Q17、Q19、Q25|
|P7 决定|决策者→机|所指意图、principal、裁决（批准 / 否决 / 过期 / 规则否决）、原因、依据、对"待决集合版本"的期望|S8、C3、A26–A28、Q7、Q19、Q20、Q21|
|P8 尝试|机→集成→venue|所指意图、尝试身份、venue 调用方键（能力允许时）、**阶段**（已预约未发 / 已发出 / 发出与否不可判定）及各阶段时间|C1、C2、Q5|
|P9 回执|venue→集成→机|所指尝试、venue 结果（原始 + 域词表映射）、venue 订单身份、成交明细|C1、A13、Q16|
|P10 对账|机↔集成；人→机|所指尝试、证据渠道、结论（found / absent / inconclusive）、若人工则 principal|C2、Q6|
|P11 外部变更|集成→机；机→机|账户、观察到的订单 / 成交 / 余额变动的原始记录、**归因结论**（某本地尝试 / 外部 / 未定）及其证据|F9、A48、Q18|
|P12 程序|作者→机；机→消费方|程序制品身份（hash）、版本、输入声明（instrument 集合 × 消费方式）、预算、状态版本、状态、trap、alert|B3、B5、C4、Q4、Q8、Q25|
|P13 会话|消费方/集成↔机|对端、principal、授权范围（账户 × 操作种类）、deadline|S11、H7、Q21|
|P14 控制|运维者→机|principal、配置变更、凭据轮换、重启某集成、装 / 卸程序、请求快照；结果（生效 / 拒绝 + 原因）|S9、A06–A09、Q12、Q21|
|P15 快照 / 保留 / 迁移|机→机|快照身份、**按存储类别的保留规则**（观察日志可压缩到窗口；效应帐票只追加不压缩，只做快照加速）、压缩点、格式版本、迁移结果|A36–A38、Q11、Q23、Q24|
|P16 健康 / readiness|机→消费方/运维者|按集成 / 账户：reach、tier、连续失败数、最后成功时间|A01–A02、A25、Q26|

### 1.3 需求

**R-A 既有行为**（A01–A51 逐条见附录 A；这里按语义归组并标注现象）

|组|入口|现象|
|---|---|---|
|目录与控制|A01 health、A02 账户列表、A06 test-connection、A07 reconnect、A08 sync、A09 simulate-price|P16、P14|
|instrument|A04 search、A18 expand、A20 details|P1、F2、F3|
|市场数据读|A14 clock、A15/A16 quote、A17 book、A19 historical|P2、P5|
|账户读|A03 equity、A05 fx-rates、A10 subaccounts、A11 account、A12 positions、A13 orders|P2（余额 / 持仓 / 订单状态）、P9|
|交易写|A26 commit、A27 reject、A28 push、A29–A32 stage-*、A33–A35 one-shot|P6、P7、P8、P9|
|帐本读|A21 log、A22 order-history、A23 trade-history、A24 show、A25 status|P6–P11 的投影|
|快照|A36、A37、A38|P15|
|模拟器|A39–A47|P2、P11（模拟外部存款 / 成交）、P14|
|后台|A48 poller、A49 快照调度、A50 catalog 刷新、A51 ephemeral purge|P11、P15、F4|

FX（A05）与 equity（A03）不改变结构的理由：它们是对 P2 余额观察的纯投影加一张汇率表；汇率来源是另一个"公共数据集成"（现为 hub / yfinance 回退链，`fx-service.ts:105-205`），在新设计里就是一个只读来源。搜索（A04）是对多个集成 P1/目录的扇出与合并，不产生新现象。模拟器（A39–A47）是一个集成实现。这三者留 §4。

**R-B 新增**（维护者原话见附录 B）

|#|需求|现象|
|---|---|---|
|B1|实时 K 线订阅、秒级清洗、每 instrument 大量增量指标、多核|P2、P4、P12|
|B2|多渠道多资产同时订阅，程序用全部渠道最新状态判断（正股波动 → 期权 + gamma）|P2、P12 输入声明|
|B3|TradingView 式可组合：自造指标、alert 回调、可编程决策；不接受每 bar 全量重跑|P12|
|B4|衍生品是有自己实时数据的 instrument，标的是它的输入；跨 instrument 观察是常态|P2、F3、P12 输入声明|
|B5|不对齐时间；消费有序与否由程序声明（await-all / latest+窗口 / ordered）|P2 序号与收到时间、P12 输入声明|
|B6|核心不处理协议；集成层洗完送内部协议|P1、P2、P8、P9 的形状|
|B7|效应：副作用怎么消费、谁和谁能关联、抽象怎么做|P6 依据字段、P7、P8、P13 授权范围|

**R-C 推出的需求**（每条：前提 → 需求 → 不保证 → 关闭事件）

|#|前提|需求|不保证|关闭事件|
|---|---|---|---|---|
|C1|F5 + H1|意图在任何尝试发出**之前**已持久化；发出后无回执的尝试进入 unknown；unknown **不得**自动产生新尝试|不保证 venue 侧只执行一次（无键的 venue 做不到）；只保证 UTA 不主动重复|—（不变量）|
|C2|F6 + F10|unknown 的收敛按 (venue, 操作种类) 使用集成在 P1 声明的证据渠道：按键回读、open-order listing + venue 订单身份、成交 / 持仓对账；渠道结论 inconclusive 或无渠道 → 停在人工，带 principal 的 P10 才能关闭|不保证收敛时限|每个集成上线时其 P1 声明完整|
|C3|S8 + H6|每个 P7 带 principal 与依据；每个 P6 带过期；过期未决 = 否决记录，不补偿|不保证 principal 的真实性（那是 C11）|H6 被证伪|
|C4|H2|一个程序的 CPU / 内存 / 意图速率 / 状态大小有预算；超预算被隔离并报告；其他程序、账户、核心不受影响|不保证被隔离程序的 state 可用|—|
|C5|H5|订阅（P4）与程序（P12）的生命周期由 UTA 拥有，与消费方连接无关；消费方重连拿到当前状态|不保证重连期间发生的推送逐条补发（见 C6）|H5 被证伪|
|C6|F11 + F6（无通用续传）|断线、配额、慢消费者、压缩过期造成的不连续必须以 P3 显式标记；有 venue 游标时可续传；无游标时不伪造连续|不保证缺失数据可回填（能力决定）|—|
|C7|H3 + C4|凭据只出现在 Alice 封存配置 → UTA 核心 → 该账户的集成进程这条注入链上；程序与消费方只见账户身份|—|H3 被证伪|
|C8|O1（既有事实）|订阅流与幂等键对每个集成都是**新写**的能力；设计不能假设复用现有 adapter 的任何行为|—|—（实施前提，不是域需求）|
|C9|F2 + O11-E1|意图的目标 instrument 必须属于目标账户（由该账户的集成确认），否则在任何 venue 调用前否决|—|—|
|C10|O11-E2/E3|意图参数在创建时校验：数量 / 名义有限且为正；目标子账户在集成已声明子账户能力且已枚举后才可指定，未枚举前拒绝写|—|—|
|C11|H7 + S11|每个 P13 会话绑定一个经传输无关机制认证的 principal；写类操作按 (principal, 账户, 操作种类) 授权；P7 对待决集合版本的期望不匹配 → 冲突，不执行|不保证传输层本身的机密性（本机）|H7 被证伪|
|C12|O11-E18/E19 + H1|自动规则（guard）的读走同一健康记账；读失败或不可判定 = 规则否决（fail-closed，记 P7 规则否决 + 原因），不放行；规则的副作用（冷却计时）只在尝试实际发出后记|—|—|
|C13|O11-E7/E8|P9 原始负载完整保留；状态映射到有限域词表（至少 accepted / partially_filled / filled / cancelled / rejected / expired / unknown），映射表按 venue 列举输入枚举，无 "其他 → rejected"|—|—|
|C14|O9 + H9|持久记录带格式版本；升级只前进；迁移失败拒绝启动而非部分迁移；程序状态带状态版本，不兼容时显式重置而非静默丢失|—|—|

### 1.4 调查结论（四份调查报告的可用结论；报告在 `investigation/`，每条结论在报告内有原文出处）

**1.4.1 Venue 原生能力**（`investigation/venue-capabilities.md:148-163`）

|Venue|推送流|调用方幂等键|按键回读|续传游标|
|---|---|---|---|---|
|Alpaca|有|`client_order_id`（重复被拒有文档，保留期未知）|有 `/v2/orders:by_client_order_id`|无|
|IBKR TWS|有|无（`orderRef` 仅为用户引用）|无|无|
|Longbridge|有|`client_request_id`（10 分钟缓存）|无（详情需 `order_id`）|无|
|Binance|有|`newClientOrderId`（只保证挂单期间唯一）|有 `origClientOrderId`|无|
|Bybit|有|`orderLinkId`（要求唯一，重复行为未知）|有|无|
|OKX|有|`clOrdId`（挂单期唯一，终态后可复用）|有|无|
|Bitget Classic|有|`clientOid`（重复报错有文档）|有|无|
|Hyperliquid|有|`cloid` 字段存在，重复幂等未被文档保证|有 `orderStatus oid=cloid`|无|
|LeverUp|无|无|无|无|
|Mock|无|无|无|无|

推论：
- 调查到的十条产品路径里没有一条有**已证明的**通用续传契约 → C6 的"无游标 → 显式 gap"是常态路径；游标续传是可选优化。
- 幂等键不是普适能力，有键的家保留期 / 重复语义大多未知 → C2 的证据渠道必须逐 (venue, 操作种类) 声明；IBKR / LeverUp 类只能靠 listing + venue 订单身份或人工。
- 配额是能力的一部分（Longbridge 500 symbol、IBKR pacing）→ P1 含配额（Q15）。
- 报告未能直接打开官方 URL（环境禁用），引文来自搜索结果；实现前须逐条重开原文（`venue-capabilities.md:167`）。

**1.4.2 Rust 生态可行性**（`investigation/rust-feasibility.md:305-326`）

- 未证明任何 Rust 不可行点；K2 不触发 TS 例外。
- 选定栈的两个缺口及其归类：
  - tonic 0.14.6 原生连接器在 Windows 返回 `uds connections are not allowed on windows`（`:165`）+ H8 → **约束 T1**（§2.1）。
  - Wasmtime 48.0.2 无 live `Store` 快照 / 恢复 API；Wizer 只快照初始化、`Module::serialize` 只保存编译产物（`:133-139`）→ 这是**证据缺口**，进入 §3 恢复语义的候选不变量（"程序逻辑状态显式可序列化，在静止边界持久化"），不是 §2 约束；替代方案（每程序独立受监督进程、确定性重放、不恢复）在 §6 比较。
- 其他影响模型 / 结构的结论：fuel 是确定性指令预算、epoch 不确定、两者都管不住阻塞的 host 调用（`:117-119`）；Component Model async ABI "very incomplete"（`:131`）；tokio `broadcast` 对落后者丢值（`:300`）；okaywal 自述不宜生产（`:189`）；gRPC 单流有序、流间独立（`:173`）；`rust_decimal` scale ≤ 28（`:215`）。
- 未做：吞吐 / 延迟基准、崩溃注入文件系统矩阵、Windows 传输实测（`:330-337`）→ §6。

**1.4.3 既有缺陷**（`investigation/existing-capabilities.md:254-277`）：已并入 O11，并逐条转为 C9–C14 与 Q16、Q19、Q20。

**1.4.4 Alice 侧消费需要**（`investigation/alice-consumers.md:192-207`）：已并入 S10、S11、P13、C11；SDK 现状缺口（`getCapabilities` 空、`getPendingOrderIds` 空、`getHistorical` 可能 404、BFF 变更匹配表漏路径，`:221-227`）→ 新边界的"能力"与"变更类操作"由 IDL 结构决定而非路径字符串。

### 1.5 需求的两簇与它们的连接

需求按现象自然分成两簇（这是问题域的分组，不是结构决定；§3 据此决定是否成为两个模型上下文）：

- **观察簇**：P1 能力、P2 观察、P3 gap、P4 订阅、P5 回填、P12 程序（输入侧）。要求：B1–B5、C5、C6、C8、A04/A11–A20、Q1–Q4、Q9、Q10、Q13–Q15、Q22、Q23。特征：高频、可丢（以 gap 显式标记）、无资金后果。
- **效应簇**：P6 意图、P7 决定、P8 尝试、P9 回执、P10 对账、P11 外部变更、P13 会话授权。要求：B7、C1–C3、C9–C13、A21–A35、A48、Q5–Q7、Q16–Q21。特征：低频、不可丢、有资金后果、必须可审计。

**连接**只有一处：P6 意图的**依据**字段引用观察簇的持久序号；P12 程序的输出是 P6（而不是对 venue 的调用）。K8 "谁和谁能关联"的答案在此：程序 ↔ 观察 通过订阅声明关联；程序 ↔ 效应 通过意图关联；效应 ↔ 效应 只通过帐票（P6–P10 的追加记录）关联。程序不直接见 P8/P9，只见 P6 的后续状态作为一种观察。

---

## 附录 B 维护者原话（R-B 新增需求 / R-C 约束）

- B1 "UTA是做交易数据的，只要有实时K线数据要订阅这一个场景，秒级的K线节点清洗，巨型object装载一堆指标是家常便饭，而UTA的核心职责是订阅后分发effect，纯单线程做这种事情并不好考虑"
- B2 "我在渠道A订阅了十年短债，渠道B订阅了hl的pre ipo代币，渠道C上下了一旦，等待某个指标满足后买一笔SPY500期权，现在ai想要指标尽可能用上所有渠道的信息，因为市场是流动的，你很难写到了多少价格以后买，何况期权的价格还是，正股波动到了什么价格去买期权同时计算gamma"
- B3 "最好的参照物是tradingview，实际上如果能逆向tradingview则uta没有存在的意义，通过自造指标serverless，alert是实质上的事件回调，而pinescript同等于一个能编程的dsl，这样你想把什么组合就能把什么组合，而uta现在只是要求effect抽象后能安全的互相影响，tradingview式的做法需要巨大的运行时成本，本质上因为挂靠的是k线，只要k线每有一条新数据，则pinescript中编译优化后的产物就运行一次"
- B4 "不应该只把期权看作指令集合，期权有自己额外的实时数据，只不过他要把正股当成自己的一部分，而，正常做交易这才是常态，不做短线而做市场波动，观察CRWV同时也看NVDA是再正常不过的事情，那假如加上pre ipo的openai则更加正常，难道这种东西不存在指令吗，最终难道你要组合数据吗"
- B5 "时间不应该要求对齐，而是先后顺序一致"；"节点顺序是否要保证先后顺序这事根本不一定，而依赖你的观察组合怎么写，如果是运算式的那是await消费而不是所谓的指令谁先谁后，如果是观察在同一时刻是否有效追求的是观察窗口的延迟范围有多大而不是谁先来谁后来，消费的时候是否要有序那是程序的事情"
- B6 "uta本身非常像反向代理，uta的核心不处理协议怎么交互，协议交互是集成层自己洗过了然后送到uta，uta然后再直接消费自己内部尽可能统一但是有扩展性的协议，并在基础上把不同功能的effect消费做好"
- B7 "这个设计并没有回答，不管有没有wasm，副作用怎么消费，谁和谁能关联在一块，抽象怎么做"

### R-C 约束（维护者原话；阶段 2 再分"约束 vs 偏好"）

- C1 "UTA本次是完全重写，且必须是独立进程，而且不考虑打包问题，UTA应该是独立二进制被调用，跨进程通讯应该序列化文本或者跨语言的rpc"
- C2 "现在是偏向rust，实在不行才有ts，其他一概不考虑"；"rust最大的不行根本不需要调查，而是rust表达不了effect system，那么，在rust如何保持优雅抽象"
- C3 "读者是以后实现uta的人，没有审批"

