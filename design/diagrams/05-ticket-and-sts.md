# 05 单据与决策代数 STS

对照：§6.2（含参数合规、交易协议）、§6.3（含冷却）、§6.4、§5.2、§6.1 写处理器、§7.6 规则文件、§8.5 单据组、W5、W11、W18。索引见 `README.md`。

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
  AwaitingDecision --> AwaitingDecision : Transfer（responsible 变更；STS 链按新负责人从授权步起重过，D5.3）
  AwaitingDecision --> Drafting : SendBack（responsible 不变）
  AwaitingDecision --> CP : Close(Prepared)，与 Prepared 同事务
  AwaitingDecision --> CR : Close(DecisionRejected)，由决定者或 STS 链
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
    停在输入约束步、审批步、lane 步或放行门前都在此态
  end note
  note left of CR
    决定者否决，或 STS 链否决：
    参数不合规、允许集合、冷却、放行门
  end note
  note left of Drafting
    没有过期：草稿不占 lane、不在 STS 链上
    失联草稿 = 强制 Transfer 后 Withdrawn
    参数不合规不挡草稿：parameter_validity 从 Draft 起可见
  end note
```

读法：

- 锁 = `responsible` 字段：不是互斥原语，没有超时释放；失联由规则处理（过期或强制 `Transfer`）。
- `Closed` 无出边，一张单据至多一次 `Close(Prepared)`；改单撤单各自起新单据。
- 其他 principal 的 `Revise` 直接被拒，不排队不分叉；协作在核心之外（移交、另起单据）。

核出：`Drafting → Close(Expired)` 上一轮已删（无触发者）。

## D5.2 意图从哪来、怎么开单

对照：§6.1 写处理器（构造规则、发出成员）、§6.2 参数合规与交易协议、§8.5 单据组、§7.6 `deadline` 缺省、§8.1 意图锚点。

```mermaid
flowchart LR
  subgraph SRC["三种读的副作用消费为写"]
    AI["下游会话（经解释层；AI 或人）<br/>principal = (os_user, actor)"]
    PROG["程序解释② Emit(EffectRequest)"]
    APPR["审批人 decide(Approve)"]
  end
  AI -->|"draft(intent) → TicketId<br/>revise / submit_for_decision"| NORM
  PROG -->|"写处理器：发出成员的装载 principal 为 responsible<br/>Draft + SubmitForDecision 同事务"| NORM
  NORM{"意图构造（parse-don't-validate）<br/>锚点：principal · WriteLaneKey · OperationKind ∈ {Place, Cancel, Replace, Close} · basis（可空）<br/>撤/改单必带 target: VenueRef 或 IdemKey（IdemKey 须是该作用域内某次尝试 SendBarrier 记为订单键的键）；平仓的 target: PositionRef（含 instrument）由核心从 basis 所指的持仓观察记录构造<br/>deadline 缺省按 (WriteLaneKey, OperationKind) 策略 → 运行期全局，填入版本"}
  NORM -->|"锚点构造不出"| MAL["会话：draft → Rejected(Malformed)，不 append；revise 使下一版构造不出 → Rejected(Malformed)，单据不变<br/>程序：EffectResponse{NotDrafted(Malformed)}，不重派"]
  NORM -->|"构造成功，下游会话（参数不在此判定）"| TK[("TicketAction 记录：Draft / SubmitForDecision<br/>每版带意图参数 schema 身份")]
  NORM -->|"构造成功，程序（参数不在此判定）"| SCOPE{"仅程序来源：作用域判定<br/>意图 WriteLaneKey 所属 (来源, 作用域) 被发出成员 Applied 所记的执行事实输入选中？"}
  SCOPE -->|"否"| NSO["EffectResponse{NotDrafted(ScopeNotObserved)}<br/>不开单、无 TicketAction"]
  SCOPE -->|"是"| TK
  TK -.-> PV["单据 fold：parameter_validity<br/>按该来源会话有效声明的意图参数 schema 与目标种类：Valid / Invalid(违反项) / CapabilityNotEstablished（Unknown 或无会话，非终结）/ NotSupported / SchemaMismatch / TargetNotAccepted"]
  TK --> STS["STS 顺序固定链（D5.3）"]
  APPR -.->|"Decision 记录（绑定 current_version）"| STS
