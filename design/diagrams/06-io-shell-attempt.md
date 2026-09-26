# 06 IO 壳：尝试、取证、放弃跟踪、时序

对照：§6.5、§6.6、§3.4、§8.2、§8.1 归因、W1、W2、W5。索引见 `README.md`。

身份约定（代数，§6.5）：一次尝试 = 一条 `Prepared` 及其后继 = 对上游的一次写，身份 `AttemptRef = attempt_position`（该 `Prepared` 的位置）。图中 `p` 指一次尝试。

## D6.1 尝试的状态机

对照：§6.5 代数（线性阶段链、发出前门、`SendBarrier`、`NotSent`、`VenueAccepted` 只认业务回执）、转移表；§6.7 恢复；§8.2 `submit` / `cancel`。

```mermaid
stateDiagram-v2
  state "Prepared（fold：确未发出）" as P
  state "SendBarrier(p)（记写操作 submit/cancel · 核心铸造的键 · 键角色；fold：可能已发出）" as SB
  state "VenueAccepted{venue_order_id, receipt, observation}" as VA
  state "VenueRejected(reason)" as VR
  state "NotSent(reason)（集成可证明未交出）" as NS
  state "Undetermined(p, NoResponse 或 CrashWindow)" as UD
  state "Expired(p, deadline)（未交出）" as EX
  state "等待结束（fold 状态，不是记录）" as FIN
  [*] --> P
  P --> SB : 发出前门：deadline 未过 ∧ 会话有效声明为 Established ∧ 意图对会话有效能力可执行（Supported、参数 schema、目标种类）→ durable append（fsync）
  P --> P : 发出前门：会话未建立或能力不可执行 → 等待（不 append 记录；会话建立 / 能力变化 / deadline 到时重新求值）
  P --> EX : 发出前门 deadline 已过（可达窗口：崩溃恢复、在发出前门等待期间）
  SB --> VA : 写调用返回 Ack → 同事务 VenueAccepted（含 Evidence：契约载荷 + 原始负载）+ 该回应的观察记录 + 计数健康观察
  SB --> VR : 写调用返回 Reject（业务级；无观察记录）
  SB --> NS : 写调用返回 NotSent（SchemaMismatch / LocalRefusal；SDK 缓冲、补发队列都算已交出，不在此列）
  SB --> UD : 写调用返回 NoResponse（集成时限内无回执 / 集成崩溃 / 传输 ACK / 5xx / 会话在返回前结束）
  SB --> UD : 重启时无后继 → Undetermined(CrashWindow)
  UD --> UD : ResolutionEvidence Inconclusive → 下一渠道；渠道穷尽 → 停等（D6.2）
  UD --> FIN : 第一条 Found / Absent（任一渠道，含 Attributed）→ 结果确立
  UD --> FIN : abandon：在途取证完成后结果仍未知 → Abandoned（结果仍未知，见 D6.4）
  VA --> FIN
  VR --> FIN
  NS --> FIN
  EX --> FIN
  FIN --> [*] : 移出 lane 阻塞头集合；保留钉释放
  note right of SB
    IO 壳永不对已有 SendBarrier 的尝试再调用写操作
    无 SendBarrier = 确未发出
    SendBarrier 无后继 = 可能已发出
    归因观察在此期间到达：只保存，不作结果（回执由写调用返回值落）
  end note
```

读法（假想运行时）：

- 正常路径 `P → SB → VA`：`SendBarrier` 单独 fsync，`VenueAccepted` 与该回应的观察记录同一事务。
- 只有 `SB → UD` 这一条边把系统带进对账；进去以后写已经"可能发生"，永不重发，只能用读收敛或由 principal 放弃跟踪。`P → P` 的等待与 `SB → NS` 都不进对账：前者没有 `SendBarrier`，后者集成确知没有交出。
- `Found` 之后没有 `VenueAccepted`：`Evidence` 在 `ResolutionEvidence` 里，该回应的观察记录供读模型 / 钩子 fold。

核出：无。

## D6.2 取证循环（`Undetermined` 之后）

对照：§6.6 对账驱动（渠道顺序、撤单尝试的取证、取证结果与记录的固定矩阵、`ReconciliationReopened`）、`replay_by_key`；§8.2 四个取证操作；§8.3 按 `barrier_at` 判断窗口；§8.5 `retry_reconciliation`；§4.2 `Gap{Channel}`。

