# 底座

## 0 定位

- **层级与元素**：core 的底座，单读链与读写链共用。上级文档：[core/design.md §4.4 模块地图](../design.md#44-模块地图)。
- **本目录**：本文（流、位置与三种进度；基础值类型；持久化视图与控制流）、[value-tree.md](value-tree.md)（组合子值树与五个 fold）、[storage.md](storage.md)（存储）。

## 3 模型

### 3.1 流、位置与三种进度

```
StreamId    = (source: Source, stream: StreamName, epoch)   // 每个范围独立维持位置单调；系统内不存在全局入口序
Source      = Integration(IntegrationId) | Program(ProgramId)
LogPosition = (StreamId, Seq)           // 一条记录的顺序身份
```

- 每个 `StreamId` 独立维持 `LogPosition` 单调递增。`Seq` 在该 `StreamId` 内**每 epoch 独立**单调。系统内不存在全局入口序。[证据：fp-04 命题 11；域 P2]
- 有序范围 = `StreamId=(source, stream, epoch)`。这一对应由 P2（`Seq` 在“来源 × 流 × epoch”有序范围内赋）、P3（新 epoch 首条记录）与本节位置定义共同给出。
- **来源是带标签的值** [设计]：集成 id 与程序 id 是两个命名空间，同名也不是同一来源。集成只拥有 `Integration(_)` 的流（声明、开 epoch、供给、作答）；`Program(_)` 的流由核心产出：流名来自程序值的 `outputs`（[program-host.md §3.1 程序值](../runtime/program-host.md#31-程序值)），控制面只在这些流上开 epoch（[derivation.md §3.1 输出契约与程序流](../read/derivation.md#31-输出契约与程序流)）。声明、采纳、会话、`route`、回填与一次性读只对 `Integration(_)` 来源成立，程序的观察输入也只取 `Integration(_)` 来源。`Program(_)` 的流只经只投递项订阅，接纳依据是控制流上该程序 id 的开始成员的 `Applied` 所记的输出契约（[consumption.md §4.1 核心↔解释层：订阅组与 rewind_cursor](../read/consumption.md#41-核心解释层订阅组与-rewind_cursor)）。

**位置作为关联。** `LogPosition` 集合只用于两项语义：输入依据（例如某决策观察到的行情流 A@120、汇率流 B@57、账户流 C@90），与重放位置。它不替代外部订单身份、幂等键或 causation id；后者属独立的名义关联（[core/design.md §3.10 五种关系，五种承载](../design.md#310-五种关系五种承载)）。撤 / 改单的目标身份（`venue_order_id` / `idempotency_key`）不放进 `basis`，放进意图自身的 `target` 字段；`basis` 只记录该身份来自哪条记录的位置（[core/design.md §3.5 允许 / 禁止关系](../design.md#35-允许--禁止关系)）。

**三种进度**互不替代，源头各不相同：

| 进度 | 含义 | 源头与承载 |
|---|---|---|
| 消费位置（cursor） | 某消费者已确认处理到的位置（`At{pos}`）；从起点或保留边界建立、还没有确认的为 `Start{from}` | 消费方的确认（程序是提交的 `Advance`）；核心在它确认之后记入订阅状态（[consumption.md §3.2 cursor 与确认](../read/consumption.md#32-cursor-与确认)） |
| 完备进度 | 来源证据证明的覆盖：某一坐标上的哪一段记录已全部到达，命题与坐标随证据写明 | 来源；核心只从来源证据 fold 出来，只在保留边界推进要删掉它所依的记录前留一个覆盖检查点（[supply.md §4.3 序号覆盖与覆盖检查点](../read/supply.md#43-序号覆盖与覆盖检查点)）。没有证据的流没有 |
| 保留边界（retention） | 每条观察流上存储仍能精确重建的最早位置（执行事实侧不压缩，没有保留边界） | 核心的存储事实（[journal.md §2.4 保留：边界与删除规则](../read/journal.md#24-保留边界与删除规则)） |

不变量：

- cursor 与完备进度没有强制关系：cursor 可以落在已证明的覆盖之内或之外。
- 引用的 `LogPosition` 一旦 < retention，其精确重建不再保证。边界推进前必须显式处理仍被引用的位置（[core/design.md §4.7.6 保留引用的登记与解除](../design.md#476-保留引用的登记与解除)）。
- **位置归核心，完备归来源** [设计]：
  - `LogPosition` 是核心在 append 时创建的，核心是它的源头；集成给的 venue 序号、游标与事件时间是来源给的字段，随记录作为副本保存。
  - 某段历史是否已全部到达，是上游历史的性质，源头是上游（[README.md §1.2 权威源头与生命周期](../../README.md#12-权威源头与生命周期)）。核心只 fold 来源给出的证据，并写明它证明的命题与坐标。契约里今天只有一种这样的证据：声明 `joinable_venue_seq` 的流上的**序号覆盖**，即“流 epoch e 内 venue 序号落在 `[from, through)` 的记录都已 append”（[supply.md §4.3 序号覆盖与覆盖检查点](../read/supply.md#43-序号覆盖与覆盖检查点)）。它的坐标是该 epoch 的 venue 序号，不是事件时间：可衔接序号与事件时间之间没有顺序关系（[trade-observation.md §3.3 精确重建的前提与完整界](../read/trade-observation.md#33-精确重建的前提与完整界-设计)）。
  - “此后不再有 `occurred_at` 早于 t 的记录”（事件时间闭合）是关于上游未来输出的命题，契约里没有证明它的证据，所以没有哪条流有事件时间上的完备进度。`received_at`、核心或运维声明的滞后界、上游文档写的迟到上限都不是证据：越过上限的迟到记录与从未发出的记录不可区分，违反不是一条可观察的记录（证伪 #29，[supply.md §6.1 证伪条件](../read/supply.md#61-证伪条件)）。
  - 没有证据的流没有完备进度；依赖它的判断得“完备未确立”，不以时钟、到达顺序或消费位置补上（[consumption.md §3.6 三种消费方式（投递侧）](../read/consumption.md#36-三种消费方式投递侧)、[trade-observation.md §3.3 精确重建的前提与完整界](../read/trade-observation.md#33-精确重建的前提与完整界-设计)）。

**为什么。** 读到哪不说明之前的记录是否已全部到达，所以 `await-all` 按证据证明的覆盖触发，而非按消费位置。三者度量三件不同的事（确认处理到哪、来源证明了哪段已全部到达、还能重建到哪），源头分别是消费方、来源与核心，任一都无法从另两者算出。代价：没有可衔接序号的流上没有完备，按时间对齐的消费者只能声明自己接受的丢失界。[证据：fp-05 案例 10④/11⑤/13⑤；fp-04 命题 10]

**不选。**

- 用单一“进度”标量同时表达消费、完备与保留：它把“读到哪”与“已全部到达”混同，`latest` 消费者会被误当作已跟上完备进度，阈值策略据此误触发。
- 由核心按 `received_at` 与声明的滞后界推导完备进度：以本地时钟推断上游的未来输出。迟到记录照常 append 而进度不退，推断失真时没有任何记录说明，依赖它的判断却已当真执行。上游文档给出的迟到上限同理，它不可证伪。
- 核心维持跨流的事件时间对齐：没有任何来源证据支撑，对齐点只是核心的猜测。

### 3.2 基础值类型

money/quantity 是交易协议处理器的值类型（[envelope.md §2.4 推论](../boundary/envelope.md#24-推论)）。它们出现在核心**计算**处：写侧基本类型的守卫字段、`cumulative_filled_quantity`、程序 / 钩子解释器。观察侧信封不含价格，行情价格只在载荷里，由程序与钩子按 schema 解释。

| 类型 | 设计 | 依据 |
|---|---|---|
| 金额 | 精确有理数/定点数 + 货币索引；离散化返回余数，不静默丢钱 | fp-04 命题 1 safe-money |
| 数量 | 精确数值；**不**做“数量带 instrument 尺度”（无证据） | — |
| 身份 | 上游身份一律 `(venue, native_id)` opaque + 智能构造器 | fp-04 命题 16；域 F2 |
| 时间 | `occurred_at` / `received_at` 分离；每条记录的信封另有**记录时间**（写者为这次 append 采样的核心本地墙钟 UTC 时刻，加单调计数；写者可以先用同一次采样做自己的判定，再以它盖记录，[README.md §2.2 机器与域共享的现象](../../README.md#22-机器与域共享的现象)），是核心自己的时间；`deadline` 以 UTC 时刻声明并随记录持久化；运行期计时器用单调钟唤醒、到点之前按 UTC 墙钟复核；来源顺序只由来源给的定序证据（venue 序号等）给出，日志顺序只是 `(stream, seq)` 偏序，都不由任何时钟推导 | fp-04 命题 9/11；域 F11 |
| 错误 | 每规则封闭 sum；venue 映射保留 `Unmapped` | fp-04 命题 8；域 C13 |
| 外部写结果 | `Prepared \| SendBarrier \| VenueAccepted \| VenueRejected \| NotSent \| Undetermined \| Expired` + `ResolutionEvidence` 记录；另有 UTA 自己的 `Abandoned`（放弃等待，不是结果）；非 `Option`/字符串 | fp-06 命题 1（MongoDB `UnknownTransactionCommitResult`、Oracle in-doubt）；fp-01 M11 DAML 反例 |

- **身份**出现在四处：投影里的 `WriteLaneKey`/`StreamId`（[integration.md §3.3 投影 Projection](../boundary/integration.md#33-投影-projection)）、意图的 `target`（[trade-protocol.md §4.1 交易协议：操作种类、目标、可执行性、检查目录](../read-write/trade-protocol.md#41-交易协议操作种类目标可执行性检查目录-交易协议)）、观察记录的 `attribution`（[resolution.md §4.6 效应侧归因处理器](../read-write/resolution.md#46-效应侧归因处理器)）、成交记录的 `execution_id`（[trade-observation.md §3.1 成交的计数身份](../read/trade-observation.md#31-成交的计数身份-设计)）。instrument 只在 venue 作用域内有意义。换名不是安全，隐藏构造器才是。
- **时间**：`deadline` 以 UTC 时刻持久化，所以发出前门与过期步跨重启仍可比。
- **记录时间** [设计]：写者为这次 append 采样的核心本地墙钟 UTC 时刻，加一个单调计数区分同一时刻的记录；写者可以先用同一次采样做自己的判定，再以它盖记录（例如 `advance_retention` 的结论记录，[journal.md §3.2 控制动作 advance_retention(to: Set<LogPosition>)](../read/journal.md#32-控制动作-advance_retentionto-setlogposition)）。每条记录（观察与执行事实）都有它。它是核心自己的事实，不是来源给的 `received_at`（集成填写的锚点，只在集成交来的观察记录上）或事件时间。读它的地方：观察侧留存窗口的下界换算（[journal.md §2.6 推进判定](../read/journal.md#26-推进判定-设计)）；程序值经记录时间访问器读一条输入记录的记录时间，作截止时刻（[value-tree.md §3.1 组合子值树与五个 fold](value-tree.md#31-组合子值树与五个-fold)）；定时器请求的 `Fired` 以自己的记录时间作触发时刻（[timer.md §4.1 定时器处理器](../runtime/timer.md#41-定时器处理器-设计)）；冷却时钟取 `SendBarrier` 的记录时间（[decision-chain.md §4.4 lane 步、冷却与过期步](../read-write/decision-chain.md#44-lane-步冷却与过期步)）。
- **值树里的时间值** [设计]：UTC 时刻与时长两种值类型；时刻 + 时长得时刻。它们只从记录（记录时间）或常量得出，程序值里没有“此刻”。
- **计时器的复核规则** [设计]：STS 的 `deadline` 计时器（[decision-chain.md §4.4 lane 步、冷却与过期步](../read-write/decision-chain.md#44-lane-步冷却与过期步)）与定时器处理器（[timer.md §4.1 定时器处理器](../runtime/timer.md#41-定时器处理器-设计)）共用这一条：单调钟只负责唤醒；醒来之后、做到点动作之前按 UTC 墙钟复核，墙钟未到点就重新挂上。所以到点动作只会迟、不会早：墙钟回拨时醒得早，复核后重新等待；墙钟前跳时单调钟仍按原来的间隔唤醒，到点动作可能晚于墙钟到点的时刻。停止、崩溃与故障期间的时间都只推迟到点动作。

## 4 结构

### 4.1 持久化视图

两侧 `Journal`、订阅表与其余核心自有的表统一持久化于**单个 SQLite 文件**（WAL 模式），核心进程独占、单写者。存储引擎、主键形状、事务、快照与格式版本的设计在 [storage.md §2.1 一个文件、一个写者](storage.md#21-一个文件一个写者-设计)。段池是可选子系统的另一套存储，不在 SQLite 里，也不是 `Journal`。

**归属表**：每份状态谁写、谁读、怎么传播；细节在所有者文档。

| 状态 | 侧 | 写 | 读 | 传播 | 所有者 |
|---|---|---|---|---|---|
| 观察 J | 观察 | 程序宿主元素（派生记录、`ProgramReset`/`ProgramFailed`）；一次性读（结果项、读结论、`Gap{Channel}`）；持久订阅（回填结果、回填进度、`backfill_incomplete`、路由结论、覆盖检查点）；IO 壳（回执与取证的观察记录）；集成会话（会话与计数健康观察、握手时的 `Gap{Source}`）；推送经集成会话与信封解析入口；控制面（程序流开 epoch 的 `Gap{Source}`） | 订阅者、程序、单据 `basis`/检查项、读模型、效应侧归因处理器 | 按 `LogPosition` 推进；`RetractableDelta` 可撤回可压缩；健康流按键压缩，程序流按流 epoch 压缩；压缩只删不改 | [journal.md](../read/journal.md) |
| 执行 J | 效应 | [core/design.md §4.7.3 执行事实的唯一写入口](../design.md#473-执行事实的唯一写入口) | 读模型、单据、lane、IO 壳、出站请求处理器（重派判定）、程序宿主元素（控制流）、集成会话（声明）；投递调度按位置搬运字节 | 纯 append；位置即顺序；lane 上尝试的等待与结果、阻塞头集合是它的 fold | [storage.md §3 接口](storage.md#3-接口) |
| 规则状态（`RuleState`） | 效应 | 无，不持久 | STS 规则链每次求值时由执行事实 fold 出 | 没有副本 | [decision-chain.md §3.4 RuleState 是记录的 fold，不另存](../read-write/decision-chain.md#34-rulestate-是记录的-fold不另存-设计) |
| 单据记录（`TicketAction`） | 效应 | 单据负责人 / 决定者 | 单据 fold、审批人视图 | 每条带 principal 与依据 | [ticket.md §4.4 单据记录的持久化](../read-write/ticket.md#44-单据记录的持久化) |
| 订阅表 / cursor / 投递缺口 | 观察 | 只有持久订阅 | 投递调度、程序宿主元素（初次接纳结果）、读模型 `subscriptions` | cursor 前进；退回只经 `rewind_cursor` 或 `Reset` | [consumption.md §4.5 订阅表（持久化）](../read/consumption.md#45-订阅表持久化) |
| 能力证据（声明版本、`CapabilityObserved`） | 记录在执行 J | 集成会话（声明版本）、IO 壳（`CapabilityObserved`） | 集成会话求出两种解释交给各读者；读模型 `sources` 直接 fold | 只 append，最近一版生效 | [integration.md §3.6 声明的两种解释](../boundary/integration.md#36-声明的两种解释-设计) |
| 程序状态（`Checkpoint`） | 观察（解释①的累加器）与效应（解释②的请求进度跟踪） | 程序宿主元素，与程序订阅上的 cursor 同事务 | `Load` | 字节形状由宿主解释器定义，核心不解释；`state_version` 可比对 | [program-host.md §4.9 与同级组件的接口](../runtime/program-host.md#49-与同级组件的接口) |
| 进程表 | 核心 | 集成会话、程序宿主元素，按 `role` 分行各写各的 | 启动第 1 步回收；同一集成拉起下一个进程之前 | 指向 OS 进程的索引，存亡只问 OS；受控停止后不留行 | [storage.md §4.3 进程表](storage.md#43-进程表) |
| 实例表 | 核心 | 存储：第 1 步 fence 事务写新一行；受控停止第 5 步写结束锚点 | 启动取下一个 `instance_id`；诊断上一实例是否受控停止 | `instance_id` 单调递增；结束锚点是本实例最后一次写 | [storage.md §4.2 实例表](storage.md#42-实例表) |
| 保留边界与引用登记 | 观察 | 观察 `Journal`（边界推进经 `advance_retention`；引用随 [core/design.md §4.7.6 保留引用的登记与解除](../design.md#476-保留引用的登记与解除) 的 append 自动写入） | 压缩、`basis_valid`、快照 | 每条观察流一个边界，只前进 | [journal.md §2.4 保留：边界与删除规则](../read/journal.md#24-保留边界与删除规则) |
| 快照 | 两侧 | 存储（按运行期参数的频率，或 `request_snapshot`） | 重启恢复 | `fold_state` 在某个位置集上的结果，近处副本，只加速读，不是源头 | [storage.md §2.4 快照是近处副本](storage.md#24-快照是近处副本-设计) |

**会话与程序的持久状态是执行事实的 fold，不另存**：集成的持久 `Halted`、未兑现的轮换、程序的活动集合与失败抑制、已安装 op 集合、登记的采纳集合，全部按控制流上的位置 fold（规则在各自所有者：集成会话、程序宿主元素）。

**控制流** [设计]：每个用户状态根一条执行事实流，不属任何来源、lane 或作用域，承载核心对自己的控制面及其监管的运行所记的事实：登记的采纳记录、除 `bypass_lane` 之外的全部控制记录（`load_program`、`unload_program`、`install_native_op`、`remove_native_op`、`reload_config`、`rotate_credential`、`restart_integration`、`request_snapshot`、`advance_retention`、`rewind_cursor` 的 `Applied | Rejected`）、`IntegrationHalted`、轮换兑现记录与 `ProgramHalted`。`bypass_lane` 的控制记录随它所绕过的 lane 的流。它有三个写者（集成会话写采纳记录、`IntegrationHalted` 与轮换兑现记录，程序宿主元素写 `ProgramHalted`，其余由控制面写），都经存储的 append 接口。它跨实例延续，不随实例开新流。

- 理由：采纳集合、持久 `Halted`、程序的活动集合与失败抑制、已安装 op 集合、未兑现的轮换都是这些记录的 fold。放在一条有序的流上，每个 fold 都只按这条流上的位置求值，不需要跨流的先后（执行事实侧没有跨流的全局序，§3.1）；任一历史 `as_of` 上健康列哪些集成、哪些程序在运行、哪些被抑制，都由一条流上的位置确定（Q18/Q21/Q32）。
- 它经自己的执行事实 selector `Control` 订阅（只能 `ordered`、可从起点，[consumption.md §4.1 核心↔解释层：订阅组与 rewind_cursor](../read/consumption.md#41-核心解释层订阅组与-rewind_cursor)），不经 `(来源, WriteScope?)` 选中；消费方由此能自行 fold 出采纳集合、`Halted` 与活动集合。
- 不选：**这些记录不指定流**：fold 就要比较不同流上记录的先后；**每种事实各一条流**（采纳一条、程序一条……）：采纳集合与活动集合就各要自己的流身份，而“哪条 `Applied` 之后”的判定仍要先说清它们在哪条流上；**按来源分流**：采纳记录一次列出全部集成，程序与控制动作不属任何来源；**以实例表承载采纳**：实例表没有位置，不能按 `as_of` 切面。

## 6 评估

### 6.1 证伪条件

- **#4 三种进度互不替代**（§3.1）：某来源的完备进度可由消费位置或保留边界之一确定性算出，即存在一个标量在全部质量场景下同时正确表达三者 → 三种进度模型可塌为单一进度，`await-all` 触发条件（[consumption.md §3.6 三种消费方式（投递侧）](../read/consumption.md#36-三种消费方式投递侧)）与保留协议（[journal.md §2.4 保留：边界与删除规则](../read/journal.md#24-保留边界与删除规则)）须重写。
