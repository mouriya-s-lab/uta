# 06 IO 壳：Attempt 链、取证、复合链、时序

对照：§5.4、§5.5、§6.3.5、§6.3.3 归因、W1、W2、W5、W12。索引见 `README.md`。

身份约定（§5.4 代数）：Attempt 身份 = `attempt_position`；腿身份 `AttemptRef = (attempt_position, leg)`，单腿 `leg = 1`，`Replace` 无原子能力时 `leg = 1` 撤单、`leg = 2` 新单。图中 `r` 指一条腿。

## D6.1 腿的状态机

对照：§5.4 代数（腿的线性阶段链、发出前门、`SendBarrier`、`VenueAccepted` 只认业务回执）、转移表（单腿）、恢复；§6.3.5 `submit`。

```mermaid
stateDiagram-v2
  state "Prepared / 上一腿已终结（fold：本腿未发出）" as P
  state "SendBarrier(r)（fold：可能已发出）" as SB
  state "VenueAccepted{venue_order_id, receipt, observation}" as VA
  state "VenueRejected(reason)" as VR
  state "Undetermined(r, NoResponse 或 CrashWindow)" as UD
  state "Expired(r, deadline)" as EX
  state "腿终结（fold 状态，不是记录）" as RS
  [*] --> P
  P --> SB : 发出前门 deadline 未过 → durable append（fsync）
  P --> EX : 发出前门 deadline 已过（可达窗口：崩溃恢复、复合链等待目标终态期间）
  SB --> VA : submit 返回 Ack → 同事务 VenueAccepted（含原始字节）+ 回执观察副本
  SB --> VR : submit 返回 Reject（业务级；无观察记录）
  SB --> UD : submit 返回 NoResponse（超时 / 集成崩溃 / 传输 ACK / 5xx）
  SB --> UD : 重启时无后继 → Undetermined(CrashWindow)
  UD --> UD : ResolutionEvidence Inconclusive → 下一渠道；渠道穷尽 → 停等 Manual 或 ReconciliationReopened
  UD --> RS : ResolutionEvidence Found（任一渠道，含 Attributed / Manual）
  UD --> RS : ResolutionEvidence Absent（ByKey 或 Manual）
  VA --> RS
  VR --> RS
  EX --> RS
  RS --> [*] : 单腿：链 Resolved；复合链撤单腿：进入 D6.4
  note right of SB
    IO 壳永不 submit 已有 SendBarrier 的腿
    本腿无 SendBarrier = 确未发出
    SendBarrier 无后继 = 可能已发出
    归因观察在此期间到达：只保存，不终结（回执由 submit 返回值落）
  end note
```

读法（假想运行时）：

- 正常路径 `P → SB → VA`：`SendBarrier` 单独 fsync，`VenueAccepted` 与回执观察副本同一事务。
- 只有 `SB → UD` 这一条边把系统带进对账；进去以后写已经"可能发生"，永不重发，只能用读收敛。
- `Found` 之后没有 `VenueAccepted`：证据字节在 `ResolutionEvidence` 里，观察副本供读模型/钩子 fold。

核出：无。

## D6.2 取证循环（`Undetermined` 之后）

对照：§5.4 对账驱动的自动化边界（取证结果与记录的固定矩阵、`ReconciliationReopened`）、先例谱系、`replay_by_key`；§6.3.5 四个读操作；§6.3.1 `CapabilityProof` 渠道声明；§6.4 `resolve`/`retry_reconciliation`；§3.2 `Gap{Channel}`。

