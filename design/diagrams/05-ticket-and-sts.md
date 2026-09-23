# 05 单据与决策代数 STS

对照：§6.2、§6.3、§6.4、§5.2、§6.1 写处理器、§8.5 单据组、W5、W11、W18。索引见 `README.md`。

## D5.1 单据状态机

对照：§6.2 `TicketAction`、穷尽转移表、与写边界的接口。

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

对照：§6.1 写处理器、§8.5 单据组、§7.6 `deadline` 缺省、§8.1 意图锚点。

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

核出：程序意图无编辑期这一点原文未写——已并入 §6.1（写处理器 `Draft` + `SubmitForDecision` 同事务）。

## D5.3 STS 顺序固定链（从 `AwaitingDecision` 到 `Prepared`）

对照：§6.3 五步表、放行门、规则版本变更；§6.2 门只看必要项；§5.2 `basis_validity`。

```mermaid
flowchart TB
  IN[("单据 AwaitingDecision(current_version)")]
  IN --> A{"授权<br/>(responsible, WriteLaneKey, OperationKind) ∈ scope？"}
  A -->|"否"| RJ1["Rejection::Unauthorized + 安全事件<br/>Close(DecisionRejected)"]
  A -->|"是"| B{"输入约束<br/>守卫字段：instrument 属账户、数量为正、子账户已枚举、阈值<br/>（步内可交换集，Validated 累积）"}
  B -->|"否"| RJ2["NonEmpty<Rejection>（组合子 kind 包装）<br/>Close(DecisionRejected)"]
  B -->|"是"| C{"审批<br/>策略要求人工？"}
  C -->|"是"| WAITC["等待 decide(Approve / Reject)<br/>待决集合对审批人可见（读模型 tickets）<br/>同 (ticket, current_version) 第二条 decide → Conflict(AlreadyDecided)"]
  WAITC -->|"Reject"| RJ3["Close(DecisionRejected)"]
  WAITC -->|"Approve（绑定版本；决定者按动作种类授权）"| D
  C -->|"否：以 rule_version 为依据通过"| D
  D{"lane<br/>该 WriteLaneKey 阻塞头集合非空？"}
  D -->|"非空，且本笔不是以阻塞头幂等键为 target 的撤单"| WAITD["停在 lane 步，单据仍 AwaitingDecision<br/>期间 alignment 照常重算"]
  WAITD -->|"集合清空（fold 变化）"| E
  D -->|"空 / 是撤阻塞头意图 / bypass_lane Decision"| E
  E{"过期步<br/>deadline（UTC）已过？"}
  E -->|"是"| RJ5["Close(Expired)：不补偿"]
  E -->|"否"| G
  G{"依据有效性门<br/>basis_validity == Fresh ∧ 必要项 alignment == Aligned（能力项恒必要）∧ 决定绑定版本 == current_version"}
  G -->|"否"| RJ4["PredicateFailure（fail-closed）<br/>Rejection 带 rule_version + checked_as_of<br/>Close(DecisionRejected)"]
  G -->|"是"| OUT[("同事务 append Prepared + Close(Prepared(position))<br/>+ Outcome（带 rule_version、checked_as_of）+ RuleState")]
  TMR["计时器（Input 超时）"] -.->|"AwaitingDecision 任一等待点到期"| RJ5
  RL["reload_config(rules)"] -.->|"待决单据放行时按当时规则从授权步重过五步：已有 Decision 仍绑定版本，决定者授权与是否需人工按新规则重判"| A
```

读法（假想运行时）：

