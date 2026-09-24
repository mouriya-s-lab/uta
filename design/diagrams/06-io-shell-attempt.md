# 06 IO 壳：Attempt 链、取证、两腿改单、时序

对照：§6.5、§6.6、§3.4、§8.2、§8.1 归因、W1、W2、W5、W12。索引见 `README.md`。

身份约定（代数，§6.5）：Attempt 身份 = `attempt_position`；腿身份 `AttemptRef = (attempt_position, leg)`，单腿计划 `leg = 1`（含原子改单）；`Replace` 按两腿计划执行时 `leg = 1` 撤单、`leg = 2` 新单，计划由首腿 `SendBarrier` 记下。图中 `r` 指一条腿。

## D6.1 腿的状态机

对照：§6.5 代数（腿的线性阶段链、发出前门、`SendBarrier`、`VenueAccepted` 只认业务回执）、转移表（单腿）；§6.7 恢复；§8.2 `submit`。

```mermaid
stateDiagram-v2
  state "Prepared / 上一腿已终结（fold：本腿未发出）" as P
  state "SendBarrier(r)（记写操作 submit/cancel · 键 · 键角色；fold：可能已发出）" as SB
  state "VenueAccepted{venue_order_id, receipt, observation}" as VA
  state "VenueRejected(reason)" as VR
  state "Undetermined(r, NoResponse 或 CrashWindow)" as UD
  state "Expired(r, deadline)" as EX
  state "腿终结（fold 状态，不是记录）" as RS
  [*] --> P
  P --> SB : 发出前门：deadline 未过 ∧ 集成会话 Established ∧ 意图对当前能力可执行（Supported、参数 schema、目标种类；首腿之后声明的腿计划与记录一致）→ durable append（fsync）
  P --> P : 发出前门：会话未建立或能力不可执行 → 等待（不 append 记录；会话建立 / 能力变化 / deadline 到时重新求值）
  P --> EX : 发出前门 deadline 已过（可达窗口：崩溃恢复、两腿改单等待目标终态期间、在发出前门等待期间）
  SB --> VA : 写调用（submit / cancel）返回 Ack → 同事务 VenueAccepted（含 Evidence：契约载荷 + 原始负载）+ 该回应的观察记录（订单状态；每笔可识别执行一条成交记录，带 execution_id）+ 计数健康观察
  SB --> VR : 写调用返回 Reject（业务级；无观察记录）
  SB --> UD : 写调用返回 NoResponse（超时 / 集成崩溃 / 传输 ACK / 5xx / 集成本地不发 / 会话在返回前结束，集成会话恰完成一次）
  SB --> UD : 重启时无后继 → Undetermined(CrashWindow)
  UD --> UD : ResolutionEvidence Inconclusive → 下一渠道；渠道穷尽 → 停等 Manual 或 ReconciliationReopened
  UD --> RS : ResolutionEvidence Found（任一渠道，含 Attributed / Manual）
  UD --> RS : ResolutionEvidence Absent（ByKey 或 Manual）
  VA --> RS
  VR --> RS
  EX --> RS
  RS --> [*] : 单腿计划：链 Resolved；两腿计划的撤单腿：进入 D6.4
  note right of SB
    IO 壳永不对已有 SendBarrier 的腿再调用写操作
    本腿无 SendBarrier = 确未发出
    SendBarrier 无后继 = 可能已发出
    归因观察在此期间到达：只保存，不终结（回执由 submit 返回值落）
  end note
```

读法（假想运行时）：

- 正常路径 `P → SB → VA`：`SendBarrier` 单独 fsync，`VenueAccepted` 与该回应的观察记录同一事务。
- 只有 `SB → UD` 这一条边把系统带进对账；进去以后写已经"可能发生"，永不重发，只能用读收敛。`P → P` 的等待不进对账：没有 `SendBarrier`，写确未发出。
- `Found` 之后没有 `VenueAccepted`：`Evidence` 在 `ResolutionEvidence` 里，该回应的观察记录供读模型/钩子 fold。

核出：无。

## D6.2 取证循环（`Undetermined` 之后）

对照：§6.6 对账驱动（取证结果与记录的固定矩阵、`ReconciliationReopened`）、先例谱系、`replay_by_key`；§8.2 四个读操作；§8.1 `CapabilityProof` 渠道声明；§8.5 `resolve`/`retry_reconciliation`；§4.2 `Gap{Channel}`。