```mermaid
flowchart TB
  UD[("Undetermined(r)")]
  UD --> CH0["渠道集 = 该 (venue, op) 当前能力证据声明的渠道<br/>固定顺序：ByKey → Listing → Fills → Replay（默认关闭，按 venue 开启，仅保留期内）<br/>已取证渠道 = 最近一次 ReconciliationReopened 之后已 append 的 ResolutionEvidence（fold）"]
  CH0 --> NEXT{"还有未取证渠道？"}
  NEXT -->|"是"| CALL["调用该渠道读操作<br/>query_by_key / list_open / list_fills / replay_by_key"]
  CALL --> RES{"返回？"}
  RES -->|"Unavailable"| GAP["只 append Gap{Channel}<br/>不算取证、不换渠道；同渠道按 pacing 再发"]
  GAP --> CALL
  RES -->|"命中带归因身份的订单 / 成交 / 原响应"| FOUND["同事务：观察副本(provenance Reconciliation{r, channel})<br/>+ ResolutionEvidence{r, channel, Found{observation, evidence 字节}}"]
  RES -->|"ByKey 明确否定"| ABS["ResolutionEvidence{r, ByKey, Absent}（唯一有否定语义的渠道）"]
  RES -->|"未命中（listing / fills 空 ≠ absent，F10）"| INC["ResolutionEvidence{r, channel, Inconclusive}"]
  INC --> NEXT
  NEXT -->|"否：渠道穷尽"| WAIT["停等：腿仍未终结，留在阻塞头集合<br/>IO 壳永不 heuristic；引用登记钉住保留边界"]
  WAIT -->|"resolve(r, Found(obs) 或 Absent, note)"| MAN["ResolutionEvidence{r, Manual}"]
  WAIT -->|"ReconciliationReopened{r, cause}<br/>cause = CancelLegTerminal(撤阻塞头腿) / SessionRestored(集成会话重建) / Manual(retry_reconciliation)"| CH0
  ATTR["被动渠道：推送观察 attribution FromAttempt(r)<br/>或 idempotency_key 经登记解析到 r，且 r 处于 Undetermined 未终结"] -->|"同事务"| ATT["ResolutionEvidence{r, Attributed, Found{observation: 该记录}}"]
  FOUND --> RS[("腿终结")]
  ABS --> RS
  MAN --> RS
  ATT --> RS
```

读法：

- 每一步取证都是一条记录，所以重启后"做到第几个渠道"由 fold 重建，不需要驱动器内存。
- `Unavailable` 不是证据：一个不可用的渠道不能被"跳过"，否则"没查到"会伪装成"查过了"；重试同渠道直到可用，或等会话重建后 `SessionRestored` 重开。
- 撤阻塞头（D6.7）让 venue 侧到达可读终态，它的作用是触发 `ReconciliationReopened{CancelLegTerminal}` 重走一轮，不是自己产生证据。

核出：撤单后"重访已取证渠道"的触发原文没有——已并入 §5.4（`ReconciliationReopened`）。

## D6.3 一次 venue 交互的记录矩阵

对照：§5.4 回执与取证的记录模型；§8.5 #17；§4 归因由谁填；§6.3.8。

| 交互 | 执行事实侧（永存，含原始字节） | 观察侧（可压缩副本） | 同事务 |
|---|---|---|---|
| `submit` → `Ack` | `VenueAccepted{venue_order_id, receipt: RawPayload, observation}` | `provenance: Receipt{r}`、`attribution: FromAttempt(r)` | 是 |
| `submit` → `Reject` | `VenueRejected(reason)`（`Unmapped(raw)` 保留） | 无（venue 侧无订单） | — |
| `submit` → `NoResponse` | `Undetermined(r, NoResponse)` | 无 | — |
| 取证命中 | `ResolutionEvidence{r, channel, Found{observation, evidence: RawPayload}}` | `provenance: Reconciliation{r, channel}`、`attribution: FromAttempt(r)` | 是 |
| ByKey 否定 | `ResolutionEvidence{r, ByKey, Absent}` | 无 | — |
| 未命中 | `ResolutionEvidence{r, channel, Inconclusive}` | 无 | — |
| 渠道不可用 | `Gap{origin: Channel, channel}`（属 r） | 无 | — |
| 推送归因命中 | `ResolutionEvidence{r, Attributed, Found{observation}}`（无 `evidence`） | 该推送记录本身 | 是 |
| 人工决议 | `ResolutionEvidence{r, Manual, outcome, principal, note}` | `Found` 引用已存在的观察记录（通常先经 `read` 造出） | — |

