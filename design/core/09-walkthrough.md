# 9 走查

本章不新增任何定义。它把 §2–§8 已定的抽象放到具体刺激上逐步走，验证每条路径都能由已定设计闭合。

## 9.1 场景 trace（W1–W20）

> 图：W1 → D6.5；W2 → D6.6、D6.4、D7.3；W5 → D6.7；W14/W19 → D9.1；W17 → D4.1、D4.3；W18 → D5.6；W20 → D9.2、D9.5。

**每步写法：** 输入 → 经过哪个元素 / 抽象（§x.y）→ 输出 / append 的记录（持久化落点）→ 对外可见结果 → 当前唯一行动者 / 恢复者与其稳定身份或 fence 依据。

**判定：**

- **走通** = 整条路径能由已定设计闭合，且每步的行动者与恢复归属可推出。
- **卡点** = 缺概念 / 选错抽象 / 接口不够 / 归属不明，登记进 §9.3。

**持久化落点简称**（归属见 §7.5）：

- **观察 J** = 观察 `Journal`；
- **执行 J** = 执行事实 `Journal`（纯 append，§3.1、§7.4）；
- **单据记录**（`TicketAction`）、**订阅表/cursor**、**程序状态**、**快照**。规则状态（`RuleState`）不是落点：它每次由执行 J fold 出（§6.3）。

### W1（Q1）正常下单闭环

**正常路径。**

1. 程序解释②在满足规则时 `Emit(EffectRequest{effect_kind: trade.place, basis, ...})`（§6.1）。请求不带调用方键：键在发出时由核心铸造（§6.5）。
   - 行动者：程序宿主（§8.6），以其**装载 principal** 为身份。
   - 输出：`EffectRequest` 值，未定型。对外可见：无。
2. 核心出站写处理器接手（§6.1、§7.3），以装载 principal 为 `responsible` 开单 `Draft{basis}`（§6.2）。
   - append 单据记录（执行 J），与步 3 的 `SubmitForDecision` 及 `EffectResponse{Drafted}` 同一事务（§6.1）。
   - 对外可见：无；`Drafting` 只在该事务内出现，不对外可见。
3. 写处理器在同一事务内 `SubmitForDecision`（程序意图无编辑期，§6.1）：冻结 `current_version`，`Drafting → AwaitingDecision`（§6.2）。
   - 对外可见：事务提交后，审批人 / 读模型 `tickets` 看到一张 `AwaitingDecision` 单据，负责人 = 装载 principal。
   - STS 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期（§6.3）。
   - 放行前读单据 fold 的 `basis_validity == Fresh`、必要项 `alignment == Aligned`、版本一致（§6.3 放行门）。
   - append 规则的 Decision/`Outcome` 与单据记录（执行 J，同事务）；规则判断所用的状态由记录 fold 出，不另写（§6.3）。
   - 行动者：STS 规则链，身份 = 决定的 principal（不要求人工时为 `rule_version`）。
4. 放行：同一事务 append `Prepared` 并 `Close(Prepared(position))`（§6.2、§7.4）。
   - `Prepared` 是单据 → IO 壳的唯一交出点（§5.3）。
   - 行动者：单据（交出）→ IO 壳（接手）。
5. IO 壳过发出前门（§6.5），durable append `SendBarrier`（fsync，执行 J；记下写操作 `submit`、声明带键时核心以 `AttemptRef` 铸造的键及其角色），再调用集成 `submit(attempt)`（§8.2）。
   - 行动者：IO 壳，身份 = `attempt_position`（`Prepared` 的 `LogPosition`，锚点，§8.1）+ `WriteLaneKey`。
6. venue 受理并给出身份 → 集成 `submit` 返回 `Ack(venue_id, receipt)`。同一事务 append：
   - `VenueAccepted{venue_order_id, receipt: Evidence, observation}`（执行 J；`Evidence` = 契约载荷 + 原始负载，§6.5）；
   - 回执观察记录（观察 J，`provenance: Receipt{AttemptRef}`，记录模型，§6.5）：订单状态一条，由 IO 壳填 `attribution: FromAttempt(AttemptRef)`；回执已含成交时，每笔可识别的执行另落一条带 `execution_id` 的成交记录（§8.1“成交与订单状态的契约语义”），其归因按该条自己的关联证据填写（§5.3）。
7. 部分成交、成交依次到达 → 集成推送带 `attribution` 的订单状态 / 成交观察记录（观察 J，§8.3、P2、P9）；成交记录带 `execution_id`，数量与价格是本笔执行的量，订单状态带累计量（§8.1）。读模型 fold 出最终成交状态（§4.4、§8.5）。
   - 对外可见：订阅者依次收到受理、部分成交、成交，字段与原生身份保真（C13）；读模型最终 = 成交。
   - 程序看自己的结果（§6.1、§8.6 程序的输入）：它在请求流上读到自己的 `EffectResponse{Drafted(ticket_id)}`；它为该账户的作用域声明了执行事实输入，该 lane 流上的记录按位置投给它，解释②在其中按 `ticket_id` 找到单据的 `Close(Prepared(p))`，再按 `p` 认出这次尝试的 `VenueAccepted`；归因到 `p` 的订单状态与成交记录经它的观察输入到达。没有声明该作用域时，写处理器不开单，得 `NotDrafted(ScopeNotObserved)`。

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

**走通。** `Undetermined` 未触发本路径；其记录模型细节见 W2。集成在交出前发现不能发写（例如参数 schema 身份已不是它此刻接受的）时，步 6 改为 `submit` 返回 `NotSent`：append `NotSent{reason, evidence}`，等待结束、lane 解除，不进对账；冷却照常从 `SendBarrier` 起算（§6.5 `NotSent`）。

### W2（Q2+Q3）SendBarrier 后崩溃、三渠道收敛、放弃跟踪、脑裂变体

**正常—失败路径（SendBarrier 后崩溃）。**

1. 承 W1 步 5：IO 壳 durable append `SendBarrier`（fsync，执行 J）后、取得业务回执前，核心 `kill -9`。
   - 持久状态：`Prepared` 有、`SendBarrier` 有、无后继。
2. 重启：IO 壳从执行 J fold 各尝试的等待与结果（§6.7）。`SendBarrier` 无后继 → append `Undetermined(CrashWindow)`（执行 J），进入对账驱动。
   - 行动者 / 恢复者：IO 壳（核心内唯一效应处），身份 = `attempt_position` + `WriteLaneKey`。
   - 对外可见：该尝试 = `Undetermined`；lane 队首阻塞（§6.4）；无第二次 `SendBarrier`（不变量 §6.9-8）。
3. 收敛按 `SendBarrier` 所记写证明声明的渠道**自动**依次取证（§6.6、§8.2；读副作用可重试）。取证在该集成新会话建立后开始；会话建立之前 IO 壳不对它发读，也不记 `Gap{origin: Channel}`（§6.6）：
   - **按键回读** `query_by_key(key, key_role, scope, barrier_at)`：`Found(state)` → 同事务 观察记录 + `ResolutionEvidence{ByKey, Found}` → 结果确立、等待结束、移出阻塞头集合；`Absent` → `ResolutionEvidence{ByKey, Absent}`，结果确立（本次未发生）。集成按 `barrier_at` 判断该键已超出上游唯一期时返回 `Unavailable`，不返回 `Absent`（§8.3）。
   - **listing+身份** `list_open`/`list_fills`：命中归因到该尝试的订单 / 成交 → 同事务 观察记录 + `ResolutionEvidence{Found}`；未命中或空列表 → `Inconclusive`（F10：listing 未见不证明未递，本渠道无 `Absent`）。
   - **无渠道**：渠道穷尽仍 `Inconclusive` → append 后停等。停等由 `ReconciliationReopened`（集成会话重建 / principal 的 `retry_reconciliation`）重开一轮、任一时刻到达的归因观察（步 4）推进，或由 principal 的 `abandon` 结束等待（§6.6）。IO 壳永不 heuristic（C1/C2/C12）。
   - 对外可见：三种环境末态可枚举：found → 回执即观察记录；absent → 未发生；无渠道 → 停等，或放弃跟踪后“已放弃跟踪，结果未知”。
   - **放弃跟踪**：运维 principal 调 `abandon(attempt, note)`（§8.5）。IO 壳停发新的取证调用，等在途的 `query_by_key` 完成并记下它自己的结果；结果仍未知才 append `Abandoned{attempt, principal, note, rule_version}`。该尝试移出阻塞头集合，lane 放行后续写；此后不再自动取证，会话恢复也不重开它。之后到达的归因观察（步 4）或 principal 再发的 `retry_reconciliation` 仍可补上结果，但不恢复等待。对外可见：`Abandoned` 带 principal；结果补上前显示“已放弃跟踪，结果未知”。