```mermaid
flowchart TB
  UD[("Undetermined(r)")]
  UD --> CH0["渠道集 = 当前能力证据中该腿的声明渠道（声明的腿计划与链记录一致、该位置的键角色与 SendBarrier 所记一致时；否则为空 → 停等）<br/>固定顺序：ByKey → Listing → Fills → Replay（默认关闭，按 venue 开启，仅保留期内）；ByKey / Replay 用该腿自己记下的键，不带键的腿没有这两条<br/>撤单腿只有 ByKey / Replay（问的是这次撤单请求）；目标订单的记录不归因到撤单腿，不是它的 Found<br/>本轮已取证渠道 = round == 当前轮 的 ResolutionEvidence（fold）；round 在发起读时取定，迟到的旧轮响应不计入新轮"]
  CH0 --> NEXT{"还有未取证渠道？"}
  NEXT -->|"是"| CALL["调用该渠道读操作<br/>query_by_key / list_open / list_fills / replay_by_key"]
  CALL --> RES{"返回？"}
  RES -->|"Unavailable"| GAP["只 append Gap{Channel}<br/>不算取证、不换渠道；同渠道按 pacing 再发"]
  GAP --> CALL
  RES -->|"命中归因到 r 的订单 / 成交 / 原响应"| FOUND["同事务：该回应的观察记录(provenance Reconciliation{r, channel}；list_fills 命中只有成交记录，observation 指同流 Seq 最小者)<br/>+ ResolutionEvidence{r, channel, Found{observation, evidence: Evidence}}"]
  RES -->|"ByKey 明确否定"| ABS["ResolutionEvidence{r, ByKey, Absent}（唯一有否定语义的渠道）"]
  RES -->|"未命中（listing / fills 空 ≠ absent，F10）"| INC["ResolutionEvidence{r, channel, Inconclusive}"]
  INC --> NEXT
  NEXT -->|"否：渠道穷尽"| WAIT["停等：腿仍未终结，留在阻塞头集合<br/>IO 壳永不 heuristic；引用登记钉住保留边界"]
  UD -.->|"任一时刻（不以穷尽为前提）：resolve(r, Found(obs) 或 Absent, note)<br/>授权 ∧ r 仍 Undetermined 未终结"| MAN["ResolutionEvidence{r, Manual, round}"]
  WAIT -->|"ReconciliationReopened{r, cause}（append 前重查 r 仍未终结）<br/>cause = CancelLegTerminal(撤阻塞头腿终结于 VenueAccepted/Found：新取证机会，非目标终态证据) / SessionRestored(集成会话重建) / Manual(retry_reconciliation)"| CH0
  ATTR["被动渠道：推送观察 attribution FromAttempt(r)<br/>或 idempotency_key 经登记解析到 r，且 r 处于 Undetermined 未终结"] -->|"同事务"| ATT["ResolutionEvidence{r, Attributed, Found{observation: 该记录, evidence: 该记录的载荷 + 原始负载}}"]
  FOUND --> RS[("腿终结")]
  ABS --> RS
  MAN --> RS
  ATT --> RS
```

读法：

- 每一步取证都是一条记录，所以重启后"做到第几个渠道"由 fold 重建，不需要驱动器内存。
- `Unavailable` 不是证据：一个不可用的渠道不能被"跳过"，否则"没查到"会伪装成"查过了"；同渠道按 pacing 重试直到可用。
- 撤阻塞头（D6.7）让 venue 侧到达可读终态，它的作用是触发 `ReconciliationReopened{CancelLegTerminal}` 重走一轮，不是自己产生证据。

核出：撤单后"重访已取证渠道"的触发原文没有——已并入 §6.6（`ReconciliationReopened`）。

## D6.3 一次 venue 交互的记录矩阵

对照：§6.5 回执与取证的记录模型；§10.5 #17；§5.3 归因由谁填；§8.3。