```mermaid
flowchart LR
  V["venue 响应（经集成 submit / query_by_key / list_open / list_fills / replay_by_key）"] --> TX
  subgraph TX["同一 SQLite 事务（Ack / 取证命中）"]
    E[("执行 J：VenueAccepted{…, receipt 字节, observation}<br/>或 ResolutionEvidence{r, channel, Found{observation, evidence 字节}}")]
    O[("观察 J：同内容的观察副本<br/>provenance: Receipt{r} 或 Reconciliation{r, channel}<br/>attribution: FromAttempt(r)（IO 壳填）")]
    E -->|"observation 位置引用（效应 → 观察）"| O
    O -.->|"provenance：不透明出处值，观察侧不解析（§2.5）"| E
  end
  E --> AUD["审计 / 恢复：读执行 J 的字节，不依赖副本是否被压缩"]
  O --> RM["读模型 orders：fold 执行事实 + 归因观察"]
  O --> HOOK["单据钩子 / 复合链读目标终态（cumulative_filled_quantity）"]
  O --> SUBS["订阅者：与推送观察同形"]
```

读法：Attempt 只回答"我的提交到达了吗"；订单是什么状态、成交了多少，在观察副本里给观察宇宙的消费者看，在执行记录的字节里给审计看。

核出：回执字节的永存归属（C13）原文只写了观察侧记录——已并入 §5.4（执行侧持有原始字节）。

## D6.4 `Replace` 复合链（无原子 cancel/replace 能力）

对照：§5.2 改单是单一意图类型、§5.4 转移表复合链（`AwaitingTargetTerminal`）、§5.5 写→读→写、W12、§7.2 #8。

```mermaid
stateDiagram-v2
  state "撤单腿 leg=1：D6.1" as C
  state "AwaitingTargetTerminal（链级 fold 状态）" as ATT
  state "新单腿 leg=2：D6.1" as N
  state "链 Resolved：无新腿" as R0
  state "链 Resolved" as R
  state "Expired(leg=2, deadline)：新腿永不发，不补偿" as EX
  [*] --> C : Prepared(Replace, target)
  C --> ATT : 撤单腿终结于 VenueAccepted 或 Found
  C --> R0 : 撤单腿终结于 VenueRejected / Absent / Expired（不解释拒绝原因；目标可能仍在时发新腿 = 加仓，H1）
  ATT --> ATT : 读到目标存在但非终态 / 未见目标（F10）→ 按 pacing 再读
  ATT --> ATT : 读返回 Unavailable → Gap{Channel}，再读
  ATT --> N : 目标终态观察到达（撤单腿回执/取证副本已含终态；attribution 指向目标的推送；IO 壳按 target 读：IdemKey→query_by_key，VenueRef→read(orders, id)）且新腿数量 > 0
  ATT --> R0 : 目标终态到达且数量 = 0（口径为剩余量且已全部成交）
  ATT --> EX : 意图 deadline 到期
  N --> R : 新腿终结（VenueAccepted / VenueRejected / Undetermined 后收敛）
  R0 --> [*]
  R --> [*]
  EX --> [*]
  note right of ATT
    有界：出口是目标终态或 deadline
    不接受 resolve；期间链仍在阻塞头集合内
    两腿各自 AttemptRef，撤单腿的回执/归因不会终结新单腿的 Undetermined
  end note
```

读法：

- 这是 `>>=`：第二腿读第一腿的终态观察，发生在 IO 壳内，不是单据层两次起单；两腿各过一次发出前门、各一条 `SendBarrier`、各自的幂等键登记。
- 撤单腿被拒、缺席或过期 → 链终结、新腿永不发；负责人看观察记录另起单据。
- 崩在撤单腿终态已持久、新腿未 `SendBarrier`（#8）：先 fold 链是否已 `Resolved`；未完则续 `AwaitingTargetTerminal`，读目标终态算量，过门后发新腿。

