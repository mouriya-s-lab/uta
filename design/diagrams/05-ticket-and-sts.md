# 05 单据与决策代数 STS

对照：§5.2、§5.3、§4、§5.1 写处理器、§6.4 单据组、W5、W11、W18。索引见 `README.md`。

## D5.1 单据状态机

对照：§5.2 `TicketAction`、穷尽转移表、与写边界的接口。

```mermaid
stateDiagram-v2
  state "Closed(Prepared)：命运属 Attempt 链" as CP
  state "Closed(DecisionRejected)" as CR
  state "Closed(Expired)" as CE
  state "Closed(Withdrawn)" as CW
  [*] --> Drafting : Draft（取锁 = responsible 存在）
  Drafting --> Drafting : Revise（仅 responsible，版本链前进）
  Drafting --> Drafting : Transfer（responsible 变更，记录在案）
  Drafting --> AwaitingDecision : SubmitForDecision（冻结 current_version）
  AwaitingDecision --> AwaitingDecision : Transfer
  AwaitingDecision --> Drafting : SendBack（responsible 不变）
  AwaitingDecision --> CP : Close(Prepared)，与 Prepared 同事务
  AwaitingDecision --> CR : Close(DecisionRejected)，由决定者
  AwaitingDecision --> CE : Close(Expired)，由 STS 过期步
  Drafting --> CW : Close(Withdrawn)，由 responsible
  AwaitingDecision --> CW : Close(Withdrawn)
  CP --> [*]
  CR --> [*]
  CE --> [*]
  CW --> [*]
  note right of AwaitingDecision
    期间 Revise 被拒（决定绑定 current_version）
    basis_validity 与 alignment 照常重算（偏离是状态）
    停在审批步或 lane 步都在此态
  end note
  note left of Drafting
    没有过期：草稿不占 lane、不在 STS 链上
    失联草稿 = 强制 Transfer 后 Withdrawn
  end note
```

读法：

- 锁 = `responsible` 字段：不是互斥原语，没有超时释放；失联由规则处理（过期或强制 `Transfer`）。
- `Closed` 无出边，一张单据至多一次 `Close(Prepared)`；改单撤单各自起新单据。
- 其他 principal 的 `Revise` 直接被拒，不排队不分叉；协作在核心之外（移交、另起单据）。

核出：`Drafting → Close(Expired)` 上一轮已删（无触发者）。

## D5.2 意图从哪来、怎么开单

对照：§5.1 写处理器、§6.4 单据组、§6.7.3 `deadline` 缺省、§6.3.2 意图锚点。

```mermaid
flowchart LR
  subgraph SRC["三种读的副作用消费为写"]
    AI["Alice 会话（AI 或人）<br/>principal = (os_user, actor)"]
    PROG["程序解释② Emit(EffectRequest)"]
    APPR["审批人 decide(Approve)"]
  end
  AI -->|"draft(intent) → TicketId<br/>revise / submit_for_decision"| NORM
  PROG -->|"写处理器：装载 principal 为 responsible<br/>Draft + SubmitForDecision 同事务"| NORM
  NORM["意图规范化（parse-don't-validate）<br/>锚点：principal · WriteLaneKey · OperationKind · basis（可空）<br/>撤/改单必带 target: VenueRef 或 IdemKey<br/>deadline 缺省按 (WriteLaneKey, OperationKind) 策略 → 运行期全局，填入版本"]
  NORM --> TK[("TicketAction 记录：Draft / SubmitForDecision")]
  TK --> STS["STS 顺序固定链（D5.3）"]
  APPR -.->|"Decision 记录（绑定 current_version）"| STS
```

读法：三种来源的记录同形（principal + 依据位置）；差别只在授权规则里"哪些 principal 的记录足以进 prepare"。

核出：程序意图无编辑期这一点原文未写——已并入 §5.1（写处理器 `Draft` + `SubmitForDecision` 同事务）。

## D5.3 STS 顺序固定链（从 `AwaitingDecision` 到 `Prepared`）

对照：§5.3 五步表、放行前依据有效性门；§5.2 门只看必要项；§4 `basis_validity`；§6.4 规则不冻结。

