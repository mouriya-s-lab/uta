# 09 Alice 会话、操作集落点、人工决议、控制动作

对照：§6.4、§6.1 信任边界、§6.7.3、W14、W19、W8。索引见 `README.md`。

## D9.1 会话建立、身份与重连（W14、W19）

对照：§6.4 会话与 principal、已定事实；§6.1 H7；W14 步 3；W19 步 1–2；§7.2 #20。

```mermaid
sequenceDiagram
  participant A as Alice 进程
  participant TR as 传输（UDS / 命名管道）
  participant C as 核心
  participant RM as 读模型
  participant J as 观察 J
  A->>TR: 连接
  TR->>C: 对端凭据 os_user（H7 信任边界）
  A->>C: handshake(contract_version, actor)
  alt 契约版本不兼容
    C-->>A: 拒绝会话，记 P14
  else 启动第 5 步之前（§6.1 最后开放 Alice 会话）
    C-->>A: 会话可建立；除 handshake / health 外的操作一律返回 Starting，不给部分状态
  else 正常
    C-->>A: Session{principal = (os_user, actor), instance_id, contract_version}
  end
  Note over A,C: 未握手的连接发写 / 控制 → 会话层拒绝 + 安全事件（W19 步 1）
  Note over A,C: 请求体伪造 principal 不参与授权：只取会话绑定的 principal（W19 步 2）
  A->>RM: read_model(kind, as_of?)
  RM-->>A: Snapshot{value, as_of: Set<LogPosition>, gaps}
  A->>C: subscribe(selector, mode, from?)（新订阅；from 缺省 = 当前流末）
  C-->>A: cursor 之后记录（未确认区间可能重复，按 LogPosition 去重）
  Note over A: Alice 崩溃 / 重启：核心不变（订阅、程序、lane、日志 owner 是核心）
  A->>C: 重连：handshake（同一 principal）→ 持久订阅自动挂接，投递从已确认 cursor 续 → read_model 取 as_of
  C-->>A: 断连期间损失以 Gap{Delivery} 显式标记，不伪造补发
```

读法：`actor` 是审计与 scope 的键，不是信任来源；信任来自 OS 对端凭据。`as_of` 与 cursor 可比对，消费者据此知道快照含哪些记录。

核出：无。

## D9.2 操作集到核心元素的落点

对照：§6.4 操作集表；§6.2 模块指南；§6.3.5 `read`。

```mermaid
flowchart LR
  subgraph OPS["Alice 操作（同一 JSON-RPC）"]
    S1["subscribe / ack / unsubscribe"]
    S2["read(scopes, selector, range?, deadline)"]
    S3["read_model(kind, as_of?)"]
    S4["draft / revise / submit_for_decision / decide / send_back / withdraw / transfer"]
    S5["load_program · unload_program · reload_config · rotate_credential · restart_integration · request_snapshot · advance_retention · rewind_cursor · bypass_lane"]
    S6["resolve(attempt: AttemptRef, Found(obs) 或 Absent, note) · retry_reconciliation(attempt)"]
    S7["health()"]
  end
  subgraph EL["核心元素"]
    SUB["持久订阅 / 投递调度"]
    RDP["读路径 → 集成 read → 观察记录 OneShot{Session}"]
    RM["读模型（只读 fold）"]
    TK["单据（TicketAction）→ STS 链"]
    CTL["控制面（控制记录 Applied / Rejected）"]
    IOR["IO 壳链：ResolutionEvidence Manual"]
    HL["健康读模型"]
  end
  S1 --> SUB
  S2 --> RDP
  S3 --> RM
  S4 --> TK
  S5 --> CTL
  S6 --> IOR
  S7 --> HL
  S2 -.->|"逐 scope：Records{as_of} / Unavailable / Unsupported"| S2
  S4 -.->|"expected_version ≠ current_version → Conflict"| S4
  S5 -.->|"越权 → Unauthorized；配置不合法 → Rejected 并保留上一有效版本"| S5
```