4. 迟到回执收敛：
   - 旧会话的回执不经旧会话进入核心：旧会话的通道随会话结束关闭，已结束的会话化身的消息读不到、不进入核心（§7.2 会话 epoch、§8.3）。
   - 集成在新会话上以**观察记录**重新送达带可关联身份的回执（`venue_order_id`/`idempotency_key` 经 `attribution`，§5.3、§8.3）。
   - 效应侧归因处理器见该记录的 `attribution: FromAttempt(r)`（集成在它声明的键作用域与唯一期内依上游关联填写，§8.3），且 r 处于 `Undetermined`、结果未知，同一事务 append `ResolutionEvidence{Attributed, Found{observation: 该记录, evidence}}`（§6.6、§8.1），结果确立，不产生第二次下单。r 已 `Abandoned` 时同样补上结果，等待仍是 `Abandoned`。
   - 集成进程本身已死则无此路径，收敛全靠对账（步 3）。

**脑裂变体（Q2③）。** 旧核心退出前已把 `submit` 交给其集成进程 A；核心重启（新 `instance_id`）并建立新集成会话 B 后，A 作为孤儿仍可能把那次 `submit` 送达 venue。

- 安全性由三条不变量共同保证，不依赖 A 的回执到达：
  - `SendBarrier` durable = “可能已发出”（不变量 §6.9-8）；
  - 阻塞头集合非空时同 lane 无新普通写（不变量 §6.9-1，队首阻塞 §6.4）；
  - 对账驱动经会话 B 独立取证收敛（§6.6）。
- A 的回执不进入新核心：A 的通道只通向已退出的旧核心，新核心不读它（§7.2 会话 epoch、第 1 步）。
- A 按 §7.2 第 1 步被回收。
- 若 venue 已受理，B 的对账取证或新会话上的观察记录（步 4）把它并入同一尝试。
- 对外可见：fixture venue 调用 ≤ 1；该意图恰一条 `SendBarrier`。

**走通**（崩溃注入验收 §10.5 #8；取证记录模型验收 §10.5 #17）。

### W3（Q4）Prepared 未发前崩溃

**失败路径。**

1. 承 W1 步 4：同事务 append `Prepared` + `Close(Prepared)` 已提交，但 IO 壳尚未 durable append `SendBarrier` 时 `kill -9`。
   - 持久状态：`Prepared` 有、无 `SendBarrier`。
2. 重启：IO 壳 fold 各尝试。`Prepared` 无 `SendBarrier` = **确未发出**（不变量 §6.9-8），仍是可安全发送的 `Prepared`（§6.7）。
   - 恢复者：IO 壳。
   - 过发出前门（§6.5）：读 `Prepared` 自带的 `deadline` 与意图身份，查核心自己的会话状态与该 `(WriteLaneKey, OperationKind)` 的会话有效能力，不触碰单据（单据已 `Closed`，§6.2）。
   - 未过期、该集成会话已建立且会话有效能力可执行：durable append `SendBarrier` 后调用写操作（继续 W1）。
   - 未过期，但该集成尚无会话（重启后还在 `Connecting`，或 `Halted`），或会话有效能力对它不可执行：尝试保持 `Prepared`、不 append 任何记录，仍占该 lane 阻塞头；会话建立或能力恢复时再过门，`deadline` 到时 `Expired`。
   - 已过期：append `Expired(deadline)`，等待结束，结果是“未交出”，由 UTA 确知。意图**不误升为 `Undetermined`**、不补偿（H6）。
3. 对外可见：崩溃窗口本身不产生 venue 调用，也没有 `SendBarrier` 记录；末态可枚举为“已发”（经 W1）、“等待后已发”或 `Expired`，任一种都不经 `Undetermined`。

**走通**（崩溃注入验收 §10.5 #8(a)/#14）。

### W4（Q6）外部变更归因

**正常路径。**

1. venue 推送一笔对不上任何本地 `attempt` 的成交 / 余额变动 → 集成产出带 `attribution` 的观察记录（观察 J，P11、§8.3）。`attribution` 是集成依上游关联证据作出的判定：上游证据表明它不是经本集成发出的写时填 `External`；判断不了（例如只有一个调用方键，而它不在集成声明的键作用域与唯一期内）填 `Unattributed`（§5.3、§8.3）。
2. 效应侧归因处理器（注册在效应侧，读观察记录；记录归观察 / 响应归效应，§5.3）只看 `FromAttempt`：这条记录不是 `FromAttempt`，不归因到任何等待中的尝试，也不 append `ResolutionEvidence`。核心不把 `Unattributed` 判为 `External`，也不按键字节补归因（§6.5 调用方键由核心铸造）。
   - 对外可见：外部变更记录存在且无意图引用，归因照集成所填（`External` 或 `Unattributed`）；订阅者收到该记录。
3. 后续证据（对账 / 回读）可引用该记录的 `LogPosition` 作依据（位置作为关联，§2.3），不回改原记录（执行 J / 观察 J append-only）。

**走通。**

### W5（Q7）同 lane 并发与队首阻塞

**正常—失败路径。**

1. UI 与 AI 在 100 ms 内各提交一笔到同一 `WriteLaneKey`（交易协议下 = (账户, 子账户)，§6.4）。
   - 两笔各自经解释层代开的会话 `draft(intent)` + `submit_for_decision`（§8.5），以各自会话 principal 为 `responsible`，进入 `AwaitingDecision`；之后同 W1 步 3 的 STS 链。
   - STS 链的 lane 步按 append 顺序放行第一笔（W1 步 4，得 `Prepared`）。
   - 第二笔因 lane 已有等待中的尝试停在 lane 步（阻塞头集合由该 lane 的执行事实 fold 出，§6.4），单据保持 `AwaitingDecision`，不产生 `Prepared`。
   - 行动者：STS 规则链（lane 规则），身份 = `WriteLaneKey`。
2. 第一笔 `submit` 后进入 `Undetermined`（承 W2）。
   - 队首阻塞：阻塞头集合非空时同 lane 无新普通写（不变量 §6.9-1），第二笔继续停在 lane 步并告警。
   - 等待期间它的 `basis_validity`/`alignment` 照常重算（偏离是状态，§6.2）；`deadline` 到期则过期步 `Close(Expired)`（H6）。
   - 队首的等待结束（结果确立或被 principal 放弃）、集合清空后，lane 步因该 lane 上的新记录重新求值（§6.3 等待与重入）而放行；链先过过期步，再过依据有效性门（§6.3），再 append `Prepared`。
   - 理由：后续写语义依赖队首结果（buying power、venue 侧顺序），是通讯协议语义，非 UTA 的锁（§6.4、§6.8）。lane 等待发生在 `Prepared` 之前，所以不存在因 lane 而等待的已放行记录；已放行的尝试只可能在发出前门等会话或能力（§6.5）。
3. 另一账户（不同 `WriteLaneKey`）的写同时进行，不等待（H4；粒度权衡，§6.4）。
   - 对外可见：fixture 调用不重叠；`Undetermined` 期间无第二次调用；其他账户不受阻。

**扩展路径（撤阻塞头与显式绕过）。** 队首取证期间，lane 上读侧决议动作（按键查询 / listing / 对账）不是队列项（§6.4）。

