# 8 接口契约

本章定义核心与集成、Alice、程序宿主、可选行情派生计算子系统之间的接口：每个操作做什么、返回什么、在核心内留下什么记录、怎样失败。

## 8.1 核心↔集成：握手、锚点、字段注册表、`payload_schema`

集成是独立 OS 进程，语言不限：venue SDK 是什么语言就用什么语言。它是上游被消费的唯一地方（§0.1）：调用编排、协议清洗、上游状态映射、对上游回应的判定都在集成内完成，对核心只以契约值出现。因此核心↔集成的契约必须跨语言。

- 契约的语义在本章定义。
- 一份 IDL 是**实现阶段制品**：由本仓库拥有、随 release 发布，这是计划不是现状。任何语言按 IDL 实现即可接入。
- 传输按 OS 选择（§7.1），不改契约语义。
- 集成本身（适配器的职责、声明的书写形式、一致性测试）的设计在 `design/integration/design.md`；本章只拥有契约。

### 契约的两部分 [设计]

| 部分 | 内容 | 核心何时读 | 由谁保证 |
|---|---|---|---|
| **声明** | `Projection`（§2.2）、锚点对齐（下表）、已注册字段、扩展载荷 schema | 握手时、任何操作之前；核心在其上做 fold（路由、门、`required_inputs` 比对、取证渠道顺序） | 它是值，不含调用、时序、时间或状态；核心按 schema 校验 |
| **行为** | §8.2 操作集与 §8.3 推送：每个操作做什么、返回哪个封闭领域值 | 每次调用与推送 | 集成的实现；以一致性测试验证（§8.3 集成义务） |

凡涉及调用、先后顺序、时间或上游状态的判断，都属行为部分，只以封闭返回值对核心可见；声明部分不承载它们。

### 握手：`Projection`

握手一次交换一个**投影**：集成对若干组合子的可解释性声明，带来源与观察时间，随握手变化。`Projection`、`WriteScope`、`Capability`、`Verdict` 的类型与语义见 §2.2。

- 消费方按投影与核心通信；核心按投影路由。
- `Verdict::Unknown` 是**能力未知**，约束启动阶段（能不能发）；它与写边界的**结果未知**（`Undetermined`，约束恢复阶段）分开（§6.5）。
- 写操作的 `CapabilityProof` 含 unknown 证据渠道声明，完备枚举为：
  - 按调用方键回读；
  - open-order listing + venue 订单身份（含“缺席需二次确认”的间隔）；
  - 成交或持仓对账；
  - 保留期内 `replay_by_key`；
  - 无。
- 这是对账渠道顺序的来源（P1、C2；§6.6）。

### 锚点表 × 链路

锚点：没有它构不成链路；闭合、必填、入口即验。缺锚点是**畸形记录**，在集成边界拒绝（不是规则否决）。锚点只服务路由与关联，不服务业务判断（§2.1）。

| 链路 | 锚点 |
|---|---|
| 观察记录 | `session_epoch`（边界接受条件，§7.2）、`StreamId(source, stream, epoch)`、`received_at`；`LogPosition` 由核心在接受时分配（§8.3） |
| 意图 | `principal`、`WriteLaneKey`（不透明，由集成从上游账户结构对齐得出）、`OperationKind`、`basis`（可为空集，但必须存在） |
| 撤单/改单意图 | 上述 + `target: VenueRef \| IdemKey`（构造前提，§6.2） |
| 尝试/决议 | `AttemptRef = (attempt_position, leg)`（`Prepared` 的 `LogPosition` + 腿序号，§6.5）、`WriteLaneKey` |
| 订阅 | stream 集、消费方式 |

### 处理器字段注册表

处理器的定义（触发条件、`required_inputs`、效果）与“字段不存在 → 不触发，不是错误”见 §2.1；`required_inputs` 的定义见 §2.5。注册表按两侧分开：

| 字段出现 | 侧 | 处理器 |
|---|---|---|
| `occurred_at` | 观察 | 事件时间完备进度；缺席则按 `received_at` 保守推导（时间权威见 §8.3） |
| `idempotency_key` | 效应 | key↔`AttemptRef` 登记；`replay_by_key` 渠道可用 |
| `attribution: FromAttempt(AttemptRef)` | 效应 | lane 决议匹配；驱动复合链第二腿；归因 |
| `cumulative_filled_quantity` | 效应[交易协议] | `Replace` 第二腿数量 |
| `deadline` | 效应 | 过期规则 |
| 守卫字段（side / instrument / 名义金额） | 效应[交易协议] | 输入约束、审批阈值 |
| `venue_order_id` | 效应 | 按 id 撤单路径 |
| `payload_schema` | 观察 | 程序 / 钩子解释器选择 |

**`idempotency_key` 处理器：**

- 登记的是 `SendBarrier` 携带的键，按腿。
- 推送观察只带该键而无 `FromAttempt` 时，由此登记解析到腿，再按 `attribution` 行处理。

**`attribution: FromAttempt(AttemptRef)` 处理器（效应侧归因处理器）：**

- 目标腿处于 `Undetermined` 未终结时，append `ResolutionEvidence{AttemptRef, Attributed, Found{observation: 该记录, evidence: 该记录的载荷与原始负载}}`（§6.6）。
- 复合链处于 `AwaitingTargetTerminal` 且该记录是目标订单终态时，驱动第二腿（§6.5）。
- 归因由谁填见 §8.3。

### `payload_schema`