| 交互 | 执行事实侧（永存，含 `Evidence` = 契约载荷 + 原始负载） | 观察侧（该回应的观察记录，可压缩） | 同事务 |
|---|---|---|---|
| 写调用（`submit` / `cancel`）→ `Ack` | `VenueAccepted{venue_order_id, receipt: Evidence, observation}`（`observation` 指订单状态记录） | 订单状态一条（`FromAttempt(r)`）+ 每笔可识别执行一条成交记录（归因按各自的关联证据）；同为 `provenance: Receipt{r}` | 是 |
| 写调用 → `Reject` | `VenueRejected(reason)`（`Unmapped(raw)` 保留） | 无（venue 侧无订单） | — |
| 写调用 → `NoResponse` | `Undetermined(r, NoResponse)` | 无 | — |
| 取证命中 | `ResolutionEvidence{r, channel, Found{observation, evidence: Evidence}}`（`observation` 指命中的那条；`list_fills` 取同一成交流上 `Seq` 最小者） | 回应含订单状态时订单状态一条 + 每笔可识别执行一条成交记录；同为 `provenance: Reconciliation{r, channel}`；只对由自己的关联证据属于 r 的记录填 `FromAttempt(r)` | 是 |
| ByKey 否定 | `ResolutionEvidence{r, ByKey, Absent}` | 无 | — |
| 未命中 | `ResolutionEvidence{r, channel, Inconclusive}` | 无 | — |
| 渠道不可用 | `Gap{origin: Channel, channel}`（属 r） | 无 | — |
| 推送归因命中 | `ResolutionEvidence{r, Attributed, round, Found{observation, evidence: 该推送的载荷 + 原始负载}}` | 该推送记录本身 | 是 |
| 人工决议 | `ResolutionEvidence{r, Manual, round, outcome, principal, note}`；`Found.evidence` = 被引用观察记录在 `resolve` 时的载荷 + 该记录保留的原始负载（所在流不保留原文时为空） | `Found` 引用已存在的观察记录（通常先经 `read` 造出，可压缩） | — |

```mermaid
flowchart LR
  V["venue 响应，经集成消费（submit / cancel / query_by_key / list_open / list_fills / replay_by_key）<br/>= 契约载荷 + 原始负载"] --> TX
  subgraph TX["同一 SQLite 事务（Ack / 取证命中）"]
    E[("执行 J：VenueAccepted{…, receipt: Evidence, observation}<br/>或 ResolutionEvidence{r, channel, Found{observation, evidence: Evidence}}")]
    O[("观察 J：该回应的观察记录（订单状态；每笔可识别执行一条成交记录，带 execution_id）<br/>provenance: Receipt{r} 或 Reconciliation{r, channel}（同一回应相同，IO 壳填）<br/>attribution：各条按自己的关联证据，属于 r 的才是 FromAttempt(r)，不因同一回应继承")]
    E -->|"observation 位置引用（效应 → 观察）"| O
    O -.->|"provenance：不透明出处值，观察侧不解析（§3.2）"| E
  end
  E --> AUD["审计 / 恢复：读执行 J 的 Evidence，不依赖观察记录是否被压缩"]
  O --> RM["读模型 orders：fold 执行事实 + 归因观察"]
  O --> HOOK["单据钩子 / 复合链读目标终态（cumulative_filled_quantity）"]
  O --> SUBS["订阅者：与推送观察同形"]
```

读法：Attempt 只回答"我的提交到达了吗"；订单是什么状态、成交了多少，在观察记录里给观察宇宙的消费者看，在执行记录的 `Evidence` 里给审计看（契约载荷是结论，原始负载是出处）。

核出：回执证据的永存归属（C13）原文只写了观察侧记录——已并入 §6.5（执行侧持有 `Evidence`）。

## D6.4 `Replace` 两腿计划（`[cancel, submit]`）

对照：§6.2 改单是单一意图类型、腿计划与可执行性；§6.5 腿计划落进记录、转移表复合链（`AwaitingTargetTerminal`、`TargetTerminal`）、按目标身份的读；§3.4 写→读→写、W12、§9.2 #8。原子计划 `[submit]` 是 D6.1 的一条腿，不经本图。

