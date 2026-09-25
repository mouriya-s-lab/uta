# 03 观察入库、readiness、回填、订阅与 gap

对照：§4.1、§4.2、§8.3、§8.4、§8.5 订阅组、W6。索引见 `README.md`。

## D3.1 一次推送的入库时序

对照：§8.3 推送表与集成义务（位置与完备、归因）、§7.2 会话 epoch、§2.1 处理器、§4.3、§4.2 投递、§6.2 偏离是状态、§8.1 来源顺序、§8.4 序号覆盖。

```mermaid
sequenceDiagram
  participant V as venue
  participant I as 集成进程
  participant B as 信封解析入口
  participant J as 观察 Journal
  participant EJ as 执行 Journal
  participant H as 处理器注册表
  participant D as 派生 DAG
  participant T as 单据 fold
  participant S as 投递调度
  V-->>I: 上游事件（成交 / 报价 / 余额）；集成在此消费上游
  I->>B: 经当前会话的通道推送观察记录{StreamId, received_at, venue seq, occurred_at?, attribution?, 契约载荷 + payload_schema, 原始负载（订单状态 / 成交必带，其余按映射声明）}
  Note over I,B: 只从当前会话的通道读入；通道关闭之后，那个会话送来的任何东西都不再读入、不 append
  alt 缺锚点
    B-->>I: 畸形记录，拒绝
  else 接受
    B->>J: 核心盖上该会话的 SessionEpoch，分配 LogPosition（按到达顺序）；venue seq 倒退/重复 → 照常 append 并打 replayed / out_of_order
    Note over J: venue 序号 → 推进序号覆盖（该 epoch 开始时声明 joinable_venue_seq 的流；只计入 append 在同会话、确认供给 All 且 refused 为空的路由结论记录之后的推送；已在覆盖里的序号再到达不改变它）<br/>occurred_at 只是事件时间，不推进任何完备进度
    J->>H: 字段出现 → 触发
    opt attribution: FromAttempt(r)（集成依上游关联证据填写；核心不凭键字节归因），且 r 处于 Undetermined、结果未确立（已 Abandoned 的同样可补）
      H->>EJ: 同事务 append ResolutionEvidence{r, Attributed, Found{observation: 该记录, evidence: 载荷 + 原始负载}}
    end
    J->>D: 受影响节点增量重算（cutoff）；派生记录写回观察 J
    J->>T: 引用该流的单据重算 basis_validity / alignment
    J->>S: 按订阅表投递给已确认 cursor 在该记录之前的订阅者（记录位于 cursor 之后）
  end
```

读法：

- 位置归核心，完备归来源：`LogPosition` 由核心按到达顺序分配，核心是它的源头，但到达顺序只是日志顺序，不是来源顺序；venue seq、`occurred_at` 是来源给的字段，随记录作为副本保存。完备只来自来源证据（序号覆盖），不从到达时刻或任何时限推导。
- 乱序与重复不被核心修正，由派生侧 fold 按种类处理：成交按 execution_id 与修订计数；订单状态与持仓按身份取最近观察，只凭四种声明的来源定序证据比较（同一 epoch 的 venue 序号、`order_revision`、声明 `query_not_lagging` 的流上不带序号的回答对 `dispatch_end` 及之前的记录、声明 `push_ordered` 的流上同一会话 epoch 且同一流 epoch 内的两条推送；上游推送通道重连由集成报为 `Gap{Source}` 开新流 epoch，两侧推送不可比），其余不可比，最近观察顺序未确立时并列；其余种类由各自的 fold 规定（§8.1）。
- 同一条推送可能同时是三件事：订阅者的一条记录、某单据偏离状态的触发、某 `Undetermined` Attempt 的决议证据。
- 迟到回执走的就是这条路：旧会话的通道关闭后不再读入（其在途调用已在会话结束时完成为 `NoResponse`）；新会话重送的在 `opt` 分支并入原 Attempt。

核出：无。

## D3.2 集成 × 流的 readiness、回填进度与流 epoch 决定

对照：§8.4 readiness 的 fold、回填进度与健康不留旧值、§7.2 会话状态、§8.2 `handshake`、§4.2 `Gap{Source}` 原因集、§7.6 `rotate_credential`。