- 集成把对上游的消费结论写成契约载荷，并打 `payload_schema` 标签；同时附上所消费的上游原文作原始负载（§2.1）。核心不解释两者，只路由字节、存输出。
- 程序（§6）与单据钩子按 `payload_schema` 选择解释器，解释契约载荷。原始负载不交给任何解释器，只作证据（C13）。
- 核心计算的精确类型（守卫字段、`cumulative_filled_quantity`、程序 / 钩子解释器用的 money/quantity）只在核心计算处出现，信封不含价格（§2.6）。

**schema 属于契约** [设计]：

- **公共 schema**：P2 所列跨 venue 共有的种类（quote、book、bar、余额、持仓、订单状态、成交等）各有一份，随 IDL 由本仓库发布。某种类有公共 schema 时，集成必须以它输出该种类的流。
- **扩展 schema**：venue 特有、公共 schema 容纳不下的内容，由集成在声明中给出 schema 文本，以单独的流输出；需要与公共流关联时，程序按记录上的身份字段 `Join`（§2.5）。扩展 schema 同样属于契约，不是上游消息格式。
- 核心不解释 schema 内容，只在 `Projection` 中转发；程序与钩子的解释器按 schema 注册。
- 理由：载荷若是上游形状，程序就成了 UTA 内第二个消费上游的地方，只能按 venue 分别写，B2/B4 要求的跨渠道组合做不成（§0.1）。

**身份与版本** [设计]：`payload_schema = (schema_id, schema_version)`，随 `StreamDecl` 在握手声明，同一 `StreamId` 内不变（§2.2）。

- 集成要换载荷版本，就为该流开新 epoch。P3：新 epoch 首条记录带 `Gap{origin: Source, reason: schema_change}`。
- 旧 epoch 的记录保留旧标签。
- 核心按 `(schema_id, schema_version)` 精确匹配解释器。未注册的组合不触发解释器，载荷留作字节（字段不存在 → 不触发）。

## 8.2 核心→集成操作集

> 图：D6.5 W1 时序、D6.6 W2 时序、D9.2 操作落点（`design/diagrams/06-io-shell-attempt.md`、`09-alice-session.md`）。

操作集小且闭合。

- IO 壳是核心中唯一调用集成写接口的地方。
- 对账取证是读副作用（可重试）；`submit`/`cancel` 是写副作用（永不重试）。
- 每个操作的**动作轴**（对外部世界是读 / 写 / 非动作）与**核心内部结果**（append / 持久化）分开写，避免“读 / 写副作用”一词混两义。
- `NoResponse` 与 `Unavailable` 是一等返回值而非异常。

### `handshake(session_epoch) → Projection`

- **语义**：声明作用域 / 流 / 能力。
- **动作轴**：非动作（声明交换）。
- **返回**：`Projection`（含契约版本）。
- **核心内部结果**：
  - 更新路由表；
  - append 一版能力证据（§7.5）；
  - `required_inputs` 比对；
  - 该集成的每条观察流按 P3 决定是否开新 `StreamId.epoch`：集成能以 venue 游标证明续接，则续用原 epoch、`Seq` 接续；否则新 epoch 首条为 `Gap{origin: Source}`。会话 epoch 与流 epoch 独立。
- **错误**：
  - 传输失败 → 重连（新 `session_seq`）；
  - 投影不合法 → 拒绝该集成；
  - 契约版本不兼容 → 拒绝该集成并记 P14 原因，不降级运行；
  - 能力比对缺失 → 引用该字段的树 fail-closed（§2.5）；
  - `session_epoch` 形状与接受条件见 §7.2 第 3 步。
- **重试**：幂等；可重发。

### `submit(attempt) → Ack | Reject | NoResponse`

- **语义**：投放一次写（按腿）。
- **动作轴**：**写**。
- **返回**：`Ack(venue_id, receipt)`（业务回执，`receipt` 是订单状态的契约载荷及其原始负载）/ `Reject(reason)` / `NoResponse`。
- **核心内部结果**（记录模型，§6.5）：
  - `Ack` → 同一事务 append `VenueAccepted{venue_order_id, receipt: Evidence, observation}`（执行 J，永存，§6.5）+ 回执观察记录（`provenance: Receipt{AttemptRef}`，`attribution: FromAttempt(AttemptRef)`）；
  - `Reject` → `VenueRejected`；
  - `NoResponse` → `Undetermined`。
- **错误**：超时 / 集成崩溃 / 传输 ACK / HTTP 5xx 全部 `NoResponse` → `Undetermined`。venue 单方面决定，无 commit ack。
- **重试**：**永不重试**；`SendBarrier` 保证至多首执一次。

### `query_by_key(key) → Found | Absent | Unavailable`

- **语义**：按调用方键回读状态。**动作轴**：**读**。
- **返回**：`Found(state)` / `Absent` / `Unavailable`。
- **核心内部结果**：
  - `Found` → 同事务 观察记录 + `ResolutionEvidence{ByKey, Found{observation, evidence}}`；
  - `Absent` → `ResolutionEvidence{ByKey, Absent}`。这是唯一有明确否定语义的渠道。
- **错误**：`Unavailable` → `Gap{origin: Channel}`，同渠道再发，不算取证；能力不支持则该渠道跳过。
- **重试**：可重试。

### `list_open(scope) → Listing | Unavailable`

- **语义**：列 open orders。**动作轴**：**读**。
- **返回**：`Listing(items)` / `Unavailable`。
- **核心内部结果**：
  - 命中带归因身份的订单 → 同事务 观察记录 + `ResolutionEvidence{Listing, Found}`；
  - 未命中 → `ResolutionEvidence{Listing, Inconclusive}`。F10：listing 未见不证明未递；本渠道没有 `Absent`。