```mermaid
stateDiagram-v2
  state "撤单腿 leg=1：D6.1" as C
  state "AwaitingTargetTerminal（链级 fold 状态）" as ATT
  state "新单腿 leg=2：D6.1" as N
  state "链 Resolved：无新腿" as R0
  state "链 Resolved" as R
  state "Expired(leg=2, deadline)：新腿永不发，不补偿" as EX
  [*] --> C : Prepared(Replace, target)；首腿过发出前门时按当时声明选定两腿计划，记进 SendBarrier(cancel)
  C --> ATT : 撤单腿终结于 VenueAccepted 或 Found（进入的事务先看 deadline，再对截至此刻的全部记录做目标终态判定：回看）
  C --> R0 : 撤单腿终结于 VenueRejected / Absent / Expired（不解释拒绝原因；目标可能仍在时发新腿 = 加仓，H1）
  ATT --> ATT : 目标订单的记录到达（推送 / 回执 / 取证 / IO 壳按目标身份 read / 回填，不论归因），判定仍不成立：最近观察非终态、为 unknown 或 Unmapped(raw)（C13）、为未被选中的较旧序号终态、剩余量口径下缺 cumulative_filled_quantity、已落到保留边界之下；跨流冲突；IdemKey 解析出不止一笔或尚无订单；或读被上游 Refused → 按 pacing 再读
  ATT --> ATT : 读返回 Unavailable → 该观察流上 Gap{Channel}，再读
  ATT --> N : 目标终态判定成立（目标订单 = VenueRef 的 venue_order_id，或能证明属于 basis 所记订单键腿的记录给出的 venue_order_id；按 §8.1 在它所在的订单状态流上取最近观察（与 orders 同一范围），是终态，剩余量口径下还带 cumulative_filled_quantity；在进入的事务或 append 目标订单记录的事务里求值）→ 同事务 append TargetTerminal{(p,2), observation, evidence, quantity}，quantity > 0
  ATT --> R0 : 判定成立，同事务 append TargetTerminal 且 quantity ≤ 0（剩余量口径且已成交到意图数量）
  ATT --> EX : 意图 deadline 到期（先于判定：到期后不再 append TargetTerminal）
  N --> R : 新腿终结（VenueAccepted / VenueRejected / Undetermined 后收敛）
  R0 --> [*]
  R --> [*]
  EX --> [*]
  note right of ATT
    有界：出口是目标终态或 deadline
    不接受 resolve；期间链仍在阻塞头集合内
    两腿各自 AttemptRef，撤单腿的回执/归因不会终结新单腿的 Undetermined
    新单腿的 submit 带 TargetTerminal.quantity；过门时声明的计划须仍是两腿，否则等待到 deadline
  end note
```

读法：

- 这是 `>>=`：第二腿读目标终态判定选中的那条观察，发生在 IO 壳内，不是单据层两次起单；每条实际发出的腿各过一次发出前门、各至多一条 `SendBarrier`（发出前过期或链已终结的腿没有）、各自记下声明的键与键角色。撤单腿不带键时没有 by-key 渠道；撤单腿都不声明 listing 与成交 / 持仓对账，不带键的撤单腿只由 `Attributed` 或 `Manual` 收敛（§6.6 撤单腿的取证）。
- 撤单腿被拒、缺席或过期 → 链终结、新腿永不发；负责人看观察记录另起单据。
- 崩在撤单腿终态已持久、新腿未 `SendBarrier`（#8）：先 fold 链是否已 `Resolved`（含 `TargetTerminal` 数量 ≤ 0）；已有数量 > 0 的 `TargetTerminal` 则数量取记录、过门后发新腿；否则续 `AwaitingTargetTerminal`，不对已有记录重新判定，此后到达的记录使目标终态判定成立时再 append `TargetTerminal`。

