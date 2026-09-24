# 9 走查

本章不新增任何定义。它把 §2–§8 已定的抽象放到具体刺激上逐步走，验证每条路径都能由已定设计闭合。

## 9.1 场景 trace（W1–W20）

> 图：W1 → D6.5；W2 → D6.6、D7.3；W5 → D6.7；W12 → D6.4；W14/W19 → D9.1；W17 → D4.1、D4.3；W18 → D5.6；W20 → D9.2、D9.5。

**每步写法：** 输入 → 经过哪个元素 / 抽象（§x.y）→ 输出 / append 的记录（持久化落点）→ 对外可见结果 → 当前唯一行动者 / 恢复者与其稳定身份或 fence 依据。

**判定：**

- **走通** = 整条路径能由已定设计闭合，且每步的行动者与恢复归属可推出。
- **卡点** = 缺概念 / 选错抽象 / 接口不够 / 归属不明，登记进 §9.3。

**持久化落点简称**（归属见 §7.5）：

- **观察 J** = 观察 `Journal`；
- **执行 J** = 执行事实 `Journal`（纯 append，§3.1、§7.4）；
- **RuleState**、**单据记录**（`TicketAction`）、**订阅表/cursor**、**程序状态**、**快照**。

### W1（Q1）正常下单闭环

**正常路径。**

1. 程序解释②在满足规则时 `Emit(EffectRequest{effect_kind: trade.place, basis, key})`（§6.1）。
   - 行动者：程序宿主（§8.6），以其**装载 principal** 为身份。
   - 输出：`EffectRequest` 值，未定型。对外可见：无。
2. 核心出站写处理器接手（§6.1、§7.3），以装载 principal 为 `responsible` 开单 `Draft{basis}`（§6.2）。
   - append 单据记录（执行 J），与步 3 的 `SubmitForDecision` 及 `EffectResponse{Drafted}` 同一事务（§6.1）。
   - 对外可见：无；`Drafting` 只在该事务内出现，不对外可见。
3. 写处理器在同一事务内 `SubmitForDecision`（程序意图无编辑期，§6.1）：冻结 `current_version`，`Drafting → AwaitingDecision`（§6.2）。
   - 对外可见：事务提交后，审批人 / 读模型 `tickets` 看到一张 `AwaitingDecision` 单据，负责人 = 装载 principal。
   - STS 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期（§6.3）。
   - 放行前读单据 fold 的 `basis_validity == Fresh`、必要项 `alignment == Aligned`、版本一致（§6.3 放行门）。
   - append RuleState + 单据记录（执行 J，同事务）。
   - 行动者：STS 规则链，身份 = 决定的 principal（不要求人工时为 `rule_version`）。
4. 放行：同一事务 append `Prepared` 并 `Close(Prepared(position))`（§6.2、§7.4）。
   - `Prepared` 是单据 → IO 壳的唯一交出点（§5.3）。
   - 行动者：单据（交出）→ IO 壳（接手）。
5. IO 壳按 lane 顺序取 `Prepared`（§6.5），durable append `SendBarrier`（fsync，执行 J），再调用集成 `submit(attempt)`（§8.2）。
   - 行动者：IO 壳，身份 = `attempt_position`（`Prepared` 的 `LogPosition`，锚点，§8.1）+ `WriteLaneKey`。
6. venue 受理并给出身份 → 集成 `submit` 返回 `Ack(venue_id, receipt)`。同一事务 append：
   - `VenueAccepted{venue_order_id, receipt: Evidence, observation}`（执行 J；`Evidence` = 契约载荷 + 原始负载，§6.5）；
   - 回执观察记录（观察 J，`provenance: Receipt{AttemptRef}`，记录模型，§6.5）：订单状态一条，由 IO 壳填 `attribution: FromAttempt(AttemptRef)`；回执已含成交时，每笔可识别的执行另落一条带 `execution_id` 的成交记录（§8.1“成交与订单状态的契约语义”），其归因按该条自己的关联证据填写（§5.3）。
7. 部分成交、成交依次到达 → 集成推送带 `attribution` 的订单状态 / 成交观察记录（观察 J，§8.3、P2、P9）；成交记录带 `execution_id`，数量与价格是本笔执行的量，订单状态带累计量（§8.1）。读模型 fold 出最终成交状态（§4.4、§8.5）。
   - 对外可见：订阅者依次收到受理、部分成交、成交，字段与原生身份保真（C13）；读模型最终 = 成交。

**扩展路径（同一笔执行经多个渠道，Q1/Q5）。**

- 步 6 的回执已含第一笔执行 x；步 7 的推送再送一次 x；此后运维者经 `read` 读成交，或对账的 `list_fills`、断线后的回填又带回 x。每次都 append 一条观察记录（观察 J，出处与质量标记各异），不在入口去重（§8.3）。
- 所有这些记录的 `execution_id` 相同，`orders` 读模型与订阅者自行 fold 都只计 x 一次（§8.1 按执行计数的 fold 规则）；两笔价格、数量、时间都相同而 `execution_id` 不同的执行计两次。
- 订单的累计成交量取自订单状态观察，不由成交相加（§8.1 增量与累计）；`positions` 不读成交（§8.5）。
- x 之后被上游修正：上游给出修正次序时，修正记录以同一 `execution_id` 与更大的 `execution_revision` 到达，fold 取修正后的内容，先到的是修正还是原记录都一样；上游没给次序而两条内容不同时，x 标为冲突、不贡献数量，两份内容照实给出（§8.1 修订）。
- 对外可见：成交列表里 x 只出现一次；逐笔之和与订单累计量不一致时照实并列。

**扩展路径（venue 专有 / 未列举状态，Q5 保真）。**

- 集成按记录映射的枚举映射表把上游状态落到契约词表，表外值输出 `Unmapped(raw)`；随记录附原始负载与原生身份（订单状态流必须保留原文，§2.2、§2.6、§8.1、§8.3）。
- 未列举状态标 `unknown` 并告警，不压成 `rejected`。
- 对外可见：累计成交量 = 回执累计字段；映射表无“其他→rejected”。

**走通。** `Undetermined` 未触发本路径；其记录模型细节见 W2。

### W2（Q2+Q3）SendBarrier 后崩溃、三渠道收敛、脑裂变体

**正常—失败路径（SendBarrier 后崩溃）。**

1. 承 W1 步 5：IO 壳 durable append `SendBarrier`（fsync，执行 J）后、取得业务回执前，核心 `kill -9`。
   - 持久状态：`Prepared` 有、`SendBarrier` 有、无后继。
2. 重启：IO 壳从执行 J 重建各 lane 链状态（§6.7）。`SendBarrier` 无后继 → append `Undetermined(CrashWindow)`（执行 J），进入对账驱动。
   - 行动者 / 恢复者：IO 壳（核心内唯一效应处），身份 = `attempt_position` + `WriteLaneKey`。
   - 对外可见：该尝试 = `Undetermined`；lane 队首阻塞（§6.4）；无第二次 `SendBarrier`（不变量 §6.9-8）。
3. 收敛按 `CapabilityProof` 声明的渠道**自动**依次取证（§6.6、§8.2；读副作用可重试）。取证在该集成新会话建立后开始；会话建立之前 IO 壳不对它发读，也不记 `Gap{origin: Channel}`（§6.6）：
   - **按键回读** `query_by_key`：`Found(state)` → 同事务 观察记录 + `ResolutionEvidence{ByKey, Found}` → 腿终结、移出阻塞头集合；`Absent` → `ResolutionEvidence{ByKey, Absent}`，腿终结（本次未发生）。
   - **listing+身份** `list_open`/`list_fills`：命中带归因身份的订单 / 成交 → 同事务 观察记录 + `ResolutionEvidence{Found}`；未命中 → `Inconclusive`（F10：listing 未见不证明未递，本渠道无 `Absent`）。
   - **无渠道**：渠道穷尽仍 `Inconclusive` → append 后停等。停等由带 principal 的人工 `resolve`，或 `ReconciliationReopened`（撤阻塞头腿终结 / 集成会话重建 / `retry_reconciliation`）重开一轮推进（§6.6）。IO 壳永不 heuristic（C1/C2/C12）。
   - 对外可见：三种环境末态可枚举：found → 回执即观察记录；absent → 未发生；无渠道 → 停等。停等态由带 principal 的决议、任一时刻到达的归因观察（步 4）或新一轮取证推进。