- **错误**：`Unavailable`（超时 / 断连 / 配额拒绝）→ `Gap{origin: Channel}`，同渠道再发；能力不支持则该渠道跳过；空 `Listing` ≠ `Absent`。
- **重试**：可重试、可换渠道。

### `list_fills(scope, since) → Fills | Unavailable`

- **语义**：列成交。**动作轴**：**读**。
- **返回**：`Fills(items, next_cursor?)` / `Unavailable`。
- **核心内部结果**：
  - 命中带归因身份的成交 → 同事务 观察记录 + `ResolutionEvidence{Fills, Found}`；
  - 未命中 → `ResolutionEvidence{Fills, Inconclusive}`（本渠道没有 `Absent`）。
- **错误**：
  - `Unavailable`（超时 / 断连 / 配额拒绝）→ `Gap{origin: Channel}`，同渠道再发；
  - 缺 `since` 游标 → 范围按声明保守取，仍只作 advisory；
  - 能力不支持则跳过。
- **重试**：可重试；分页按 `next_cursor` 续。

### `cancel(venue_id | key)`

- **语义**：撤单。**动作轴**：**写**。
- **返回**：同 `submit` 的回执形态。
- **核心内部结果**：进 `Prepared` 链；也是 `Replace` 的 cancel 腿。
- **错误**：
  - 无回执 → `Undetermined`；
  - 无 cancel-by-key 能力 → 能力项 `Diverged`，不放行。能力项恒为必要项；构造期只查目标存在（§6.2）。
- **重试**：永不重试。

### `replay_by_key(key) → Original | Unavailable`

- **语义**：保留期内重放取原响应。**动作轴**：**读**（形式似写、语义是读）。
- **返回**：`Original(response)` / `Unavailable`。
- **核心内部结果**：
  - `Original` → 同事务 观察记录 + `ResolutionEvidence{Replay, Found}`；
  - 保留期内无此键 → `ResolutionEvidence{Replay, Inconclusive}`。
- **错误**：保留期声明错误即重复下单。默认关闭、按 venue 显式开启、只在声明保留期内调用、渠道顺序排最后（§6.6）。
- **重试**：保留期内幂等；期外不可用。

### `backfill(stream, from, page) → Page | Unavailable`

- **语义**：历史回填（P5，§8.4）。**动作轴**：**读**。
- **返回**：`Page{records, next_cursor?}` / `Unavailable`。
- **核心内部结果**：记录打 `backfilled` 标记 append 观察 `Journal`；覆盖到 `live_from` 即边界闭合。
- **错误**：
  - `Unavailable` → `Gap{origin: Channel}`，可再发；
  - 能力不支持则该流无回填；
  - 穷尽未达 `live_from` → `Gap{origin: Source, reason: backfill_incomplete}`。
- **重试**：可重试；分页按 `next_cursor` 续；有 pacing。

### `read(stream, selector, range?) → Records | Unavailable`

- **语义**：一次性读。发起者是读处理器、钩子 `InputMissing` 取证、消费方 `read`（§3.4）。**动作轴**：**读**。
- **返回**：`Records(items)` / `Unavailable`。
- **核心内部结果**：
  - 记录与该 `stream` 的推送观察同形，打质量标记 `one_shot`，append 观察 `Journal`；`LogPosition` 由核心分配（§8.3）。
  - 记录带 `provenance: OneShot{origin: Request(LogPosition) | Ticket(TicketId) | Session(Principal)}`，三种 `origin` 依次对应读处理器、钩子取证、消费方 `read`。
  - 不参与实时边界、不推进 frontier。
- **错误**：
  - `Unavailable`（超时 / 断连 / 配额拒绝）→ `Gap{origin: Channel}`，可再发；
  - `stream` 未声明或能力 `Unsupported`/`Unknown` → 核心不调用集成，向发起方返回 typed `Unsupported`（Q16，空结果与之可区分）。
- **重试**：可重试、可批处理（只读批处理条件，§2.2）。

### 演进

- 新增 IDL 操作 = 改所有集成（IDL 轴）。它与轴 B 的 `OperationKind` 不同轴：后者只改实现该协议的集成（§8.3）。
- 操作集、返回值与错误语义已定。消息 schema 的文本形式（IDL 文件）随 release 发布，其中构造子的序列化直接取值树的 enum（§2.5）。

## 8.3 集成→核心推送、错误映射、集成义务、扩展三轴

### 集成→核心的推送

> 图：D3.1 推送入库时序、D1.3 边界接受（`design/diagrams/03-observation-ingest.md`、`01-process-topology.md`）。

推送不是对外部世界的读 / 写动作，而是集成把观察与状态送进核心；核心内部结果是 append 或进度推进。

| 推送 | 语义 | 核心内部结果 |
|---|---|---|
| 观察记录 | 集成推送观察记录（含 `session_epoch`、venue seq/cursor 证据、`attribution`、契约载荷 + `payload_schema`、原始负载）；`LogPosition` 由核心按到达顺序分配 | append 观察 `Journal`、推进 cursor/frontier、触发处理器与 DAG |
| `Gap{origin: Source}` | 集成负责的观察流断代 | 记来源 gap（新 epoch 首条记录，含前一范围与最后 `Seq`、原因） |
| 能力变更 | 握手后能力/配额变化 | IO 壳 append `CapabilityObserved`（执行 J，§7.5）、重算受影响单据的 `alignment` |
| readiness / 健康（P16） | 按集成/账户：reach、tier、连续失败数、最后成功时间；回填 readiness（backfilling/live） | 派生健康观察；订阅状态派生（非损失，不需确认） |

