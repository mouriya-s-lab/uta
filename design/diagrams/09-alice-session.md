# 09 下游会话（经解释层）、操作集落点、人工决议、控制动作

对照：§8.5、§7.1 信任边界、§7.6、W14、W19、W8、W20；`design/downstream/design.md`。索引见 `README.md`。

## D9.1 会话建立、身份与重连（W14、W19）

对照：§8.5 会话与 principal、已定事实；§7.1 H7；W14 步 3；W19 步 1–2；§9.2 #20；`design/downstream/design.md` 第 3、6、7 节。

```mermaid
sequenceDiagram
  participant D as 下游（Alice / CLI / 外部客户程序）
  participant L as 解释层
  participant TR as 传输（UDS / 命名管道）
  participant C as 核心
  participant RM as 读模型
  participant J as 观察 J
  D->>L: 连接（一次性命令或长连接）；对端凭据 os_user，非本用户拒绝；自报 actor
  L->>TR: 为该下游连接开一个核心会话
  TR->>C: 对端凭据 os_user（H7 信任边界）
  L->>C: handshake(contract_version, actor)
  alt 契约版本不兼容
    C-->>L: 拒绝会话，记 P14
  else 启动第 5 步之前（§7.2 最后开放下游会话）
    C-->>L: 会话可建立；除 handshake 外的操作（含 health）一律返回 Starting，不给部分状态
  else 正常
    C-->>L: Session{principal = (os_user, actor), instance_id, contract_version}
  end
  Note over L,C: 未握手的连接发写 / 控制 → 核心的会话入口拒绝 + 安全事件（W19 步 1）
  Note over L,C: 请求体伪造 principal 不参与授权：只取会话绑定的 principal（W19 步 2）
  L->>RM: read_model(sources)（声明：账户、流、能力；D9.5）
  L->>RM: read_model(kind, as_of?)
  RM-->>L: Snapshot{value, as_of?, gaps?}（orders / positions / lanes / health / sources 带 as_of 与 gaps，可按历史 as_of 读；gaps 只含观察输入的缺口，lanes、sources 为空；tickets、subscriptions 只给当前态）
  L-->>D: 翻成对外概念（账户、订单、持仓、审批…）
  L->>C: subscribe(selector, mode, from?)（观察流：一组 (来源, 流, 主体集?) 项，可跨来源；或执行事实；新订阅 from 缺省 = 各流当前流末）
  C-->>L: cursor 之后记录（未确认区间可能重复，按 LogPosition 去重）
  L-->>D: 推送；附续传令牌；Gap 翻成缺失通知
  Note over D,L: 下游或解释层崩溃 / 重启：核心不变（订阅、程序、lane、日志 owner 是核心）；解释层无状态可丢
  D->>L: 重连，交回续传令牌
  L->>C: 重连：handshake（同一 principal）→ 持久订阅自动挂接，投递从已确认 cursor 续 → read_model 取 as_of
  C-->>L: 断连期间损失以 Gap{Delivery} 显式标记，不伪造补发
  L-->>D: 缺失通知
```

读法：`actor` 是审计与 scope 的键，不是信任来源；信任来自 OS 对端凭据。`as_of` 与 cursor 在解释层与核心之间比对，下游只见续传令牌与缺失通知。

核出：无。

## D9.2 操作集到核心元素的落点

对照：§8.5 操作集；§7.3 模块指南；§3.4 读副作用、§4.4 读模型；§8.2 `read`。这些是解释层代下游发出的核心操作，下游看到的对外概念见 `design/downstream/design.md` 第 2 节。