- 链是事件驱动的：`SubmitForDecision` 跑到第一个等待点；`decide`、阻塞头集合清空、超时各自把它往下推一步；每推一步把该步产生的记录（`Vec<Outcome>` 或 `NonEmpty<Rejection>`）与 `RuleState` 同事务持久化。
- 等待都发生在 `Prepared` 之前：单据在等，不是已放行的记录在等；所以正常路径下同 lane 至多一条未终结 Attempt。
- 过期步是第五步：等待结束后先看 `deadline` 再进门；计时器只是让等待中的单据也能到期，不是让过期单据仍能 `Prepared` 的旁路。
- 门读的是单据 fold 已算好的字段，规则自己不算；世界变了单据先变 `Diverged`，审批人看得到，放行时门自然失败。
- 重启后链从 `RuleState` 续跑：停在审批步的仍等 `decide`；停在 lane 步的等集合清空；计时器按 `deadline`（UTC）重装（D1.2 第 4 步）。

核出：授权步的主体（`responsible`）、不要求人工时审批步的依据（`rule_version`）、门失败的去向（`Close(DecisionRejected)`）、一个版本至多一条 Decision（`Conflict(AlreadyDecided)`）、Decision/`Outcome` 携带 `checked_as_of` 原文未写——已并入 §6.3。

## D5.4 lane 阻塞头

对照：§6.4 lane、队首阻塞协议语义、无第二类越顶队列、显式绕过；不变量 §6.9-1；§6.5 `Resolved`。

```mermaid
stateDiagram-v2
  state "Free：集合为空" as Free
  state "Blocked{p}：正常路径，一条未终结 Attempt" as B1
  state "Blocked{p, q, …}：例外扩大的集合" as BN
  [*] --> Free
  Free --> B1 : STS 放行一笔 → Prepared（IO 壳紧接执行）
  B1 --> Free : 该 Attempt 链 Resolved（各腿终结且无下一腿）
  B1 --> BN : 例外一：以阻塞头幂等键为 target 的撤单意图放行（lane 步不等待）
  B1 --> BN : 例外二：bypass_lane(ticket) Decision（principal 承担协议违反）
  BN --> BN : 集合内任一链 Resolved 但集合非空（只移出自己）
  BN --> BN : 集合非空时再加入获准的例外（再一笔撤阻塞头 / 再一次绕过）
  BN --> Free : 集合清空
  note right of B1
    未终结 = Prepared 无 SendBarrier / SendBarrier 无后继 /
    Undetermined 未终结 / 复合链未完（含 AwaitingTargetTerminal）
    后续普通写停在 STS lane 步（AwaitingDecision）
    其他 lane 不受影响
  end note
  note right of BN
    IO 壳按 Prepared 位置顺序执行集合内每条，各自对账独立收敛
    撤单腿回执不直接决议阻塞头：撤单腿终结于 VenueAccepted / Found 且阻塞头仍 Undetermined 未终结 → ReconciliationReopened{CancelLegTerminal} → 阻塞头重走一轮取证
    by-key 读到目标 → Found；by-key 否定 → Absent；listing 未见仍 Inconclusive（F10）
  end note
```

读法：阻塞不是锁——后续写的语义依赖队首结果（buying power、待撤订单是否存在、venue 侧顺序），所以是与 venue 的通讯协议语义。撤阻塞头的意图不依赖队首结果，它存在的目的就是让队首结果可判定，所以是唯一不等待的写。任何终结只移出自己；集合为空才放行普通写。

核出：集合语义曾按"单条终结即解除"表述——已统一为集合（§6.4、§6.5、§8.5、§10.5 #3）。

## D5.5 两层对账状态的重算触发

对照：§6.2 偏离是状态、两层分开的原因、`InputMissing` 可行动；§5.2 `basis_validity`；§8.3 能力变更。

