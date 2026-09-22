# 06 IO 壳：Attempt 链、取证、复合链、时序

对照：§5.4、§5.5、§6.3.5、§6.3.3 归因、W1、W2、W5、W12。索引见 `README.md`。

## D6.1 Attempt 单腿状态机

对照：§5.4 代数（Attempt、发出前门、`SendBarrier`、`VenueAccepted` 只认业务回执）、转移表、恢复；§6.3.5 `submit`。

```mermaid
stateDiagram-v2
  state "Prepared（fold：未发出）" as P
  state "SendBarrier（fold：可能已发出）" as SB
  state "VenueAccepted(venue_order_id, receipt)" as VA
  state "VenueRejected(reason)" as VR
  state "Undetermined(NoResponse 或 CrashWindow)" as UD
  state "Expired(deadline)" as EX
  state "Resolved（fold 状态，不是记录）" as RS
  [*] --> P : 单据放行，同事务 Prepared + Close(Prepared)
  P --> SB : 发出前门 deadline 未过 → durable append（fsync）
  P --> EX : 发出前门 deadline 已过（可达窗口：崩溃恢复、复合链腿间）
  SB --> VA : submit 返回 Ack → 同事务 回执观察记录 + VenueAccepted
  SB --> VR : submit 返回 Reject（业务级）
  SB --> UD : submit 返回 NoResponse（超时 / 集成崩溃 / 传输 ACK / 5xx）
  SB --> UD : 重启时无后继 → Undetermined(CrashWindow)
  UD --> UD : ResolutionEvidence Inconclusive → 下一渠道；渠道穷尽 → 停，等 Manual
  UD --> RS : ResolutionEvidence Found(observation) 任一渠道
  UD --> RS : ResolutionEvidence Absent 任一渠道
  VA --> RS
  VR --> RS
  EX --> RS
  RS --> [*] : lane 阻塞头解除；basis 与取证引用解除登记
  note right of SB
    IO 壳永不 submit 已有 SendBarrier 的记录
    Prepared 无 SendBarrier = 确未发出
    SendBarrier 无后继 = 可能已发出
  end note
```

读法（假想运行时）：

- 正常路径 `P → SB → VA` 在毫秒级内完成，三条记录两次事务（`SendBarrier` 单独 fsync）。
- 只有 `SB → UD` 这一条边把系统带进对账；进去以后写已经"可能发生"，永不重发，只能用读收敛。
- `Found` 之后没有 `VenueAccepted`：回执就是被引用的观察记录；读模型从观察记录 fold 出受理/拒绝/成交。

核出：无。

## D6.2 取证循环（`Undetermined` 之后）

对照：§5.4 对账驱动的自动化边界、先例谱系、`replay_by_key`；§6.3.5 四个读操作；§6.3.1 `CapabilityProof` 渠道声明；§6.4 `resolve`；§3.2 `Gap{Channel}`。

```mermaid
flowchart TB
  UD[("Undetermined @p")]
  UD --> CH0["渠道集 = 该 (venue, op) 当前能力证据声明的渠道<br/>固定顺序：ByKey → Listing → Fills → Replay（默认关闭，按 venue 开启，仅保留期内）<br/>已取证渠道 = 已 append 的 ResolutionEvidence（fold）"]
  CH0 --> NEXT{"还有未取证渠道？"}
  NEXT -->|"是"| CALL["调用该渠道读操作<br/>query_by_key / list_open / list_fills / replay_by_key"]
  CALL --> RES{"返回？"}
  RES -->|"Unavailable"| GAP["append Gap{Channel}<br/>同渠道可再发（读可重试）"]
  GAP --> CALL
  RES -->|"命中带归因身份的订单 / 成交 / 原响应"| FOUND["同事务：观察记录(provenance Reconciliation) + ResolutionEvidence Found(pos)"]
  RES -->|"by-key 明确 Absent"| ABS["ResolutionEvidence Absent"]
  RES -->|"未命中（listing 空 ≠ absent，F10）"| INC["ResolutionEvidence Inconclusive"]
  INC --> NEXT
  NEXT -->|"否：渠道穷尽"| WAIT["停下：等待带 principal 的 resolve（Manual）<br/>IO 壳永不 heuristic；lane 保持阻塞；引用登记钉住保留边界"]
  WAIT -->|"resolve(p, Found(obs) 或 Absent, note)"| MAN["ResolutionEvidence Manual"]
  ATTR["被动渠道：推送观察带 attribution FromAttempt(p) 或 idempotency_key → p<br/>且 p 处于 Undetermined"] -->|"同事务"| ATT["ResolutionEvidence Attributed Found(该记录)"]
  FOUND --> RS[("Resolved")]
  ABS --> RS
  MAN --> RS
  ATT --> RS
```