- **撤阻塞头**：以第一笔下单尝试 `SendBarrier` 记为订单键的调用方键为 `target`（`IdemKey`）起一张撤单单据（按尝试精确匹配，§6.4）。该来源的撤单只接受 venue 订单身份时，这张单据在输入约束步得 `TargetNotAccepted`、`Prepared` 之前关闭，不会多出一次 `Undetermined` 的撤单尝试（§6.2 可执行性）；此时只剩显式绕过、等取证收敛或放弃跟踪。
  - STS 全链照走，lane 步不等待阻塞头（唯一不违反协议的写例外，§6.4）；放行后阻塞头集合 = {第一笔, 撤单}。
  - 撤单尝试自身的回执或 `Found` 只说明撤单请求到达了上游，不决议第一笔，也不证明第一笔已结束（§6.6 撤单尝试的取证）。
  - 撤单有了结果之后，第一笔仍只由它自己的取证与 `Attributed` 收敛；IO 壳不自动重开第一笔的取证。principal 可以看了撤单的结果后对第一笔发 `retry_reconciliation`，新一轮从 by-key 重走：读到目标（任何状态）即 `Found`；by-key 明确否定即 `Absent`；listing 未见仍 `Inconclusive`（F10，§6.6）。
- **显式绕过**：运维 principal 对第二笔 `bypass_lane(ticket)`，得控制记录 `Applied`（对协议的自觉违反，不是 Decision，§6.4），记下第二笔当时的 `current_version` 与当时的阻塞头位置集 {第一笔}。
  - 第二笔已获批准（或不需人工）时越过 lane 步得 `Prepared`，阻塞头集合扩大；未获批准则照常停在审批步，批准时这一版本仍受这条绕过覆盖。之后加入集合的阻塞头不在其内。
  - 各自的对账驱动独立收敛，任一的等待结束只移出自己，集合清空即 lane 解除。
- 对外可见：两种路径下该 lane 都有两条 `SendBarrier`（各属不同尝试）；前者无 `bypass_lane` 控制记录，后者有；第二笔单据的 Decision 仍只是它的批准；其他 lane 不受影响。

**走通。**

### W6（Q11+Q12+Q31）行情断线 gap、慢消费者、三种消费方式

**正常—失败路径（断线无续传，Q11）。**

1. 订阅 200 instrument tick（订阅表，§4.2、§7.3）；集成断线 30 s 重连，venue 无游标（F11/C6）。
   - 握手时集成不能以游标证明续接 → 集成会话结束前一 epoch、创建新 epoch。新 epoch 首条是 `Gap{origin: Source}`（观察 J，§4.2、§8.2 `handshake`），含前一 `StreamId` 与最后 `Seq`、原因 `disconnect`；同一事务写该逻辑流的 `None{epoch}`（§8.4）。
   - 恢复者：核心（`LogPosition` 与流 epoch 是它创建的，§2.3）；断代本身由来源的证据（不能续接）决定，核心不推断续接。
   - 重连握手后，核心对该流重发一次需求全集（这 200 个主体，§8.2 `route`；新会话里核心尚未接受该流的会话内 gap，`generation` 为 0）；集成在收到它之前不推送该流，`Routed` 的路由结论记录落在新 epoch 上，带这次生效的全集，标出这些主体在新 epoch 里的记录从这里起开始。序号覆盖另按整条流计，只在这条流被确认为 `All` 且 `refused` 为空之后才计入推送（§8.4）。同一会话里之后若集成再上报 `Gap{origin: Source}` 开新流 epoch，这条 gap 带集成加一后的 `generation`，核心接受它即为该流起一项 `route` 义务（已有则并入），此后发出的 `route` 带这个 `generation`，新 epoch 的供给由它的 `Routed` 确认；此前发出、带旧 `generation` 的 `route` 由集成答 `Unavailable`，核心不另判。
   - 断线期间与重连后握手之前，`health` 里该流的 readiness 是由会话状态派生的 `Disconnected`，核心不为它 append 任何记录（§8.4）。
   - 对外可见：订阅者先收到 gap 再收新 epoch 记录；无静默跳过。
2. **有游标变体**：集成重连报可信续传游标 → 续用原 epoch，`Seq` 接续，不新建 gap（§8.2 `handshake`、§8.4）。

**失败路径（慢消费者，Q12）。**

3. 三订阅者中一个不确认，缓冲耗尽 → 投递调度请持久订阅在该订阅的这条流上写下投递缺口 `Gap{origin: Delivery}`（原因 `slow_consumer`，带 from/to 位置，订阅表，§4.2、§7.5），写下之后对该订阅者停投并交出这条缺口，需显式确认。它是订阅的状态，不是流上的记录：流本身不缺这些记录，其他订阅者看不到它。
   - 行动者：投递调度检测、持久订阅记录（§7.3），身份 = （订阅，流），与该流的 cursor 同一粒度。
   - 对外可见：慢者收缺口且停投；两快者不受影响、cursor 持续推进（跨订阅者不阻塞）。
4. 确认语义（§4.2）：`ack` = 消费方已处理。缺口先于 `to` 之后的记录交出；慢者确认一个不低于缺口 `to`（被跳过的最后一个位置）的 cursor，即确认了这次损失，缺口与 cursor 推进在同一次写里从订阅表删除，投递从 `to` 之后续；确认之前重连，缺口先于记录再次交出。未确认区间可能重复可见，按 `LogPosition` 去重；已确认区间不重投。

**扩展路径（三种消费方式，Q31）。**

5. 同一组流被三种消费者声明消费（§4.2）：
   - `await-all` 按来源证据证明的覆盖触发，而非 cursor（§2.3、§8.4 序号覆盖）：只有这组 tick 流声明 `joinable_venue_seq`、程序在 `inputs` 里把它们声明为整条流的供给（§4.3、§8.6 程序的输入）、且它们不在配额池里（需求为 `All`）时才有序号覆盖，要求点是当前 epoch 的一个 venue 序号，覆盖越过它才触发；本走查按 200 个主体订阅，需求不是 `All`，没有序号覆盖，要求覆盖的 `await-all` 声明在装载时被拒，程序改为等核心日志位置、用 `ordered`，或声明自己的等待窗口；
   - `ordered` 按单流顺序背压，不跳过；
   - `latest/conflated` 允许合并，损失以订阅上的 `Gap{origin: Delivery, conflated}` 或消费者声明的等待窗口记。
   - 对外可见：每次合并可追溯到声明窗口或缺口（B5/C6）。读模型消费者拿到的 `Snapshot` 带 `as_of` 与 `gaps`（只列 `Gap{origin: Source}`，§8.5），与自己的 cursor 比对即知快照覆盖范围。

**走通。**

### W7（Q17）核心 append 中途崩溃

**失败路径。**

1. 核心在“追加观察记录”或“替换订阅表”中途 `kill -9`。存储为单个 SQLite 文件、单写者、每次 append 原子（§7.4）。
2. 重启：半写事务不提交，`fold_state` 从已提交记录重建（§4.1、§7.4）；半写状态不可见（§7.4 事务原子性）。
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

3. 控制面 `rotate_credential(integration)`（认证 principal，§8.5）：
   - 集成会话结束该集成的会话：在途的写得 `NoResponse`（→ `Undetermined`）、读得 `Unavailable`，各计数一次，然后关闭通道（§7.2 第 3 步）；请求该集成进程退出，OS 确认退出之后清除进程表的行。凭据的旧副本随这个进程结束（§7.1 凭据链）。
   - 之后才重读封存文件，拉起新进程，把凭据经继承句柄交给它，并为新通道分配新 `session_seq`（进入 `Connecting`）；新会话握手时各流开新 epoch，首条为 `Gap{origin: Source, reason: credential_rotated}`，由集成会话 append（§7.6）。其他集成 `Seq` 连续（§8.3）。
   - 恢复者：核心（进程与会话的结束、拉起与凭据交付）+ 该集成（握手）。
   - 对外可见：目标集成的旧进程退出先于新进程拉起；会话重建与原因可见；其他流不受影响。

**失败路径（上游拒绝凭据）。**