4. 迟到回执收敛：
   - 旧会话的回执不经旧会话进入核心：旧 epoch 的推送在边界被拒（§8.3）。
   - 集成在新会话上以**观察记录**重新送达带可关联身份的回执（`venue_order_id`/`idempotency_key` 经 `attribution`，§5.3、§8.3）。
   - 效应侧归因处理器见目标腿处于 `Undetermined`，同一事务 append `ResolutionEvidence{Attributed, Found{observation: 该记录, evidence}}`（§6.6、§8.1），腿终结，不产生第二次下单。
   - 集成进程本身已死则无此路径，收敛全靠对账（步 3）。

**脑裂变体（Q2③）。** 旧核心退出前已把 `submit` 交给其集成进程 A；核心重启（新 `instance_id`）并建立新集成会话 B 后，A 作为孤儿仍可能把那次 `submit` 送达 venue。

- 安全性由三条不变量共同保证，不依赖 A 的回执到达：
  - `SendBarrier` durable = “可能已发出”（不变量 §6.9-8）；
  - 阻塞头集合非空时同 lane 无新普通写（不变量 §6.9-1，队首阻塞 §6.4）；
  - 对账驱动经会话 B 独立取证收敛（§6.6）。
- A 的回执归属由会话 epoch 定（§7.2 第 3 步）：A 的 epoch 已作废，它送来的回执在边界丢弃。
- A 按 §7.2 第 1 步被回收。
- 若 venue 已受理，B 的对账取证或新会话上的观察记录（步 4）把它并入同一腿。
- 对外可见：fixture venue 调用 ≤ 1；该意图恰一条 `SendBarrier`。

**走通**（崩溃注入验收 §10.5 #8；取证记录模型验收 §10.5 #17）。

### W3（Q4）Prepared 未发前崩溃

**失败路径。**

1. 承 W1 步 4：同事务 append `Prepared` + `Close(Prepared)` 已提交，但 IO 壳尚未 durable append `SendBarrier` 时 `kill -9`。
   - 持久状态：`Prepared` 有、无 `SendBarrier`。
2. 重启：IO 壳重建链。`Prepared` 无 `SendBarrier` = **确未发出**（不变量 §6.9-8），仍是可安全发送的 `Prepared`（§6.7）。
   - 恢复者：IO 壳。
   - 过发出前门（§6.5）：读 `Prepared` 自带的 `deadline` 与意图身份，查核心自己的会话状态与该 `(WriteLaneKey, OperationKind)` 的当前能力证据，不触碰单据（单据已 `Closed`，§6.2）。
   - 未过期、该集成会话已建立且当前能力可执行：durable append `SendBarrier` 后调用该腿的写操作（继续 W1）。
   - 未过期，但该集成尚无会话（重启后还在 `Connecting`，或 `Halted`），或当前能力对它不再可执行：腿保持 `Prepared`、不 append 任何记录，仍占该 lane 阻塞头；会话建立或能力恢复时再过门，`deadline` 到时 `Expired`。
   - 已过期：append `Expired(deadline)` 终结该链。意图**不误升为 `Undetermined`**、不补偿（H6）。
3. 对外可见：崩溃窗口本身不产生 venue 调用，也没有 `SendBarrier` 记录；末态可枚举为“已发”（经 W1）、“等待后已发”或 `Expired`，任一种都不经 `Undetermined`。

**走通**（崩溃注入验收 §10.5 #8(a)/#14）。

### W4（Q6）外部变更归因

**正常路径。**

1. venue 推送一笔对不上任何本地 `attempt` 的成交 / 余额变动 → 集成产出带 `attribution` 的观察记录（观察 J，P11、§8.3）。集成填不出归因时记 `Unattributed`（§5.3）。
2. 效应侧归因处理器（注册在效应侧，读观察记录；记录归观察 / 响应归效应，§5.3）尝试匹配本地 `attempt`。
   - 无匹配 → 结论 `External`，不归因到任何 pending 意图。
   - 对外可见：外部变更记录存在且无意图引用；订阅者收到该记录。
3. 后续证据（对账 / 回读）可引用该记录的 `LogPosition` 作依据（位置作为关联，§2.3），不回改原记录（执行 J / 观察 J append-only）。

**走通。**

### W5（Q7）同 lane 并发与队首阻塞

**正常—失败路径。**

1. UI 与 AI 在 100 ms 内各提交一笔到同一 `WriteLaneKey`（交易协议下 = (账户, 子账户)，§6.4）。
   - 两笔各自经解释层代开的会话 `draft(intent)` + `submit_for_decision`（§8.5），以各自会话 principal 为 `responsible`，进入 `AwaitingDecision`；之后同 W1 步 3 的 STS 链。
   - STS 链的 lane 步按 append 顺序放行第一笔（W1 步 4，得 `Prepared`）。
   - 第二笔因 lane 已有未终结 Attempt 停在 lane 步（`RuleState` 的 lane 阻塞头，§6.4），单据保持 `AwaitingDecision`，不产生 `Prepared`。
   - 行动者：STS 规则链（lane 规则），身份 = `WriteLaneKey`。
2. 第一笔 `submit` 后进入 `Undetermined`（承 W2）。
   - 队首阻塞：阻塞头集合非空时同 lane 无新普通写（不变量 §6.9-1），第二笔继续停在 lane 步并告警。
   - 等待期间它的 `basis_validity`/`alignment` 照常重算（偏离是状态，§6.2）；`deadline` 到期则过期步 `Close(Expired)`（H6）。
   - 队首终结、集合清空后 lane 规则放行；链先过过期步，再过依据有效性门（§6.3），再 append `Prepared`。
   - 理由：后续写语义依赖队首结果（buying power、venue 侧顺序），是通讯协议语义，非 UTA 的锁（§6.4、§6.8）。lane 等待发生在 `Prepared` 之前，所以不存在因 lane 而等待的已放行记录；已放行的腿只可能在发出前门等会话或能力（§6.5）。
3. 另一账户（不同 `WriteLaneKey`）的写同时进行，不等待（H4；粒度权衡，§6.4）。
   - 对外可见：fixture 调用不重叠；`Undetermined` 期间无第二次调用；其他账户不受阻。

**扩展路径（撤阻塞头与显式绕过）。** 队首取证期间，lane 上读侧决议动作（按键查询 / listing / 对账）不是队列项（§6.4）。

- **撤阻塞头**：以第一笔下单腿 `SendBarrier` 记为订单键的调用方键为 `target`（`IdemKey`）起一张撤单单据（按腿精确匹配，§6.4）。该来源的撤单只接受 venue 订单身份时，这张单据在输入约束步得 `TargetNotAccepted`、`Prepared` 之前关闭，不会多出一条 `Undetermined` 的撤单腿（§6.2 可执行性）；此时只剩显式绕过或等取证收敛。
  - STS 全链照走，lane 步不等待阻塞头（唯一不违反协议的写例外，§6.4）；放行后阻塞头集合 = {第一笔, 撤单}。
  - 撤单腿自身的回执不决议第一笔。
  - 撤单腿终结（`VenueAccepted`/`Found`）时，IO 壳为第一笔 append `ReconciliationReopened{CancelLegTerminal}`，取证从 by-key 重走一轮：读到目标（任何状态）即 `Found`；by-key 明确否定即 `Absent`；listing 未见仍 `Inconclusive`（F10，§6.6）。
