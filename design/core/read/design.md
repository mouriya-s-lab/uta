# 单读链

## 0 定位

- **层级与元素**：core 的单读链：读副作用读进来只在核心内消费。上级文档：[core/design.md §4.4 模块地图](../design.md#44-模块地图)。
- **本目录**：本文（gap 的三种来源）、[supply.md](supply.md)（供给）、[backfill.md](backfill.md)（回填）、[one-shot-read.md](one-shot-read.md)（一次性读）、[journal.md](journal.md)（观察 `Journal`）、[consumption.md](consumption.md)（持久订阅与投递调度）、[derivation.md](derivation.md)（派生）、[trade-observation.md](trade-observation.md)（交易协议观察）。

## 3 模型

### 3.1 gap 的三种来源

三种 gap 按 `origin` 区分。它们说的事、源头与落点各不相同，不是同一种记录：

| | `Gap{origin: Source, reason}` | `Gap{origin: Delivery, reason}` | `Gap{origin: Channel, channel}` |
|---|---|---|---|
| 说的事 | 来源流本身不连续：新 epoch 的首条记录（带前一范围与最后 `Seq`；逻辑流的第一个 epoch 记为无前驱），或回填未能补齐时标出未覆盖区间的 epoch 内记录 | 某个订阅在某条流上没有收到位置 `from` 到 `to`（两端都含）的记录；流本身并不缺这些记录 | UTA 自己的那次读调用在这个渠道上没有拿到结果；不说来源少了什么记录 |
| 源头 | 来源的证据（不能续接、供给中断、回填穷尽），或核心对程序产出流开 epoch 的决定 | 核心的投递 | 核心的那次调用（UTA 自有事实） |
| 落点 | 观察 J 该流上的记录 | 订阅表里，按（订阅，流）记，与该流的 cursor 同一粒度：`{流, from, to, reason}`；不在任何流上 | 取证渠道：执行 J，属该尝试；回填与一次性读：观察 J 该流上的控制记录 |
| 计入读模型 `gaps` | 是 | 否（读模型直接 fold 日志、不经投递） | 否 |
| 细节 | 本节下表；握手与会话内开 epoch [integration.md §4.3.2 集成→核心的推送](../boundary/integration.md#432-集成核心的推送) | 写入与删除 [consumption.md §3.3 投递缺口 Gap{origin: Delivery}](consumption.md#33-投递缺口-gaporigin-delivery)；检测 [consumption.md §4.8 背压、conflation 与停投](consumption.md#48-背压conflation-与停投) | 取证 [resolution.md §4.1 对账驱动](../read-write/resolution.md#41-对账驱动)；回填 [backfill.md §4.1 核心→集成：backfill](backfill.md#41-核心集成backfillstream-window-subjects--covered--unavailable--refused)；一次性读 [one-shot-read.md §4.1 核心→集成：read](one-shot-read.md#41-核心集成readstream-request-range--answered--unavailable--refused) |

`Gap{origin: Source}` 的 `reason` 取 P3 的集合，各自的触发者与写者：

| `reason` | 触发者与写者 |
|---|---|
| `start`、`disconnect`、`quota`、`ingress_overflow` | 集成来源的流：集成上报（会话内上报的断代由集成推送，集成会话编排它的事务）；握手时决定开新 epoch 的，由集成会话 append（[integration.md §4.3.1 handshake](../boundary/integration.md#431-handshake--projection--refused--unavailable)）。程序产出的派生流上的 `start`：让程序 id 进入活动集合的 `load_program`（首次装载，或卸载之后再装载），控制面在它的 `Applied` 事务里、在新成员的每条程序流上 append（[derivation.md §3.1 输出契约与程序流](derivation.md#31-输出契约与程序流)） |
| `credential_rotated` | 有未兑现轮换的集成此后第一次成功的握手，集成会话 append（[integration.md §4.9 轮换强制的新流 epoch](../boundary/integration.md#49-轮换强制的新流-epoch-设计)） |
| `schema_change` | 载荷版本变化，集成上报（[envelope.md §3.3 payload_schema 与 schema 发布](../boundary/envelope.md#33-payload_schema-与-schema-发布)） |
| `backfill_incomplete` | 回填穷尽，持久订阅 append（[backfill.md §4.2 实时边界、回填任务与回填进度](backfill.md#42-实时边界回填任务与回填进度)）；它标出 epoch 内未覆盖的区间，不结束也不开 epoch |
| `program_upgrade` | 不沿用旧状态的替换（程序升级、输出契约改变或运维冷启动，即 `Reset`），控制面在替换的 `Applied` 事务里、在新成员的每条程序流上 append（[derivation.md §3.1 输出契约与程序流](derivation.md#31-输出契约与程序流)） |

- `Delivery` 的 `reason ∈ {slow_consumer, compacted, conflated}`：`latest` 订阅的投递缓冲耗尽而被停投、订阅位置已被压缩到保留边界之下、`latest` 消费合并。程序经它的程序订阅消费，`latest` 输入滞后被跳过的区间是该订阅上的 `Delivery` gap；`ordered` 与 `await-all` 只背压，从不因慢被跳过。执行事实订阅没有这种缺口。
- 一次性读的 `Channel` gap 带这次调用的 `provenance: OneShot{origins, request}`、`dispatch_end` 与发出会话的 `session_epoch`，与它的结论记录会带的相同，等待者凭它们认出自己那次读。
- **集成崩溃的观察流面**：集成在观察流侧崩溃（订阅 / 推送进程掉线）时，仅波及其负责的流，记录为 `Gap{origin: Source}`，核心不受影响。另一故障面与判别边界见 [resolution.md §4.5 崩溃恢复与集成崩溃两故障面](../read-write/resolution.md#45-崩溃恢复与集成崩溃两故障面)。

**为什么分三种。** 三者的源头不同：来源断代只有来源能证明（或是核心对自己产出流的决定），投递损失是核心对某个订阅做的事，渠道失败是核心某次调用的结果。合成一种就会把“流缺记录”“这个订阅没收到”“这次没问到”混同：读模型 `gaps` 会报出流本身并不缺的段，或漏报真正的断代。

## 6 评估

### 6.1 验收标准

- **#21 一次性读与配额**（[one-shot-read.md §4.1 核心→集成：read](one-shot-read.md#41-核心集成readstream-request-range--answered--unavailable--refused)、[supply.md §4.1 核心↔解释层：订阅组与 rewind_cursor（配额）](supply.md#41-核心解释层订阅组与-rewind_cursor配额)）：（对应 Q13/Q15/Q16）
    - 多 target `read` 中一个来源不可用时，其余 target 独立返回，不可用项在 `deadline` 内可见；
    - 能力 `Unsupported` 的读返回 typed `Unsupported`，而非空结果；
    - 超过投影配额的订阅被 typed 拒绝，集成未收到该订阅，且既有订阅不受影响。

- **#87 流上的调用跨过会话内的流 epoch 更替**（[one-shot-read.md §4.1 核心→集成：read](one-shot-read.md#41-核心集成readstream-request-range--answered--unavailable--refused)、[backfill.md §4.1 核心→集成：backfill](backfill.md#41-核心集成backfillstream-window-subjects--covered--unavailable--refused)、[supply.md §4.2 核心→集成：route](supply.md#42-核心集成routestream-subjects-generation--routedrefused--unavailable)、[trade-observation.md §3.2 订单身份、来源顺序与最近观察](trade-observation.md#32-订单身份来源顺序与最近观察-设计)、[integration.md §4.3.2 集成→核心的推送](../boundary/integration.md#432-集成核心的推送)、[integration.md §4.3.2 集成→核心的推送](../boundary/integration.md#432-集成核心的推送)）：fixture 集成在一个已建立的会话里，某条声明 `joinable_venue_seq` 的订单状态流处于 epoch e1（`generation` g），e1 的回填任务有一个窗口在途、一次一次性读在途、一次 `route`（`All`，带 g）在途，三者都已到达集成，另有一笔 `submit` 在途；fixture 上游推送通道断开重连：（对应 Q1/Q11/Q14/Q15）
    - 合格集成：fixture 观测到三次读侧调用（`read`、`backfill`、`route`）的 `Unavailable` 都先于 `Gap{origin: Source}` 送出，这条 gap 带 g + 1；`read` 与 `backfill` 各一条 `Gap{origin: Channel}` 落在 e1 里，`route` 没有记录；每次调用恰计一次失败；核心若在接受 gap 之前按 pacing 再发，那次 `route` 带 g、得 `Unavailable`、计一次失败；接受之后再发的都带 g + 1。fixture 此后不改变需求、不再开新 epoch，并让第一次带 g + 1 的 `route` 返回 `Routed`：带 g + 1 的 `route` 恰一次，核心不再因这条 gap 另发（重试与 gap 触发的重发是同一项义务）；
    - 跨越的 `route`：同一场景里，该流的需求在集成送出 gap 之后、核心接受它之前变了，核心发出一次带 g 的 `route`（此前的 `route` 已返回）：集成答 `Unavailable`，供给不变，fixture 上游没有收到新的订阅请求，e2 在带 g + 1 的 `route` 之前没有推送；核心不 append 路由结论记录，这次计一次失败；随后核心发出带 g + 1、全集为最新需求的 `route`，fixture 让它返回 `Routed`：这样的 `route` 恰一次，它的 `Routed` 在 e2 上 append 路由结论记录，e2 的推送在它之后才计入序号覆盖；
    - fixture 集成违约（已收到 `read`、`backfill` 之后先送出 `Gap{origin: Source}` 开 e2，再对二者分别返回 `Answered`、`Covered`）：二者各在 e2 上恰一条 `Gap{origin: Channel}`，没有它们的 item 记录、结果记录与读结论记录；`read` 的那条带这次调用的 `OneShot{origins, request}`、e1 里的 `dispatch_end` 与发出它的会话的 `session_epoch`；e1 回填任务的进度不再变化，e2 的回填进度与 `covered_to` 里没有这次 `Covered` 的内容；每次调用恰计一次失败；该读此前得 `Pending{from, request, instance_id}` 的发起方以只投递项从 `from` 订阅，越过 `Gap{origin: Source}` 恰收到 e2 上的那条 `Gap{origin: Channel}`，按请求身份、`origins`、`dispatch_end` 与 `session_epoch.instance_id` 认出它；
    - 合规的竞态：同一场景的变体里，核心在 e1 里发出的 `read` 与回填窗口由 fixture 在集成送出 gap 之后才交给集成，集成分别返回 `Answered`、`Covered`：核心的处置与上一条相同，一致性测试不判它们不合格；
    - `submit` 的回执在 e2 开始之后到达，含订单状态与一笔成交：两条观察记录照常分别 append 在该作用域的订单状态流与成交流上并进 `Evidence`，`provenance` 相同，各带自己那条流在调用发出时的 `dispatch_end`；订单状态记录的 `dispatch_end` 在 e1 里，结果照常确立；它带的 venue 序号不与 e2 的推送定序、不计入序号覆盖：e2 上一条带序号的推送与它内容不同时，流不声明 `order_revision` 的，`orders` 并列二者并标出顺序未确立（声明 `query_not_lagging` 时回执也只取代它 `dispatch_end` 及之前 append 的记录，不取代 e2 上的这条推送）；成交流没有更替，成交记录的序号照常作来源顺序证据；取证调用（`list_open`）的回答跨过更替时同样如此；
    - 集成一致性：已收到的 `read`、`backfill` 在 gap 之后才作答（上文违约一条）被一致性测试判为不合格，gap 之后才收到的照常作答（上文合规的竞态）不判；集成对带旧 `generation` 的 `route` 答 `Routed`、或在收到带新 `generation` 的 `route` 之前推送 e2，被一致性测试判为不合格（核心不另判）。