```mermaid
flowchart TB
  UD[("Undetermined(p)，等待 Active")]
  UD --> CH0["渠道集 = 会话有效能力中与 SendBarrier 所记写操作、键角色一致的写证明所声明的渠道（不一致 → 空 → 停等）<br/>固定顺序：ByKey → Listing → Fills → Replay（默认关闭，按 venue 开启）；ByKey / Replay 用该尝试 SendBarrier 所记的键并带 barrier_at，不带键的尝试没有这两条<br/>撤单尝试只有 ByKey / Replay（问的是这次撤单请求）；目标订单的记录不归因到撤单尝试，不是它的 Found<br/>本轮已取证渠道 = round == 当前轮 的 ResolutionEvidence（fold）；round 在发起读时取定，迟到的旧轮响应不计入新轮"]
  CH0 --> NEXT{"还有未取证渠道？"}
  NEXT -->|"是"| CALL["调用该渠道读操作<br/>query_by_key / list_open / list_fills / replay_by_key"]
  CALL --> RES{"返回？"}
  RES -->|"Unavailable（含集成按 barrier_at 判断已出上游保留期 / 唯一期）"| GAP["只 append Gap{Channel}<br/>不算取证、不换渠道；同渠道按 pacing 再发"]
  GAP --> CALL
  RES -->|"命中归因到 p 的订单 / 成交 / 原响应"| FOUND["同事务：该回应的观察记录(provenance Reconciliation{p, channel}；list_fills 命中只有成交记录，observation 指同流 Seq 最小者)<br/>+ ResolutionEvidence{p, channel, Found{observation, evidence: Evidence}}"]
  RES -->|"ByKey 在唯一期内的明确否定"| ABS["ResolutionEvidence{p, ByKey, Absent}（唯一有否定语义的渠道）"]
  RES -->|"未命中（listing / fills 空列表永远不是 Absent，F10）"| INC["ResolutionEvidence{p, channel, Inconclusive}"]
  INC --> NEXT
  NEXT -->|"否：渠道穷尽"| WAIT["停等：等待仍 Active，留在阻塞头集合<br/>IO 壳永不 heuristic；保留钉仍在"]
  WAIT -->|"ReconciliationReopened{p, SessionRestored}（append 前重查结果仍未知）<br/>集成会话重建，只对停等、等待 Active 者"| CH0
  MAN["retry_reconciliation（principal）"] -->|"ReconciliationReopened{p, Manual}（append 前重查结果仍未知）<br/>任一结果未知的 Undetermined（Active 或 Abandoned）；Abandoned 者这一轮依序取证一遍、渠道穷尽即停，等待仍是 Abandoned"| CH0
  WAIT -->|"abandon（principal）：见 D6.4"| AB[("Abandoned：等待结束，结果未知")]
  ATTR["被动渠道：带 attribution FromAttempt(p) 的观察记录，不论来源（推送、回执、取证响应、一次性读的结果项；集成在声明的键作用域与唯一期内填写；核心不按键字节归因），且 p 处于 Undetermined、结果未知"] -->|"同事务"| ATT["ResolutionEvidence{p, Attributed, Found{observation: 该记录, evidence: 该记录的载荷 + 原始负载}}"]
  FOUND --> RS[("结果确立")]
  ABS --> RS
  ATT --> RS
```

读法：

- 每一步取证都是一条记录，所以重启后"做到第几个渠道"由 fold 重建，不需要驱动器内存。
- `Unavailable` 不是证据：一个不可用的渠道不能被"跳过"，否则"没查到"会伪装成"查过了"；同渠道按 pacing 重试直到可用。
- 按键渠道的窗口由集成按 `barrier_at` 判断，核心不持有上游的保留期或唯一期。
- 撤阻塞头（D6.7）的结果不重开阻塞头的取证，也不是阻塞头的证据；要再问一次由 principal 发 `retry_reconciliation`。

核出：无。

## D6.3 一次 venue 交互的记录矩阵

对照：§6.5 回执与取证的记录模型；§10.5 #17；§5.3 归因由谁填；§8.3。