```mermaid
stateDiagram-v2
  state "Disconnected（派生值，不是记录）" as DC
  state "Live{live_from}（非 Degraded）" as LN
  [*] --> DC : 该集成的会话为 Connecting 或 Halted
  DC --> Starting : 会话 Established（新 SessionEpoch）；该流在本会话、当前流 epoch 里尚无 readiness
  Starting --> Live : 集成声明 live_from（核心盖上本会话的 SessionEpoch 与该流当前流 epoch 后 append）：声明 joinable_venue_seq 的流于上游确认实时订阅即声明（下一个期望序号，不等首条记录）；其余流为首条实时记录的事件时间，与该记录同时或先于它声明
  state Live {
    [*] --> LN
    LN --> Degraded : 集成上报：上游降级 / 上游限流 / 能力收紧（子态，接受条件不变；与核心配额挂起订阅互相独立）
    Degraded --> LN : 恢复
  }
  Starting --> DC : 会话离开 Established（最近的会话记录不再是 Established）
  Live --> DC : 会话离开 Established（旧会话的 Live 随会话记录更替失效）
  Live --> Starting : 会话内集成上报 Gap{Source} 开新流 epoch（旧 epoch 的 Live 随该逻辑流的 None{新 epoch} 失效；上报之前集成已以 Unavailable 完成该流在途的 read / backfill / route）；核心为新 epoch 重发 route，集成收到后才推送，再为新 epoch 声明 Live{live_from}
  note right of DC
    由会话状态派生：没有记录、没有自己的时刻
    只随附当前会话状态及其 since（会话记录的时刻，不是断线时刻）
    核心不替集成 append readiness，也不回填崩溃发生的时刻
  end note
  note right of Starting
    流 epoch 的开始（首条 Gap{Source}，与该逻辑流的 None{epoch} 同事务）：
    握手时每条流决定：能以 venue 游标证明续接 → 续原 epoch，Seq 接续；
    否则 → 新 epoch，首条 Gap{Source, disconnect / start / quota / ingress_overflow}
    rotate_credential → 强制新 epoch，reason credential_rotated
    载荷版本变化 → 新 epoch，reason schema_change
    会话内：集成上报供给中断（含上游推送通道重连）→ 新 epoch，与握手开的新 epoch 同样处理：
    readiness 回到 Starting、核心重发 route、新 live_from、回填任务在它声明时判定
  end note
```

```mermaid
stateDiagram-v2
  state "None{epoch}（无回填任务：尚未声明 live_from / 续接原 epoch / 能力不支持 / 需求为空 / 回填深度为 0 / 深度为 Origin 而会话有效声明无 backfill_from_origin；health 不列）" as NONE
  state "Backfilling{through}" as BF
  state "Closed" as CL
  state "Reached" as RE
  state "Incomplete{through}" as INC
  [*] --> NONE : 流 epoch 开始（新 epoch：None{epoch} 与起点 Gap{Source} 同事务 append）
  NONE --> BF : live_from 声明时一次判定，全部成立才建任务：epoch 以 Gap{Source} 开始 ∧ 会话有效声明 backfill Supported ∧ 需求非空（route 全集，含核心的 All）∧ 回填深度 ≠ 0（为 Origin 时须 backfill_from_origin）→ 持久订阅建立任务，append Backfilling{起点}；起点 = live_from − 回填深度，或 Origin
  BF --> BF : 读结论 covered_to 推进 through；Unavailable 不推进；能力不再 Supported 或无已建立会话期间不发调用、停在 Backfilling，恢复后从最近的 covered_to 续
  BF --> CL : joinable_venue_seq 的流：covered_to 连到 live_from（序号连续，边界闭合；序号覆盖的起点下延到任务起点）
  BF --> RE : 只有事件时间的流：covered_to 连到 live_from（已取得 [起点, live_from)，衔接未证明；没有完备进度，Gap{Source} 不视为闭合）
  BF --> INC : 历史穷尽或 Refused 而未达 live_from → Gap{Source, backfill_incomplete}；序号覆盖的起点停在 live_from
  note right of BF
    核心判定、持久订阅 append 健康观察（与读结论 / backfill_incomplete 同事务）
    只请求 < live_from 的窗口；实时记录同时照常到达
    深度、起点、主体集在建立时取定；判定之后能力、需求、深度或声明的变化只影响下一个流 epoch
    本 epoch 不补建、改建或撤销任务；新 epoch 开新判定，旧任务的终态不沿用
    进度值带所属 epoch；health 只列当前 epoch 的非 None 值
  end note
```