核出：等待目标终态的完整出边（非终态 / 未见 / 不可用 / `deadline`）与腿身份原文没有——已并入 §5.4。

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
  participant EJ as 执行 J
  participant OJ as 观察 J
  participant A as Alice 订阅者
  H->>O: Emit(EffectRequest{trade.place, basis, key})（已随 Advance 提交）
  O->>EJ: 同事务 Draft{responsible = 装载 principal, basis ∋ 请求位置} + SubmitForDecision + EffectResponse{Drafted}
  T->>S: AwaitingDecision(v1)
  S->>EJ: 授权 ✓ 输入约束 ✓ 审批（策略不要求人工，rule_version）✓ lane 集合空 ✓ 过期 ✓ 门 ✓（Outcome 带 checked_as_of）
  S->>EJ: 同事务 Prepared @p + Close(Prepared(p)) + RuleState
  IO->>IO: 发出前门：deadline 未过
  IO->>EJ: durable append SendBarrier(p,1)（fsync）
  IO->>I: submit(attempt (p,1), idempotency_key)
  I->>V: 上游下单
  V-->>I: 业务回执（受理，venue_order_id）
  I-->>IO: Ack(venue_id, receipt)
  IO->>EJ: 同事务：VenueAccepted{venue_order_id, receipt 字节, observation}
  IO->>OJ: 同事务：回执观察副本（Receipt{(p,1)}, FromAttempt((p,1))）
  Note over IO: 腿终结 → 链 Resolved → 移出阻塞头集合（集合空 → lane 解除）；basis 引用登记解除
  V-->>I: 部分成交 / 成交推送
  I->>OJ: 观察记录（attribution FromAttempt((p,1))，cumulative_filled_quantity）
  OJ-->>A: 依次投递：受理、部分成交、成交（字段与原生身份保真）
  A->>A: read_model(orders) 或自 fold → 最终 = 成交
```

读法：从 `Emit` 到 `VenueAccepted` 是一串各自原子的事务（D1.5）、1 次 fsync 屏障、1 次 venue 写调用；此后一切都是观察推送。

核出：无。

## D6.6 `NoResponse` → 取证 → 停等 → 人工（W2）

对照：W2 步 1–4；§5.4 恢复、对账驱动；§6.1 第 1/3 步；§6.4 `read`、`resolve`。

```mermaid
sequenceDiagram
  participant IO as 核心（IO 壳 / 控制面）
  participant I as 集成（会话 A → B）
  participant V as venue
  participant EJ as 执行 J
  participant OJ as 观察 J
  participant OP as 运维 principal
  IO->>EJ: SendBarrier(p,1)（fsync）
  IO->>I: submit(attempt (p,1))
  Note over IO,I: 核心 kill -9（或集成崩溃 / 超时）
  Note over IO: 重启第 1 步：instance_id += 1；第 2 步：SendBarrier 无后继
  IO->>EJ: append Undetermined((p,1), CrashWindow)（同事务回查已到达、归因到 (p,1) 的观察）
  Note over IO,I: 重启第 3 步：新会话 B（新 session_seq）；第 4 步：启动对账
  IO->>I: query_by_key(key)
  alt Found(state)
    I-->>IO: Found
    IO->>EJ: ResolutionEvidence{(p,1), ByKey, Found{observation, evidence}}
    IO->>OJ: 同事务 观察副本(Reconciliation{(p,1), ByKey})
    Note over IO: 腿终结
  else Absent
    I-->>IO: Absent
    IO->>EJ: ResolutionEvidence{(p,1), ByKey, Absent} → 腿终结（本次未发生）
  else Unavailable
    I-->>IO: Unavailable
    IO->>EJ: Gap{Channel, ByKey}；同渠道稍后再发
  else 无 by-key 能力 / 未命中
    IO->>I: list_open(scope) → list_fills(scope, since) → replay_by_key（若开启）
    I-->>IO: 未命中 → Inconclusive × 渠道数
    IO->>EJ: 渠道穷尽：停等；腿留在阻塞头集合
    OP->>IO: read(scopes, orders, by venue_order_id)（§6.4；核心 → 集成 read）
    IO->>OJ: 观察记录 @obs（provenance OneShot{Session(OP)}）
    OP->>IO: resolve((p,1), Found(obs), note)
    IO->>EJ: 授权 ✓ 腿处于 Undetermined ✓ obs 属该 WriteScope ✓ → ResolutionEvidence{(p,1), Manual, Found{obs}} → 腿终结
  end
  Note over I,V: 迟到回执：旧会话 A 的回执在边界被丢；venue 已受理则 B 以观察记录重送（attribution FromAttempt((p,1)) 或 idempotency_key）
  I->>OJ: 观察记录 → (p,1) 处于 Undetermined → 同事务 EJ: ResolutionEvidence{(p,1), Attributed, Found} → 腿终结