读法：

- 每一步取证都是一条记录，所以重启后"做到第几个渠道"由 fold 重建，不需要驱动器内存。
- 自动化止于"重建状态 / 触发查询 / 一次安全重放"（权威在系统外，F1）；穷尽即停，人工决议必须带 principal。
- 撤阻塞头的意图（D5.4 例外一）是让 venue 侧到达一个 by-key 读得出的终态，它本身不产生这条循环里的证据。

核出：无（`Attributed`/`Manual` 渠道上一轮已并入 §5.4/§6.4）。

## D6.3 一次 venue 交互落两条记录

对照：§5.4 回执与取证的记录模型；§4 归因由谁填；§6.3.8。

```mermaid
flowchart LR
  subgraph TX["同一 SQLite 事务"]
    O[("观察 J：一条观察记录<br/>venue 返回的订单 / 成交状态（载荷直通）<br/>provenance: Receipt{p} 或 Reconciliation{p, channel}<br/>attribution: FromAttempt(p)（IO 壳填）")]
    E[("执行 J：一条以位置引用它的记录<br/>VenueAccepted(venue_order_id, receipt = O.pos)<br/>或 ResolutionEvidence{p, channel, Found(O.pos)}")]
    E -->|"位置引用（效应 → 观察，与 basis 同向）"| O
  end
  V["venue 响应（经集成 submit / query_by_key / list_open / list_fills / replay_by_key）"] --> TX
  O --> RM["读模型 orders：fold 执行事实 + 归因观察"]
  O --> HOOK["单据钩子 / 复合链读目标终态（cumulative_filled_quantity）"]
  O --> SUBS["订阅者：与推送观察同形"]
```

读法：Attempt 只回答"我的提交到达了吗"；订单是什么状态、成交了多少，永远在观察记录里。`VenueRejected` 例外：venue 侧没有订单，无观察记录，`reason` 保留 `Unmapped(raw)`。

核出：回执内容的落点原文未写（见 D2.5 核出）。

## D6.4 `Replace` 复合链（无原子 cancel/replace 能力）

对照：§5.2 改单是单一意图类型、§5.4 转移表复合链、§5.5 写→读→写、W12、§7.2 #8。

```mermaid
stateDiagram-v2
  state "Prepared(Replace, target)" as P
  state "撤单腿：发出前门 → SendBarrier(cancel) → submit cancel(target)" as C
  state "撤单腿 Undetermined → 取证循环（D6.2）" as CU
  state "读目标终态观察：回执已是终态即用；否则 query_by_key(target) 直到终态（读，可重试）" as RT
  state "按意图口径算新腿数量" as Q
  state "新腿：发出前门 → SendBarrier(new) → submit" as N
  state "Resolved：无新腿" as R0
  state "Resolved" as R
  state "Expired(deadline)：新腿永不发，不补偿" as EX
  [*] --> P
  P --> C
  C --> RT : VenueAccepted
  C --> CU : NoResponse / CrashWindow
  CU --> RT : Found
  CU --> R0 : Absent（目标未撤，撤单未发生）
  C --> R0 : VenueRejected（不解释原因：已成交与不存在不可靠区分）
  C --> EX : 撤单腿过门失败
  RT --> Q
  Q --> N : 数量 > 0 且 deadline 未过
  Q --> EX : deadline 已过
  Q --> R0 : 数量 = 0（口径为剩余量且目标已全部成交）
  N --> R : 新腿同单腿终态（VenueAccepted / VenueRejected / Undetermined 后收敛）
  R0 --> [*]
  R --> [*]
  EX --> [*]
```