```

读法：开单的记录对各来源同形（principal + 依据位置）；差别只在授权规则里"哪些 principal 的记录足以进 prepare"，以及程序来源在构造成功之后多一步作用域判定：意图的作用域不在发出成员 `Applied` 所记的执行事实输入之内即 `NotDrafted(ScopeNotObserved)`，不开单（§6.1 构造规则、发出成员）。这一步只读 UTA 自己的事实（发出成员的 `Applied` 与请求锚点），不看参数。参数合规对所有来源同一规则：不挡开单，送审时由输入约束步读取（D5.3）。

核出：程序意图无编辑期这一点原文未写——已并入 §6.1（写处理器 `Draft` + `SubmitForDecision` 同事务）。

## D5.3 STS 顺序固定链（从 `AwaitingDecision` 到 `Prepared`）

对照：§6.3 五步表、等待与重入、放行门、规则版本变更、移交；§6.2 门只看必要项；§5.2 `basis_validity`。

```mermaid
flowchart TB
  IN[("单据 AwaitingDecision(current_version)")]
  IN --> A{"授权<br/>(responsible, WriteLaneKey, OperationKind) ∈ scope？"}
  A -->|"否"| RJ1["Rejection::Unauthorized + 安全事件<br/>Close(DecisionRejected)"]
  A -->|"是"| B{"输入约束（步内可交换集，Validated 累积）<br/>parameter_validity == Valid（无条件；含可执行性：Supported、参数 schema、目标种类）<br/>子账户已枚举、instrument ∈ 策略允许集合（允许集合对 Cancel 不适用：它没有 instrument）<br/>instrument 是否属于该账户不在此判：由上游拒绝"}
  B -->|"否"| RJ2["NonEmpty<Rejection>：全部违反项（参数违反 / NotSupported / SchemaMismatch / TargetNotAccepted 可区分）<br/>Close(DecisionRejected)"]
  B -->|"parameter_validity == CapabilityNotEstablished"| WAITB["停在输入约束步，单据仍 AwaitingDecision，不产生记录<br/>声明版本 / CapabilityObserved / 会话进入或离开 Established 时重新求值"]
  WAITB -->|"能力确立"| B
  B -->|"是"| C{"审批<br/>这一版已有批准的 Decision？（规则变更或移交之后重过时仍算数）<br/>没有时看策略：总是 / 从不 / 名义 > N（以数量定量 = 需人工；Cancel 只可总是 / 从不）"}
  C -->|"已有批准的 Decision：仍算数，不再等"| D
  C -->|"已有 Decision，而规则变更后原决定者已无该授权：审批步否决"| RJ3
  C -->|"没有 Decision，需人工"| WAITC["等待 decide(Approve / Reject)<br/>待决集合对审批人可见（读模型 tickets）<br/>同 (ticket, current_version) 第二条 decide → Conflict(AlreadyDecided)"]
  WAITC -->|"Reject"| RJ3["Close(DecisionRejected)"]
  WAITC -->|"Approve（绑定版本；决定者按动作种类授权）"| D
  C -->|"没有 Decision，不需人工：以 rule_version 为依据通过，本步对 current_version 的 Outcome，不写 Decision；当前规则仍不要求人工时 decide 被拒、不 append"| D
  D{"lane<br/>该 WriteLaneKey 阻塞头集合非空？"}
  D -->|"非空，且本笔既不是以阻塞头中某次尝试记为订单键的调用方键为 target 的撤单，也没有覆盖当前全部阻塞头的 bypass_lane 控制记录"| WAITD["停在 lane 步，单据仍 AwaitingDecision<br/>期间 alignment 照常重算"]
  WAITD -->|"该 lane 执行事实或 bypass_lane 提交：重跑 lane 步"| D
  D -->|"空 / 是撤阻塞头意图 / bypass_lane 控制记录（本版本、所记阻塞头）覆盖当前全部阻塞头"| CD
  CD{"冷却（经 lane 步放行时判定，重入时重判；bypass 不豁免）<br/>Place / Replace 且 now < (WriteLaneKey, instrument) 最近下单写 SendBarrier 时间 + 该 (WriteLaneKey, OperationKind) 的间隔？<br/>或同键有绕过产生、尚未越过屏障的下单写？"}
  CD -->|"是"| RJ6["Rejection::Cooldown{until}<br/>Close(DecisionRejected)"]
  CD -->|"否 / 未配置"| E
  E{"过期步<br/>deadline（UTC）已过？"}
  E -->|"是"| RJ5["Close(Expired)：不补偿"]
  E -->|"否"| G
  G{"依据有效性门<br/>basis_validity == Fresh ∧ 必要项 alignment == Aligned（能力项恒必要，按同一可执行性谓词核对会话有效能力）∧ 审批依据绑定 current_version（有批准的 Decision：它绑定的版本 == current_version，规则变更或移交之后仍算数；没有 Decision（不需人工）：审批步对 current_version 的 Outcome）"}
  G -->|"否"| RJ4["PredicateFailure（fail-closed）<br/>Rejection 带 rule_version + checked_as_of<br/>Close(DecisionRejected)"]
  G -->|"能力项未确立（Unknown 或无会话）"| WAITG["停在放行门，单据仍 AwaitingDecision，不产生记录；同 WAITB 的事件重新求值"]
  WAITG -->|"能力确立：从 lane 步起重跑（lane → 冷却 → 过期 → 门），不直接进 Prepared"| D
  G -->|"是"| OUT[("同事务 append Prepared + Close(Prepared(position))<br/>+ Outcome（带 rule_version、checked_as_of）")]
  TMR["deadline 计时器"] -.->|"AwaitingDecision 任一等待点到期（审批、lane、能力未确立）"| RJ5
  RL["reload_config(rules)"] -.->|"待决单据放行时按当时规则从授权步重过五步：已有 Decision 仍绑定版本，决定者授权与是否需人工按新规则重判"| A
  TR["AwaitingDecision 中的 Transfer"] -.->|"事务提交后按新负责人从授权步重过五步：授权与允许集合按新负责人；已有绑定 current_version 的 Decision 仍算数，没有时按新负责人的规则判是否需人工（需要 → 等 Decision；不需要 → 本步对这一版的 Outcome）"| A