| 交互 | 执行事实侧（永存，含 `Evidence` = 契约载荷 + 原始负载） | 观察侧（该回应的观察记录，可压缩） | 同事务 |
|---|---|---|---|
| 写调用（`submit` / `cancel`）→ `Ack` | `VenueAccepted{venue_order_id, receipt: Evidence, observation}`（`observation` 指订单状态记录） | 订单状态一条 + 每笔可识别执行一条成交记录，归因各按自己的关联证据（撤单回执里目标订单的状态是目标订单的记录，不归到撤单）；同为 `provenance: Receipt{p}` | 是 |
| 写调用 → `Reject` | `VenueRejected(reason)`（`Unmapped(raw)` 保留） | 无（venue 侧无订单） | — |
| 写调用 → `NotSent` | `NotSent{reason, evidence}`（`raw` 是集成据以判定未交出的本地原文，可为空） | 无（没有交给上游） | — |
| 写调用 → `NoResponse` | `Undetermined(p, NoResponse)` | 无 | — |
| 取证命中 | `ResolutionEvidence{p, channel, Found{observation, evidence: Evidence}}`（`observation` 指命中的那条；`list_fills` 取同一成交流上 `Seq` 最小者） | 回应含订单状态时订单状态一条 + 每笔可识别执行一条成交记录；同为 `provenance: Reconciliation{p, channel}`；只对由自己的关联证据属于 p 的记录填 `FromAttempt(p)` | 是 |
| ByKey 否定 | `ResolutionEvidence{p, ByKey, Absent}` | 无 | — |
| 未命中 | `ResolutionEvidence{p, channel, Inconclusive}` | 无 | — |
| 渠道不可用 | `Gap{origin: Channel, channel}`（属 p） | 无 | — |
| 归因命中（带 `FromAttempt(p)` 的观察记录，不论来自推送、回执、取证响应还是一次性读） | `ResolutionEvidence{p, Attributed, round, Found{observation, evidence: 该记录的载荷 + 原始负载}}` | 该记录本身 | 是（与带来它的推送、回执或响应同一事务） |
| 放弃跟踪 | `Abandoned{p, principal, note, rule_version}`（在途取证全部完成、结果仍未知之后） | 无 | — |

```mermaid
flowchart LR
  V["venue 响应，经集成消费（submit / cancel / query_by_key / list_open / list_fills / replay_by_key）<br/>= 契约载荷 + 原始负载"] --> TX
  subgraph TX["同一 SQLite 事务（Ack / 取证命中）"]
    E[("执行 J：VenueAccepted{…, receipt: Evidence, observation}<br/>或 ResolutionEvidence{p, channel, Found{observation, evidence: Evidence}}")]
    O[("观察 J：该回应的观察记录（订单状态；每笔可识别执行一条成交记录，带 execution_id）<br/>provenance: Receipt{p} 或 Reconciliation{p, channel}（同一回应相同，IO 壳填）<br/>attribution：各条按自己的关联证据，属于 p 的才是 FromAttempt(p)，不因同一回应继承")]
    E -->|"observation 位置引用（效应 → 观察）"| O
    O -.->|"provenance：不透明出处值，观察侧不解析（§3.2）"| E
  end
  E --> AUD["审计 / 恢复：读执行 J 的 Evidence，不依赖观察记录是否被压缩"]
  O --> RM["读模型 orders：fold 执行事实 + 归因观察"]
  O --> HOOK["单据钩子"]
  O --> SUBS["订阅者：与推送观察同形"]
```

读法：尝试只回答"我的写到达了吗"；订单是什么状态、成交了多少，在观察记录里给观察宇宙的消费者看，在执行记录的 `Evidence` 里给审计看（契约载荷是结论，原始负载是出处）。

核出：无。

## D6.4 等待与结果两根轴（放弃跟踪）

对照：§6.5 等待与结果：两根轴；§6.6 放弃跟踪 `abandon`、重开与轮次、被动渠道 `Attributed`；§8.5 结果未知组；W2。