4. 新凭据被上游拒绝（或会话中凭据被吊销：集成关闭通道并退出，核心结束该会话、在旧进程 OS 确认退出之后拉起新进程再握手）：集成 `handshake` 返回 `Refused(reason)`（§8.2）。
   - 集成会话：会话状态 → `Halted{Refused(reason)}`，同一事务 append `IntegrationHalted`（P14）与健康观察；`IntegrationHalted` 记下的是核心不再自动握手的决定。提交之后才结束会话、终止进程，OS 确认退出之后清除进程表的行；核心在此之前崩溃的，由继任实例第 1 步回收。核心重启从执行事实恢复 `Halted`，也不解除（§7.2 第 3 步）。
   - 在途调用：会话结束时经该会话通道尚未返回的写得 `NoResponse`（→ `Undetermined`）、读得 `Unavailable`，各计数一次；通道随后关闭，旧会话的迟到回应读不到（§7.2 第 3 步）。
   - 写：该集成各 lane 上已放行未发出的尝试在发出前门等待（没有会话有效声明），`deadline` 到时 `Expired`；不产生 `SendBarrier`，也就不产生 `Undetermined`（§6.5）。已在 `Undetermined` 的尝试不取证，停在原处，会话重建后续跑；停等且等待 Active 者按 `SessionRestored` 重开（§6.6）。
   - 读：订阅不挂起、需求保留（§8.2）；该集成各流的 readiness 由 fold 派生为 `Disconnected`（§8.4），核心不为它们 append 记录。
   - 恢复：运维修好凭据后 `rotate_credential` 或 `restart_integration`：`Applied` 带被解除的 `IntegrationHalted` 位置，与 `Connecting` 健康观察同一事务提交，旧进程已 OS 确认退出之后才拉起新进程、握手。
   - 对外可见：健康里该集成是“停止并待处理：凭据被拒”，与“断开、正在重连”（`Connecting`）可区分；上游不再收到重复登录。

**失败路径（上游暂时不可达）。**

5. 握手时集成连不上上游：`handshake` 返回 `Unavailable`（§8.2），会话保持 `Connecting`，核心按 pacing 在同一通道上重握手，不换进程；其余同上一条的“写 / 读”两项，但无需运维动作，会话建立即恢复。

**扩展路径（集成登记改动）。**

6. Alice 在集成登记文件里新加一个集成 Y、删去一个集成 Z，核心不重读文件（§7.6）。
   - 运维 `restart_integration(Y)`：核心重读文件中 Y 的条目，`Applied` 带文件 hash（采纳），拉起 Y 的进程并握手（§7.2 第 3 步）。
   - 运维 `restart_integration(Z)`：Z 已不在文件里，得 `Rejected(UnknownIntegration)`；Z 本实例的运行照常继续。
   - 下一次启动：第 3 步的采纳记录不含 Z；Z 没有运行、没有会话，`health()` 不再列它；它的流记录、声明历史、健康观察与订阅都在，核心不为它补写记录；新的一次性读与供给项对它得“来源未登记”；以只投递项或执行事实 selector 订它历史上声明过的流与作用域仍被接纳，从 `from` 收到已 append 的记录（§8.5 订阅组）。
   - 对外可见：Z 显示为已移除的来源，历史与订阅保留，历史仍可订阅审计（`design/downstream/design.md` 第 5 节）。

**扩展 / 失败路径（重载失败）。**

- 文件原子替换（临时文件 + rename）保证无半写可见态。
- 重载失败保留上一有效版本，控制记录 `Rejected(reason)`（§7.6、§8.5）。
- 对外可见：控制记录可读；生效配置版本 hash 未变。

**走通**（验收 §10.5 #36、#79、#80）。

### W9（Q20）双实例

**失败路径。**

1. 第二个核心对同一用户状态根启动。单实例由 OS 文件锁 + fence 保证（H10、§7.1）；第二实例以专用退出码拒绝启动，**不做接管**。
   - 对外可见：退出码 / 诊断可观测；不产生双写。
   - 行动者：OS 文件锁持有者判定。
2. **持有者死亡后接管变体。** 原持有者死亡，锁随进程释放；它拉起的集成 / 宿主进程可能仍存活（父死子活，H10 的孤儿）。
   - 新实例取 fence 接管（§7.2），身份 = fence 令牌。
   - 接管后在途 `SendBarrier`/`Undetermined` 由新实例 IO 壳从执行 J 重建（§6.7，同 W2）。
3. 接管边界（§7.2 第 1 步）：
   - 新实例取 fence 即在实例表写入新一行、`instance_id` 加一；旧实例没有结束锚点，它的失权点 = 该事务提交，它的全部会话与在途调用随之结束。
   - 旧实例登记的集成 / 宿主进程按进程表回收：请求退出、超时强制终止，OS 确认退出之后清除该行。
   - 孤儿的通道只通向已退出的旧核心，新实例从不读它；新实例为每个集成拉起新进程、创建新通道（§7.2 第 3 步）。
   - 旧在途 `submit` 的命运与 W2 脑裂变体相同：由 `SendBarrier` 记录 + 新实例对账收敛，不依赖旧实例。
   - 对外可见：接管不产生双写（§7.1 单实例、§7.4 单写者，fence 与 H10）；进程表中无旧 `instance_id` 名下的存活进程。
4. **受控停止变体。** 原持有者收到 OS 的停止请求，按 §7.2“受控停止”从内到外结束：
   - 第 1 步关闭会话入口与全部消费方会话，核心不再发起新的集成调用；解释层看到连接关闭（`design/downstream/design.md` 第 5 节“服务已停止”）。
   - 第 2 步每个程序等在途 `Advance` 的事务提交后 `Unload`，活动集合不变。
   - 第 3 步每个集成会话以“在途调用恰好完成一次”结束：在途写得 `NoResponse`（→ `Undetermined`），在途读得 `Unavailable`（一次性读的 `Gap{origin: Channel}` 与计数同事务提交，`Pending` 的等待者重连后从 `from` 收到它）。
   - 第 4 步 OS 确认全部集成与宿主进程退出，进程表清空；第 5 步写实例结束锚点；第 6 步释放 fence。
   - 新实例启动：第 1 步没有要回收的行，第 2 步没有无后继的 `SendBarrier`；第 3 步为每个采纳的集成开始新的运行，第 5 步装载同一活动集合。
   - 有进程得不到 OS 确认退出时，原持有者不写结束锚点、不释放 fence，停止以失败报告；之后它被外力结束，按变体 2 接管。
   - 对外可见：受控停止之后没有孤儿进程；每个在途调用都有结果；实例表显示上一实例有结束锚点。

**走通**（验收 §10.5 #78、#81）。

### W10（Q25）程序超预算隔离

**失败路径。**

1. 程序死循环 / 超内存 / 超意图速率（H2/C4）。预算由**程序宿主**进程 / 沙箱隔离，而非类型系统（§6.1、§8.6）。
   - 行动者：程序宿主，身份 = 程序制品 hash + 装载 principal（P12）。
2. 超预算 → 宿主隔离该程序并产出失败记录；其他程序、账户、核心不受影响（C4、§8.6）。
   - 对外可见：预算违规、隔离状态、其他主体可用性可观测。
3. 宿主协议（§8.6）：
   - 超预算或 trap = 核心在同一事务 append 执行事实 `ProgramHalted{reason: Budget(kind) | Trap}` 与程序观察 `ProgramFailed{reason}`，提交之后终止宿主进程，OS 确认退出之后清除进程表的行。
   - 程序停在失败抑制：它仍在活动集合里，但本实例与此后各实例都不自动装载它，直到控制面 `load_program`，其 `Applied` 以位置引用这条 `ProgramHalted`、并钉住所装载的内容 hash。
   - 程序状态经最近 `Checkpoint`（与 cursor 同事务持久化，§7.5），在重新装载时 `Load` 交回；`Checkpoint` 的保留引用在抑制期间不解除（§2.4）。
   - 预算值由装载时的 `load_program` `Applied` 钉住（P12），不是设计常量。