```mermaid
flowchart LR
  subgraph TRIG["触发（都不是 TicketAction）"]
    T1["basis 引用的流推进 / 被撤回 / 出现 gap"]
    T1b["required_inputs 流被撤回 / 出现 gap"]
    T2["required_inputs 流有新观察（推送、回填、一次性读、回执/取证副本）"]
    T3["能力变更推送 / CapabilityObserved"]
    T4["保留边界推进"]
    T5a["reload_config(rules)：Lag 变化"]
    T5b["reload_config(rules)：必要项集变化"]
  end
  subgraph L1["第一层 basis_validity（只依赖位置）"]
    V["basis_valid(basis, world, Lag) →<br/>Fresh / Stale(Lag) / Retracted(pos) / BeyondRetention(pos)<br/>比较切面：各流完备位置（D8.3）"]
  end
  subgraph L2["第二层 alignment（按 Intent 分派的检查集）"]
    IM{"该项 required_inputs 各流观察侧都存在？"}
    CK["每项 AlignmentCheck.eval(intent, 各流当前流末 fold_state) →<br/>CheckResult：Aligned / Diverged(Divergence) / Undecidable(Gap)<br/>记 checked_as_of = 实际消费的位置集"]
    MISS["IntentAlignment 该项 = InputMissing(缺哪些 StreamKind)"]
    CAP["能力项：(WriteLaneKey, OperationKind) Supported → Aligned，否则 Diverged；恒为必要项"]
  end
  T1 --> V
  T4 --> V
  T5a --> V
  T1b --> IM
  T2 --> IM
  IM -->|"是"| CK
  IM -->|"否"| MISS
  T3 --> CK
  T3 --> CAP
  T5b --> GATE
  V --> GATE{"门：Fresh ∧ 必要项 Aligned"}
  CK --> GATE
  MISS --> GATE
  CAP --> GATE
  MISS -->|"venue 有一次性读能力"| RD["策略可选先查后判：read(...) → 观察记录（one_shot，不推进完备进度）→ 该项重算"]
  RD --> T2
  GATE -->|"否"| DIV["审批人看到 Diverged 单据；放行时 PredicateFailure"]
  GATE -->|"是"| OK["可进 prepare；Outcome 记 checked_as_of"]
```

读法：第一层只看位置（对所有单据可算），第二层看值（依赖意图类型与 venue 给的观察）。第二层读的是各流**当前流末**的 `fold_state`（含 `one_shot`/`backfilled` 记录，各带质量标记），不是完备位置处的截断值——否则为补齐输入读来的一次性观察永远看不见；也不是 `basis` 位置处的旧值——否则世界变了单据不会变。每次评估消费的位置集记为 `checked_as_of`，进入放行/否决记录。

核出：第二层取值位置原文写作"最新完备进度处"，会看不见一次性读——已改为当前流末（§6.2）；`checked_as_of` 进 Decision/`Outcome` 原文未写——已并入 §6.2、§6.3。

## D5.6 人工审批、过期与版本冲突（W18）

对照：W18 步 3–5、§6.2 决定绑定 `current_version`、§8.5 `decide`、C11。

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
  S->>S: 第一条：Decision 记录（绑定 v3）
  S-->>P2: 第二条：(ticket, v3) 已有 Decision → Conflict(AlreadyDecided)，不执行（即使单据仍 AwaitingDecision）
  alt lane 阻塞头集合非空
    S->>S: 停在 lane 步，单据仍 AwaitingDecision(v3)，版本不变；此时任何同版本 decide 同样 Conflict(AlreadyDecided)
    Note over S: 集合清空
  end
  S->>S: 过期步 ✓ → 门 ✓（checked_as_of）
  S->>T: 同事务 Prepared + Close(Prepared(pos)) + Outcome
  T->>IO: Prepared @pos（D6.1）
  Note over R,IO: 另一笔：deadline 到期仍无 Decision
  S->>T: 过期步 Input 超时 → Close(Expired)，无 Prepared / SendBarrier
```

读法：冲突不是锁：一个 `current_version` 至多一条 Decision，第二个决定返回冲突记录即可；`Closed` 不是挡第二条决定的条件——lane 等待期间单据仍开着、版本未变。

核出：同版本第二次决定在 lane 等待窗口内无判据——已并入 §6.3 审批步（`Conflict(AlreadyDecided)`）。