```

读法（假想运行时）：

- 链是事件驱动的：`SubmitForDecision` 跑到第一个等待点；`decide`、该 lane 上的新执行事实、能力或会话变化、`reload_config(rules)`、`Transfer`、`deadline` 到时各自让它重新求值（§6.3 等待与重入）；每推一步只持久化该步产生的记录（`Vec<Outcome>` 或 `NonEmpty<Rejection>`）。规则判断所用的状态（`RuleState`）每次由记录 fold 出，不另存。
- 重入都经过 lane 步：停在审批步、lane 步或放行门前的单据从 lane 步起重跑，停在输入约束步的从输入约束步起重跑，依次过 lane（阻塞头）→ 冷却 → 过期 → 门；没有从放行门前的等待直达 `Prepared` 的边。同 lane 两张单据离线时都停在门前，会话恢复后先放行的那张成为阻塞头，另一张停在 lane 步（§6.3 等待与重入）。规则版本变更与 `Transfer` 从授权步起重过（`RL`、`TR` 两条边）：授权、允许集合与是否需人工都以负责人为主体，换了负责人，此前各步的通过不再作依据（§6.3 移交）。
- 审批、lane 与能力未确立的等待都发生在 `Prepared` 之前：单据在等，不是已放行的记录在等；停在门前的单据也不是阻塞头，所以才要重过 lane 步。已放行的尝试只会在发出前门等会话或能力（D6.1）。
- 过期步是第五步：每次重跑都先看 `deadline` 再进门；计时器只是让等待中的单据也能到期，不是让过期单据仍能 `Prepared` 的旁路。
- 门读的是单据 fold 已算好的字段，规则自己不算；世界变了单据先变 `Diverged`，审批人看得到，放行时门自然失败。
- 重启后链按记录重新求值：停在审批步的仍等 `decide`；停在 lane 步的等集合清空；停在能力未确立的等能力确立；计时器按 `deadline`（UTC）重装（D1.2 第 4 步）；冷却时钟由 `SendBarrier` 记录 fold 出。
- 冷却的时钟在 `SendBarrier` 持久化时设（D6.1），不在检查通过时设；撤单与平仓既不设也不受。

核出：授权步的主体（`responsible`）、不要求人工时审批步的依据（`rule_version`）、门失败的去向（`Close(DecisionRejected)`）、一个版本至多一条 Decision（`Conflict(AlreadyDecided)`）、Decision/`Outcome` 携带 `checked_as_of` 原文未写——已并入 §6.3。

## D5.4 lane 阻塞头

对照：§6.4 lane、队首阻塞协议语义、无第二类越顶队列、显式绕过；不变量 §6.9-1；§6.5 等待与结果。

```mermaid
stateDiagram-v2
  state "Free：集合为空" as Free
  state "Blocked{p}：正常路径，一次等待中的尝试" as B1
  state "Blocked{p, q, …}：例外扩大的集合" as BN
  [*] --> Free
  Free --> B1 : STS 放行一笔 → Prepared（交给 IO 壳，过发出前门即发；门不成立时在门前等待，仍是阻塞头，D6.1）
  B1 --> Free : 该尝试的等待结束（结果确立、Expired，或 principal abandon → Abandoned）
  B1 --> BN : 例外一：以阻塞头中某次尝试 SendBarrier 记为订单键的调用方键为 target 的撤单意图放行（lane 步不等待；按尝试精确匹配；来源不接受 IdemKey 目标时已在输入约束步 TargetNotAccepted）
  B1 --> BN : 例外二：bypass_lane(ticket) 控制记录 Applied（principal 承担协议违反；只对所记版本与所记阻塞头；不是 Decision）
  BN --> BN : 集合内任一尝试的等待结束但集合非空（只移出自己）
  BN --> BN : 集合非空时再加入获准的例外（再一笔撤阻塞头 / 再一次绕过）
  BN --> Free : 集合清空
  note right of B1
    等待中 = Prepared 无 SendBarrier 且未 Expired / SendBarrier 无后继 /
    Undetermined 既无 Found、Absent 也无 Abandoned
    集合是该 lane 执行事实的 fold，不另存
    后续普通写停在 STS lane 步（AwaitingDecision）
    其他 lane 不受影响
  end note
  note right of BN
    IO 壳按 Prepared 位置顺序执行集合内每条，各自对账独立收敛
    撤单尝试的回执或 Found 只说明撤单到达，不决议阻塞头，也不自动重开它的取证
    再问一次由 principal 发 retry_reconciliation；by-key 读到目标 → Found；by-key 否定 → Absent；listing 未见仍 Inconclusive（F10）
  end note