4. **跨重启变体。** 失败抑制中的程序所在的核心重启：第 5 步按活动集合与 `ProgramHalted` 的 fold 不装载它（§7.2）；运维 `load_program` 之后才拉起宿主。
   - 另一变体：活动集合里的程序值文件被作者改过，重启时内容 hash 与 `Applied` 所钉的不符 → 不装载，append `ProgramHalted{ContentUnavailable}` 与 `ProgramFailed`；运维以 `load_program` 钉住新内容后装载。
   - 对外可见：程序显示为“失败，待重新装载”（附原因），重启不会把它悄悄装回来，也不会装入另一份内容。

**走通**（三 OS 预算与隔离验收 §10.5 #15；状态跨重启验收 §10.5 #16；活动集合与失败抑制验收 §10.5 #82）。

### W11（Q26）单据并发编辑与 SendBack

**正常—失败路径。**

1. 单据已有负责人 `responsible`（锁 = 该字段存在，§6.2），`current_version` 已前进。第二个 principal 对该单据 `Revise` → 被拒（持锁期间仅 `responsible` 可 `Revise`，不排队不分叉）。
   - 行动者：单据锁；线性化点 = 单写者 append 顺序 + `Ticket` fold（§4.1、§7.4）。
   - 对外可见：冲突结果、负责人、版本可读。
2. **SendBack 分支。** 送审后 `AwaitingDecision`，审批人 `SendBack{reason}` → `AwaitingDecision → Drafting`，`responsible` 不变（穷尽转移，§6.2）。
   - `SendBack` 后 `Revise` 使 `current_version` 前进，旧 Decision 自然失效。
   - `AwaitingDecision` 期间 `Revise` 被拒（决定绑定 `current_version`，C11）。
   - append 单据记录（执行 J，每条带 principal 与依据）。
   - 负责人 `revise` 的 diff 使下一版意图构造不出锚点（例如把 `target` 改成不是该作用域内任何尝试记为订单键的 `IdemKey`）→ `Rejected(Malformed)`，不 append，单据与 `current_version` 不变（§6.2、§8.5）。
3. **决定版本冲突（Q10 同型）。** 两 principal 对同一 `current_version` 决定，第二个返回 `Conflict(AlreadyDecided)`、不执行、不改状态（C11、§6.3）。即使第一条决定后单据仍停在 lane 步、版本未变，也是如此（审批步，§6.3）。
   - 下游的自动决定者（检查目录之外的 guard，§6.2）与人工审批人同受此约束：它批准后，人就不能再决定这一版；它否决则单据关闭；它想改判一个已批准的版本，只能经 `SendBack` 退回草稿、由负责人 `Revise` 出新版本。`SendBack` 不是 Decision，不占这一版的决定。退回与否决的原因随版本可读（读模型 `tickets`）。
4. 负责人失联：由策略层处理（权衡，§6.2）。
   - 过期步在 `deadline` 到期 `Close(Expired)`；
   - 或持有控制授权的 principal 经 `transfer(ticket, to)` 强制转移（§8.5），转移记录带 principal 与依据。
   - 对外可见：单据要么 `Closed(Expired)`，要么 `responsible` 变更且可追溯。

**走通。**

### W12（Q27）Replace：原子改单，或由调用方组合撤单与下单

**正常路径（来源能原子改单）。**

1. 改单意图类型 `Replace [交易协议]`，构造期必须携带 `target: VenueRef | IdemKey`（parse-don't-validate，§6.2、§8.1）。`target` 是只记为请求键、或不是该作用域内任何尝试记为订单键的 `IdemKey` 时，意图构造不出（`Malformed`，§6.2）。
2. 输入约束步读该 `(scope, Replace)` 的会话有效能力（§6.2 参数合规）：
   - `Supported`、参数 schema 与目标种类都接受 → 照常走 STS 链；放行后 IO 壳过发出前门，durable append 一条 `SendBarrier`（写操作 `submit`），集成在上游以**一次写**完成改单（§8.2、§8.3）。这是一次尝试，与 W1 同形。
   - 数量口径是意图参数，由上游在这一次写里按 schema 的字面含义执行；核心不读目标的累计成交量、不算新单大小（§6.2 `Replace`）。
   - `Unsupported`（上游只能先撤后下）→ `NotSupported`，`Rejection` + `Close(DecisionRejected)`，没有任何外部写。
   - 会话有效能力未确立（`Unknown`，或该来源此刻没有已建立的会话）→ `CapabilityNotEstablished`，单据停在输入约束步等待，不产生记录；会话建立或能力证据变化时重新求值，`deadline` 到期由过期步关闭（§6.2、§6.3）。
   - `target` 的种类不在该来源接受之列（例如 `target` 是调用方键 `IdemKey`、而来源只接受 venue 订单身份）→ `TargetNotAccepted`，单据在 `Prepared` 之前关闭（§6.2 可执行性）。
3. 改单的 `submit` 无回执 → `Undetermined`，与 W2 同样取证、停等或放弃跟踪（§6.6）。
   - 行动者：IO 壳，身份 = `attempt_position` + `WriteLaneKey`。
   - 对外可见：一张单据、一次尝试、fixture 上游恰一次写调用。

**失败路径（来源不能原子改单：调用方组合两张单据）。**

1. 调用方（程序或下游）先起一张 `Cancel` 单据，`target` 是原单。它走完整 STS 链，放行后是一次撤单尝试（写操作 `cancel`，§8.2）。
2. 撤单尝试的结果只说明撤单请求到达没有（§6.6 撤单尝试的取证）：
   - `Ack` → `VenueAccepted`，回执里原单的状态是原单自己的记录，按它自己的关联证据归因，不归到撤单（§6.5）。`VenueAccepted` 不证明原单已结束。
   - 无回执 → `Undetermined`：取证只用撤单尝试自己请求键的 by-key / replay-by-key；listing 与成交对账不属撤单，不带键的撤单尝试没有自动取证渠道，只由 `Attributed` 收敛或由 principal 放弃跟踪。原单的记录（例如 listing 上仍在或已不在）不是撤单尝试的 `Found`。
3. 调用方看**原单自己的记录**决定下一步：订阅或读 `orders` 读模型，按 §8.1 的最近观察看原单是否已到终态、累计成交多少（§8.1“订单身份与最近观察”）。这是解释层或程序的判断，核心不替它做。
4. 原单已结束，调用方另起一张 `Place` 单据，数量由调用方按它自己的口径算出，`basis` 引用它所依据的原单观察位置（§5.1）。这张单据照常经授权、输入约束、审批、lane 与冷却；撤单尝试仍在等待时，它停在 lane 步（§6.4）。
5. 两张单据各有自己的 `deadline`，各自到期、各自关闭；没有跨单据的“第二步”由核心等待或放行。
   - 对外可见：两张单据、两次尝试（各一条 `SendBarrier`），关联由下单单据的 `basis` 追溯；核心里没有把二者连成一条的状态。

**扩展路径（取证记录模型）。** 撤单尝试或改单尝试的每次命中取证，同一事务落一条观察记录（`provenance: Reconciliation{AttemptRef}`）与一条 `ResolutionEvidence{Found}`（含 `Evidence`）；`Absent`/`Inconclusive` 只有 `ResolutionEvidence`（§6.5）。`AttemptRef` 使撤单尝试的回执观察不会被当作另一次尝试的归因证据。

**走通**（验收 §10.5 #14/#17）。

### W13（Q28）保留边界推进与被引用位置

**失败路径。**

1. retention 推进时，仍有单据 `basis` 或程序 `Checkpoint` cursor 引用的 `LogPosition`（§2.4、§5.1）。
   - 核心 `Window` 节点的累加器随 `Checkpoint` 持久化，不回读历史。
   - `Pooled` 窗口从 `Journal` 重洗历史但不登记，越界只得 `BeyondRetention`（条件 4，§8.7）。
   - 观察侧可压缩（`compact_below_retention` 仅对 `RetractableDelta`，§4.1、§7.4）；执行事实只 append 不压缩（§3.1）。
2. 边界推进前必须显式处理仍被引用的位置（不变量 §6.9-3）：保留对应历史、保存必要证据，或缩小重放承诺（`AS OF ≥ 保留边界`，§2.4）；否则拒绝推进。要删掉某流当前 epoch 的记录时，持久订阅先为它 append 覆盖检查点，序号覆盖不因压缩改变（§8.4 序号覆盖）。
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