```mermaid
flowchart LR
  subgraph OPS["核心↔解释层操作（同一 JSON-RPC，§8.5）"]
    S1["subscribe(观察流 {(来源, 流, 主体集?)} 或 执行事实 (来源, 作用域?)) / ack / unsubscribe"]
    S2["read(targets = (来源, 流, request_schema 身份, request, range?), deadline)"]
    S3["read_model(kind, as_of?)"]
    S4["draft / revise / submit_for_decision / decide / send_back / withdraw / transfer"]
    S5["load_program(manifest_ref, cold_start?) · unload_program · reload_config · rotate_credential · restart_integration · request_snapshot · advance_retention · rewind_cursor · bypass_lane"]
    S6["resolve(attempt: AttemptRef, Found(obs) 或 Absent, note)"]
    S6b["retry_reconciliation(attempt: AttemptRef)"]
    S7["health()"]
  end
  subgraph EL["核心元素"]
    SUB["持久订阅 / 投递调度"]
    RDP["一次性读（§7.3）→ 判定 → 同一集成会话 epoch 内同一 identity 的在途调用并入 → 集成 read（经集成会话）→ item 观察记录 + 读结论记录或 Gap{Channel} + 计数观察，OneShot{origins ∋ Session, request}"]
    RM["读模型（只读 fold；含 sources：执行 J 声明版本的 fold）"]
    TK["单据（TicketAction）→ STS 链"]
    CTL["控制面（控制记录 Applied / Rejected）"]
    IOR["控制面 append ResolutionEvidence{Manual} → 腿终结 → 链重算（D9.3）"]
    RRO["控制面 append ReconciliationReopened{Manual} → IO 壳重开一轮取证（D6.2）"]
    HL["健康读模型"]
  end
  S1 --> SUB
  S2 --> RDP
  S3 --> RM
  S4 --> TK
  S5 --> CTL
  S6 --> IOR
  S6b --> RRO
  S7 --> HL
  S2 -.->|"逐 target，按序判定：UnknownTarget / Unavailable{source_state}（从未有声明）/ Unsupported / Unconfirmed / InvalidRequest / Unavailable{source_state}（无会话，不调用不记 gap）/ Answered{conclusion, items} / Refused{conclusion, reason} / Unavailable{gap}（渠道失败，已记 Gap{Channel}）/ Pending{from, instance_id}（deadline 到而调用在途；之后照常记结论或 gap，从 from 订阅只投递项可收到；核心实例已换则不再保证）"| S2
  S1 -.->|"逐项判定，没有任何一项被接纳 → Rejected{items}：来源未登记 / 流不在最近声明里 / 配额池流上不带主体集的供给项 → 该项拒绝；来源从未有声明 → 该项待接纳；供给项超池上限 → 该项 QuotaExceeded{quota, limit}；只投递项不进需求、不占配额；执行事实非 ordered 或作用域键不在任何声明版本里 → 拒绝"| S1
  S3 -.->|"kind 未定义 → 拒绝；as_of 有位置尚未提交 → NotYetAvailable{positions}（各流已提交的流末）；tickets / subscriptions 带历史 as_of → 拒绝"| S3
  S4 -.->|"expected_version ≠ current_version → Conflict；同版本已有 Decision → Conflict(AlreadyDecided)"| S4
  S5 -.->|"越权 → Unauthorized；配置不合法 → Rejected 并保留上一有效版本；advance_retention 逐流判定 → NotForward / ReferencedBelow / InsideWindow"| S5
  S6 -.->|"越权 → Unauthorized；腿非 Undetermined → Rejected(NotUndetermined)"| S6
```

读法：写类按 `(principal, WriteLaneKey, OperationKind)` 授权，控制与决议按 `(principal, 动作种类)` 授权，同一规则族；三组都留下带 principal 的记录。

核出：`read` 与 `resolve` 两组上一轮已并入 §8.5；一次性读按流寻址、读结论记录、`sources` 读模型与执行事实订阅已并入 §2.2、§8.2、§8.5。

## D9.3 人工决议流程

对照：§8.5 决议组；§6.6 对账驱动；§3.4 读即观察记录。

```mermaid
flowchart TB
  L["读模型 lanes：某 lane 的 Undetermined 腿 r 已渠道穷尽（Inconclusive），停等"]
  L --> OP["运维 principal 判断"]
  OP --> R1{"能从 venue 读到该订单？"}
  R1 -->|"能"| RD["read(该作用域的订单流, 按 venue 身份) → 观察记录 @obs"]
  RD --> RS1["resolve(r, Found(obs), note)"]
  R1 -->|"确认未发生"| RS2["resolve(r, Absent, note)"]
  R1 -->|"venue 当时不可达 / 想再自动查一轮"| RT["retry_reconciliation(r) → ReconciliationReopened{r, Manual}（D6.2）"]
  R1 -->|"仍不确定"| KEEP["不决议：腿留在阻塞头集合；可起撤单意图让 venue 侧到达可读终态并自动重开取证（D6.7）"]
  RS1 --> CHK1{"授权 ∧ r 处于 Undetermined 未终结 ∧ obs 存在且属该 WriteScope？"}
  RS2 --> CHK2{"授权 ∧ r 处于 Undetermined 未终结？"}
  CHK1 -->|"否"| RJ["Unauthorized / Rejected(NotUndetermined) / Rejected(reason)"]
  CHK2 -->|"否"| RJ
  CHK1 -->|"是"| EV["append ResolutionEvidence{r, Manual, round, outcome, principal, note}（Found.evidence = obs 当时的载荷 + 该记录保留的原始负载）<br/>腿终结 → 重算链：链 Resolved 才移出阻塞头集合（撤单腿 Found → 链进 AwaitingTargetTerminal，仍占阻塞头）；集合空才解除 lane；引用登记随链解除"]
  CHK2 -->|"是"| EV
```