核出：等待目标终态的完整出边（非终态 / 未见 / 不可用 / `deadline`）与腿身份原文没有——已并入 §6.5。

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
  H->>O: Emit(EffectRequest{trade.place, basis, key})（已随 Advance 提交）
  O->>EJ: 同事务 Draft{responsible = 装载 principal, basis ∋ 请求位置} + SubmitForDecision + EffectResponse{Drafted}
  T->>S: AwaitingDecision(v1)
  S->>EJ: 授权 ✓ 输入约束 ✓ 审批（策略不要求人工，rule_version）✓ lane 集合空 ✓ 过期 ✓ 门 ✓（Outcome 带 checked_as_of）
  S->>EJ: 同事务 Prepared @p + Close(Prepared(p)) + RuleState
  IO->>IO: 发出前门：deadline 未过 ∧ 会话已建立 ∧ 意图对当前能力可执行（Place、参数 schema）
  IO->>EJ: durable append SendBarrier(p,1)（fsync）
  IO->>I: submit(attempt (p,1)；WriteLaneKey、Place；意图参数 + 意图参数 schema 身份；idempotency_key)（SendBarrier 记该键为订单键；Place 不带 target）
  I->>V: 上游下单
  V-->>I: 业务回执（受理，venue_order_id）
  I-->>IO: Ack(venue_id, receipt)
  IO->>EJ: 同事务：VenueAccepted{venue_order_id, receipt: Evidence, observation}
  IO->>OJ: 同事务：该回应的观察记录（Receipt{(p,1)}）：订单状态（FromAttempt((p,1))）；回执已含执行则每笔一条成交记录（带 execution_id，归因按各自的关联证据）
  Note over IO: 腿终结 → 链 Resolved → 移出阻塞头集合（集合空 → lane 解除）；basis 引用登记解除
  V-->>I: 部分成交 / 成交推送
  I->>OJ: 观察记录（attribution FromAttempt((p,1))，cumulative_filled_quantity；成交记录带 execution_id）
  OJ-->>A: 依次投递：受理、部分成交、成交（字段与原生身份保真；orders 对同一 execution_id 只计一次）
  A->>A: read_model(orders) 或自 fold → 最终 = 成交
```

读法：从 `Emit` 到 `VenueAccepted` 是一串各自原子的事务（D1.5）、1 次 fsync 屏障、1 次 venue 写调用；此后一切都是观察推送。

核出：无。

## D6.6 `NoResponse` → 取证 → 停等 → 人工（W2）

对照：W2 步 1–4；§6.7 恢复、§6.6 对账驱动；§7.2 第 1/3 步；§8.5 `read`、`resolve`。

```mermaid
sequenceDiagram
  participant IO as 核心（IO 壳 / 控制面）
  participant I as 集成（会话 A → B）
  participant V as venue
  participant EJ as 执行 J
  participant OJ as 观察 J
  participant OP as 运维 principal
  Note over IO,I: 发出前门已过：会话 A 已建立、能力可执行（D6.1）
  IO->>EJ: SendBarrier(p,1)（fsync）
  IO->>I: submit(attempt (p,1))
  alt 核心存活：集成崩溃 / 超时 / 传输 ACK / 5xx / 集成本地不发
    I-->>IO: NoResponse
    IO->>EJ: append Undetermined((p,1), NoResponse)（同事务回查已到达、归因到 (p,1) 的观察）
  else 核心 kill -9
    Note over IO: 重启第 1 步：instance_id += 1；第 2 步：SendBarrier 无后继
    IO->>EJ: append Undetermined((p,1), CrashWindow)（同事务回查）
    Note over IO,I: 重启第 3 步：新会话 B（新 session_seq）建立后，第 4 步才启动对账（无会话不取证、不记 Gap）
  end
  Note over IO: 渠道集 = 该腿声明的渠道（与记录的计划与键角色一致），按固定顺序取证
  alt 有 by-key 能力
    IO->>I: query_by_key(key)
    alt Found(state)
      I-->>IO: Found
      IO->>EJ: ResolutionEvidence{(p,1), ByKey, Found{observation, evidence}}
      IO->>OJ: 同事务 该回应的观察记录(Reconciliation{(p,1), ByKey})
      Note over IO: 腿终结
    else Absent
      I-->>IO: Absent
      IO->>EJ: ResolutionEvidence{(p,1), ByKey, Absent} → 腿终结（本次未发生）
    else Unavailable
      I-->>IO: Unavailable
      IO->>EJ: Gap{Channel, ByKey}；同渠道按 pacing 再发（不换渠道）
    end
  else 无 by-key 能力（该渠道跳过）→ listing 起
    IO->>I: list_open(scope)
    I-->>IO: Listing（核心按归因身份匹配）
    IO->>EJ: 未命中 → ResolutionEvidence{(p,1), Listing, Inconclusive}
    IO->>I: list_fills(scope, since) → 同上；replay_by_key（若开启）→ 同上
    IO->>EJ: 渠道穷尽：停等；腿留在阻塞头集合
    OP->>IO: read([(来源, 该作用域的订单流, 按 venue 身份的请求)])（§8.5；核心 → 集成 read）
    IO->>OJ: 观察记录 @obs（provenance OneShot{origins ∋ Session(OP), request}），另有读结论记录
    OP->>IO: resolve((p,1), Found(obs), note)
    IO->>EJ: 授权 ✓ 腿处于 Undetermined ✓ obs 属该 WriteScope ✓ → ResolutionEvidence{(p,1), Manual, Found{obs}} → 腿终结
  end
  Note over I,V: 迟到回执：旧会话 A 的回执在边界被丢；venue 已受理则 B 以观察记录重送（attribution FromAttempt((p,1)) 或 idempotency_key）
  I->>OJ: 观察记录 → (p,1) 处于 Undetermined → 同事务 EJ: ResolutionEvidence{(p,1), Attributed, Found} → 腿终结
