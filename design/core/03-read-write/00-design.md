# 读写链

## 0 定位

- **层级与元素**：core 的读写链：读的副作用消费为写。上级文档：[core/00-design.md §4.4 模块地图](../00-design.md#44-模块地图)。
- **本目录**：本文（无锁定位；`Prepared` 双认）、[01-ticket.md](01-ticket.md)（单据）、[02-trade-protocol.md](02-trade-protocol.md)（交易协议）、[03-intent.md](03-intent.md)（意图来源）、[04-decision-chain.md](04-decision-chain.md)（STS 规则链）、[05-lane.md](05-lane.md)（lane 驱动）、[06-write.md](06-write.md)（写边界：IO 壳）、[07-resolution.md](07-resolution.md)（决议）。

## 3 模型

### 3.1 无锁定位

UTA 唯一的锁是单据；执行阶段与观察侧没有锁。

- **唯一的锁是单据**：提交是消费动作，只有有权消费者才谈得上锁。意图形成期的单据确实有多个编辑者争同一份东西，单据本身就是锁（[01-ticket.md §4.2 状态机与穷尽转移](01-ticket.md#42-状态机与穷尽转移)）。过期读由 C11 的依据版本**可见**，不需要锁让它不可能。
- **venue 域事实无锁（F1）**：账户、持仓、订单、成交、价格由 venue 消费与裁决，UTA 无权决定是否消费，因此对它们**不存在 UTA 的锁**。
- **UTA 自身记录的权威不是锁**：审批、意图、尝试、队列顺序、程序装载、订阅表是 UTA 说出的话，UTA 对它们有权威。但 append-only 日志上顺序就是位置本身：单写者（H10）、每次 append 原子、没有第二个写者争同一位置。
- **对 venue 的写入通道**：每 `WriteLaneKey` 一条有序队列（H4）。其有序与队首阻塞来自**通讯协议**（后续写的含义依赖队首结果），不是 UTA 抢占通道的锁（[05-lane.md §3.2 队首阻塞是通讯协议，不是锁](05-lane.md#32-队首阻塞是通讯协议不是锁)）。UTA 不能通过“先拿到通道”改变 venue 消费什么，只能决定自己以什么顺序把请求交给协议。
- **记录追加与因果引用**：执行事实侧只 append，位置即顺序；SQLite 单写者是记录器的实现事实而非域语义。C11 的“待决集合版本期望”是决定对 UTA 自身单据版本的**因果引用**：Decision 绑定 `(ticket, current_version)`；期望版本不等于 `current_version`，或该版本已有 Decision → `Conflict`，不执行、不改状态（[04-decision-chain.md §4.3 审批步](04-decision-chain.md#43-审批步)）。这是 UTA 对自己记录的版本判定，不是对 venue 状态的锁；决定所依据的观察是否已过时，另由依据有效性门判定。
- **对外部状态无锁**：“依据有效性”只是对证据新鲜度的有界赌注，不阻止世界在 prepare 与 venue 执行之间变化（那是 F5 / 对账的领域）。venue 若提供真正的锁（cancel/replace 引原单身份、条件单、expected-version 改单、幂等键唯一性、保证金预占），那把锁是 **venue 的**：它经能力证据声明后由 IO 壳使用；未声明则不存在，不得假装。

**结论。** 悲观锁由消费者自身持有、不侵入外部；乐观锁要比较的版本住在被消费状态的拥有者那里。UTA 不是消费者，既做不了悲观锁，做乐观校验时锁也不在它这里。DB 隔离级别的词汇（可重复读、串行化、悲观 / 乐观锁）不用于描述 UTA 自身。

**读触发的写同形；授权是规则不是记录种类。** AI 发请求、程序满足规则、审批人作决定，都是“读的副作用消费为写”，记录形状相同：带 principal、带依据 `LogPosition`。差别只在**哪些 principal 的记录足以让写进入 prepare**，这由授权规则（[04-decision-chain.md §4.2 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期](04-decision-chain.md#42-顺序固定链授权--输入约束--审批--lane--过期)）决定；“谁、何时、依据什么”由记录上的 principal 与依据字段满足。

不变量：UTA 对 venue 域事实无锁；DB 隔离词汇不描述 UTA 自身。由 F1 + 本节定位保证。

不选：**给 UTA 自身造悲观 / 乐观锁**：UTA 不是消费者，锁不在它这里；过期读用依据版本可见即可。**为审批另设记录种类**：读触发的写同形，principal + 依据字段已足够。

## 4 结构

### 4.1 `Prepared` 双认

`Prepared` 是同一条记录，是单据 → IO 壳的**唯一交出点**。两者同属效应宇宙。

- 单据侧只认它是“我已交出”：`Close(Prepared(position))` 的结果（[01-ticket.md §4.2 状态机与穷尽转移](01-ticket.md#42-状态机与穷尽转移)）。
- IO 壳只认它是“我该做的”：一次尝试的起点 `Prepared → SendBarrier → …`，尝试的身份 `AttemptRef` 就是这条记录的位置（[06-write.md §3.3 尝试与 AttemptRef](06-write.md#33-尝试与-attemptref)）。

两边各认一半，中间隔着整条 STS 链。唯一耦合点是 `Close(Prepared)` 与 `Prepared` 的 append 必须在同一事务（[core/00-design.md §4.7.4 同事务集合](../00-design.md#474-同事务集合)）。单据锁不驱动 IO 壳：意图形成期可以任意长、任意多次退回，IO 壳完全不感知；单据只单向读 IO 壳的记录作依据。

## 6 评估

### 6.1 验收标准

- **#3 写边界状态**（[05-lane.md](05-lane.md)、[06-write.md](06-write.md)）：（对应 Q2/Q4/Q7）
   - 普通写路径：阻塞头集合非空时，同 lane 无新普通写尝试。
   - 撤阻塞头路径：以阻塞头中某次尝试 `SendBarrier` 记为订单键的调用方键为 `target` 的撤单意图，在该来源接受 `IdemKey` 目标时放行后，集合为二、各自独立结束等待、任一结束只移出自己，集合空才放行普通写；撤单的 `VenueAccepted` 或 `Found` 不改变阻塞头的等待，也不 append 阻塞头的 `ReconciliationReopened`；以撤单尝试的键、或以记为请求键的键为 `target` 的撤单照常等待；来源不接受 `IdemKey` 目标时，这笔撤单在输入约束步得 `TargetNotAccepted`，无 `Prepared`。
   - 显式绕过路径：同上，且带 `bypass_lane` 控制记录；该单据版本上的 Decision 数不变（至多一条）；`tickets` 在该版本上给出这次绕过；绕过之后加入集合的阻塞头仍使该单据等待，`SendBack` 后改出的新版本不继承绕过。
   - 重启后，无后继的 `SendBarrier` 被记为 `Undetermined`；无 `SendBarrier` 的 `Prepared` 不被误升；已 `Expired` 或 `Abandoned` 的尝试不被再自动驱动。

- **#41 一次写与改单**（[01-integration.md §3.3 投影 Projection](../04-boundary/01-integration.md#33-投影-projection)、[02-trade-protocol.md §4.1 交易协议：操作种类、目标、可执行性、检查目录](02-trade-protocol.md#41-交易协议操作种类目标可执行性检查目录-交易协议)、[06-write.md](06-write.md)、[07-trade-observation.md §3.1 成交的计数身份](../02-read/07-trade-observation.md#31-成交的计数身份-设计)）：以 fixture 上游与 fixture 集成驱动：（对应 Q2/Q3/Q17/Q27）
    - 每种操作种类的一次尝试恰一条 `SendBarrier`、fixture 上游写调用数 ≤ 1；声明 `Replace` 为 `Supported` 的来源上，一笔改单恰一次尝试、写操作 `submit`，数量口径由上游执行，核心交出的参数与意图相同；
    - 声明 `Replace` 为 `Unsupported` 的来源上，改单意图送审即 `NotSupported` + `Close(DecisionRejected)`，无 `Prepared`；调用方组合的撤单与下单是两张单据、两次尝试，下单单据在撤单尝试等待期间停在 lane 步；撤单的 `VenueAccepted` 不使核心放行任何写；
    - 握手声明撤单带订单键、不带键的写证明声明 by-key 或 replay-by-key 渠道、撤单声明 listing 或成交 / 持仓对账渠道、`Cancel`/`Replace` 的 `target_kinds` 为空：投影不合法，集成 `Halted{ProjectionInvalid}`；
    - 不带键的撤单尝试 `Undetermined`：取证从不调用 `query_by_key`/`replay_by_key`，按它自己的渠道推进或停等；
    - `SendBarrier` 之后重握手改变该操作的键角色：这次尝试的取证渠道在声明与记录不一致期间为空，尝试停等；此后同一会话里的能力变更（`CapabilityObserved`）使声明恢复一致时，在同一会话里续跑本轮，从本轮尚未取证的渠道起，不等重连或 `retry_reconciliation`，不 append `ReconciliationReopened`。