```

读法：阻塞不是锁——后续写的语义依赖队首结果（buying power、待撤订单是否存在、venue 侧顺序），所以是与 venue 的通讯协议语义。撤阻塞头的意图不依赖队首结果，它存在的目的是把 venue 侧推到一个读得出的状态，所以是唯一不违反协议而不等待的写；绕过是另一种不等待，但它是记在控制记录里的协议违反。任何一次尝试的等待结束只移出自己；集合为空才放行普通写。

核出：集合语义曾按"单条终结即解除"表述——已统一为集合（§6.4、§6.5、§8.5、§10.5 #3）。

## D5.5 两层对账状态的重算触发

对照：§6.2 偏离是状态、两层分开的原因、缺观察可行动、参数合规、检查目录；§5.2 `basis_validity`；§8.3 能力变更。

```mermaid
flowchart LR
  subgraph TRIG["触发（都不是 TicketAction）"]
    T1["basis 引用的流推进 / 被撤回 / 出现 gap"]
    T1b["required_inputs 流被撤回 / 出现 gap"]
    T2["required_inputs 流有新观察（推送、回填、一次性读、回执 / 取证的观察记录）"]
    T3["能力变更推送 / CapabilityObserved / 会话进入或离开 Established"]
    T4["保留边界推进"]
    T5a["reload_config(rules)：Lag 变化"]
    T5b["reload_config(rules)：必要项集或检查参数变化"]
  end
  subgraph L1["第一层 basis_validity（只依赖位置）"]
    V["basis_valid(basis, world, Lag) →<br/>Fresh / Stale(Lag) / Retracted(pos) / BeyondRetention(pos)<br/>比较切面：各流当前已提交的流末（Seq 距离 ≤ Lag，缺省 Lag = 0；D8.3）"]
  end
  subgraph L2["第二层 alignment（按 Intent 分派的检查集）"]
    IM{"该项 required_inputs 各流观察侧都存在？"}
    CK["检查目录中策略列出的每项（可交易性 · 敞口 · 持仓在 · 原单仍在）<br/>AlignmentCheck.eval(intent, 各流当前流末 fold_state, 该项参数) →<br/>CheckResult：Aligned / Diverged(Divergence) / Undecidable（gap 或缺该主体的观察）<br/>记 checked_as_of = 实际消费的位置集"]
    MISS["IntentAlignment 该项 = InputMissing(缺哪些 StreamKind)"]
    CAP["能力项：会话有效能力对 (WriteLaneKey, OperationKind) 可执行 → Aligned；已确立而不可执行 → Diverged；未确立（Unknown 或无会话）→ 放行门等待；恒为必要项"]
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
  T3 --> PV["参数合规 parameter_validity 重算（不属两层对账；输入约束步读取，D5.3）"]
  T5b --> GATE
  V --> GATE{"门：Fresh ∧ 必要项 Aligned"}
  CK --> GATE
  MISS --> GATE
  CAP --> GATE
  CK -->|"Undecidable：缺该主体的观察，且策略对该项声明先查后判"| RD["送审时核心发一次 read（OneShot origins ∋ Ticket）→ 观察记录（one_shot，不推进完备进度）→ 该项重算；每版至多一次"]
  RD --> T2
  GATE -->|"否"| DIV["审批人看到 Diverged 单据；放行时 PredicateFailure"]
  GATE -->|"是"| OK["可进 prepare；Outcome 记 checked_as_of"]