读法：

- 这是 `>>=`：第二腿读第一腿的终态观察，发生在 IO 壳内，不是单据层两次起单；两腿各过一次发出前门、各一条 `SendBarrier`。
- 撤单腿被拒、缺席或过期 → 链终结、新腿永不发：目标可能仍在时发新腿等于加仓（H1）；负责人看观察记录另起单据。
- 崩在撤单腿终态已持久、新腿未 `SendBarrier`（#8）：重启续跑，读目标终态算量，过门后发新腿（确未发出语义）。

核出：撤单 `Ack` 常只表示"撤单请求已受理"，目标终态可能晚到——原文未写等待方式，已并入 §5.4 转移表（`query_by_key(target)` 读到终态为止）。

## D6.5 正常下单闭环时序（W1）

对照：W1 步 1–7；§5.1、§5.2、§5.3、§5.4、§6.3.5、§6.3.6、§6.4 读模型。

```mermaid
sequenceDiagram
  participant H as 程序宿主
  participant O as 出站写处理器
  participant T as 单据
  participant S as STS 链
  participant IO as IO 壳
  participant I as 集成
  participant V as venue
  participant J as 观察 J
  participant A as Alice 订阅者
  H->>O: Emit(EffectRequest{trade.place, basis, key})（已随 Advance 提交）
  O->>T: Draft{responsible = 装载 principal, basis ∋ 请求位置} + SubmitForDecision（同事务）
  T->>S: AwaitingDecision(v1)
  S->>S: 授权 ✓ 输入约束 ✓ 审批（策略不要求人工，rule_version）✓ lane 空 ✓ 门 ✓
  S->>T: 同事务 Prepared @p + Close(Prepared(p)) + Outcome + RuleState
  T->>IO: Prepared @p（lane 顺序）
  IO->>IO: 发出前门：deadline 未过
  IO->>IO: durable append SendBarrier（fsync）
  IO->>I: submit(attempt @p, idempotency_key)
  I->>V: 上游下单
  V-->>I: 业务回执（受理，venue_order_id）
  I-->>IO: Ack(venue_id, receipt)
  IO->>J: 同事务：回执观察记录（Receipt{p}, FromAttempt(p)）+ VenueAccepted(venue_order_id, receipt)
  Note over IO: lane 解除；basis 引用登记解除
  V-->>I: 部分成交 / 成交推送
  I->>J: 观察记录（attribution FromAttempt(p)，cumulative_filled_quantity）
  J-->>A: 依次投递：受理、部分成交、成交（字段与原生身份保真）
  A->>A: read_model(orders) 或自 fold → 最终 = 成交
```

读法：从 `Emit` 到 `VenueAccepted` 是一串各自原子的事务（D1.5）、1 次 fsync 屏障、1 次 venue 写调用；此后一切都是观察推送。

核出：无。

## D6.6 `NoResponse` → 取证 → 人工（W2）

对照：W2 步 1–4；§5.4 恢复、对账驱动；§6.1 第 3 步；§6.4 `resolve`、`read`。