错误与 undesired events：

- **观察记录**：
  - 畸形记录（缺锚点）在边界拒绝。
  - `session_epoch ≠ 当前 epoch` 的推送在边界拒绝、不 append（§7.2 第 3 步）。
  - 同一 `StreamId` 内 venue seq 倒退或重复的记录**照常 append**，并打质量标记（P2：`replayed` / `out_of_order`）；不去重、不重排。核心不伪造流顺序；重复与乱序由派生侧 fold 按 venue seq 处理；frontier 不因倒退记录后退。
- **`Gap{origin: Source}`**：只波及其流；核心不受影响。
- **能力变更**：能力收紧使待决单据 `Diverged`。
- **readiness / 健康**：断线 → `Gap{origin: Source}`；健康面经读模型对 Alice 可见（§8.5）。

### 领域返回值 → 线缆错误的映射（同一失败三层）

领域状态转换由效应宇宙（§6）拥有。本表只给线缆表示到领域值的映射，不新增第三套业务状态。

| 领域值（§6 拥有） | 线缆/来源 | 三层中的层 |
|---|---|---|
| `Undetermined` | `submit` 返回 `NoResponse`（超时/集成崩溃/传输 ACK/5xx） | `submit` 的领域返回值，唯一映射到写边界 in-doubt |
| `Gap{origin: Channel}` | 对账读取渠道返回 `Unavailable` | 读证渠道的领域返回值，可重试 |
| `Gap{origin: Source}` | 集成上报观察流断代 | 流内记录，可续/可标 gap |
| `Gap{origin: Delivery}` | 慢消费者/conflated（§4.2） | 投递侧损失，需显式确认 |
| `VenueRejected(reason)` | `submit` 返回 `Reject` | 写已发出、venue 拒了——终态 |
| `Unmapped(raw)` | 集成的上游状态映射无对应词表值 | 保留原始，不伪造穷尽映射（C13） |

### 集成义务清单

集成由 IDL 固定的职责，按契约的两部分（§8.1）分列。

**声明义务**（握手时交给核心的值）：

- 声明投影：作用域、流及其 `payload_schema`、能力与取证渠道、扩展载荷 schema（§2.2）。
- 交互契约的对齐：把上游账户结构、市场地址、身份体系对齐到锚点契约（`WriteLaneKey`、`basis` 可引用的流、`target` 用的身份）。这是判断性设计动作，每个集成自己负责，做错只影响它自己的流。

**行为义务**（每次调用与推送）：

- 消费上游：调用编排、协议清洗、上游状态映射（§2.2）都在集成内完成；有公共 schema 的种类以公共 schema 输出（§8.1）。
- 按流位置推进观察记录。
- 填锚点与已注册字段（含 `attribution`，见下）+ 契约载荷并打 `payload_schema` + 原始负载。
- 把当前 `session_epoch` 回填到每条推送与回执（§7.2 第 3 步）。
- 响应投放、对账查询、一次性读与回填（仅限核心调用），返回值只取 §8.2 的封闭集合，且每个值的含义严格成立：
  - `Ack` 只在上游给出业务回执时返回；`Reject` 只在上游明确拒绝时返回；其余一律 `NoResponse`（§8.2）。
  - `Absent` 只在上游对该键给出明确否定、且这个否定足以证明该键对应的写未发生时返回。上游的“查不到”不足以证明时（例如键已超出上游保证唯一或可查的期限，§1.6.1），返回 `Unavailable`，不返回 `Absent`。
  - 一次 `submit` / `cancel` 在上游至多产生一次写调用：集成内部不重发写，上游 SDK 自带的写重试必须关闭。写的重试由核心决定，而核心永不重试写（§8.2）。
- **不以拷贝回答读**：`query_by_key`、`list_open`、`list_fills`、`replay_by_key`、`backfill`、`read` 的回答必须来自本次对上游的询问，不来自集成自己保存的状态；问不到上游即 `Unavailable`。集成为消费推送流而维持的状态（如由增量重建的盘口）只用于产出推送记录，不用于回答读。理由：读的回答被核心当作证据（`Absent` 终结一条腿，§6.6），而集成保存的状态是上游原值在过去某刻的拷贝（§0.1）。
- 上报 `Gap{origin: Source}` 与 readiness（§8.4）。

集成**不持有**规则状态、不做决策、不接触 SQLite。

行为义务无法由声明的构造保证，以一致性测试验证：以 fixture 上游驱动集成，逐条义务观测其返回值与推送（验收 §10.5 #22；测试构成见 `design/integration/design.md`）。

**时间权威。** `LogPosition` 与完备进度只由核心裁定；集成只提供证据（venue seq/cursor/事件时间）（§2.3）。

**归因由谁填。** 规则见 §5.3：集成填 `attribution`；IO 壳在回执与取证观察记录上填 `FromAttempt(AttemptRef)`；集成填不出的记 `Unattributed`，IO 壳按键回读补。集成 IDL 因此含 `attribution` 字段。

### 扩展代价三轴