读法：人工决议不要求渠道已穷尽（可在任一时刻），但永远带 principal；核心自己永不 heuristic。

核出：无。

## D9.4 控制动作触发的记录

对照：§8.5 控制组；§7.6 每文件契约与原子替换；§7.2 第 3 步；§4.2 gap 原因；W8。

```mermaid
flowchart LR
  subgraph ACT["控制动作（带 principal，先授权）"]
    A1["reload_config(rules)"]
    A1r["reload_config(runtime)"]
    A2["rotate_credential(integration)"]
    A3["restart_integration(id)"]
    A4["load_program / unload_program"]
    A5["request_snapshot"]
    A6["advance_retention(to)"]
    A7["rewind_cursor(subscription, to)"]
    A8["bypass_lane(ticket)"]
  end
  A1 --> F1["读策略/审批规则文件（Alice 原子替换写入）<br/>合法 → Applied，规则版本 = 内容 hash，写进此后每条 Outcome / Rejection<br/>不合法 → Rejected，保留上一有效版本"]
  A1 --> F1b["待决单据放行时按新规则重过五步（不冻结）；必要项集 / Lag 变化触发 alignment 重算（D5.5）"]
  A1r --> F1c["读运行期参数文件：快照频率 · 派生侧留存窗口 · deadline 全局缺省 · 投递缓冲上限<br/>合法 → Applied；不合法 → Rejected，保留上一有效版本；不改规则版本"]
  A2 --> F2["凭据链 文件 → 核心 → 集成；该集成新 session_seq<br/>各流强制新 epoch Gap{Source, credential_rotated}"]
  A3 --> F3["终止并重新拉起集成进程；新 session_seq；各流按游标证明决定续接或新 epoch"]
  A4 --> F4["宿主 Load / Unload（D4.2）；cold_start → 不携带 Checkpoint，ProgramReset{Operator}"]
  A5 --> F5["写快照（仅加速重建，不改 append-only）"]
  A6 --> F6["D8.2"]
  A7 --> F7["cursor 退回；已确认区间重投（显式控制动作，不是恢复路径）"]
  A8 --> F8["控制记录 Applied（自觉违反，不是 Decision）：记单据当时的 current_version 与阻塞头位置集；单据不在 AwaitingDecision → Rejected<br/>lane 步只对这些阻塞头不等待，其余各步照常 → Prepared；阻塞头成集合（D5.4）"]
```

读法：控制动作不经进程信号或 flag 文件；每个动作的结果是一条带 principal 与配置版本 hash 的控制记录，生效动作再触发相应记录。

核出：无。

## D9.5 解释层取得声明与执行事实推送（W20）

对照：§2.2 Projection / `StreamDecl` / `account_ref`；§7.5 能力证据；§8.2 `handshake`；§8.5 订阅组、一次性读、`sources`；W20；`design/downstream/design.md` 第 2、3.2、4 节。

```mermaid
sequenceDiagram
  participant I as 集成（来源 X）
  participant C as 核心
  participant EJ as 执行 J
  participant RM as 读模型 sources
  participant L as 解释层
  participant D as 下游
  I->>C: handshake → Projection（作用域 + account_ref、流声明、写能力、配额）
  C->>C: 静态校验（§8.1）；account_ref 与同来源其他作用域重复或与历史绑定不一致 → 该引用标不可解析（不拒绝握手、不影响路由）
  C->>EJ: append 声明版本（session_epoch）
  C->>EJ: 运行期能力变化：IO 壳 append CapabilityObserved（带该声明版本的 session_epoch；lane 能力随 lane 流，逻辑流读 / 回填能力随来源声明流）
  L->>RM: read_model(sources)
  RM-->>L: 每来源：最近声明版本加引用该版本的 CapabilityObserved（账户 = account_ref + label + 挂的流、流的 read/backfill 与名义等级、写能力、配额；account_ref 是否可解析）
  L->>C: subscribe(执行事实 (X, 作用域?), ordered, from)
  EJ-->>C: 已提交的新声明版本 / CapabilityObserved / 单据与腿的执行事实（存储按位置交出字节）
  C-->>L: 持久订阅与投递调度按位置原样搬运（不解析、不经读模型）
  L->>RM: 收到新声明版本或 CapabilityObserved → 重读 sources
  L-->>D: 账户列表、能力（支持 / 不支持 / 未确认）、待审事项、结果未知；引用冲突的账户显示“需要处理”
  Note over L,D: 待审事项是否偏离不推送：呈现时读 tickets（§8.5）
```

读法：声明是执行事实，所以取得它是一次读模型 fold、它的变化随执行事实订阅到达，不依赖当前会话；会话状态另在 `health`（§8.4）。按旧 `account_ref` 发出的命令在引用不可解析时不解析到任何账户。

核出：无。