```mermaid
sequenceDiagram
  participant IO as 核心（IO 壳；步末的一次性读走核心读路径）
  participant I as 集成（会话 A → B）
  participant V as venue
  participant J as 两侧 Journal
  participant OP as 运维 principal
  IO->>J: SendBarrier @p（fsync）
  IO->>I: submit(attempt @p)
  Note over IO,I: 核心 kill -9（或集成崩溃 / 超时）
  Note over IO: 重启第 2 步：SendBarrier 无后继
  IO->>J: append Undetermined(CrashWindow)（同事务回查已到达的归因观察）
  Note over IO,I: 重启第 3 步：新会话 B（session_seq += 1）
  IO->>I: query_by_key(key)
  alt Found(state)
    I-->>IO: Found
    IO->>J: 观察记录(Reconciliation{p, ByKey}) + ResolutionEvidence ByKey Found → Resolved
  else Absent
    I-->>IO: Absent
    IO->>J: ResolutionEvidence ByKey Absent → Resolved（未发生）
  else Unavailable
    I-->>IO: Unavailable
    IO->>J: Gap{Channel, ByKey}；稍后再发
  else 无 by-key 能力 / 不确定
    IO->>I: list_open(scope) → list_fills(scope, since) → replay_by_key（若开启）
    I-->>IO: 未命中 → Inconclusive × 渠道数
    IO->>J: 渠道穷尽：停，lane 保持阻塞
    OP->>IO: 经 §6.4 read(scopes, orders, by venue_order_id)（核心 → 集成 read）
    IO->>J: 观察记录 @obs（provenance OneShot{Session(OP)}）
    OP->>J: resolve(p, Found(obs), note) → ResolutionEvidence Manual → Resolved
  end
  Note over I,V: 迟到回执：旧会话 A 的回执在边界被丢；venue 已受理则 B 以观察记录重送（attribution FromAttempt(p)）
  I->>J: 观察记录 FromAttempt(p)，p 处于 Undetermined → 同事务 ResolutionEvidence Attributed Found → Resolved
```

读法：三种末态可枚举（found → 回执即观察记录 / absent → 未发生 / 无渠道 → 人工）；人工态只能由带 principal 的 `resolve` 关闭，且通常先用一次性 `read` 造出可引用的观察记录。

核出：无。

## D6.7 同 lane 并发与撤阻塞头（W5）

对照：W5 正常—失败路径与扩展路径；§5.3 lane 步、无第二类越顶队列；§5.2 目标身份来源之二。

```mermaid
sequenceDiagram
  participant UI as UI principal
  participant AI as AI principal
  participant S as STS 链
  participant IO as IO 壳
  participant I as 集成
  par 100 ms 内两笔到同一 WriteLaneKey
    UI->>S: 单据 T1 SubmitForDecision
    AI->>S: 单据 T2 SubmitForDecision
  end
  S->>IO: T1 放行 → Prepared @p1（lane 阻塞头 = p1）
  S->>S: T2 停在 lane 步（AwaitingDecision，alignment 照常重算）
  IO->>I: SendBarrier @p1 → submit
  I-->>IO: NoResponse → Undetermined @p1
  Note over S: T2 继续等待并告警；deadline 到期则 Close(Expired)
  IO->>I: 取证循环（D6.2）… Inconclusive
  AI->>S: 单据 T3：cancel，target = IdemKey(p1 的幂等键)
  S->>S: T3 授权 ✓ 输入约束 ✓ 审批 ✓ lane 步不等待（唯一写例外）✓ 门 ✓
  S->>IO: Prepared @p3（阻塞头集合 = {p1, p3}）
  IO->>I: SendBarrier @p3 → submit cancel
  I-->>IO: Ack → VenueAccepted @p3（撤单腿终态，不决议 p1）
  IO->>I: query_by_key(p1 key)
  I-->>IO: Found（目标已撤）→ ResolutionEvidence ByKey Found → p1 Resolved
  Note over S: 集合清空 → lane 解除 → T2 放行，重过门 → Prepared @p2
  Note over UI,I: 另一 lane（不同账户）的写全程不等待
```

读法：撤单让 venue 侧到达一个读得出的终态，阻塞头仍由取证收敛；两条路径（撤阻塞头 / `bypass_lane`）都让该 lane 出现两条 `SendBarrier`，区别只在有无 `bypass_lane` Decision。

核出：无。