| 轴 | 加什么 | 代价 |
|---|---|---|
| 轴 A（provider） | 新集成 / 新协议字段 | 加处理器，不改锚点、不改核心 crate（additive） |
| 轴 B（`OperationKind`） | 新操作种类 | 改**实现该协议的所有集成**（`OperationKind` 是锚点），只波及该协议 |
| IDL 轴 | 新 IDL 操作 | 改**全部集成**（跨协议契约） |

## 8.4 回填、实时边界、readiness、健康面

> 图：D3.2 readiness 与流 epoch、D3.3 回填与实时边界（`design/diagrams/03-observation-ingest.md`）。

**回填**（P5）是 IDL 读操作 `backfill(stream, from: Seq | Range, page) → Page{records, next_cursor?} | Unavailable`（§8.2）。

- 由核心按订阅需求与 pacing 发起。
- 返回的记录与实时推送同形、同 epoch，打质量标记 `backfilled` 后 append。`LogPosition` 仍由核心按到达顺序分配。
- `Unavailable` → `Gap{origin: Channel}`，可再发。能力不支持则该流无回填，断代只能标 gap。

**实时边界。**

- 集成在 readiness 进入 `Live` 时声明 `live_from: Seq`，即首条实时记录的 venue seq。
- 核心只请求 `< live_from` 的回填范围，回填与实时记录因此按范围不重叠。Q14：同 epoch 无重复 bar，不靠逐条去重。
- 回填页覆盖到 `live_from` 之前即边界闭合，frontier 才允许越过它。
- 回填穷尽仍未达 `live_from`，则 append `Gap{origin: Source, reason: backfill_incomplete}`，frontier 跳过该区间（C6：不伪造连续）。

**readiness 状态机**（按集成 × 流）：

- `Starting → Backfilling{through: Seq} → Live`。
- 该流无回填可做时 `Starting → Live`：握手以 venue 游标证明续接原 epoch（§8.2 `handshake`），或能力不支持回填（断代只能标 gap）。
- 任一状态可进 `Disconnected{since}`；重连回 `Starting`（新 `session_seq`）。
- `Degraded{reason}` 是 `Live` 的子态（能力收紧、配额受限），不改变记录接受条件。
- readiness 变化是派生健康观察，不需确认。

**健康面**（P16）字段：按集成 / 账户 `reach: Reachable | Unreachable`、`tier`、`consecutive_failures`、`last_success_at`、`readiness`。经读模型对 Alice 可见（§8.5）。

## 8.5 核心↔Alice

> 图：D9.1 会话与重连、D9.2 操作集落点、D9.3 人工决议、D9.4 控制动作、D3.4 订阅（`design/diagrams/09-alice-session.md`、`03-observation-ingest.md`）。

Alice 是独立生命周期的消费方与控制方，经同一 JSON-RPC 与核心通信。

### 会话与 principal

会话以 `handshake(contract_version, actor) → Session{principal, instance_id, contract_version}` 建立。

- 传输给出 OS 对端凭据（UDS peer credential / 命名管道 ACL），这是 H7 信任边界。
- `actor` 是 Alice 自报的会话内身份（哪个 AI / 哪个人）；`principal = (os_user, actor)`。
- 同用户进程视为用户本人（H7），所以 `actor` 无需第二重认证：它是审计与 scope 的键，不是信任来源。
- 契约版本不兼容 → 拒绝会话并记 P14。
- 启动第 5 步之前（§7.2），会话可建立，但除 `handshake`/`health` 外的操作一律返回 `Starting`，不给部分状态。

**授权。** 写类操作按 `(principal, WriteLaneKey, OperationKind)` 授权（C11）；控制动作与人工决议按 `(principal, 动作种类)` 授权。三者同一规则族（授权步，§6.3）。

**订阅归属 principal。** 同一 principal 的新会话自动重新挂接其持久订阅，投递从已确认 cursor 续（§4.2）；不需要重新 `subscribe`。

### 操作集

同一 JSON-RPC；动作轴与核心内部结果分开写，同 §8.2。

**订阅组**：`subscribe(selector, mode, from?) → Subscription`；`ack(subscription, cursor)`；`unsubscribe`。

- 动作轴：非动作。
- 核心内部结果：
  - 建 / 改订阅表；`ack` 推进 cursor（确认 = 已处理，§4.2）。
  - `from` 缺省 = 各选中流的当前流末：新订阅不补历史，要历史就显式给 `from`。
  - 订阅持久、归属 principal，重连自动挂接。
- 错误：
  - selector 引用未声明的流 → 拒绝；
  - `from` < 保留边界 → `BeyondRetention`；
  - 超过投影声明的配额（P1、F7）→ `QuotaExceeded{scope, limit}`。核心在路由前判定，集成不收到超限订阅，既有订阅不受影响（Q13）；
  - 断连后从已确认 cursor 重投。

**一次性读**：`read(scopes, selector, range?, deadline) → Vec<ScopeResult>`，`ScopeResult = Records{as_of: LogPosition} | Unavailable | Unsupported`。

- 动作轴：读（经核心→集成 `read`，§8.2）。
- 核心内部结果：每 scope 一次 `read`，结果作观察记录 append（`provenance: OneShot{origin: Session(principal)}`，§3.4）。`as_of` 是该记录的位置，消费方据此在订阅流中定位。
- 错误：
  - 各 scope 独立返回，一个 venue 不可用不影响其余（Q15）；
  - `deadline` 内未返回的 scope 为 `Unavailable`；
  - 流未声明或能力不支持的 scope 为 `Unsupported`（Q16）。

**读模型**：`read_model(kind, as_of?) → Snapshot{value, as_of: Set<LogPosition>, gaps}`。