- **显式绕过**：运维 principal 对第二笔 `bypass_lane(ticket)`，得控制记录 `Applied`（对协议的自觉违反，不是 Decision，§6.4），记下第二笔当时的 `current_version` 与当时的阻塞头位置集 {第一笔}。
  - 第二笔已获批准（或不需人工）时越过 lane 步得 `Prepared`，阻塞头集合扩大；未获批准则照常停在审批步，批准时这一版本仍受这条绕过覆盖。之后加入集合的阻塞头不在其内。
  - 各自的对账驱动独立收敛，任一终结只移出自己，集合清空即 lane 解除。
- 对外可见：两种路径下该 lane 都有两条 `SendBarrier`（各属不同 Attempt）；前者无 `bypass_lane` 控制记录，后者有；第二笔单据的 Decision 仍只是它的批准；其他 lane 不受影响。

**走通。**

### W6（Q11+Q12+Q31）行情断线 gap、慢消费者、三种消费方式

**正常—失败路径（断线无续传，Q11）。**

1. 订阅 200 instrument tick（订阅表，§4.2、§7.3）；集成断线 30 s 重连，venue 无游标（F11/C6）。
   - 集成上报不能续接 → 核心结束前一 epoch、创建新 epoch。新 epoch 首条是 `Gap{origin: Source}`（观察 J，§4.2、§8.3），含前一 `StreamId` 与最后 `Seq`、原因 `disconnect`。
   - 恢复者：核心（时间权威，§2.3、§8.3）。
   - 重连握手后，核心对该流重发一次需求全集（这 200 个主体，§8.2 `route`）；集成在收到它之前不推送该流，`Routed` 的路由结论记录落在新 epoch 上，标出这些主体从这里起有覆盖。
   - 对外可见：订阅者先收到 gap 再收新 epoch 记录；无静默跳过。
2. **有游标变体**：集成重连报可信续传游标 → 续用原 epoch，`Seq` 接续，不新建 gap（§8.2 `handshake`、§8.4）。

**失败路径（慢消费者，Q12）。**

3. 三订阅者中一个不确认，缓冲耗尽 → 投递调度对该订阅者停投，并 append `Gap{origin: Delivery}`（原因 `slow_consumer` 或 `conflated`，§4.2），带 from/to `Seq`，需显式确认。
   - 行动者：投递调度（§7.3），身份 = 订阅 cursor。
   - 对外可见：慢者收 gap 且停投；两快者不受影响、cursor 持续推进（跨订阅者不阻塞）。
4. 确认语义（§4.2）：`ack` = 消费方已处理。停投的慢者恢复后，从其已确认 cursor 之后重收；未确认区间可能重复可见，按 `LogPosition` 去重；已确认区间不重投。

**扩展路径（三种消费方式，Q31）。**

5. 同一组流被三种消费者声明消费（§4.2）：
   - `await-all` 按**完备进度**（frontier）触发，而非 cursor（§2.3）；
   - `ordered` 按单流顺序背压，不跳过；
   - `latest/conflated` 允许合并，损失以 `Gap{origin: Delivery, conflated}` 或消费者声明的窗口界记。
   - 对外可见：每次合并可追溯到声明窗口或 gap（B5/C6）。读模型消费者拿到的 `Snapshot` 带 `as_of` 与 `gaps`（§8.5），与自己的 cursor 比对即知快照覆盖范围。

**走通。**

### W7（Q17）核心 append 中途崩溃

**失败路径。**

1. 核心在“追加观察记录”或“替换订阅表”中途 `kill -9`。存储为单个 SQLite 文件、单写者、每次 append 原子（§7.4）。
2. 重启：半写事务不提交，`fold_state` 从已提交记录重建（§4.1、§7.4）；半写状态不可见（不变量 §6.9-2）。
   - 恢复者：核心（独占 SQLite，H10 OS 文件锁与 SQLite 锁同向，§7.4）。
   - 快照加速重建、不改 append-only（§7.5）。
3. 对外可见：无半条记录；订阅者按 cursor 续接（订阅表 / cursor 恢复）。

**走通。**

### W8（Q18）配置热变更 / 换凭据（经控制面与统一路径文件）

**正常路径（改规则）。**

1. 运维者经**控制面** `reload_config(rules)`（认证 principal，P14、§8.5）提交策略变更。
   - 规则文件在 `OPENALICE_HOME` 统一路径（每文件唯一写者 = Alice，§7.6），版本 = 内容 hash。
   - 核心重载并 append 控制记录 `Applied(position)`。
   - 对外可见：下一笔意图使用新策略；此后每条 `Outcome`/`Rejection` 带新 `rule_version`。
2. 待决单据并存：规则不冻结进单据（§6.3）。
   - `AwaitingDecision` 单据的必要项 `alignment`/授权在规则变更后由单据 fold 重算（偏离是状态，§6.2）。
   - 放行时按当时规则重过五步，收紧规则使其 `Rejection`（带 `rule_version`）（§6.3 规则版本变更）。
   - 世界 / 规则变了 → 单据呈 `Diverged`，审批人看到（§6.2）。

**正常路径（换凭据）。**

3. 控制面 `rotate_credential(integration)`：凭据经链 `统一路径封存文件 → UTA 核心 → 该集成进程`（C7、§7.6）。
   - 只该集成会话重建（进入 `Connecting`，新 `session_seq`，§7.2 第 3 步），并显式标 `Gap{origin: Source}`；其他集成 `Seq` 连续（§8.3）。
   - 恢复者：核心（凭据注入）+ 该集成（会话重建）。
   - 对外可见：目标集成会话重建与原因可见；其他流不受影响。

**失败路径（上游拒绝凭据）。**

4. 新凭据被上游拒绝（或会话中凭据被吊销：集成结束会话，核心重握手）：集成 `handshake` 返回 `Refused(reason)`（§8.2）。
   - 集成会话：会话状态 → `Halted{Refused(reason)}`，同一事务 append `IntegrationHalted`（P14）与健康观察，随后终止该集成进程，不再自动握手；核心重启从执行事实恢复 `Halted`，也不解除（§7.2 第 3 步）。
   - 在途调用：会话结束时经调用通道尚未返回的写得 `NoResponse`（→ `Undetermined`）、读得 `Unavailable`，各计数一次；之后到达的旧 epoch 回应在边界拒绝（§7.2 第 3 步）。
   - 写：该集成各 lane 上已放行未发出的腿在发出前门等待（无会话），`deadline` 到时 `Expired`；不产生 `SendBarrier`，也就不产生 `Undetermined`（§6.5）。已在 `Undetermined` 的腿不取证，停在原处，会话重建后续跑或按 `SessionRestored` 重开（§6.6）。
   - 读：订阅不挂起、需求保留（§8.2）；该集成各流 readiness 为 `Disconnected`（§8.4）。
   - 恢复：运维修好凭据后 `rotate_credential` 或 `restart_integration`：`Applied` 带被解除的 `IntegrationHalted` 位置，与 `Connecting` 健康观察同一事务提交，之后才握手。
   - 对外可见：健康里该集成是“停止并待处理：凭据被拒”，与“断开、正在重连”（`Connecting`）可区分；上游不再收到重复登录。

**失败路径（上游暂时不可达）。**

5. 握手时集成连不上上游：`handshake` 返回 `Unavailable`（§8.2），会话保持 `Connecting`，核心按 pacing 重握手；其余同上一条的“写 / 读”两项，但无需运维动作，会话建立即恢复。

**扩展 / 失败路径（重载失败）。**

- 文件原子替换（临时文件 + rename）保证无半写可见态。
- 重载失败保留上一有效版本，控制记录 `Rejected(reason)`（§7.6、§8.5）。
- 对外可见：控制记录可读；生效配置版本 hash 未变。

**走通。**

### W9（Q20）双实例

**失败路径。**

1. 第二个核心对同一用户状态根启动。单实例由 OS 文件锁 + fence 保证（H10、§7.1）；第二实例以专用退出码拒绝启动，**不做接管**。
   - 对外可见：退出码 / 诊断可观测；不产生双写。
   - 行动者：OS 文件锁持有者判定。