1. Alice 或解释层崩溃 / 重启，UTA 独立存活；订阅与程序 owner 是核心（H5/C5、§7.1）。解释层不持有状态，没有要恢复的东西（`design/downstream/design.md` 3.3）。消费方会话随传输关闭而结束，它只在内存里、不拥有订阅、单据或在途读，核心不为它的结束写任何记录（§8.5 会话的生命周期）。行动者：核心（生命周期独立）。
2. Alice 凭续传令牌重连；解释层代它开核心会话（P13），经**读模型**（§4.4、§8.5）取当前状态，并取 cursor 之后记录。断连期间的投递损失是订阅上未确认的 `Gap{origin: Delivery}`（订阅表，§4.2），重新挂接时先于记录交出，不伪造逐条补发（§8.5）。
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

1. 程序含 `DerivationNode::Pooled{input, window}`（§4.5、§2.5）。结构校验不看子系统，`load_program` 照常得 `Applied`；本实例没有子系统 → 声明校验不成立（§8.6 装载期校验、§8.7 失败语义），同一事务 `ProgramHalted{LoadRejected(reason)}` + `ProgramFailed`，错误指出依赖本实例没有的子系统；其余不含 `Pooled` 的程序不受影响（核心层最小验收，§8.7）。
   - 恢复者：程序宿主元素的声明校验（§8.6）；该程序停在失败抑制，直到以位置引用这条 `ProgramHalted` 的 `load_program` `Applied` 重新装载。
   - 对外可见：含 `Pooled` 程序被拒，其余程序照常装载运行。

**环境二：有子系统且 op 崩溃。**

2. 四条前置条件由声明校验以输出类型 fold 判定（定长 / 位置线性 / 无指针 / 可容忍 ring 回收，§8.7）；满足则 `Pooled` 输入交子系统物化为段视图（一次洗入，§8.7；实现见 hpc-derivation/design.md §1.4–§1.5、§5）。
   - 行动者：子系统洗入器；段身份由子系统契约表标定（hpc-derivation/design.md §5.2）。
3. 原生 op 借用只读段计算，输出是普通节点值；下游节点像读任何节点值一样读它，经 `outputs` 导出时才落在程序流上（§8.7、§4.3）。
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

1. 程序解释②满足条件 `Emit(EffectRequest{effect_kind: fetch.bars, basis})`（§6.1），注册为**读处理器**（§6.1、§7.3）。
   - 行动者：程序宿主（装载 principal）。
2. 读处理器**立即执行**（读副作用，§3.4）：按请求值定出目标流与参数，走 §8.2 `read` 的判定顺序；来源有已建立会话、该会话有效声明对该流 `read` 为 `Supported`、参数合该流 `request_schema` 时，经 `read(stream, request, range)` 调用集成（§8.2）。
   - 集成作答：同一事务在该流上 append 每个结果项一条观察记录（与推送观察同形，质量标记 `one_shot`）与一条读结论记录，都带 `provenance: OneShot{origins ∋ Request(该 EffectRequest 位置), request}`；`EffectResponse` 指向读结论记录（观察 J / 执行 J；读即观察记录，§3.4）。回答为空时只有读结论记录。
   - 恢复者：读可重试、可换渠道、可标 `Gap{origin: Channel}`（§4.2、§3.4），无 in-doubt。核心崩溃于响应持久化前，则按 §9.2 #21 重派。
3. 程序按位置推进看到该观察记录与读结论（cursor 之后；输入 = 位置推进，§4.3）：**闭环走观察侧**，不进单据 / STS / IO 壳（读 / 写处理器分派，§6.1）。
   - 对外可见：`fetch.bars`（读）与 `trade.place`（写）、`notify.telegram`（写）对程序是同一构造子 `EffectRequest`，差别在注册的读 / 写（§6.1 不变量）。
4. 未调用集成（Q16 同型）：来源此刻无会话，或该会话有效声明里该流 `read` 为 `Unsupported` 或 `Unknown`，或参数不合 schema 时，不调用集成、不 append 观察记录；`EffectResponse` 记下这一结论（无会话 / 不支持 / 未确认 / 请求不合法，无会话先于其余判定：离线时上一次声明不断言此刻的能力），程序的决策半边从自己请求的 `EffectResponse` 看到它（§6.1）。无论哪种都不返回看似成功的空数组（§8.2、F6、C2）。

**走通。**

### W18（Q8+Q9+Q10）送审即否决、人工审批与过期、决定版本冲突、冷却

**失败路径（Q8，三种输入）。**

1. 消费方经会话（P13，principal 已认证，§8.5、C11）提交三笔意图：instrument 属他账户、数量非法或参数不合该来源的意图参数 schema（例如限价单缺限价）、只读账户下单。
   - 每笔经 `draft(intent)`（§8.5）以会话 principal 为 `responsible` 开单 `Draft`（§6.2），append 单据记录（执行 J）。消费方开单不经出站写处理器：写处理器只接程序的 `EffectRequest`，以装载 principal 开单（§6.1）。
   - 参数不合规不挡开单：单据 fold 的 `parameter_validity` 从 `Draft` 起即为 `Invalid(违反项)`，负责人经读模型 `tickets` 看得到，可 `revise` 改正（§6.2 参数合规）。锚点构造不出的请求（如缺 `WriteLaneKey`）才在 `draft` 被拒 `Rejected(Malformed)`，不开单、不 append。
   - 行动者：单据，身份 = principal。
2. 负责人经 `submit_for_decision`（§8.5）送审后，STS 链逐步读记录（§6.3）：
   - 只读账户在**授权**步被拒（`Rejection::Unauthorized`），另 append 安全事件记录（C11）；
   - 参数不合规在**输入约束**步被拒：该步读 `parameter_validity` 与策略的 instrument 允许集合（C9/C10），一次否决列出全部违反。
   - instrument 属他账户：核心不校验 instrument 归属于哪个账户，那是上游的事实，核心没有它的源头（§6.3 输入约束步）。它在允许集合内、参数合规时照常放行，由上游拒绝：`submit` 返回 `Reject` → `VenueRejected`，`reason` 保留原文（§8.2）。策略的允许集合不含它时，它在输入约束步被拒，与其他违反同形。
   - 本地每笔否决 = `Close(DecisionRejected)` + 规则 `Rejection` 记录，带 principal、时间、依据位置、违反项与 `rule_version`（执行 J）；
   - 链在拒绝处停止，后续步不执行。
   - 对外可见：两对意图 / 否决记录（只读账户、参数不合规），安全事件可读，二者 fixture venue 调用数 = 0、无 `Prepared`；他账户那笔有一条 `SendBarrier`、fixture venue 调用数 = 1，终于 `VenueRejected`（允许集合排除它时则同样是一对本地意图 / 否决记录）。
   - 程序变体：程序 `Emit` 同样的非法参数，写处理器 `Draft` + `SubmitForDecision` 同一事务（§6.1），输入约束步给出同形的否决；程序经自己请求的 `EffectResponse{Drafted}` 与单据状态看到结果。请求载荷构造不出锚点时得 `EffectResponse{NotDrafted(Malformed)}`，不开单、不重派。

**正常—失败路径（Q9，人工审批与过期）。**

3. 策略要求人工：**审批**步把单据留在 `AwaitingDecision(current_version)`（§6.2），待决集合对审批人可见（读模型，§4.4）。
   - 审批人 principal 提交 Decision，引用 `current_version` 与期望的待决集合版本（C11）。
   - 批准 → 链继续 lane（阻塞头集合为空后判冷却，§6.3）→ 过期 → 放行，append `Prepared` + `Close(Prepared)`（W1 步 4）；否决 → `Close(DecisionRejected)`。
   - 冷却变体：同 `(WriteLaneKey, instrument)` 上一次下单尝试的 `SendBarrier` 距今不足该 `(WriteLaneKey, OperationKind)` 的冷却间隔 → lane 步 `Rejection::Cooldown{until}` + `Close(DecisionRejected)`；已批准不豁免冷却，`bypass_lane` 也不豁免；上一次尝试以 `NotSent` 结束也不撤销冷却（§6.3）。
   - 每条 Decision 带谁、何时、依据 `LogPosition`、绑定版本（执行 J）。