读法：readiness 是集成在一个会话里、对该流一个流 epoch 的实时供给的陈述：集成推送，核心盖上到达会话的 `SessionEpoch` 与该流当时的流 epoch 后 append；fold 只取同时带着最近一条会话记录的 `SessionEpoch` 与该流当前流 epoch 的值，且只在那条会话记录是 `Established` 时；已建立而当前流 epoch 里没有这样的值为 `Starting`，会话不在 `Established` 时为派生的 `Disconnected`。会话内上报的 `Gap{Source}` 开新流 epoch，readiness 回到 `Starting`，与握手开的新 epoch 同样处理：同一会话里 e0 为 `Live`、之后 `Gap{Source}` 与 `None{e1}` 已 append 而 e1 尚无 readiness，该流是 `Starting`，不是 e0 的 `Live`。流上的 `read`、`backfill`、`route` 调用是发出时那个流 epoch 的内层：集成在上报 `Gap{Source}` 之前以 `Unavailable` 完成它们，迟到的结果由核心完成为 `Unavailable`、不作新 epoch 的记录（§8.2、§8.4）。回填进度由核心凭读结论记录判定、持久订阅 append，只说当前流 epoch 的 `[起点, live_from)` 补齐到哪。二者都是健康观察（不需确认），经 `health` 读模型对解释层可见，再由它翻成下游的连接状态；它们不改变记录接受条件——核心只从当前会话的通道读入。

核出：readiness 原把 `Backfilling` 放在 `Live` 之前，而回填窗口的终点 `live_from` 要到 `Live` 才有，`through` 又是核心凭读结论记录才证明得了的——已拆为集成推送的 readiness 与核心判定的回填进度（§8.4）。

## D3.3 回填与实时边界

对照：§8.2 `backfill`、§8.4 回填、实时边界、衔接、序号覆盖；§8.1 精确重建的前提；Q14。

```mermaid
flowchart LR
  subgraph EPOCH["同一流 epoch"]
    direction LR
    G["Seq 1<br/>Gap{Source}"] --- BF1["窗口 1 的记录<br/>backfilled + 读结论（covered_to）"] --- BF2["窗口 2 的记录<br/>backfilled + 读结论（covered_to）"] --- LF["live_from<br/>实时供给起点"] --- L1["实时 …"]
  end
  CORE["核心按回填任务切窗口<br/>backfill(stream, window, subjects)；坐标：joinable_venue_seq 用 venue 序号，否则事件时间<br/>首窗口 from = 起点（live_from − 回填深度，或 Origin）；subjects = 任务建立时被路由的主体集，各窗口不变；上游分页与 pacing 在适配器内"] -->|"只请求 < live_from"| BF1
  BF1 -->|"下一窗口从 covered_to 续（崩溃重启同样）"| BF2
  BF2 -->|"covered_to 连到 live_from ∧ joinable_venue_seq"| CLOSE["Closed：边界闭合，序号覆盖的起点下延到任务起点<br/>回填与实时之间无重叠无洞（Q14）"]
  BF2 -->|"covered_to 连到 live_from ∧ 只有事件时间"| REACH["Reached：衔接未证明，没有完备进度<br/>Gap{Source} 仍列在 gaps，成交完整性条件 1 不成立"]
  CLOSE -.->|"仅当声明 backfill_from_origin ∧ 起点 = Origin ∧ subjects = All"| C1["成交完整性条件 1 成立（§8.1）：序号覆盖为 [Origin, n)<br/>有限起点的 Closed 不闭合 epoch 开头的 Gap{Source}"]
  C1 -.->|"另需条件 2"| C2["从 live_from 到 n 有效供给为 All：<br/>这段推送都计入了序号覆盖，即都在 e 上同会话、确认供给 All 且 refused 为空的路由结论记录之后<br/>（流在配额池里时核心不要求 All，不成立）"]
  BF2 -->|"上游历史穷尽（covered_to 未达）或 Refused"| INC["append Gap{Source, backfill_incomplete}<br/>序号覆盖的起点停在 live_from，不越过未覆盖区间（不伪造连续）"]
  CORE -->|"Unavailable（任一页失败即整体失败）"| CH["Gap{Channel, backfill}<br/>可再发"]
  CORE -->|"结果到达时该流已开新 epoch（发出 epoch = 任务所在 epoch 已结束）"| STALE["核心完成为 Unavailable：Gap{Channel} 记在当前 epoch<br/>不 append 结果与读结论记录；旧任务不再推进（§8.2 backfill）"]
```