2. **持有者死亡后接管变体。** 原持有者死亡，锁随进程释放；它拉起的集成 / 宿主进程可能仍存活（父死子活，H10 的孤儿）。
   - 新实例取 fence 接管（§7.2），身份 = fence 令牌。
   - 接管后在途 `SendBarrier`/`Undetermined` 由新实例 IO 壳从执行 J 重建（§6.7，同 W2）。
3. 接管边界（§7.2 第 1 步）：
   - 新实例取 fence 即 `instance_id` 加一；旧实例失权时点 = 该事务提交。
   - 旧实例登记的集成 / 宿主进程按进程表回收。
   - 旧 epoch 的推送与回执在边界丢弃（§8.3）。
   - 旧在途 `submit` 的命运与 W2 脑裂变体相同：由 `SendBarrier` 记录 + 新实例对账收敛，不依赖旧实例。
   - 对外可见：接管不产生双写（不变量 §6.9-2 单写者 + fence）；进程表中无旧 `instance_id` 名下的存活进程。

**走通。**

### W10（Q25）程序超预算隔离

**失败路径。**

1. 程序死循环 / 超内存 / 超意图速率（H2/C4）。预算由**程序宿主**进程 / 沙箱隔离，而非类型系统（§6.1、§8.6）。
   - 行动者：程序宿主，身份 = 程序制品 hash + 装载 principal（P12）。
2. 超预算 → 宿主隔离该程序并产出失败观察（观察 J，派生失败记录）；其他程序、账户、核心不受影响（C4、§8.6）。
   - 对外可见：预算违规、隔离状态、其他主体可用性可观测。
3. 宿主协议（§8.6）：
   - 超预算或 trap = 核心终止宿主进程，append `ProgramFailed{reason: Budget(kind) | Trap}`；程序停在 `Failed`，直到控制面重新 `load_program`。
   - 程序状态经最近 `Checkpoint`（与 cursor 同事务持久化，§7.5），在重新装载时 `Load` 交回。
   - 预算值由程序装载时声明（P12），不是设计常量。

**走通**（三 OS 预算与隔离验收 §10.5 #15；状态跨重启验收 §10.5 #16）。

### W11（Q26）单据并发编辑与 SendBack

**正常—失败路径。**

1. 单据已有负责人 `responsible`（锁 = 该字段存在，§6.2），`current_version` 已前进。第二个 principal 对该单据 `Revise` → 被拒（持锁期间仅 `responsible` 可 `Revise`，不排队不分叉）。
   - 行动者：单据锁；线性化点 = 单写者 append 顺序 + `Ticket` fold（§4.1、§7.4）。
   - 对外可见：冲突结果、负责人、版本可读。
2. **SendBack 分支。** 送审后 `AwaitingDecision`，审批人 `SendBack{reason}` → `AwaitingDecision → Drafting`，`responsible` 不变（穷尽转移，§6.2）。
   - `SendBack` 后 `Revise` 使 `current_version` 前进，旧 Decision 自然失效。
   - `AwaitingDecision` 期间 `Revise` 被拒（决定绑定 `current_version`，C11）。
   - append 单据记录（执行 J，每条带 principal 与依据）。
3. **决定版本冲突（Q10 同型）。** 两 principal 对同一 `current_version` 决定，第二个返回 `Conflict(AlreadyDecided)`、不执行、不改状态（C11、§6.3）。即使第一条决定后单据仍停在 lane 步、版本未变，也是如此（审批步，§6.3）。
   - 下游的自动决定者（检查目录之外的 guard，§6.2）与人工审批人同受此约束：它批准后，人就不能再决定这一版；它否决则单据关闭；它想改判一个已批准的版本，只能经 `SendBack` 退回草稿、由负责人 `Revise` 出新版本。`SendBack` 不是 Decision，不占这一版的决定。退回与否决的原因随版本可读（读模型 `tickets`）。
4. 负责人失联：由策略层处理（权衡，§6.2）。
   - 过期步在 `deadline` 到期 `Close(Expired)`；
   - 或持有控制授权的 principal 经 `transfer(ticket, to)` 强制转移（§8.5），转移记录带 principal 与依据。
   - 对外可见：单据要么 `Closed(Expired)`，要么 `responsible` 变更且可追溯。

**走通。**

### W12（Q27）Replace 按两腿计划执行（含撤单腿 Undetermined）

**失败路径。**

1. 改单意图类型 `Replace [交易协议]`，构造期必须携带 `target: VenueRef | IdemKey`（parse-don't-validate，§6.2、§8.1）。
   - 该 `(scope, Replace)` 声明两腿计划 `[cancel, submit]` → IO 壳在**同一条 Attempt 链**解释为 `SendBarrier(cancel) → TargetTerminal（目标终态判定成立）→ SendBarrier(new)`（§6.2、§6.5）。计划在撤单腿过发出前门时选定并记进它的 `SendBarrier`，之后能力声明换成原子计划也不改这条链。这是 `>>=`：第二腿读第一腿结果，发生在 IO 壳内。
   - `target` 的种类不在该来源接受之列（例如撤一条只有请求键的腿、来源只接受 venue 订单身份）→ 输入约束步 `TargetNotAccepted`，单据在 `Prepared` 之前关闭（§6.2 可执行性）。
   - 行动者：IO 壳，身份 = `attempt_position` + `WriteLaneKey`。
2. 撤单腿（`leg = 1`）`cancel` 无回执 → `Undetermined`（§6.5）→ 整条链停在对账，新单腿**不发**（§6.2）；取证只用撤单腿自己请求键的 by-key / replay-by-key，listing 与成交对账不属撤单腿，不带键的撤单腿没有自动取证渠道，只由 `Attributed` 或 `Manual` 收敛；目标订单的记录（例如 listing 上仍在或已不在）不是撤单腿的 `Found`（§6.6 撤单腿的取证）。
   - 撤单腿终结于 `VenueAccepted`/`Found` 后，链进入 `AwaitingTargetTerminal`（转移表，§6.5）。进入的那个事务先看 `deadline`，再对截至此刻的全部记录做目标终态判定（§6.5）：目标订单（`VenueRef` 即该 `venue_order_id`；`IdemKey` 取能证明属于记下该键的那条腿的记录所带的 `venue_order_id`）在进入时该作用域所挂订单状态流（按进入之前最近的声明版本定下，此后不变）上的最近观察（§8.1）是终态（剩余量口径下还须带 `cumulative_filled_quantity`），就在这个事务 append `TargetTerminal`。撤单腿还 `Undetermined` 时就已推送到达的目标终态（例如外部订单的 `External` 推送），撤单腿之后经 `Manual` 终结时同样在此成立。
   - 判定不成立时，IO 壳按 `target` 身份对这组流中 `read` 为 `Supported`、请求 schema 能表达该身份的各条逐条 `read`（`VenueRef` 写 venue 订单身份，`IdemKey` 写调用方键，§6.5），直到目标终态或 `deadline`。这些读的记录带 `OneShot{origins ∋ Attempt((p, 2))}`，不产生 `ResolutionEvidence`。每个 append 目标订单记录的事务（读、推送、回执、取证、回填，不论归因）都重新判定；选中的若是带序号而较旧的终态，或目标订单跨流冲突、`IdemKey` 解析出两笔订单，判定不成立，链继续等。没有可读的候选流时只靠这些到达的记录。
   - 判定成立的事务 append `TargetTerminal`，新单大小按意图口径算出并记在其中（§6.2、§6.5）；数量 ≤ 0 则链 `Resolved`、不发新单腿。新单腿同样过发出前门（§6.5），`submit` 的参数带这个大小、不带 `target`（§8.2）：该集成此刻无会话，或当前能力对该意图不可执行、或声明的计划已不是两腿时，新单腿等待而不发，`deadline` 到时 `Expired(leg = 2)`；会话断开期间 IO 壳也不发目标终态的读。
   - 对外可见：读模型显示原操作与后续操作的关联及未决状态。