```mermaid
stateDiagram-v2
  state "等待（UTA 自己的记录 fold）" as C {
    state "Active" as ACT
    state "Finished（结果已确立）" as FINW
    state "Expired（发出前到期，未交出）" as EXW
    state "Abandoned（吸收态）" as ABW
    [*] --> ACT
    ACT --> FINW : VenueAccepted / VenueRejected / NotSent / 第一条 Found 或 Absent
    ACT --> EXW : 发出前门 deadline 已过
    ACT --> ABW : abandon：①停发新取证 ②在途取证各自完成并 append ③一个事务里重查结果仍未知 → append Abandoned{principal, note}
  }
  state "结果（上游事实的副本，从不作为门）" as O {
    state "Unknown（Undetermined）" as UNK
    state "Accepted / Rejected / NotSent / Found / Absent" as KNOWN
    [*] --> UNK
    UNK --> KNOWN : 回执 / NotSent / 第一条 Found 或 Absent（Active 或 Abandoned 时都可以补上）
    UNK --> UNK : Abandoned 后 retry_reconciliation 那一轮得 Inconclusive → 什么都不变（等待仍 Abandoned，结果仍未知；渠道穷尽即停）
  }
  note right of O
    Abandoned 之后：
    不再自动取证，SessionRestored 不重开
    Attributed 与 retry_reconciliation 仍可补上结果
    补上结果不改变等待，不重新阻塞 lane，不恢复保留钉
    结果未补上时对外显示“已放弃跟踪，结果未知”
  end note
```

读法：

- 左轴回答"UTA 还要不要等"，源头是 UTA；右轴回答"这次写发生没有"，源头是上游。principal 只能动左轴：没有人工写结果的操作。
- `abandon` 没有中间记录：①只是 IO 壳进程内的停发；③之前崩溃，日志里没有 `Abandoned`，恢复后仍 `Active`（§9.2 #8）。在途调用已给出结果时不写 `Abandoned`，返回 `Rejected(NotUndetermined)`。受控停止开始时已在执行的 `abandon` 照常完成：②里没有返回的调用在停止第 3 步被强制完成为 `Unavailable`，③在实例结束锚点之前得出（§7.2 受控停止“停止之前已在执行的控制动作”）。
- 离开 `Active` 的那一刻移出 lane 阻塞头集合、释放保留钉（§6.4、§2.4）。

核出：无。

## D6.5 正常下单闭环时序（W1）