读法：回填记录与实时记录同形、同 epoch，只多一个 `backfilled` 标记；回填窗口都在 `live_from` 之前。有可衔接序号的流上二者按序号不重叠也不留洞，不需要逐条去重。只有事件时间的流上，实时订阅确认前上游已发出的记录可能两边都不在，迟到的实时记录也可能与回填的同一对象并存，所以只称“到达”，也没有完备进度。续点是读结论记录里的覆盖边界，不需要任何一方保存游标。

核出：无。

## D3.4 订阅、route、cursor、ack、慢消费者与重连

对照：§4.2 cursor 与确认、§8.5 订阅组（两种 selector、逐项接纳、配额、订阅状态）、§8.2 `route`、§7.5 订阅表、W6 步 3–4、W14、W20。

```mermaid
sequenceDiagram
  participant C as 消费方（解释层代下游，或程序）
  participant SUB as 持久订阅元素
  participant I as 集成进程
  participant DL as 投递调度
  participant J as 观察 Journal
  participant EJ as 执行 Journal（经存储按位置读，不解析）
  C->>SUB: subscribe(selector, mode, from?)（mode = ordered | latest；selector = 观察流：一组 (来源, 流, 主体集?, 用途：供给 | 只投递) 项，可跨来源；或 执行事实 (来源, WriteScope?)；或 控制（核心的控制流））
  alt 执行事实 / 控制：mode ≠ ordered，或执行事实的 WriteScope.key 不在该来源任何声明版本里（不看采纳集合）
    SUB-->>C: 拒绝
  else 观察流：逐项判定（各项独立，按项在请求里的次序）
    Note over SUB: 每项：来源不在采纳集合且从未有声明 → 拒绝；在采纳集合而从未有声明 → 待接纳；供给项：来源不在采纳集合 → 拒绝（来源未登记），流不在最近声明 / 配额池里不带主体集 → 拒绝，放不下 → QuotaExceeded（集成不收到超限主体）；只投递项：流在该来源任何一个声明版本里出现过 → 接纳（不看采纳集合、不要求最近声明、不要求会话；不进需求、不占配额），从未声明过 → 拒绝；from < 保留边界 → 该项 BeyondRetention
    alt 没有任何一项被接纳或待接纳
      SUB-->>C: Rejected{items}
    else 至少一项被接纳或待接纳
      SUB->>SUB: 写订阅表，cursor 每条选中流一个位置 = from（缺省 = 当前流末，不补历史）；订阅归属 principal，持久
      SUB-->>C: Subscription{id, items}（逐项结果；整体状态由逐项派生）
    end
  else 执行事实 / 控制：接受
    SUB->>SUB: 写订阅表；执行事实的 selector 是成员规则：此后出现的 lane 流自动加入，从其首条记录起；控制订阅只有控制流一条；from 可为起点
    SUB-->>C: Subscription{id, items}
  end
  opt 某流的需求（各供给项主体之并 ∪ 核心自己的需求；只投递项与挂起项不计）变了，且该集成会话已建立
    SUB->>I: route(stream, 主体全集 | All | 空集)
    alt Routed{refused}，且该流的当前流 epoch 仍是发出时的 epoch
      I-->>SUB: Routed{refused}
      SUB->>J: 同事务 append 路由结论记录（这次生效的主体全集：All 或主体集，refused 及原因；不记增减，相对同一流 epoch 内上一条的增减由相邻两条算出）；refused 主体在所属各项内列为“来源拒绝”
    else Unavailable
      I-->>SUB: Unavailable（不 append；按 pacing 重发其时最新的全集；同一流至多一次 route 在途）
    else Routed 到达时该流已开新 epoch（集成本应在上报 Gap{Source} 之前以 Unavailable 完成它）
      I-->>SUB: Routed{refused}（核心按 Unavailable 完成：不 append 路由结论记录，不记 Gap{Channel}；重发即新 epoch 本来要做的那次 route）
    end
  end
  loop 记录到达
    J->>DL: 新记录 pos（观察流订阅）
    EJ->>DL: 新记录 pos（执行事实订阅：存储按位置交出已提交的字节，投递不读读模型、不解析）
    DL->>C: 投递 pos（数据记录按信封 subject ∈ 项的主体集过滤，整条流的项不过滤；流上的控制记录 Gap{Source} / Gap{Channel} / 读结论 / 路由结论投给该流每一项；同一订阅每条记录至多一次；每条流各自有序，流与流之间不定序；已投未确认 = 消费者内存里的事）
    C->>SUB: ack(subscription, cursor = pos)（确认 = 已处理）
    SUB->>SUB: cursor 推进（单写者，串行化）
  end
  alt 慢消费者：缓冲耗尽
    DL->>SUB: 请求写下投递缺口
    SUB->>SUB: 订阅表该（订阅, 流）写下 Gap{Delivery} {流, from, to = 被跳过的最后一个位置, slow_consumer}；写下之后才跳过
    DL->>C: 停投；先交出这条缺口（需显式确认），再交 to 之后的记录
    C->>SUB: ack(subscription, cursor ≥ to)（显式确认这次损失）
    SUB->>SUB: cursor 推进与删除该缺口在同一次写里
    DL->>C: 从 to 之后恢复投递
  end
  alt 断连 / 崩溃
    C--xDL: 连接断
    Note over SUB: 订阅 owner 是核心，订阅、cursor 与未确认的投递缺口保留
    C->>SUB: 重连：同一 principal 重新 handshake → 自动挂接其持久订阅（不需再 subscribe）
    DL->>C: 先交出未确认的投递缺口，再从 cursor 之后重投；未确认区间可能重复，按 LogPosition 去重
  end
  opt 重新握手使某流进入配额池
    SUB->>SUB: 该流上整条流的供给项挂起（WholeStreamInPool），核心自己的 All 撤去；离开所有配额池时恢复
    SUB->>I: route(stream, 重算后的全集)
  end
  opt 会话内集成上报 Gap{Source}，该流开新流 epoch
    SUB->>I: route(stream, 当前全集)：新流 epoch 在收到 route 之前不推送
    I-->>SUB: Routed{refused} → SUB 在新 epoch 里 append 路由结论记录（全集、refused 及原因；推送只在同会话、全集为 All 且 refused 为空的一条之后才计入序号覆盖）
  end
  Note over SUB,I: 集成每次会话建立后与每个会话内新开的流 epoch 上，SUB 对每条需求非空的流重发 route；每个流 epoch 有自己的路由结论记录，在该 epoch 第一次 route 之前集成不推送该流
```