3. 取证记录模型（§6.5）：撤单腿的每次命中取证，同一事务落一条观察记录（`provenance: Reconciliation{AttemptRef}`）与一条 `ResolutionEvidence{Found}`（含 `Evidence`）；`Absent`/`Inconclusive` 只有 `ResolutionEvidence`。
4. 链的时限即该意图的 `deadline`（H6；`deadline` 处理器，§8.1），由 IO 壳在每条腿的发出前门与 `AwaitingTargetTerminal` 等待期间读取（§6.5）：
   - `AwaitingTargetTerminal` 期间、尚无 `TargetTerminal` 时到期 → append `Expired(deadline)`（`leg = 2`），不再判定目标终态、也不 append `TargetTerminal`，链 `Resolved`，新单腿永不发出、不补偿，链移出阻塞头集合。已有 `TargetTerminal` 而新单腿在发出前门等到到期，才是“`TargetTerminal` 之后 `Expired(leg = 2)`”。
   - 撤单腿仍 `Undetermined` 时到期**不终结链**：未知的写只能以证据终结（转移表，§6.5；未终结 Attempt 阻塞，§6.4），链留在阻塞头集合。
   - 撤单腿之后终结于 `VenueAccepted`/`Found`，则进入 `AwaitingTargetTerminal`，并因 `deadline` 已过立即 `Expired(leg = 2)`，即使目标终态已在记录里（`deadline` 优先，§6.5）。
   - 撤单腿之后终结于 `VenueRejected`/`Absent`，则链直接 `Resolved`。
5. 腿身份 `AttemptRef` 使撤单腿的回执观察不会被当作新单腿的归因证据（代数，§6.5）。

**走通**（验收 §10.5 #14/#17）。

### W13（Q28）保留边界推进与被引用位置

**失败路径。**

1. retention 推进时，仍有单据 `basis` 或程序 `Checkpoint` cursor 引用的 `LogPosition`（§2.4、§5.1）。
   - 核心 `Window` 节点的累加器随 `Checkpoint` 持久化，不回读历史。
   - `Pooled` 窗口从 `Journal` 重洗历史但不登记，越界只得 `BeyondRetention`（条件 4，§8.7）。
   - 观察侧可压缩（`compact_below_retention` 仅对 `RetractableDelta`，§4.1、§7.4）；执行事实只 append 不压缩（§3.1）。
2. 边界推进前必须显式处理仍被引用的位置（不变量 §6.9-3）：保留对应历史、保存必要证据，或缩小重放承诺（`AS OF ≥ retention frontier`，§2.4）；否则拒绝推进。
   - 恢复者：核心（保留协议）。
   - 落到边界下的 `basis` 位置在 prepare 只读校验时为 `BeyondRetention`（§5.2）。
   - 对外可见：压缩后所有仍被引用的 `LogPosition` 均在保留边界内。
3. 登记与审批（§2.4）：
   - 引用由核心在 `Prepared`/`Checkpoint`/`ResolutionEvidence` append 时自动登记。
   - 推进经控制面 `advance_retention(to)`，`to` 每条流一个新边界、逐流判定：越过该流已登记引用最早位置 → `Rejected(ReferencedBelow{min})`；越过该流留存窗口下界 → `Rejected(InsideWindow{bound})`。
   - 留存窗口是运行期参数（§7.6）。
   - 对外可见：控制记录可读；压缩后无已登记引用落到其所在流的边界之下，执行事实无记录被删（§10.5 #4）。

**走通。**

### W14（Q29）下游（Alice）断连重连

**失败路径。**

1. Alice 或解释层崩溃 / 重启，UTA 独立存活；订阅与程序 owner 是核心（H5/C5、§7.1）。解释层不持有状态，没有要恢复的东西（`design/downstream/design.md` 3.3）。行动者：核心（生命周期独立）。
2. Alice 凭续传令牌重连；解释层代它开核心会话（P13），经**读模型**（§4.4、§8.5）取当前状态，并取 cursor 之后记录。断连期间损失以 `Gap{origin: Delivery}` 显式标记，不伪造逐条补发（§8.5）。
   - 对外可见：当前状态、断连后的推送、缺失通知（由解释层翻译）。
3. 重连协议（§8.5）：
   - `handshake(contract_version, actor)` 取 `principal = (os_user, actor)`；
   - 同一 principal 的持久订阅自动重新挂接，投递从已确认 cursor 续（不需再 `subscribe`；新需求才 `subscribe`）；
   - 解释层还持有各来源的执行事实订阅（§8.5 订阅组），重连后同样从已确认 cursor 续：断连期间的待审事项、结果未知与取证终结都会补到，执行事实订阅没有投递损失；声明版本或 `CapabilityObserved` 到达时重读 `sources`。
   - `read_model(kind)` 得 `Snapshot`（`subscriptions`、`tickets` 只给当前态）；
   - 未确认区间可能重复可见，解释层按 `LogPosition` 去重或原样标出（§4.2）。
   - 对外可见：重复只出现在未确认区间；`as_of` 与 cursor 只在解释层与核心之间比对。

**走通。**

### W15（Q24）含 Pooled 的程序：无子系统 / 有子系统且 op 崩溃

**环境一：无子系统（装载期拒绝）。**

1. 程序含 `DerivationNode::Pooled{input, window}`（§4.5、§2.5）。子系统未安装 → 该程序在**装载期被拒绝**（§8.7），错误指出依赖未安装子系统；其余不含 `Pooled` 的程序不受影响（核心层最小验收，§8.7）。
   - 恢复者：装载器（输出类型 fold，§2.5）。
   - 对外可见：含 `Pooled` 程序被拒，其余程序照常装载运行。

**环境二：有子系统且 op 崩溃。**

2. 四条前置条件由输出类型 fold 在装载期判定（定长 / 位置线性 / 无指针 / 可容忍 ring 回收，§8.7）；满足则 `Pooled` 输入交子系统物化为段视图（一次洗入，§8.7；实现见 hpc-derivation/design.md §1.4–§1.5、§5）。
   - 行动者：子系统洗入器；段身份由子系统契约表标定（hpc-derivation/design.md §5.2）。
3. 原生 op 借用只读段计算、发布输出派生流；下游像读任何派生流一样读它（§8.7）。
   - op 进程 panic/OOM → 只死计算进程，核心记失败观察（观察 J 派生失败记录），**不改名**为 `Gap{origin: Source}` 或 `NoResponse`（§8.7；hpc-derivation/design.md §5.3/§6.3）。
   - 恢复者：子系统的借用回收 + H10 fence（hpc-derivation/design.md §5.3/§6.5）。
4. 核心崩溃时 op 成孤儿，由 fence 回收（§7.2；hpc-derivation/design.md §6.5）。若消费者产生外部写，仍经单据 → STS → IO 壳（§8.7，走 W1）。
5. 核心层对外可见结论到此闭合：装载期拒绝、失败观察、孤儿回收、外部写仍经效应路径，都由 §8.7 与 §7.2 给出。段的跨平台映射与回收、布局 hash 规范化、触发与合并语义是子系统内部决定（hpc-derivation/design.md §5/§6，验收其 §10），不改变核心可见结论。

**走通**（核心层最小验收 §10.5 #7；子系统验收在其 §10）。

### W16 行情修订撤回旧派生信号但不动已发执行事实

**正常路径（唯一边 + 撤回代数不对称）。**

1. 迟到 tick 修订 bar：派生侧以 `RetractableDelta` 撤回旧贡献、加新贡献、重算受影响子图（§4.1、§3.1、§4.3）；旧派生信号（alert）随之撤回（观察 J，可撤回可压缩）。
   - 行动者：派生 DAG（解释①，§4.3）。
2. 若该信号此前已产生意图并已发出（`SendBarrier`/`VenueAccepted`）：执行事实**只 append，永不改写为“从未发生”**（不变量 §6.9-2）。
   - 已发出的买单仍然存在；反向交易属于新行为（§3.1）。
   - 执行事实侧引用不因年龄变假（§5.2）。