4. 另一笔超过 `deadline` 仍未获决定：**过期**步由 `deadline` 到时触发 → `Close(Expired)`（H6；状态机允许 `AwaitingDecision → Closed`，§6.2），不补偿、不递送。
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
   - X 此刻会话在重连（`health` 为 Connecting）：两个持仓 target 得 `Unavailable{source_state}`，核心不调用集成、不记 gap（§8.2 `read`）。即使 X 上一次声明里持仓流的 `read` 为 `Unsupported`，离线时也得 `Unavailable{source_state}`：无会话的判定在能力之前，上一次会话的声明不断言此刻的能力（§8.5 一次性读）。
   - P 有会话、报价流 `read` 为 `Supported`：集成作答，同一事务 append 报价记录（`one_shot`，带 `dispatch_end`）与读结论记录，target 得 `Answered{conclusion, items}`。
   - 若 P 的会话有效声明里某条历史 bar 流 `read` 为 `Unknown`：该 target 得 `Unconfirmed`，不调用；为 `Unsupported` 则得 `Unsupported`。
   - 上游对某 instrument 明确拒绝（未开通该行情）：得 `Refused{conclusion, reason}`，该流上只有读结论记录，流的能力不变。
   - P 对另一个 instrument 迟迟不答，`deadline` 先到：该 target 得 `Pending{from, instance_id}`，没有结论也没有 gap。解释层以 `from` 订阅该报价流的一个只投递项（主体集为这个 instrument，§8.5 按主体投递）：它不进入 `route` 需求、不占配额池的用量，只要该流曾被声明就被接受，与 P 此刻有没有会话、重新握手后的声明里还有没有这条流无关。稍后上游作答，该流上照常 append item 记录与结论记录（同一请求身份，`origins` 含该会话），这一项由此收到结果；这次调用在途时会话结束，则该流上出现 `Gap{origin: Channel}`，同样经这一项送到。若核心在作答之前重启，解释层重连时握手得到新的 `instance_id`，与 `Pending` 所带的不同，就知道这次读的结果不再有保证：从 `from` 起没收到结论的，报为需要重新读，不让下游一直等。
   - 对外可见：各 target 独立；“来源重连中”“不支持”“能力未确认”“来源拒绝”“尚未作答”“空结果”彼此可区分，没有看似成功的空数组。
4. 下游在一个订阅里选 P 的报价流（属配额池，带主体集）、P 最近声明里没有的一条流、X 某账户的持仓流三项：第二项被拒，第一项的主体集并入后超过配额池上限得 `QuotaExceeded{quota, limit}`，第三项为“活”；订阅照常建立，逐项结果交给下游，集成不收到超限主体（§8.5 订阅组）。
   - X 会话建立后，核心对该持仓流 `route` 一次全集（`All`，§8.2 `route`）；此后 X 的持仓推送才到达该订阅。

**扩展路径（声明变化与推送）。**

5. X 重连成功，新声明版本把某作用域的 `account_ref` 改成另一个值：核心照常按键路由，该引用在新声明版本里标为不可解析（§2.2）。解释层的执行事实订阅收到新声明版本，重读 `sources`：该账户显示为“账户引用冲突，需要处理”，按旧名字的命令不再解析到任何账户。
6. X 上一张单据送审、另一次尝试进入 `Undetermined`：两条执行事实经解释层的执行事实订阅到达，翻成“待审事项”“结果未知，正在核实，请勿重下”；取证确立结果的 `ResolutionEvidence` 到达时翻成“已确认发生 / 未发生”。运维对那次尝试 `abandon` 时，`Abandoned` 到达，翻成“已放弃跟踪，结果未知”；之后若被动归因或 `retry_reconciliation` 补上结果，再翻成该结果，放弃记录仍在。待审事项此刻是否偏离，解释层在呈现时读 `tickets`（§8.5）。
   - 对外可见：推送经执行事实订阅，重连后从已确认 cursor 续，没有投递损失。

**走通**（验收 §10.5 #25–#29、#47、#48）。

## 9.2 崩溃矩阵（#1–#21）

> 图：D7.1 恢复判定、D7.2 崩溃窗口在时序上的位置、D7.3 脑裂/孤儿、D7.4 程序侧/派生侧/快照崩溃；逐行对照见 D7.5（`design/diagrams/07-crash-recovery.md`）。

行 = 崩溃窗口；列 = 崩溃后持久状态 / 重启后恢复动作（由谁）/ 对外可见结果 / 依据 / 验收项（§10.5 编号或子系统验收编号）。每行恢复结论由状态机（§6.2、§6.5）与存储归属（§7.4、§7.5）推出。