- 动作轴：读（核心内 fold）。核心内部结果：无。
- 错误：`kind` 未定义 → 拒绝；`as_of` 未达 → `NotYetAvailable{frontier}`。读模型非权威（§4.4）。

**单据组**：`draft(intent) → TicketId`；`revise(ticket, expected_version, diff)`；`submit_for_decision(ticket, expected_version)`；`decide(ticket, expected_version, Approve | Reject(reason))`；`send_back`；`withdraw`；`transfer(ticket, to: principal)`。

- 动作轴：写（append `TicketAction`）。
- 核心内部结果：单据 fold 转移（§6.2）；`Approve` 触发 STS 链放行（§6.3）。
- 错误：
  - `expected_version ≠ current_version` → `Conflict`，不执行；
  - `decide` 时该 `(ticket, current_version)` 已有 Decision → `Conflict(AlreadyDecided)`。单据可能仍停在 lane 步而版本未变（§6.3）；
  - 越权 → `Unauthorized`；
  - `Closed` 后任何动作 → `Rejected(Closed)`；
  - 无 `responsible` 的 `revise` → `Rejected(NotResponsible)`。

**控制组（P14）**：`load_program(manifest_ref, cold_start?)`；`unload_program(id)`；`reload_config(kind)`；`rotate_credential(integration)`；`restart_integration(id)`；`request_snapshot`；`advance_retention(to: Set<LogPosition>)`；`rewind_cursor(subscription, to)`；`bypass_lane(ticket)`。

- 动作轴：写（append 控制记录）。
- 核心内部结果：
  - 每个动作的结果 `Applied(position) | Rejected(reason)` 作为控制记录 append，带 principal、动作、所读配置版本 hash。
  - 生效动作再触发相应记录：新 epoch、`Gap`、绕过 Decision、保留边界推进。
  - `advance_retention` 的 `to` 为每条要推进的观察流一个新边界（§2.4）。
  - `load_program` 的 `cold_start` 缺省为假；为真时不携带已持久化的 `Checkpoint` 装载，记 `ProgramReset{Operator}`（§8.6）。
- 错误：
  - 越权 → `Unauthorized`；
  - 配置文件不合法 → `Rejected(reason)`，并保留上一有效版本（§7.6）；
  - `bypass_lane` 记为对协议的自觉违反（§6.4）。
- `advance_retention` 逐流判定，任一流不通过即整体拒绝：
  - 新边界不高于该流当前边界 → `Rejected(NotForward)`（边界只前进，§7.5）；
  - 越过该流已登记引用最早位置 → `Rejected(ReferencedBelow{min})`（§2.4）；
  - 越过该流配置留存窗口下界 → `Rejected(InsideWindow{bound})`（§2.4）。

**决议组**：`resolve(attempt: AttemptRef, outcome: Found{observation: LogPosition} | Absent, note)`；`retry_reconciliation(attempt: AttemptRef)`。

- 动作轴：写（append `ResolutionEvidence::Manual` / `ReconciliationReopened{Manual}`）。
- `resolve` 的核心内部结果：
  - append `ResolutionEvidence{attempt, Manual, round, outcome}`，带 principal 与 `note`。
  - `Found` 的 `evidence` 取被引用观察记录当时的载荷与原始负载（`Evidence`，§6.5）。被引用记录通常先经 `read` 造出；观察副本可压缩，`evidence` 永存。
  - 该腿终结后重算整条链：链 `Resolved` 才从 lane 阻塞头集合移出，集合为空才解除（§6.4、§6.5）。撤单腿的 `Manual Found` 使链进入 `AwaitingTargetTerminal`，仍占阻塞头。
- `retry_reconciliation` 的核心内部结果：重开一轮自动取证（新 `round`，旧轮在途响应不计入，§6.6）。
- 错误：
  - 越权 → `Unauthorized`；
  - 腿不处于 `Undetermined` 或已终结 → `Rejected(NotUndetermined)`；
  - `Found` 引用的观察记录不存在或不属该 `WriteScope` → `Rejected(reason)`；
  - `AwaitingTargetTerminal` 的链不接受 `resolve`：它有界，出口是目标终态或 `deadline`（§6.5）。
- 不要求渠道已穷尽：人工可在任一时刻决议；自动取证仍在进行时的决议同样记为 `Manual`。

**健康**：`health() → Vec<IntegrationHealth>`。动作轴：读。核心内部结果：无。启动期返回 `Starting`（§7.2 第 5 步）。

### 读模型集合与一致性

核心维护的读模型：

- `orders`：按 `WriteScope`，执行事实 + 归因观察的 fold；
- `positions`；
- `lanes`：每 lane 的未终结 Attempt 与 `Undetermined` 列表；
- `tickets`、`subscriptions`、`health`。

每个 `Snapshot` 带 `as_of: Set<LogPosition>`（fold 吃到的位置集）与 `gaps`（该范围内生效的 `Gap` 记录）。消费者把 `as_of` 与自己的 cursor 比对，即知快照含哪些记录。

原始记录是消费契约；读模型是对同一 `Journal` 的确定性 fold。Alice 自 fold 与核心读模型在同一 `as_of` 下相等（验收 §10.5 #18）。

### 授权与审批策略的表示

- principal → scope 与控制动作、人工决议授权，写在统一路径的策略 / 审批规则文件（§7.6）。
- 规则版本 = 内容 hash，经 `reload_config` 生效。
- 规则不冻结进单据；待决单据在放行时按当时规则重过五步（§6.3）。收紧规则可使待决单据在放行时 `Rejection`，记录带 `rule_version`。
- 负责人失联由规则处理：过期步 `Close(Expired)`，或带 principal 的强制 `transfer`（§6.2）。
- 每条 `Outcome`/`Rejection`/控制记录都带所依据的规则版本 hash，审计由此回链。