```mermaid
flowchart TB
  IN[("单据 AwaitingDecision(current_version)")]
  IN --> A{"授权<br/>(responsible, WriteLaneKey, OperationKind) ∈ scope？"}
  A -->|"否"| RJ1["Rejection::Unauthorized + 安全事件<br/>Close(DecisionRejected)"]
  A -->|"是"| B{"输入约束<br/>守卫字段：instrument 属账户、数量为正、子账户已枚举、阈值<br/>（步内可交换集，Validated 累积）"}
  B -->|"否"| RJ2["NonEmpty<Rejection>（组合子 kind 包装）<br/>Close(DecisionRejected)"]
  B -->|"是"| C{"审批<br/>策略要求人工？"}
  C -->|"是"| WAITC["等待 decide(Approve / Reject)<br/>待决集合对审批人可见（读模型 tickets）"]
  WAITC -->|"Reject"| RJ3["Close(DecisionRejected)"]
  WAITC -->|"Approve（绑定版本；决定者按动作种类授权）"| D
  C -->|"否：以 rule_version 为依据通过"| D
  D{"lane<br/>该 WriteLaneKey 有未终结 Attempt？"}
  D -->|"有，且本笔不是以阻塞头幂等键为 target 的撤单"| WAITD["停在 lane 步，单据仍 AwaitingDecision<br/>期间 alignment 照常重算"]
  WAITD -->|"阻塞头 Resolved（fold 变化）"| G
  D -->|"无 / 是撤阻塞头意图 / bypass_lane Decision"| G
  G{"依据有效性门<br/>basis_validity == Fresh ∧ 必要项 alignment == Aligned（能力项恒必要）∧ 决定绑定版本 == current_version"}
  G -->|"否"| RJ4["PredicateFailure（fail-closed）<br/>Close(DecisionRejected)"]
  G -->|"是"| OUT[("同事务 append Prepared + Close(Prepared(position))<br/>+ Outcome + RuleState")]
  E["过期步（Input 超时，deadline UTC）"] -.->|"AwaitingDecision 任一等待点到期"| RJ5["Close(Expired)：不补偿"]
  RL["reload_config(rules)"] -.->|"待决单据放行时按当时规则重过五步"| A
```

读法（假想运行时）：

- 链是事件驱动的：`SubmitForDecision` 跑到第一个等待点；`decide`、阻塞头 `Resolved`、超时各自把它往下推一步；每推一步 append 一条记录并更新 `RuleState`（同事务）。
- 等待都发生在 `Prepared` 之前：单据在等，不是已放行的记录在等；所以同 lane 至多一条未终结 Attempt。
- 门读的是单据 fold 已算好的字段，规则自己不算；世界变了单据先变 `Diverged`，审批人看得到，放行时门自然失败。
- 重启后链从 `RuleState` 续跑：停在审批步的仍等 `decide`；停在 lane 步的等阻塞头；计时器按 `deadline`（UTC）重装（D1.2 第 4 步）。

核出：授权步的主体（`responsible`）、不要求人工时审批步的依据（`rule_version`）、门失败的去向（`Close(DecisionRejected)`，与其他步否决同形）原文未写——已并入 §5.3。

## D5.4 lane 阻塞头

对照：§5.3 lane、队首阻塞协议语义、无第二类越顶队列、显式绕过；§2.5 不变量 1；§5.4 `Resolved`。

```mermaid
stateDiagram-v2
  [*] --> Free : lane 上无未终结 Attempt
  Free --> Blocked1 : STS 放行一笔 → Prepared（IO 壳紧接执行）
  Blocked1 --> Free : 该 Attempt 达终态（VenueAccepted / VenueRejected / Expired / Undetermined 后 Found 或 Absent）
  Blocked1 --> BlockedN : 例外一：以阻塞头幂等键为 target 的撤单意图放行（lane 步不等待）
  Blocked1 --> BlockedN : 例外二：bypass_lane(ticket) Decision（principal 承担协议违反）
  BlockedN --> BlockedN : 集合内任一 Attempt Resolved 但集合非空
  BlockedN --> Free : 集合清空
  note right of Blocked1
    阻塞头 = 未终结 Attempt：
    Prepared 无 SendBarrier / SendBarrier 无后继 / Undetermined 未 Resolved
    后续意图停在 STS lane 步（AwaitingDecision）
    其他 lane 不受影响
  end note
  note right of BlockedN
    IO 壳按 Prepared 位置顺序执行集合内每条，各自对账独立收敛
    撤单腿回执不直接决议阻塞头；阻塞头仍由取证收敛（撤后读到已撤 → Found，读不到 → Absent）
  end note
```

