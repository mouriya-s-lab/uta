# 意图来源

## 3 模型

### 3.1 解释②：决策

- `rules`（`DecisionStep`：`On(Pattern, _)`、`Emit(EffectRequest)`、`Require(Guard, OnFail)`、`Expire(Id, _)`）解释为对日志的 fold。纯性由数据结构本身保证，不靠开发约定。[证据：fp-01 M7 Mercury Workflow]
- **唯一出口**：`Emit(EffectRequest{effect_kind, payload, basis})`。请求是值，不是调用；它随 `Output.effects` 交回，是否与何时被处理由核心的出站请求处理器决定（[02-requests.md §3.1 EffectRequest：唯一出口，请求是值](../05-runtime/02-requests.md#31-effectrequest唯一出口请求是值)）。请求不带调用方键。本进程没有任何写能力。
- **解释②读的输入**：`facts` 声明的执行事实输入与该程序自己的请求流（隐含输入），另可读派生节点的值。解释①的节点不消费执行事实与请求流。
- **请求进度跟踪**：程序对自己所发请求的记账——它发过哪些请求（在请求流上的位置由 `EffectRequest` 记录得知；程序按自己放进载荷的内容认出它们）、各自得到什么 `EffectResponse`、写请求之后的单据与尝试走到哪一步。解释②在进程内按自己的锚点（`Drafted(ticket_id)` 所指单据、`Close(Prepared(p))` 所给的 `p`）从执行事实输入里挑出属于自己的事实；请求流与 lane 流之间没有投递顺序，`Draft` 可能先于指向它的 `Drafted` 到达，按 `ticket_id` 两种次序都能匹配。本进程的投递接收不做关联，只按位置接收（[§6.1 权衡](#61-权衡)）。
- 这份记账只是程序自己的，写进 `Checkpoint`；放行与否仍由核心按执行事实判定，它不是任何门的输入。

## 4 结构

### 4.1 写处理器 [设计]

请求被当作意图，进入单据 → STS → IO 壳的完整效应路径；结果是执行事实与决议记录。按下列顺序判定，每步只读 UTA 自己的事实：

1. **构造**：从请求载荷构造意图，只要求意图的锚点（`WriteLaneKey`、交易协议的操作种类之一、`basis`，撤单 / 改单还要 `target`；平仓的 `target` 由核心从 `basis` 所指的持仓观察记录构造）。锚点构造不出即不是意图，不开单，记 `NotDrafted(Malformed{reason})`。构造规则的完整定义与各种构造不出的情形见 [01-ticket.md §3.3 意图构造与锚点：构造不出的不是意图](01-ticket.md#33-意图构造与锚点构造不出的不是意图)。参数（守卫字段与载荷）不在构造时判定：参数是否合规是单据 fold 的状态，送审时由输入约束步否决（[01-ticket.md §3.4 参数合规：意图参数 schema](01-ticket.md#34-参数合规意图参数-schema-设计)、[04-decision-chain.md §4.2 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期](04-decision-chain.md#42-顺序固定链授权--输入约束--审批--lane--过期)），所以程序发出的非法参数与人起的非法参数得到同一对记录（意图 + 否决，Q8）。
2. **作用域判定（构造规则）**：意图的 `WriteLaneKey` 所属的 `(来源, 作用域)` 不被发出成员的 `Applied` 所记的任何执行事实输入选中时，不开单，记 `NotDrafted(ScopeNotObserved)`。它在 `Malformed` 判定之后，只由 UTA 自己的事实（发出成员的 `Applied` 与请求锚点）判定，所以程序不会写进一个自己看不到结果的作用域。选中的含义与执行事实订阅同一个 selector（[03-consumption.md §3.1 订阅与项](../02-read/03-consumption.md#31-订阅与项)）。
3. **开单**：以**发出成员的装载 principal** 为 `responsible` 开单。程序本身不是 principal；该 principal 的授权范围决定单据能否不经人工直接放行（[04-decision-chain.md §4.2 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期](04-decision-chain.md#42-顺序固定链授权--输入约束--审批--lane--过期)）。
   - `Draft.basis` 含该 `EffectRequest` 记录的位置：执行事实侧位置作因果依据（[core/00-design.md §3.6 basis：方向、基数、可追溯性](../00-design.md#36-basis方向基数可追溯性)）。
   - 程序意图没有编辑期：同一事务 `Draft` 并 `SubmitForDecision`，单据直接进入 `AwaitingDecision`；同一事务 append `EffectResponse{Drafted(ticket_id)}`。
   - 之后的退回、改写、移交由该 principal 经单据组进行（[01-ticket.md §4.3 单据组（核心↔解释层）](01-ticket.md#43-单据组核心解释层)），与人起的单据无异。

理由（作用域判定）：`NotCalled` 与 `NotDrafted` 都不产生观察记录，程序只有经响应才看得到它们，而伪造观察记录去承载它们会把效应侧结论放进观察宇宙；程序若要组合多次写（例如先撤单、确认后再下单，[02-trade-protocol.md §4.1 交易协议：操作种类、目标、可执行性、检查目录](02-trade-protocol.md#41-交易协议操作种类目标可执行性检查目录-交易协议)），它自己的尝试有没有结局只能从它声明的执行事实输入得知。写进未声明的作用域，结局就永远到不了程序。

## 5 走查

**W1 步 1–3 的出站处理器细化。** 程序 `Emit(EffectRequest{trade.place, basis, …})`，随 `Advance` 输出事务提交，记录带 `member`。写处理器接手：锚点构造成功 → 意图的 `(来源, 作用域)` 在发出成员所记的执行事实输入之内 → 同一事务 `Draft{responsible = 装载 principal, basis ∋ 请求位置}`、`SubmitForDecision`、`EffectResponse{Drafted(ticket_id)}`。对外：事务提交后读模型 `tickets` 看到一张 `AwaitingDecision` 单据；程序在请求流上读到 `Drafted`，再在它声明的执行事实输入上按 `ticket_id` 找到 `Close(Prepared(p))`、按 `p` 认出尝试的记录（[§3.1 解释②：决策](#31-解释②决策)）。请求流与 lane 流之间不定投递顺序：`Draft` 与 `Drafted` 同事务提交，程序可能先看到单据记录、后看到 `Drafted`，解释②按 `ticket_id` 两种次序都能匹配（[03-consumption.md §3.7 投递的单位与次序](../02-read/03-consumption.md#37-投递的单位与次序)）。

**W18 程序变体。** 程序 `Emit` 参数不合 schema 的下单：构造成功（参数不在构造时判定）→ 开单 → 输入约束步否决，程序在请求流上看到 `Drafted`、在执行事实输入上看到 `Rejection` 与 `Close(DecisionRejected)`。载荷缺 `WriteLaneKey`：`EffectResponse{NotDrafted(Malformed)}`，不开单、重启不重派。

## 6 评估

### 6.1 权衡

- **程序经声明的执行事实输入看自己的结果**（本文 §4.1、[01-program-host.md §3.3 程序的输入](../05-runtime/01-program-host.md#33-程序的输入)）：没有宿主侧的关联索引，写到未声明作用域的请求在开单前就得 `NotDrafted(ScopeNotObserved)`；代价是声明作用域里其他 principal 的单据与尝试同样投给程序、唤起 `Advance`、计入程序预算（Q25）。这是接受的代价。
  - 不选：**宿主按锚点动态关联、只投递程序自己的单据与尝试**：宿主要按已看到的 `EffectResponse` 维护一份可达集合决定投递资格，是一个新的关联索引机制（[README.md §1.2 权威源头与生命周期](../../README.md#12-权威源头与生命周期)“机制即信号”）；**为程序另设结果订阅种类**：同一事实两条投递路径；**只给 `EffectResponse`**：程序看不到尝试的结果；**把撤单结果当作目标的结果**：把一次撤单请求的受理当作原单已结束（[07-resolution.md §4.1 对账驱动](07-resolution.md#41-对账驱动)）。

### 6.2 验收

62. **程序看得到自己写请求的结果**（本文 §4.1、[01-program-host.md §3.3 程序的输入](../05-runtime/01-program-host.md#33-程序的输入)）：（对应 Q1/Q25）
    - 程序声明执行事实输入 `(X, S)` 后 `Emit` 一个写到作用域 S 的请求：它的请求流上先有该 `EffectRequest`、后有 `EffectResponse{Drafted}`；声明的执行事实流上投来该单据的 `Close` 与尝试的 `SendBarrier`/`VenueAccepted`/`VenueRejected`/`NotSent`/`Undetermined`/`ResolutionEvidence`/`ReconciliationReopened`/`Abandoned`；解释②按 `ticket_id` 与 `p` 认出它们，单据记录先于 `Drafted` 投到时同样认出；
    - 同一程序写到未声明的作用域 T：`EffectResponse{NotDrafted(ScopeNotObserved)}`，无 `Draft`、无单据；以 `NotCalled` 结束的读请求的响应同样在请求流上；
    - 声明作用域里另一 principal 的单据与尝试同样投给程序（唤起 `Advance`、计入其预算），它们的 `ticket_id` 与 `p` 不在该程序的 `Drafted` 与 `Close(Prepared)` 所给之内；宿主进程与投递不读任何记录的内容来决定投递；
    - 在 `Advance` 提交之后、结果到达之前重启：程序从 `Checkpoint` 与同事务的 cursor 续跑，恰收到之后的结果各一次；`cold_start` 的 `Reset` 之后全部输入 cursor 按声明的起点重建，与 `ProgramReset` 同事务。