### 已定事实

- **独立生命周期**：Alice 退出 / 崩溃 / 重启不改变核心的订阅、程序、lane、日志（H5/C5）。
- **重连语义**：Alice 重连后握手，按 `as_of` 读模型取当前状态，从已确认 cursor 之后订阅记录。断连期间的投递损失按 `Gap{origin: Delivery}` 显式标记，不伪造连续性。
- **读模型**：对执行事实 `Journal` 的可重建只读 fold，非权威、不被规则引用；消费方也可直接订阅原始记录自行 fold（S10）。
- **没有“同步”操作**：核心不提供把自己持有的状态对齐到上游的操作，因为它不持有上游原值（§0.1）。旧入口 A08 `sync`（及 A37 快照捕获前的 best-effort `sync`）在新边界上是一次性 `read`：产生新的观察记录，读模型随之 fold；返回的是这些记录的 `as_of`，不是“更新了几条”。
- **控制面**是同一 RPC 的一组操作，由认证 principal 传入，不经进程信号或 flag 文件。
- 消息 schema 的文本形式随 IDL 文件发布；本节定义的是操作、返回值与错误语义。

## 8.6 核心↔程序宿主

> 图：D4.1 一轮 `Advance`、D4.2 程序生命周期、D7.4 程序侧崩溃分支（`design/diagrams/04-program-host.md`、`07-crash-recovery.md`）。

**宿主 = 受监督子进程** [设计]。

- 每个程序一个宿主进程，由核心拉起并登记（进程表，§7.2 第 1 步）。
- 宿主二进制是随核心发布的值树解释器。程序值不编译：“编译单元”就是效应宇宙值代数的规范序列化形式。
- 理由：三 OS 无需额外运行时即可构建与运行（Q25）；预算由 OS 进程机制限制；状态经宿主协议显式序列化。
- Wasm（wasmtime）不选为初始宿主（§6.1、§10.1）：无 live `Store` 快照 / 恢复 API、fuel 不限制阻塞 host 调用、三 OS 开箱即用未证。它可作实现阶段的替代宿主，走同一宿主协议，不改本节。

**程序与宿主的约束：**

- 程序 = 值代数的规范序列化形式：按 schema 校验的 JSON 值树，与 `DerivationNode`/`DecisionStep` 一一对应。任何面向 AI 的文本糖必须编译到同一值，且不是核心的一部分；表达力扩展 = 加构造子（§2.5）。
- 装载期校验：`Id` 越界与环、`required_inputs` 与握手声明比对，缺失即 fail-closed。
- 预算靠宿主不靠类型：CPU / 内存 / 意图速率 / 状态大小的预算由宿主进程隔离（C4）。超预算被隔离并报告，其他程序、账户、核心不受影响。
- 状态显式可序列化：程序状态只经 `Checkpoint` 序列化，不依赖运行时快照。
- 隔离边界：程序无写能力（Intent 是值而非外部调用）；程序不接触 SQLite 与凭据（C7、H2）；宿主仅作为解释器的宿主，不进入设计中心。

### 宿主协议

核心 ↔ 宿主进程，同一 IDL 传输。

**`Load(program, checkpoint?, budget) → Loaded{state_version} | LoadRejected(reason)`**（核心→宿主）

- 装载程序值；`checkpoint` 是最近一次持久化的 `Checkpoint{bytes, state_version}`。
- 装载期校验失败 → `LoadRejected`，程序不运行。
- `checkpoint` 的 `state_version` 不在程序声明接受的版本内，不是装载失败，按状态迁移处理（见下）：核心在 `Load` 前比对，不携带该 `checkpoint` 装载，并 `Reset`。

**`Advance(records, to: cursor) → Output{effects, derivations, checkpoint}`**（核心→宿主）

- 推进一批记录（解释① / ② 纯语义）；每次返回都带新 `Checkpoint`。
- 核心把 `effects`（append 为 `EffectRequest` 记录）、`derivations`（append 派生记录）、`checkpoint` 与该程序的 cursor **同一事务**持久化。
- 事务未提交则这批推进视为未发生，重启后重放同一批记录，因此不会重复 `Emit`（§10.5 #16）。
- 宿主超时 / 崩溃 → 视同 trap；`Output` 超预算（意图速率 / 状态大小）→ 不持久化，按预算语义处理（见下）。

**`Reset(reason)`**（核心→宿主）

- 丢弃状态、冷启动；`reason ∈ {StateVersionMismatch, Replace, Operator}`，三者依次由装载时版本不符、替换程序时新程序不接受旧版本、控制面 `load_program(…, cold_start)` 触发。
- append 程序观察 `ProgramReset{reason}`；程序流开新 epoch（`Gap{origin: Source, reason: program_upgrade}`，§4.2），按 H9 回填。

**`Unload`**（核心→宿主）

- 终止宿主进程并清除登记；最近已持久化的 `Checkpoint` 保留供 `Load`。

### 预算、迁移与恢复

- **预算语义**：CPU 时间与内存由 OS 进程限制（rlimit / job object）；意图速率与状态大小由核心在 `Output` 上检查。超预算或 trap（宿主进程异常退出）时：
  - 核心终止宿主进程，append 失败观察 `ProgramFailed{reason: Budget(kind) | Trap}`；
  - 程序停在 `Failed`，直到控制面 `load_program` 重新装载；
  - 其他程序、账户、核心不受影响。