对照：W1 步 1–7；§6.1、§6.2、§6.3、§6.5、§8.2、§8.3、§8.5 读模型。

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
  participant A as 下游订阅者（经解释层）
  H->>O: Emit(EffectRequest{trade.place, basis, …})（已随 Advance 提交；不带键）
  O->>EJ: 同事务 Draft{responsible = 装载 principal, basis ∋ 请求位置} + SubmitForDecision + EffectResponse{Drafted}
  T->>S: AwaitingDecision(v1)
  S->>EJ: 授权 ✓ 输入约束 ✓ 审批（策略不要求人工，rule_version）✓ lane 集合空 ✓ 过期 ✓ 门 ✓（Outcome 带 checked_as_of；规则状态由记录 fold，不另写）
  S->>EJ: 同事务 Prepared @p + Close(Prepared(p))
  IO->>IO: 发出前门：deadline 未过 ∧ 会话已建立 ∧ 意图对会话有效能力可执行（Place、参数 schema）
  IO->>EJ: durable append SendBarrier(p)（fsync；记写操作 submit、键 K(p)、键角色）
  IO->>I: submit(attempt p；WriteLaneKey、Place；意图参数 + 意图参数 schema 身份；键 K(p)）（Place 不带 target）
  I->>V: 上游下单（一次写）
  V-->>I: 业务回执（受理，venue_order_id）
  I-->>IO: Ack(venue_id, receipt)
  IO->>EJ: 同事务：VenueAccepted{venue_order_id, receipt: Evidence, observation}
  IO->>OJ: 同事务：该回应的观察记录（Receipt{p}）：订单状态（FromAttempt(p)）；回执已含执行则每笔一条成交记录（带 execution_id，归因按各自的关联证据）
  Note over IO: 结果确立、等待结束 → 移出阻塞头集合（集合空 → lane 解除）；保留钉释放
  V-->>I: 部分成交 / 成交推送
  I->>OJ: 观察记录（attribution FromAttempt(p)，cumulative_filled_quantity；成交记录带 execution_id）
  OJ-->>A: 依次投递：受理、部分成交、成交（字段与原生身份保真；orders 对同一 execution_id 只计一次）
  A->>A: read_model(orders) 或自 fold → 最终 = 成交
  H->>H: 解释②在请求流上读到 EffectResponse{Drafted(ticket)}，在声明的执行事实输入上按 ticket_id → Close(Prepared(p)) → p 的记录认出自己的结果
```

读法：从 `Emit` 到 `VenueAccepted` 是一串各自原子的事务（D1.5）、1 次 fsync 屏障、1 次 venue 写调用；此后一切都是观察推送。

核出：无。

## D6.6 `NoResponse` → 取证 → 停等 → 放弃跟踪（W2）

对照：W2 步 1–4；§6.7 恢复、§6.6 对账驱动与放弃跟踪；§7.2 第 1/3 步；§8.5 结果未知组。

```mermaid
sequenceDiagram
  participant IO as 核心（IO 壳 / 控制面）
  participant I as 集成（会话 A → B）
  participant V as venue
  participant EJ as 执行 J
  participant OJ as 观察 J
  participant OP as 运维 principal
  Note over IO,I: 发出前门已过：会话 A 已建立、会话有效能力可执行（D6.1）
  IO->>EJ: SendBarrier(p)（fsync）
  IO->>I: submit(attempt p)
  alt 核心存活：集成时限内无回执 / 集成崩溃 / 传输 ACK / 5xx
    I-->>IO: NoResponse
    IO->>EJ: append Undetermined(p, NoResponse)（同事务回查已到达、归因到 p 的观察）
  else 核心 kill -9
    Note over IO: 重启第 1 步：instance_id += 1；第 2 步：SendBarrier 无后继
    IO->>EJ: append Undetermined(p, CrashWindow)（同事务回查）
    Note over IO,I: 重启第 3 步：新会话 B 建立后，第 4 步才启动对账（无会话不取证、不记 Gap）
  end
  Note over IO: 渠道集 = 与 SendBarrier 所记写操作、键角色一致的写证明所声明的渠道，按固定顺序取证
  alt 有 by-key 能力
    IO->>I: query_by_key(K(p), key_role, scope, barrier_at)
    alt Found(state)
      I-->>IO: Found
      IO->>EJ: ResolutionEvidence{p, ByKey, Found{observation, evidence}}
      IO->>OJ: 同事务 该回应的观察记录(Reconciliation{p, ByKey})
      Note over IO: 结果确立、等待结束
    else Absent（集成判断该键仍在上游唯一期内）
      I-->>IO: Absent
      IO->>EJ: ResolutionEvidence{p, ByKey, Absent} → 结果确立（本次未发生）
    else Unavailable（含已出唯一期）
      I-->>IO: Unavailable
      IO->>EJ: Gap{Channel, ByKey}；同渠道按 pacing 再发（不换渠道）
    end
  else 无 by-key 能力（该渠道不在声明里）→ listing 起
    IO->>I: list_open(scope)
    I-->>IO: Listing（核心按归因身份匹配）
    IO->>EJ: 未命中 → ResolutionEvidence{p, Listing, Inconclusive}
    IO->>I: list_fills(scope, since) → 同上；replay_by_key（若开启）→ 同上
    IO->>EJ: 渠道穷尽：停等；尝试留在阻塞头集合
    OP->>IO: abandon(p, note)（§8.5）
    IO->>IO: 授权 ✓；停发新取证；等在途取证完成（各自 append 结果）
    IO->>EJ: 结果仍未知 → Abandoned{p, principal, note, rule_version} → 等待结束、移出阻塞头集合（结果仍未知）
  end
  Note over I,V: 迟到回执：旧会话 A 的回执在边界被丢；venue 已受理则 B 以观察记录重送（集成在声明的键作用域与唯一期内填 FromAttempt(p)）
  I->>OJ: 观察记录 → p 处于 Undetermined、结果未知 → 同事务 EJ: ResolutionEvidence{p, Attributed, Found} → 结果确立（已 Abandoned 时只补结果，等待不变）
```

读法：末态可枚举（found → `Evidence` + 观察记录 / absent → 未发生 / 无渠道 → 停等）；停等态之后仍有多条入口：任一时刻到达的 `Attributed Found`、`ReconciliationReopened`（会话重建 / `retry_reconciliation`）重开的取证，或 principal 的 `abandon`：它只结束等待，结果仍等上游证据。

核出：无。

## D6.7 同 lane 并发与撤阻塞头（W5）

对照：W5 正常—失败路径与扩展路径；§6.4 lane 步、无第二类越顶队列；§6.2 目标身份来源之二；§6.6 撤单尝试的取证。

```mermaid
sequenceDiagram
  participant UI as UI principal
  participant AI as AI principal
  participant OP as 运维 principal
  participant S as STS 链
  participant IO as IO 壳
  participant I as 集成
  par 100 ms 内两笔到同一 WriteLaneKey
    UI->>S: 单据 T1 SubmitForDecision
    AI->>S: 单据 T2 SubmitForDecision
  end
  S->>IO: T1 放行 → Prepared @p1（阻塞头集合 = {p1}）
  S->>S: T2 停在 lane 步（AwaitingDecision，alignment 照常重算）
  IO->>I: SendBarrier(p1) → submit
  I-->>IO: NoResponse → Undetermined(p1)
  Note over S: T2 继续等待并告警；deadline 到期则 Close(Expired)
  IO->>I: 取证循环（D6.2）… 渠道穷尽 → 停等
  AI->>S: 单据 T3：cancel，target = IdemKey(p1 的 SendBarrier 记为订单键的键)
  S->>S: T3 授权 ✓ 输入约束 ✓（来源接受 IdemKey 目标；否则 TargetNotAccepted，T3 关闭，集合仍 {p1}）审批 ✓ lane 步不等待（唯一写例外）✓ 过期 ✓ 门 ✓
  S->>IO: Prepared @p3（阻塞头集合 = {p1, p3}）
  IO->>I: SendBarrier(p3) → cancel(target = p1 的订单键；键 K(p3) 若声明请求键)
  I-->>IO: Ack → VenueAccepted(p3)（撤单请求到达；这个结果本身不决议 p1，不证明 p1 已结束）
  Note over IO: 本图设回执里目标订单的记录不带 FromAttempt(p1) 的证据（带时它经 Attributed 确立 p1 的结果、结束 p1 的等待，§6.6）；p3 移出集合；p1 仍停等，IO 壳不自动重开 p1 的取证
  OP->>IO: retry_reconciliation(p1)（principal 看了撤单结果后决定再问一次）
  IO->>IO: append ReconciliationReopened{p1, Manual(principal)}；按声明的渠道从头取证（D6.2）
  alt 有 by-key 能力
    IO->>I: query_by_key(p1 自己的键, …, barrier_at)
    alt 读到目标（已撤或任何状态）
      I-->>IO: Found → ResolutionEvidence{p1, ByKey, Found} → 结果确立
    else ByKey 在唯一期内明确否定
      I-->>IO: Absent → ResolutionEvidence{p1, ByKey, Absent} → 结果确立
    else Unavailable
      I-->>IO: Unavailable → Gap{Channel, ByKey}；同渠道按 pacing 再发（不换渠道）
    end
  else 无 by-key 能力
    IO->>I: list_open(scope)
    I-->>IO: Listing 未见目标 → Inconclusive（F10）→ 下一渠道；穷尽仍停等，或 principal abandon(p1)
  end
  Note over S: 集合清空 → lane 步重新求值 → T2 过冷却、过期步，再过门 → Prepared @p2
  Note over UI,I: 另一 lane（不同账户）的写全程不等待
```

读法：撤单把 venue 侧推到一个读得出的状态，但它自己的回执与 `Found` 只说明撤单请求到达，不是阻塞头的证据，也不自动重开阻塞头的取证；阻塞头仍只由自己的取证与 `Attributed` 收敛，或由 principal 放弃跟踪。T3 的 `target` 必须是 p1 的 `SendBarrier` 记为订单键的键（按尝试精确匹配）；只能按 venue 订单身份撤单的来源上，T3 在输入约束步以 `TargetNotAccepted` 关闭。两条路径（撤阻塞头 / `bypass_lane`）都让该 lane 出现两条 `SendBarrier`，区别只在有无 `bypass_lane` 控制记录（它不是 Decision）。

核出：无。