```

读法：末态可枚举（found → 证据字节 + 观察副本 / absent → 未发生 / 无渠道 → 停等）；停等态只能由带 principal 的 `resolve`、或 `ReconciliationReopened`（撤阻塞头 / 会话重建 / `retry_reconciliation`）推进。

核出：无。

## D6.7 同 lane 并发与撤阻塞头（W5）

对照：W5 正常—失败路径与扩展路径；§5.3 lane 步、无第二类越顶队列；§5.2 目标身份来源之二；§5.4 `ReconciliationReopened{CancelLegTerminal}`。

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
  S->>IO: T1 放行 → Prepared @p1（阻塞头集合 = {p1}）
  S->>S: T2 停在 lane 步（AwaitingDecision，alignment 照常重算）
  IO->>I: SendBarrier(p1,1) → submit
  I-->>IO: NoResponse → Undetermined((p1,1))
  Note over S: T2 继续等待并告警；deadline 到期则 Close(Expired)
  IO->>I: 取证循环（D6.2）… 渠道穷尽 → 停等
  AI->>S: 单据 T3：cancel，target = IdemKey(p1 的幂等键)
  S->>S: T3 授权 ✓ 输入约束 ✓ 审批 ✓ lane 步不等待（唯一写例外）✓ 过期 ✓ 门 ✓
  S->>IO: Prepared @p3（阻塞头集合 = {p1, p3}）
  IO->>I: SendBarrier(p3,1) → submit cancel
  I-->>IO: Ack → VenueAccepted((p3,1))（撤单腿终结，不决议 p1）
  IO->>IO: append ReconciliationReopened{(p1,1), CancelLegTerminal((p3,1))}
  IO->>I: 重走一轮：query_by_key(p1 key)
  alt 读到目标（已撤或任何状态）
    I-->>IO: Found → ResolutionEvidence{(p1,1), ByKey, Found} → 腿终结
  else ByKey 明确否定
    I-->>IO: Absent → ResolutionEvidence{(p1,1), ByKey, Absent} → 腿终结
  else listing 未见
    I-->>IO: Inconclusive（F10）→ 仍停等
  end
  Note over S: 集合清空 → lane 解除 → T2 放行，先过期步再过门 → Prepared @p2
  Note over UI,I: 另一 lane（不同账户）的写全程不等待
```

读法：撤单让 venue 侧到达一个读得出的终态，并触发阻塞头重开一轮取证；阻塞头仍只由取证收敛，撤单不提升任何渠道的证明力（listing 未见仍是 `Inconclusive`）。两条路径（撤阻塞头 / `bypass_lane`）都让该 lane 出现两条 `SendBarrier`，区别只在有无 `bypass_lane` Decision。

核出："读不到即 `Absent`"的原表述违反 F10——已改为 by-key 否定才 `Absent`（§5.3/W5）。