| # | 崩溃窗口 | 崩溃后持久状态 | 重启后恢复动作（由谁） | 对外可见结果 | 依据 | 验收 |
|---|---|---|---|---|---|---|
| 1 | 单据 `Close(Prepared)` + `Prepared` 同事务中途 | 事务未提交 → 二者皆无 | 核心：SQLite 事务原子回滚；单据仍 `AwaitingDecision` | 无 `Prepared`；单据可重新放行 | §6.2；§7.4（同事务） | §10.5 #8(d) |
| 2 | `Prepared` 已持久、`SendBarrier` 未持久 | `Prepared` 有、无 `SendBarrier` | IO 壳：确未发出 → 过发出前门（§6.5）：`deadline` 未过且该集成会话已建立、意图对会话有效能力可执行，则 durable append `SendBarrier` 后调用写操作；会话或能力条件不成立则等待（不 append 记录）；`deadline` 已过则 append `Expired(deadline)`，等待结束 | 崩溃窗口不产生 venue 调用；不误升 `Undetermined`；末态为“已发”（可能先等待）或 `Expired` | §6.7；不变量 §6.9-8 | §10.5 #8(a)/#14 |
| 3 | `SendBarrier` 已 fsync、写调用未发 | `SendBarrier` 有、无后继 | IO 壳：可能已发出 → append `Undetermined(CrashWindow)` 进对账 | 尝试 = `Undetermined`；lane 阻塞；无第二 `SendBarrier` | §6.7；不变量 §6.9-1/8 | §10.5 #8(b)(c) |
| 4 | 写调用已发、回执未到 | `SendBarrier` 有、无回执 | IO 壳：同 #3，对账驱动按渠道取证收敛 | `Undetermined` → found/absent，或停等后由 principal 放弃跟踪 | §6.6；C1/C2 | §10.5 #3/#17 |
| 5 | 回执或 `NotSent` 已到、未 append | `SendBarrier` 有、结果丢在内存 | IO 壳：视为无后继 → `Undetermined(CrashWindow)` → 对账（按键回读会重得同一状态；集成当时未交出的，按键回读在唯一期内得 `Absent`） | `Undetermined` 经 `ResolutionEvidence{ByKey, Found / Absent}` 收敛；`Evidence` 在执行 J，该回应的观察记录在观察 J | §6.6；§3.4（读可重试） | §10.5 #3/#17/#59 |
| 6 | 记录已 append 提交，投递/cursor 推进前崩溃 | 记录已提交、cursor 未推进 | 核心：`fold_state` 重建；订阅者从已确认 cursor 之后重收，未确认的记录可重复可见，按 `LogPosition` 去重 | 记录不丢；重复只出现在未确认区间 | §4.2；§7.4 | — |
| 7 | 对账取证中途 | 部分 `ResolutionEvidence` 已 append | IO 壳：取证是读副作用、可重放；等待仍 `Active` 者继续按渠道取证（下一渠道由 fold 重建） | 收敛进度不丢；渠道穷尽仍 `Inconclusive` 停等 | §6.5；§6.6 | §10.5 #17 |
| 8 | `abandon` 进行中：IO 壳已停发新取证、在途取证已完成或未完成，`Abandoned` 未 append | 在途取证已提交的 `ResolutionEvidence` 有或无；无 `Abandoned` | IO 壳：见表下 | 不出现“放弃中”的中间态；尝试仍 `Active`、仍占阻塞头；principal 未收到返回，可重发 `abandon` | §6.6 放弃跟踪；§6.7 | §10.5 #58 |
| 9 | 观察 append 中途 | 半写事务未提交 | 核心：单写者原子回滚；半写不可见；`fold_state` 重建 | 无半条记录；订阅者按 cursor 续接 | §4.1；§7.4 | §10.5 #8(d) |
| 10 | 派生 DAG 重算中途：核心崩溃，或只有宿主进程崩溃 | 核心崩溃：这批 `Advance` 的输出事务未提交，派生记录、`EffectRequest`、`Checkpoint` 与 cursor 一条也没有落；只有宿主进程崩溃：同 #16 的 trap，同事务有 `ProgramHalted{Trap}` 与 `ProgramFailed` | 核心崩溃：重启第 5 步从与 cursor 同事务持久化的 `Checkpoint` 重新 `Load`，从已提交的 cursor 重新推进（同 #16）；宿主崩溃：trap，按失败抑制处理 | 程序流上只有已提交批次的记录，不出现半批；核心崩溃后的重算可与崩溃前分批不同，每条记录照常满足 `basis` 契约 | §4.3；§8.6 `Advance`、失败抑制 | §10.5 #16 |
| 11 | 快照写入中途 | 快照部分写、原记录完整 | 核心：快照仅加速；半写快照丢弃，从保留边界 `fold_state` 重建 | 不改 append-only 语义；重启延迟增大 | §7.4；§7.5 | — |
| 12 | 配置文件重载中途 | 统一路径文件原子替换半途 | 核心：见表下 | 重载成功或整体拒绝；无半写可见态 | §7.6；C14；O9 | — |
| 13 | 集成崩溃（观察流侧） | 观察流断代 | 核心+集成：记 `Gap{origin: Source}`，按回填补齐或标 gap | 该流 gap 显式；核心与其他流不受影响 | §4.2；§6.7（两故障面）；§8.4 | §10.5 #9 |
| 14 | 集成崩溃（写调用中） | 已 `SendBarrier`、`submit`/`cancel` 中途 | IO 壳：`NoResponse` = `Undetermined` → 对账；不区分“集成挂”与“venue 没回”；会话结束时在途调用恰好完成一次（§7.2 第 3 步） | 尝试 `Undetermined`，靠证据非猜 | §6.7；§8.3 | §10.5 #9 |
| 15 | 旧核心已退出但其集成/宿主进程仍存活（孤儿），新核心接管 | 孤儿进程持已退出核心的通道、可能有在途回执 | 新实例：见表下 | 不产生双写；已结束的会话化身的消息不进入核心 | §7.2；§8.3；§6.6 | §10.5 #8(c) |
| 16 | 程序宿主崩溃（trap），或核心在 `Advance` 输出持久化前崩溃 | 程序 state 依最近已提交的 `Checkpoint`；未提交的 `Output` 整体不存在；trap 时同事务有 `ProgramHalted{Trap}` 与 `ProgramFailed` | 核心：trap → 失败抑制，只有引用它的 `load_program` `Applied` 才从最近 `Checkpoint` 重新装载；核心在 `Advance` 提交前崩溃 → 重启第 5 步从与 cursor 同事务持久化的 `Checkpoint` 重新 `Load`，重放该 cursor 之后的记录；`Load` 不比对版本：本成员可交回的 `Checkpoint` 的 `state_version` 总在它接受的集合内（`Output` 时已检查，§8.6） | 程序从 checkpoint 续跑，不重复 `Emit`；trap 后在重新装载前不运行 | §4.3；§7.5；§8.6 程序的活动集合与失败抑制 | §10.5 #16 |
| 17 | 可选子系统 op 崩溃 | 段借用未释放 | 子系统回收借用 + H10 fence；核心记失败观察 | op 失败观察；核心与其他消费者不受影响 | §8.7；hpc-derivation/design.md §5.3/§6.5 | hpc §10 #3/#4 |
| 18 | 可选子系统 op 已产生结果、核心在结果持久化前崩溃/断连 | 输出段在段池，op 的输出值还没有随 `Advance` 的输出事务提交 | 核心：段池不持久，重启由 `Pooled` 重洗重算；op 的输出是普通节点值，只有 `outputs` 导出它时才随 `Advance` 的输出事务写上程序流（§4.5），未提交的不半接入下游 | 结果重算；下游只见已提交的程序流记录 | §8.7；hpc-derivation/design.md §1.5/§5.3 | hpc §10 #7/#13 |
| 19 | 核心崩溃时可选子系统 op 孤儿 | op 进程存活、核心死 | 新核心 + fence：op 为孤儿由 H10 fence 回收；重连后重新握手 | 无双写；孤儿被回收 | §7.2；hpc-derivation/design.md §6.5 | hpc §10 #3 |
| 20 | Alice（或其他下游）或解释层崩溃 | 核心订阅/程序/lane 完整；解释层无持久状态 | 核心：独立存活；下游凭续传令牌重连，解释层代它握手取 principal，读模型取 `as_of`，从已确认 cursor 之后订阅原始记录（§8.5） | 断连损失以缺失通知给出，不伪造补发 | §7.1；§8.5；H5/C5；`design/downstream/design.md` 3.3 | §10.5 #23 |
| 21 | `EffectRequest` 记录已随 `Advance` 输出提交，处理器未执行或其 `EffectResponse` 未持久化 | `EffectRequest` 有、无 `EffectResponse` | 核心：见表下 | 每条请求最终恰一条 `EffectResponse`；写至多一张单据；`Unhandled` 不重派；读结论等观察记录被压缩不影响判定 | §6.1；§8.6；§3.4 | §10.5 #16 |

**#8 的恢复动作（IO 壳）：**

- 日志里没有 `Abandoned`，也没有任何表示“放弃中”的记录：停发只是 IO 壳进程内的状态，随崩溃消失（§6.6）。
- 恢复按 §6.7 第 4 条处理这次尝试：等待仍是 `Active`，在该集成会话建立后按本轮进度续跑取证；已提交的在途取证结果照常计入，若其中已有 `Found`/`Absent`，结果已确立、等待结束，无需放弃。
- principal 重发 `abandon` 是一次新调用，按 §8.5 的次序重做：在途取证完成后结果仍未知才 append `Abandoned`。

**#12 的恢复动作（核心）：**

- rename 未落，则旧文件完整可读；rename 已落，则新文件完整。
- 重载失败保留上一有效版本，并 append 控制结果 `Rejected(reason)`。
- 启动期失败按 C14 拒绝启动。

**#15 的恢复动作（新实例）：**

- 取 fence、`instance_id` 加一；按进程表回收孤儿（§7.2 第 1 步）。
- 孤儿的通道只通向已退出的旧核心，新实例不读它；未登记或匹配失败的孤儿同样无法写入（§7.1、§7.2 第 1 步）。
- venue 已受理的写，经新会话观察记录或对账取证并入同一尝试（§8.3、W2 步 4）。

**#21 的恢复动作（核心）：** fold 出无 `EffectResponse` 的已注册请求重派（§6.1）。

- 读处理器重新执行一次；
- 写处理器重新开单。`Draft` 与 `EffectResponse{Drafted}` 同事务，不存在“有 `Draft` 无响应”。重派按请求所记发出成员 `member` 所指 `Applied` 的事实处理：负责人是该 `Applied` 的装载 principal，`ScopeNotObserved` 按它所记的执行事实输入判定；该成员在崩溃前后已被替换或卸载、程序值文件已改或删去时亦然（§6.1）。

**可进验收（§10.5）的可测项：**

- #2/#3 的 fsync 崩溃注入，证明“`Prepared` 无 `SendBarrier` 确未发、`SendBarrier` 无后继升 `Undetermined`”（§10.5 #8/#14）；#8 的崩溃注入证明放弃跟踪没有中间态（§10.5 #58）；
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