- **状态迁移**：`Checkpoint` 带 `state_version`；程序值声明它接受的 `state_version`，核心在 `Load` 前比对：
  - 接受 → 携带 `checkpoint` 装载，续跑；
  - 不接受 → `Reset(StateVersionMismatch)`；替换程序 = `Unload` 旧 + `Load` 新（携旧 checkpoint），新程序不接受旧版本则 `Reset(Replace)`；
  - 两种都显式记录（C14），不以 `LoadRejected` 拒绝运行。
- **运维冷启动**：`load_program(manifest_ref, cold_start = true)`（§8.5）不携带 `checkpoint` 装载，`Reset(Operator)`。这是 `Failed` 程序在已持久化状态本身导致反复 trap 时的出口；运行中的程序经 `unload_program` 再 `load_program(…, cold_start = true)` 冷启动。
- **崩溃恢复**：核心重启后，从与 cursor 同事务持久化的最近 `Checkpoint` `Load`（§7.2 第 5 步）；宿主崩溃同上。程序因此不会看到已折入状态的记录（§4.2）。

## 8.7 核心↔可选行情派生计算子系统

行情派生高性能计算子系统是可选项，独立于核心，不属于核心；核心在没有它时完整可运行。

核心与它之间只有一个组合子接口，本节完整拥有这个接口。实现细节在 `design/hpc-derivation/design.md`，其证据在 `design/hpc-derivation/research/`。

### 存在理由（核心层）

- **完整窗口**：黑盒闭包计算每次触发可见指定输入的完整最新窗口，而非 delta。核心不理解其算法，只把已注册的行情读数据与触发信号交给它，把它产生的值交给程序中已约定的消费者。
- **一次洗入**：进入这种内存要求高度对齐的布局，边界上的一次拷贝就是洗入（记录 → 对齐布局），有意为之。零拷贝指洗入之后每次调用不再搬窗口。
- **段池与 `Journal` 分离**：段池是为高性能计算设计的运行期快照，由 `Pooled` 洗入，不持久；`Journal` 是记录的载体，持久于 SQLite。两套存储互不派生，只共用 `LogPosition` 标定；持久化行情归 `Journal`。
- **扇出复用**：同一个指标被 N 个策略复用，并派生出一堆计算。共享只读映射让派生 DAG 里每条“一个段被多个消费者读”的边成本为零，每个段只物化一份。按消费者复制的方案成本是 O(窗口 × 消费者)。
- **独立故障域**：只读共享内存映射让计算在独立进程里，既零拷贝又有故障域。op 进程 panic/OOM 只死计算进程，核心记失败观察。独立进程与扇出共享同时成立，只有共享段这一条路。
- **无 sandbox / 安装即授权**：只读映射保证计算不能写核心内存，但仍可任意 syscall：是故障域，不是 sandbox。原生 artifact 由 principal 经控制面安装，安装即授权（信任边界在安装期）；值树程序仍不可信、受预算。

### 接口只有一个组合子

```rust
DerivationNode::Pooled { input: Id, window: Window }   // 输出不是逐条值流，而是可借用的完整窗口段视图
```

`Window(Id, W)` 与 `Pooled{input, window}` 是不同入口，不是同一物（§4.3）。

### 四条前置条件及装载期判定

由“输出类型” fold（§2.5）在装载期判定，不满足即拒绝该程序。`input` 的记录类型须：

1. **定长**：洗入后每字段元素字节数固定；
2. **位置线性**：`LogPosition` 单调、同列内相邻位置相邻；
3. **无指针**：无引用 / `Vec` / 字符串；
4. **可容忍 ring 回收**：旧位置被回收只产生 `BeyondRetention`，不产生错误结果。

quote/bar/tick 及其指标满足；余额 / 持仓 / 订单状态 / 新闻不满足。这是条件不满足，不是架构隔离。

### 失败语义

- 子系统未安装时，含 `Pooled` 的程序在装载期被拒绝，其余程序不受影响。
- 前置条件不满足时，该程序在装载期被拒绝（同一 fold 判定），错误指出违反了哪一条件。
- 两种都是装载期 fail-closed，不影响运行期其他程序。

### 对核心的零影响

- `Journal`、`LogPosition`、五个 fold（§2.5）、处理器注册表（§8.1）的含义不变。
- `required_inputs` 遇到 `Pooled` 交给子系统解析，其余解析到普通流。
- 效应侧、单据、读模型、保留语义（§2.4）不受影响。

### 原生 op 是注册表黑盒

- 原生计算不是新节点种类，而是注册表（§8.1）里由子系统提供的黑盒 op，要求输入是 `Pooled` 的。
- 输出是一条派生观察流，下游节点像读任何派生流一样读它。
- “程序不是黑盒函数”对决策（解释②）继续成立。
- 若产生外部写，走单据 → STS → IO 壳，不增设旁路。“唤醒对应订单的 AI”是程序里的 `On(pattern) → Emit(EffectRequest)`。

### 验收

- **核心层最小验收**：含 `Pooled` 的程序在无子系统时被拒，且其余程序不受影响；子系统 op 的输出对下游是普通派生流（下游节点无需知道它由原生 op 产出）。
- **实现细节与验收**：见 `design/hpc-derivation/design.md`。核心只保留一个“子系统是否安装”的外部依赖状态，不复制其验收标准。