读法：

- 确认语义唯一：`ack` = 已处理。已投未确认的记录在崩溃后会再见一次；已确认的永不重投（退回只经控制面 `rewind_cursor`）。
- 程序是同一种订阅者：它的 cursor 与 `Checkpoint` 同事务持久化（D4.1），所以程序永远不会看到已折入状态的记录。
- 需求按流的全集下发：多个订阅对同一主体只路由一次，配额在核心计量。序号覆盖只看记录：推送只在该流 epoch 上同会话、确认供给 `All` 且 `refused` 为空的路由结论记录之后才计入；之后一条不确认这一点的路由结论记录使计入停住，直到下一条确认的记录（§8.4）。订阅表与核心自己的需求都不是它的输入。
- 投递按主体：数据记录只看信封上的 `subject`，不论来自推送、回填、一次性读还是回执；控制记录投给该流每一项。只投递项不改变需求与配额，一次性读得 `Pending{from, instance_id}` 后从 `from` 订阅一个只投递项，即收到那条结论或 gap（同一核心实例内）。
- 投递缺口是订阅的状态，不是流上的记录：投递调度要跳过时先请持久订阅写进订阅表，写下之后才跳过；每次投递与重新挂接都先交出它；确认不低于 `to` 的 cursor 时与 cursor 推进同一次写删除，取消订阅或该流从订阅里移除时随之删除。`subscriptions` 读模型逐项列出未确认的缺口；其他订阅者看不到它。
- 执行事实订阅走同一套 cursor 与 ack，但只能 `ordered`：执行事实不压缩、不合并，所以慢消费者只背压自己，没有投递缺口；投递经存储按位置读出已提交的记录、原样搬运，不经读模型、不解析，所以观察侧的订阅与投递元素不依赖效应侧类型（§7.3 uses 图）。它不经 `route`，不受声明与会话影响。