读法：写类按 `(principal, WriteLaneKey, OperationKind)` 授权，控制与决议按 `(principal, 动作种类)` 授权，同一规则族；三组都留下带 principal 的记录。

核出：`read` 与 `resolve` 两组上一轮已并入 §6.4。

## D9.3 人工决议流程

对照：§6.4 决议组；§5.4 对账驱动的自动化边界；§3.4 读即观察记录。

```mermaid
flowchart TB
  L["读模型 lanes：某 lane 的 Undetermined 腿 r 已渠道穷尽（Inconclusive），停等"]
  L --> OP["运维 principal 判断"]
  OP --> R1{"能从 venue 读到该订单？"}
  R1 -->|"能"| RD["read(scopes, orders, 按 venue 身份) → 观察记录 @obs"]
  RD --> RS1["resolve(r, Found(obs), note)"]
  R1 -->|"确认未发生"| RS2["resolve(r, Absent, note)"]
  R1 -->|"venue 当时不可达 / 想再自动查一轮"| RT["retry_reconciliation(r) → ReconciliationReopened{r, Manual}（D6.2）"]
  R1 -->|"仍不确定"| KEEP["不决议：腿留在阻塞头集合；可起撤单意图让 venue 侧到达可读终态并自动重开取证（D6.7）"]
  RS1 --> CHK1{"授权 ∧ r 处于 Undetermined 未终结 ∧ obs 存在且属该 WriteScope？"}
  RS2 --> CHK2{"授权 ∧ r 处于 Undetermined 未终结？"}
  CHK1 -->|"否"| RJ["Unauthorized / Rejected(NotUndetermined) / Rejected(reason)"]
  CHK2 -->|"否"| RJ
  CHK1 -->|"是"| EV["append ResolutionEvidence{r, Manual, outcome, principal, note}<br/>腿终结 → 移出阻塞头集合（集合空才解除 lane）；引用登记解除"]
  CHK2 -->|"是"| EV
```

读法：人工决议不要求渠道已穷尽（可在任一时刻），但永远带 principal；核心自己永不 heuristic。

核出：无。

## D9.4 控制动作触发的记录

对照：§6.4 控制组；§6.7.3 每文件契约与原子替换；§6.1 第 3 步；§3.2 gap 原因；W8。

```mermaid
flowchart LR
  subgraph ACT["控制动作（带 principal，先授权）"]
    A1["reload_config(rules / runtime)"]
    A2["rotate_credential(integration)"]
    A3["restart_integration(id)"]
    A4["load_program / unload_program"]
    A5["request_snapshot"]
    A6["advance_retention(to)"]
    A7["rewind_cursor(subscription, to)"]
    A8["bypass_lane(ticket)"]
  end
  A1 --> F1["读统一路径文件（Alice 原子替换写入）<br/>合法 → Applied，版本 = 内容 hash，写进此后每条 Outcome / Rejection<br/>不合法 → Rejected，保留上一有效版本"]
  A1 --> F1b["待决单据放行时按新规则重过五步（不冻结）"]
  A2 --> F2["凭据链 文件 → 核心 → 集成；该集成新 session_seq<br/>各流强制新 epoch Gap{Source, credential_rotated}"]
  A3 --> F3["终止并重新拉起集成进程；新 session_seq；各流按游标证明决定续接或新 epoch"]
  A4 --> F4["宿主 Load / Unload（D4.2）"]
  A5 --> F5["写快照（仅加速重建，不改 append-only）"]
  A6 --> F6["D8.2"]
  A7 --> F7["cursor 退回；已确认区间重投（显式控制动作，不是恢复路径）"]
  A8 --> F8["bypass_lane Decision（自觉违反）；单据越过 lane 步 → Prepared；阻塞头成集合（D5.4）"]
```

读法：控制动作不经进程信号或 flag 文件；每个动作的结果是一条带 principal 与配置版本 hash 的控制记录，生效动作再触发相应记录。

核出：无。