3. 关联的单据 `basis` 引用被撤回的派生位置 → `basis_validity` 变 `Retracted`/`Stale`（§5.2），单据呈 `Diverged`（偏离是状态，§6.2）。
   - `AwaitingDecision` 单据的审批人看到偏离；`Prepared` 之后则由对账（resolution）而非撤回代数处理。
   - 对外可见：派生信号消失，但发送历史与已发意图完整保留；撤回不跨过唯一边（反向不存在，§5.1）。

**走通**：清晰演示唯一边（§5）+ 撤回 / append 不对称（§3.1）：行情修订撤回旧派生信号、不抹发送历史。

### W17 程序 Emit 读处理器（fetch.bars）闭环走观察侧

**正常路径。**

1. 程序解释②满足条件 `Emit(EffectRequest{effect_kind: fetch.bars, basis, key})`（§6.1），注册为**读处理器**（§6.1、§7.3）。
   - 行动者：程序宿主（装载 principal）。
2. 读处理器**立即执行**（读副作用，§3.4）：按请求值定出目标流与参数，走 §8.2 `read` 的判定顺序；来源有已建立会话、最近声明版本对该流 `read` 为 `Supported`、参数合该流 `request_schema` 时，经 `read(stream, request, range)` 调用集成（§8.2）。
   - 集成作答：同一事务在该流上 append 每个结果项一条观察记录（与推送观察同形，质量标记 `one_shot`）与一条读结论记录，都带 `provenance: OneShot{origins ∋ Request(该 EffectRequest 位置), request}`；`EffectResponse` 指向读结论记录（观察 J / 执行 J；读即观察记录，§3.4）。回答为空时只有读结论记录。
   - 恢复者：读可重试、可换渠道、可标 `Gap{origin: Channel}`（§4.2、§3.4），无 in-doubt。核心崩溃于响应持久化前，则按 §9.2 #21 重派。
3. 程序按位置推进看到该观察记录与读结论（cursor 之后；输入 = 位置推进，§4.3）：**闭环走观察侧**，不进单据 / STS / IO 壳（读 / 写处理器分派，§6.1）。
   - 对外可见：`fetch.bars`（读）与 `trade.place`（写）、`notify.telegram`（写）对程序是同一构造子 `EffectRequest`，差别在注册的读 / 写（§6.1 不变量）。
4. 未调用集成（Q16 同型）：该流最近声明版本的 `read` 为 `Unsupported` 或 `Unknown`、来源此刻无会话、参数不合 schema 时，不调用集成、不 append 观察记录；`EffectResponse` 记下这一结论（不支持 / 未确认 / 无会话 / 请求不合法），程序的决策半边从自己请求的 `EffectResponse` 看到它（§6.1）。无论哪种都不返回看似成功的空数组（§8.2、F6、C2）。

**走通。**

### W18（Q8+Q9+Q10）送审即否决、人工审批与过期、决定版本冲突、冷却

**失败路径（Q8，三种输入）。**

1. 消费方经会话（P13，principal 已认证，§8.5、C11）提交三笔意图：instrument 属他账户、数量非法或参数不合该来源的意图参数 schema（例如限价单缺限价）、只读账户下单。
   - 每笔经 `draft(intent)`（§8.5）以会话 principal 为 `responsible` 开单 `Draft`（§6.2），append 单据记录（执行 J）。消费方开单不经出站写处理器：写处理器只接程序的 `EffectRequest`，以装载 principal 开单（§6.1）。
   - 参数不合规不挡开单：单据 fold 的 `parameter_validity` 从 `Draft` 起即为 `Invalid(违反项)`，负责人经读模型 `tickets` 看得到，可 `revise` 改正（§6.2 参数合规）。锚点构造不出的请求（如缺 `WriteLaneKey`）才在 `draft` 被拒 `Rejected(Malformed)`，不开单、不 append。
   - 行动者：单据，身份 = principal。
2. 负责人经 `submit_for_decision`（§8.5）送审后，STS 链逐步读记录（§6.3）：
   - 只读账户在**授权**步被拒（`Rejection::Unauthorized`），另 append 安全事件记录（C11）；
   - 他账户 instrument 与参数不合规在**输入约束**步被拒：该步读 `parameter_validity`，并校验 instrument 归属与策略的允许集合（C9/C10），一次否决列出全部违反；
   - 每笔否决 = `Close(DecisionRejected)` + 规则 `Rejection` 记录，带 principal、时间、依据位置、违反项与 `rule_version`（执行 J）；
   - 链在拒绝处停止，后续步不执行。
   - 对外可见：三对意图 / 否决记录；安全事件可读；fixture venue 调用数 = 0，无 `Prepared`。
   - 程序变体：程序 `Emit` 同样的非法参数，写处理器 `Draft` + `SubmitForDecision` 同一事务（§6.1），输入约束步给出同形的否决；程序经自己请求的 `EffectResponse{Drafted}` 与单据状态看到结果。请求载荷构造不出锚点时得 `EffectResponse{NotDrafted(Malformed)}`，不开单、不重派。

**正常—失败路径（Q9，人工审批与过期）。**

3. 策略要求人工：**审批**步把单据留在 `AwaitingDecision(current_version)`（§6.2），待决集合对审批人可见（读模型，§4.4）。
   - 审批人 principal 提交 Decision，引用 `current_version` 与期望的待决集合版本（C11）。
   - 批准 → 链继续 lane（阻塞头集合为空后判冷却，§6.3）→ 过期 → 放行，append `Prepared` + `Close(Prepared)`（W1 步 4）；否决 → `Close(DecisionRejected)`。
   - 冷却变体：同 `(WriteLaneKey, instrument)` 上一笔下单腿的 `SendBarrier` 距今不足该 `(WriteLaneKey, OperationKind)` 的冷却间隔 → lane 步 `Rejection::Cooldown{until}` + `Close(DecisionRejected)`；已批准不豁免冷却，`bypass_lane` 也不豁免。
   - 每条 Decision 带谁、何时、依据 `LogPosition`、绑定版本（执行 J）。
4. 另一笔超过 `deadline` 仍未获决定：**过期**步按 `Input::超时` 触发 → `Close(Expired)`（H6；状态机允许 `AwaitingDecision → Closed`，§6.2），不补偿、不递送。
   - 对外可见：读模型可回答谁、何时、依据什么批准；过期意图无 `Prepared`/`SendBarrier`。

**失败路径（Q10，决定版本冲突）。**

5. 两个 principal 以同一期望版本对同一意图作决定：第一条 Decision 记录绑定 `current_version`；第二条到达时该 `(ticket, current_version)` 已有 Decision → `Conflict(AlreadyDecided)`（一个版本至多一条 Decision，§6.3），不执行、不改状态。
   - 无论第一条决定之后单据已 `Close(Prepared)`/`Close(DecisionRejected)`，还是仍停在 lane 步、版本未变，结果相同。
   - 对外可见：冲突记录存在；意图状态不变。

**走通**（策略表示在 §7.6、§8.5：principal → scope 表、人工审批条件与名义阈值、允许集合、冷却间隔、检查目录的必要项与参数，版本 = 内容 hash；验收 §10.5 #11、#30、#32）。

### W19（Q19）会话身份与未授权控制

**失败路径（三类输入）。**

1. 未认证连接发起写或控制请求：会话未完成 `handshake`（传输给出 OS 对端凭据，`principal = (os_user, actor)`，§8.5），核心的会话入口拒绝（§7.3），不进入任何处理器。
   - 会话入口 append 安全事件记录（执行 J，P7）。
   - 对外可见：无单据、无 venue 调用、无配置变更。
2. 已认证会话在请求体里伪造另一 principal：单据操作（`draft` 等，§8.5）只取**会话绑定的 principal**（C11 “每个 P13 会话绑定一个 principal”），请求体身份字段不参与授权。
   - 以会话 principal 开单后，授权步按 (principal, `WriteLaneKey`, `OperationKind`) 判定；越权则 `Close(DecisionRejected)` + STS 授权步 append 的安全事件。
   - 对外可见：安全事件可读；无 `Prepared`。