```

读法：第一层只看位置（对所有单据可算），第二层看值（依赖意图类型与 venue 给的观察）。第二层读的是各流**当前流末**的 `fold_state`（含 `one_shot`/`backfilled` 记录，各带质量标记），不按任何完备进度截断——否则为补齐输入读来的一次性观察永远看不见；也不是 `basis` 位置处的旧值——否则世界变了单据不会变。每次评估消费的位置集记为 `checked_as_of`，进入放行/否决记录。

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
  S->>S: 冷却 ✓（该 instrument 最近下单写的 SendBarrier 已超出间隔，或未配置）
  S->>S: 过期步 ✓ → 门 ✓（checked_as_of）
  S->>T: 同事务 Prepared + Close(Prepared(pos)) + Outcome
  T->>IO: Prepared @pos（D6.1）
  Note over R,IO: 另一笔：deadline 到期仍无 Decision
  S->>T: 过期步 deadline 到时 → Close(Expired)，无 Prepared / SendBarrier
```

读法：冲突不是锁：一个 `current_version` 至多一条 Decision，第二个决定返回冲突记录即可；`Closed` 不是挡第二条决定的条件——lane 等待期间单据仍开着、版本未变。

核出：同版本第二次决定在 lane 等待窗口内无判据——已并入 §6.3 审批步（`Conflict(AlreadyDecided)`）。