```

读法：末态可枚举（found → `Evidence` + 观察记录 / absent → 未发生 / 无渠道 → 停等）；停等态之后仍有多条收敛入口：带 principal 的 `resolve`、任一时刻到达的 `Attributed Found`、或 `ReconciliationReopened`（撤阻塞头 / 会话重建 / `retry_reconciliation`）重开的自动取证。

核出：无。

## D6.7 同 lane 并发与撤阻塞头（W5）

对照：W5 正常—失败路径与扩展路径；§6.4 lane 步、无第二类越顶队列；§6.2 目标身份来源之二；§6.6 `ReconciliationReopened{CancelLegTerminal}`。

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
  AI->>S: 单据 T3：cancel，target = IdemKey(p1 下单腿 SendBarrier 记为订单键的键)
  S->>S: T3 授权 ✓ 输入约束 ✓（来源接受 IdemKey 目标；否则 TargetNotAccepted，T3 关闭，集合仍 {p1}）审批 ✓ lane 步不等待（唯一写例外）✓ 过期 ✓ 门 ✓
  S->>IO: Prepared @p3（阻塞头集合 = {p1, p3}）
  IO->>I: SendBarrier(p3,1) → cancel(target = p1 的订单键)
  I-->>IO: Ack → VenueAccepted((p3,1))（撤单腿终结，不决议 p1）
  IO->>IO: append ReconciliationReopened{(p1,1), CancelLegTerminal((p3,1))}
  Note over IO: 重走一轮：按能力证据声明的渠道从头取证（D6.2）
  alt 有 by-key 能力
    IO->>I: query_by_key(p1 下单腿自己的键)
    alt 读到目标（已撤或任何状态）
      I-->>IO: Found → ResolutionEvidence{(p1,1), ByKey, Found} → 腿终结
    else ByKey 明确否定
      I-->>IO: Absent → ResolutionEvidence{(p1,1), ByKey, Absent} → 腿终结
    else Unavailable
      I-->>IO: Unavailable → Gap{Channel, ByKey}；同渠道按 pacing 再发（不换渠道）
    end
  else 无 by-key 能力（该渠道跳过）
    IO->>I: list_open(scope)
    I-->>IO: Listing 未见目标 → Inconclusive（F10）→ 下一渠道；穷尽仍停等
  end
  Note over S: 集合清空 → lane 解除 → T2 放行，先过期步再过门 → Prepared @p2
  Note over UI,I: 另一 lane（不同账户）的写全程不等待
```

读法：撤单让 venue 侧到达一个读得出的终态，并触发阻塞头重开一轮取证；阻塞头仍只由取证收敛，撤单不提升任何渠道的证明力（listing 未见仍是 `Inconclusive`）。T3 的 `target` 必须是 p1 某条腿 `SendBarrier` 记为订单键的键（按腿精确匹配）；只能按 venue 订单身份撤单的来源上，T3 在输入约束步以 `TargetNotAccepted` 关闭，不会成为第二条 `Undetermined`。两条路径（撤阻塞头 / `bypass_lane`）都让该 lane 出现两条 `SendBarrier`，区别只在有无 `bypass_lane` 控制记录（它不是 Decision）。

核出："读不到即 `Absent`"的原表述违反 F10——已改为 by-key 否定才 `Absent`（§6.4、W5）。