3. 已认证但无 scope 的控制请求（`load_program`/`rotate_credential`/`restart_integration`/`request_snapshot` 等，§8.5）：控制动作按 (principal, 动作种类) 授权。策略文件（§7.6）的 scope 不含该动作 → 控制记录 `Rejected(Unauthorized)`，控制面另 append 安全事件，无其他副作用。
   - 对外可见：配置文件未变（每文件唯一写者，§7.6）、集成会话未重建、除控制记录与安全事件外没有记录。

**走通**（验收 §10.5 #13）。

### W20（Q13+Q15+Q16）解释层取得声明、按账户与无账户来源读、推送待审与结果未知

**正常路径（取得声明与账户）。**

1. 集成 X（两个作用域）与公共行情来源 P（无作用域）各自握手成功：核心各 append 一个声明版本（执行 J，§7.5、§8.2 `handshake`）；X 的两个 `account_ref` 都可解析（§2.2）。
   - 行动者：核心握手路径，身份 = 会话 epoch（§7.2）。
2. 解释层 `read_model(sources)`（§8.5）：得到 X 的两个账户（`account_ref`、`label`）与各自挂的流、写能力，P 的流及其 `read` / `backfill` 能力与名义数据等级；P 不出现在账户列表里。
   - 解释层再以 `(X, 作用域?)`、`(P)` 订阅执行事实（`ordered`，§8.5 订阅组）。
   - 对外可见：账户列表按 (来源, `account_ref`) 给出；能力翻成“支持 / 不支持 / 未确认”；`WriteLaneKey` 与 `Verdict` 不外露（`design/downstream/design.md` 第 2、4 节）。

**正常—失败路径（一次性读，Q15/Q16）。**

3. 下游读 X 两个账户的持仓与 P 的某 instrument 报价：解释层按 `sources` 把 (账户, 持仓) 解析到各作用域唯一的持仓流，把 (P, 报价) 解析到 P 的报价流，发 `read(targets, deadline)`，每个 target 带流的 `request_schema` 身份与参数（§8.5 一次性读）。
   - X 此刻会话在重连（`health` 为 Connecting）：两个持仓 target 得 `Unavailable{source_state}`，核心不调用集成、不记 gap（§8.2 `read`）。
   - P 有会话、报价流 `read` 为 `Supported`：集成作答，同一事务 append 报价记录（`one_shot`）与读结论记录，target 得 `Answered{conclusion, items}`。
   - 若 P 的某条历史 bar 流 `read` 为 `Unknown`：该 target 得 `Unconfirmed`，不调用；为 `Unsupported` 则得 `Unsupported`。
   - 上游对某 instrument 明确拒绝（未开通该行情）：得 `Refused{conclusion, reason}`，该流上只有读结论记录，流的能力不变。
   - P 对另一个 instrument 迟迟不答，`deadline` 先到：该 target 得 `Pending`，没有结论也没有 gap；稍后上游作答，该流上照常 append 结论记录（同一请求身份，`origins` 含该会话），订阅了该流的解释层由此收到结果。
   - 对外可见：各 target 独立；“来源重连中”“不支持”“能力未确认”“来源拒绝”“尚未作答”“空结果”彼此可区分，没有看似成功的空数组。
4. 下游在一个订阅里选 P 的报价流（属配额池，带主体集）、P 最近声明里没有的一条流、X 某账户的持仓流三项：第二项被拒，第一项的主体集并入后超过配额池上限得 `QuotaExceeded{quota, limit}`，第三项为“活”；订阅照常建立，逐项结果交给下游，集成不收到超限主体（§8.5 订阅组）。
   - X 会话建立后，核心对该持仓流 `route` 一次全集（`All`，§8.2 `route`）；此后 X 的持仓推送才到达该订阅。

**扩展路径（声明变化与推送）。**

5. X 重连成功，新声明版本把某作用域的 `account_ref` 改成另一个值：核心照常按键路由，该引用在新声明版本里标为不可解析（§2.2）。解释层的执行事实订阅收到新声明版本，重读 `sources`：该账户显示为“账户引用冲突，需要处理”，按旧名字的命令不再解析到任何账户。
6. X 上一张单据送审、另一条腿进入 `Undetermined`：两条执行事实经解释层的执行事实订阅到达，翻成“待审事项”“结果未知，正在核实，请勿重下”；取证终结的 `ResolutionEvidence` 到达时翻成“已确认发生 / 未发生”。待审事项此刻是否偏离，解释层在呈现时读 `tickets`（§8.5）。
   - 对外可见：推送经执行事实订阅，重连后从已确认 cursor 续，没有投递损失。

**走通**（验收 §10.5 #25–#29、#48、#49）。

## 9.2 崩溃矩阵（#1–#21）

> 图：D7.1 恢复判定、D7.2 崩溃窗口在时序上的位置、D7.3 脑裂/孤儿、D7.4 程序侧/派生侧/快照崩溃；逐行对照见 D7.5（`design/diagrams/07-crash-recovery.md`）。

行 = 崩溃窗口；列 = 崩溃后持久状态 / 重启后恢复动作（由谁）/ 对外可见结果 / 依据 / 验收项（§10.5 编号或子系统验收编号）。每行恢复结论由状态机（§6.2、§6.5）与存储归属（§7.4、§7.5）推出。