读法：阻塞不是锁——后续写的语义依赖队首结果（buying power、待撤订单是否存在、venue 侧顺序），所以是与 venue 的通讯协议语义。撤阻塞头的意图不依赖队首结果，它存在的目的就是让队首结果可判定，所以是唯一不等待的写。

核出：无（撤阻塞头例外上一轮已并入 §5.3）。

## D5.5 两层对账状态的重算触发

对照：§5.2 偏离是状态、两层分开的原因、`InputMissing` 可行动；§4 `basis_validity`；§6.3.6 能力变更。

```mermaid
flowchart LR
  subgraph TRIG["触发（都不是 TicketAction）"]
    T1["basis 引用的流推进 / 被撤回 / 出现 gap"]
    T2["required_inputs 流有新观察（含一次性读结果）"]
    T3["能力变更推送 / CapabilityObserved"]
    T4["保留边界推进"]
    T5["reload_config(rules)：必要项集 / Lag 变化"]
  end
  subgraph L1["第一层 basis_validity（只依赖位置）"]
    V["basis_valid(basis, world, Lag) →<br/>Fresh / Stale(Lag) / Retracted(pos) / BeyondRetention(pos)"]
  end
  subgraph L2["第二层 alignment（按 Intent 分派的检查集）"]
    CK["每项 AlignmentCheck.eval(intent, 当前 fold_state) →<br/>Aligned / Diverged / Undecidable(Gap) / InputMissing(缺哪些流)"]
    CAP["能力项：(WriteLaneKey, OperationKind) Supported → Aligned，否则 Diverged；恒为必要项"]
  end
  T1 --> V
  T4 --> V
  T2 --> CK
  T3 --> CK
  T3 --> CAP
  T5 --> GATE
  V --> GATE{"门：Fresh ∧ 必要项 Aligned"}
  CK --> GATE
  CAP --> GATE
  CK -->|"InputMissing 且 venue 有一次性读能力"| RD["策略可选先查后判：read(...) → 观察记录 → 该项重算"]
  RD --> T2
  GATE -->|"否"| DIV["审批人看到 Diverged 单据；放行时 PredicateFailure"]
  GATE -->|"是"| OK["可进 prepare"]
```

读法：第一层只看位置（对所有单据可算），第二层看值（依赖意图类型与 venue 给的观察）。第二层读的是各流**当前**的 `fold_state`，不是 `basis` 位置处的旧值——否则世界变了单据不会变。

核出：第二层读当前值这一点原文写作"经 basis 读到的观察值"，与"世界变了单据变 Diverged"不一致——已改为读 `required_inputs` 各流当前 `fold_state`（§5.2）。

## D5.6 人工审批、过期与版本冲突（W18）

对照：W18 步 3–5、§5.2 决定绑定 `current_version`、§6.4 `decide`、C11。

```mermaid
sequenceDiagram
  participant R as responsible（AI）
  participant T as 单据 fold
  participant S as STS 链
  participant P as 审批人 P1
  participant P2 as 审批人 P2
  participant IO as IO 壳
  R->>T: submit_for_decision(ticket, v3)
  T->>S: AwaitingDecision(v3)
  S->>S: 授权 ✓ → 输入约束 ✓ → 审批：策略要求人工 → 等待
  Note over T: 待决集合对审批人可见（读模型 tickets，带 alignment）
  par 两人对同一版本决定
    P->>S: decide(ticket, expected_version = v3, Approve)
    P2->>S: decide(ticket, expected_version = v3, Approve)
  end
  S->>S: 第一条：Decision 记录（绑定 v3）→ lane ✓ → 门 ✓
  S->>T: 同事务 Prepared + Close(Prepared(pos))
  S-->>P2: 第二条：单据已 Closed，版本已前进 → Conflict，不执行
  T->>IO: Prepared @pos（D6.1）
  Note over R,IO: 另一笔：deadline 到期仍无 Decision
  S->>T: 过期步 Input 超时 → Close(Expired)，无 Prepared / SendBarrier
```

读法：冲突不是锁：第二个决定建立在过期的读上（依据版本不匹配），返回冲突记录即可；不需要互斥。

核出：无。