核出：无。

## D3.5 三种消费方式

对照：§4.2 消费方式表、`await-all` 的要求点与装载期检查；§2.3 位置归核心、完备归来源。

| 方式 | 触发依据 | 是否跳过 | 损失记法 |
|---|---|---|---|
| await-all | 所有指定输入都达到要求点：核心日志位置（核心自己可判定，不说任何完备），或来源证据证明的覆盖在它自己坐标上的一点（今天只有序号覆盖：当前流 epoch 的一个 venue 序号，`through` 越过即满足） | 否（等待；无完备证据的输入上覆盖要求永不满足） | — |
| ordered | 单流顺序；多条流各自有序，流与流之间不定序 | 否（背压） | — |
| latest / conflated | 最新值 | 是 | 订阅上的投递缺口 `Gap{Delivery, conflated}`，或消费者声明的等待窗口 |

读法：

- `await-all` 的覆盖要求看来源证据证明的覆盖，不看 cursor——读到 Seq 60 不说明之前的记录已全部到达。没有 `joinable_venue_seq` 的输入没有完备证据：按最近声明，要求覆盖的 `await-all` 在装载时被拒（与 `required_inputs` 缺字段同属 fail-closed），程序改等核心日志位置、用 `ordered`，或声明自己的等待窗口；装载后某个新流 epoch 不再有证据时，该 epoch 上的覆盖要求不满足，程序不因它推进。
- 跨流按事件时间对齐没有来源证据，不提供；要按时间截止的消费者声明等待窗口：它是消费者的意图，不是流的元数据，也不冒充完备，窗口之后到达的记录照常 append、照常投递。
- 损失语义由消费者显式声明：UI 报价显示可接受 conflation，依赖完整状态路径的阈值策略不能默认接受。

核出：无。

## D3.6 gap 来源判定

对照：§4.2 gap 三来源、§7.5 观察 `Journal` 写者与订阅表、§8.3 映射表、§6.7 两故障面。

```mermaid
flowchart TB
  Q{"缺口出在哪一段？"}
  Q -->|"来源流本身断代<br/>断线 / 配额 / 溢出 / 换凭据 / 载荷换版 / 回填穷尽 / 程序升级"| S["Gap{origin: Source, reason}<br/>观察 J 该流上：新 epoch 首条或流内记录<br/>写者：集成推送入口（流内）/ 集成会话（握手开新 epoch）/ 持久订阅（backfill_incomplete）/ 核心（程序流新 epoch）"]
  Q -->|"核心到某个订阅的投递<br/>慢消费者 / 订阅位置被压缩 / conflated"| D["Gap{origin: Delivery, reason}<br/>订阅的状态：订阅表里按（订阅, 流）记 {流, from, to, reason}，不在任何流上<br/>写者：持久订阅（应投递调度请求，先写后跳）；订阅者确认不低于 to 的 cursor 即删除"]
  Q -->|"核心发起的读渠道不可用<br/>取证 / 回填 / 一次性读 返回 Unavailable"| C["Gap{origin: Channel, channel}<br/>该次调用的结果，可再发<br/>写者：IO 壳（取证，执行事实侧属该 Attempt）/ 持久订阅（回填）/ 一次性读元素（观察侧该流）"]
  Q -->|"submit 无业务回执"| U["不是 gap：Undetermined<br/>写边界 in-doubt（D6.1）"]
```

读法：集成一个进程崩溃可能同时产生 `Gap{Source}`（它的流）和 `Undetermined`（它在途的 `submit`）；两面各自收敛，互不替代。读模型的 `Snapshot.gaps` 只列 `Gap{Source}`：`Channel` 是调用结果，`Delivery` 属订阅。

核出：`Gap{Channel}` 的落点（取证 → 执行事实侧属该 Attempt；回填 / 一次性读 → 观察侧该流）原文未写——已并入 §4.2。