| # | 崩溃窗口 | 崩溃后持久状态 | 重启后恢复动作（由谁） | 对外可见结果 | 依据 | 验收 |
|---|---|---|---|---|---|---|
| 1 | 单据 `Close(Prepared)` + `Prepared` 同事务中途 | 事务未提交 → 二者皆无 | 核心：SQLite 事务原子回滚；单据仍 `AwaitingDecision` | 无 `Prepared`；单据可重新放行 | §6.2；§7.4（同事务） | §10.5 #8(d) |
| 2 | `Prepared` 已持久、`SendBarrier` 未持久 | `Prepared` 有、无 `SendBarrier` | IO 壳：确未发出 → 过发出前门（§6.5）：`deadline` 未过且该集成会话已建立、意图对当前能力可执行，则 durable append `SendBarrier` 后调用该腿的写操作；会话或能力条件不成立则等待（不 append 记录）；`deadline` 已过则 append `Expired(deadline)` 终结该链 | 崩溃窗口不产生 venue 调用；不误升 `Undetermined`；末态为“已发”（可能先等待）或 `Expired` | §6.7；不变量 §6.9-8 | §10.5 #8(a)/#14 |
| 3 | `SendBarrier` 已 fsync、写调用未发 | `SendBarrier` 有、无后继 | IO 壳：可能已发出 → append `Undetermined(CrashWindow)` 进对账 | 尝试 = `Undetermined`；lane 阻塞；无第二 `SendBarrier` | §6.7；不变量 §6.9-1/8 | §10.5 #8(b)(c) |
| 4 | 写调用已发、回执未到 | `SendBarrier` 有、无回执 | IO 壳：同 #3，对账驱动按渠道取证收敛 | `Undetermined` → found/absent/人工 | §6.6；C1/C2 | §10.5 #3/#17 |
| 5 | 回执已到、未 append | `SendBarrier` 有、回执丢在内存 | IO 壳：视为无后继 → `Undetermined` → 对账（按键回读会重得同一状态） | `Undetermined` 经 `ResolutionEvidence{ByKey, Found}` 收敛；`Evidence` 在执行 J，该回应的观察记录在观察 J | §6.6；§3.4（读可重试） | §10.5 #3/#17 |
| 6 | 记录已 append 提交，投递/cursor 推进前崩溃 | 记录已提交、cursor 未推进 | 核心：`fold_state` 重建；订阅者从已确认 cursor 之后重收，未确认的记录可重复可见，按 `LogPosition` 去重 | 记录不丢；重复只出现在未确认区间 | §4.2；§7.4 | — |
| 7 | 对账取证中途 | 部分 `ResolutionEvidence` 已 append | IO 壳：取证是读副作用、可重放；未收敛者继续按渠道取证（下一渠道由 fold 重建） | 收敛进度不丢；渠道穷尽仍 `inconclusive` 停人工 | §6.5；§6.6 | §10.5 #17 |
| 8 | `Replace` 两腿计划：cancel 腿（`leg = 1`）终态已持久、new 腿（`leg = 2`）未过 `SendBarrier` 时崩溃 | cancel 腿终态有；可能已有 `TargetTerminal`；无 `leg = 2` 的 `SendBarrier` | IO 壳：见表下 | 复合操作按链续；new 腿未重复；已完的链不被再驱动 | §6.5；§6.7；不变量 §6.9-8 | §10.5 #8(c)/#14 |
| 9 | 观察 append 中途 | 半写事务未提交 | 核心：单写者原子回滚；半写不可见；`fold_state` 重建 | 无半条记录；订阅者按 cursor 续接 | §4.1；§7.4；不变量 §6.9-2 | §10.5 #8(d) |
| 10 | 派生 DAG 重算中途 | 派生记录部分 append（`RetractableDelta`） | 核心：派生侧可重算，未提交贡献重建；无自反馈环 | 派生结果最终一致；可撤回可压缩 | §4.1；§4.3 | — |
| 11 | 快照写入中途 | 快照部分写、原记录完整 | 核心：快照仅加速；半写快照丢弃，从保留边界 `fold_state` 重建 | 不改 append-only 语义；重启延迟增大 | §7.4；§7.5 | — |
| 12 | 配置文件重载中途 | 统一路径文件原子替换半途 | 核心：见表下 | 重载成功或整体拒绝；无半写可见态 | §7.6；C14；O9 | — |
| 13 | 集成崩溃（观察流侧） | 观察流断代 | 核心+集成：记 `Gap{origin: Source}`，按回填补齐或标 gap | 该流 gap 显式；核心与其他流不受影响 | §4.2；§6.7（两故障面）；§8.4 | §10.5 #9 |
| 14 | 集成崩溃（写调用中） | 已 `SendBarrier`、`submit`/`cancel` 中途 | IO 壳：`NoResponse` = `Undetermined` → 对账；不区分“集成挂”与“venue 没回”；会话结束时在途调用恰好完成一次（§7.2 第 3 步） | 尝试 `Undetermined`，靠证据非猜 | §6.7；§8.3 | §10.5 #9 |
| 15 | 旧核心已退出但其集成/宿主进程仍存活（孤儿），新核心接管 | 孤儿进程持旧会话 epoch、可能有在途回执 | 新实例：见表下 | 不产生双写；旧会话回执不经旧会话进入 | §7.2；§8.3；§6.6 | §10.5 #8(c) |
| 16 | 程序宿主崩溃，或核心在 `Advance` 输出持久化前崩溃 | 程序 state 依最近已提交的 `Checkpoint`；未提交的 `Output` 整体不存在 | 核心：从与 cursor 同事务持久化的 `Checkpoint` 重新 `Load`，重放该 cursor 之后的记录（§8.6）；`state_version` 不兼容则 `Reset` 记录 + 冷启动回填（H9） | 程序从 checkpoint 续跑，不重复 `Emit`；或显式冷启动 | §4.3；§7.5；§8.6 | §10.5 #16 |
| 17 | 可选子系统 op 崩溃 | 段借用未释放 | 子系统回收借用 + H10 fence；核心记失败观察 | op 失败观察；核心与其他消费者不受影响 | §8.7；hpc-derivation/design.md §5.3/§6.5 | hpc §10 #3/#4 |
| 18 | 可选子系统 op 已产生结果、核心在结果持久化前崩溃/断连 | 输出段在段池、未接入派生流持久点 | 核心：段池不持久，重启由 `Pooled` 重洗重算；未发布结果不半接入下游 | 结果重算；下游只见成功发布的派生流 | §8.7；hpc-derivation/design.md §1.5/§5.3 | hpc §10 #7/#13 |
| 19 | 核心崩溃时可选子系统 op 孤儿 | op 进程存活、核心死 | 新核心 + fence：op 为孤儿由 H10 fence 回收；重连后重新握手 | 无双写；孤儿被回收 | §7.2；hpc-derivation/design.md §6.5 | hpc §10 #3 |
| 20 | Alice（或其他下游）或解释层崩溃 | 核心订阅/程序/lane 完整；解释层无持久状态 | 核心：独立存活；下游凭续传令牌重连，解释层代它握手取 principal，读模型取 `as_of`，从已确认 cursor 之后订阅原始记录（§8.5） | 断连损失以缺失通知给出，不伪造补发 | §7.1；§8.5；H5/C5；`design/downstream/design.md` 3.3 | §10.5 #23 |
| 21 | `EffectRequest` 记录已随 `Advance` 输出提交，处理器未执行或其 `EffectResponse` 未持久化 | `EffectRequest` 有、无 `EffectResponse` | 核心：见表下 | 每条请求最终恰一条 `EffectResponse`；写至多一张单据；`Unhandled` 不重派；读结论等观察记录被压缩不影响判定 | §6.1；§8.6；§3.4 | §10.5 #16 |

**#8 的恢复动作（IO 壳）：**

- 先 fold 链是否已 `Resolved`：cancel 腿 `VenueRejected`/`Absent`/`Expired`，或已有数量 ≤ 0 的 `TargetTerminal` → 已完，无动作。链是否为两腿由 cancel 腿的 `SendBarrier` 定，不重读声明。
- 否则若已有数量 > 0 的 `TargetTerminal`：新单量取该记录，不重算，new 腿过发出前门后发出（确未发出，同 #2 语义；会话、能力或计划条件不成立则等待）。
- 否则链处于 `AwaitingTargetTerminal`：不对已有记录重新判定（判定在进入等待与此后每个 append 目标订单记录的事务里都已求值，§6.5）；该集成会话建立后按目标身份 `read`，此后到达的目标订单记录使目标终态判定成立时，同事务 append `TargetTerminal`，再按上一条处理。
- `deadline` 已过：尚无 `TargetTerminal` 的链记 `Expired(deadline)`（`leg = 2`），不再判定目标终态；已有 `TargetTerminal` 的新单腿在发出前门记 `Expired(deadline)`。两者 new 腿都永不发。

**#12 的恢复动作（核心）：**

- rename 未落，则旧文件完整可读；rename 已落，则新文件完整。
- 重载失败保留上一有效版本，并 append 控制结果 `Rejected(reason)`。
- 启动期失败按 C14 拒绝启动。

**#15 的恢复动作（新实例）：**

- 取 fence、`instance_id` 加一；按进程表回收孤儿（§7.2 第 1 步）。
- 旧 epoch 推送与回执在边界丢弃。
- venue 已受理的写，经新会话观察记录或对账取证并入同一 Attempt（§8.3、W2 步 4）。

**#21 的恢复动作（核心）：** fold 出无 `EffectResponse` 的已注册请求重派（§6.1）。

- 读处理器重新执行一次；
- 写处理器重新开单。`Draft` 与 `EffectResponse{Drafted}` 同事务，不存在“有 `Draft` 无响应”。

**可进验收（§10.5）的可测项：**

- #2/#3/#8 的 fsync 崩溃注入，证明“`Prepared` 无 `SendBarrier` 确未发、`SendBarrier` 无后继升 `Undetermined`、复合链不重复投放”（§10.5 #8/#14）；
- #3 的 fixture venue 调用 ≤ 1；
- #9 半写不可见、无半条记录；
- #1 同事务原子回滚；
- #13/#14 两故障面唯一判别边界（写调用路径与否）；
- #16/#21 程序状态跨重启与请求不重派为第二张单据（§10.5 #16）。

## 9.3 卡点

无。

- W1–W20 与崩溃矩阵 #1–#21 每步的行动者、恢复归属与对外可见结论，都由 §2–§8 已定内容推出。
- 需要实测才能给出数字的步（崩溃注入、宿主预算、秒级负载）是 §10.5 的验收项，不是卡点。
- 可选子系统的实现期数字是 hpc-derivation/design.md §10 的验收项，核心接口（§8.7）不依赖其结果。
