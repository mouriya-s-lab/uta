# 03 观察入库、readiness、回填、订阅与 gap

对照：§4.1、§4.2、§8.3、§8.4、§8.5 订阅组、W6。索引见 `README.md`。

## D3.1 一次推送的入库时序

对照：§8.3 推送表与集成义务（时间权威与归因）、§2.1 处理器、§4.3、§4.2 投递、§6.2 偏离是状态。

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
  I->>B: 推送观察记录{session_epoch, StreamId, received_at, venue seq, attribution?, 契约载荷 + payload_schema, 原始负载（订单状态 / 成交必带，其余按映射声明）}
  alt 缺锚点
    B-->>I: 畸形记录，拒绝
  else session_epoch != 当前
    B-->>I: 边界丢弃，不 append
  else 接受
    B->>J: 分配 LogPosition（按到达顺序）；venue seq 倒退/重复 → 照常 append 并打 replayed / out_of_order
    J->>H: 字段出现 → 触发
    H->>J: occurred_at → 推进完备进度（无 occurred_at 按 received_at − 滞后界）
    opt attribution: FromAttempt(r) 或 idempotency_key 经登记解析到腿 r，且 r 处于 Undetermined 未终结
      H->>EJ: 同事务 append ResolutionEvidence{r, Attributed, Found{observation: 该记录, evidence: 载荷 + 原始负载}}
    end
    J->>D: 受影响节点增量重算（cutoff）；派生记录写回观察 J
    J->>T: 引用该流的单据重算 basis_validity / alignment
    J->>S: 按订阅表投递给已确认 cursor 在该记录之前的订阅者（记录位于 cursor 之后）
  end
```

读法：

- 核心是时间权威：`LogPosition` 按到达顺序分配，venue seq 只是证据；乱序与重复不被核心修正，由派生侧 fold 按种类处理：成交按 execution_id 与修订计数，订单状态与持仓按身份取最近观察（带 venue 序号的较早记录不取代较新的），其余种类由各自的 fold 规定（§8.1）。
- 同一条推送可能同时是三件事：订阅者的一条记录、某单据偏离状态的触发、某 `Undetermined` Attempt 的决议证据。
- 迟到回执走的就是这条路：旧 epoch 的被丢在第二个分支；新会话重送的在 `opt` 分支并入原 Attempt。

核出：无。

## D3.2 集成 × 流的 readiness、回填进度与流 epoch 决定

对照：§8.4 readiness 状态机与回填进度、§8.2 `handshake`、§4.2 `Gap{Source}` 原因集、§7.6 `rotate_credential`。

```mermaid
stateDiagram-v2
  state "Live{live_from}（非 Degraded）" as LN
  [*] --> Starting : 会话建立（新 session_seq）；集成推送
  Starting --> Live : 集成声明 live_from：声明 joinable_venue_seq 的流于上游确认实时订阅即声明（下一个期望序号，不等首条记录）；其余流为首条实时记录的事件时间，与该记录同时或先于它声明
  state Live {
    [*] --> LN
    LN --> Degraded : 集成上报：上游降级 / 上游限流 / 能力收紧（子态，接受条件不变；与核心配额挂起订阅互相独立）
    Degraded --> LN : 恢复
  }
  Starting --> Disconnected : 断线 / 集成进程退出（集成会话 append）
  Live --> Disconnected : 断线 / 集成进程退出（集成会话 append）
  Disconnected --> Starting : 重连（新 session_seq）
  note right of Starting
    握手时每条流决定 epoch：
    能以 venue 游标证明续接 → 续原 epoch，Seq 接续
    否则 → 新 epoch，首条 Gap{Source, disconnect / start / quota / ingress_overflow}
    rotate_credential → 强制新 epoch，reason credential_rotated
    载荷版本变化 → 新 epoch，reason schema_change
  end note
```

```mermaid
stateDiagram-v2
  state "None{epoch}（无回填任务：尚无 live_from / 续接原 epoch / 能力不支持 / 需求为空 / 回填深度为 0 / 深度为 Origin 而未声明 backfill_from_origin；health 不列）" as NONE
  state "Backfilling{through}" as BF
  state "Closed" as CL
  state "Reached" as RE
  state "Incomplete{through}" as INC
  [*] --> NONE : 流 epoch 开始（新 epoch：None{epoch} 与起点 Gap{Source} 同事务 append）
  NONE --> BF : 新 epoch 以 Gap{Source} 开始 ∧ backfill Supported ∧ 已有 live_from ∧ 需求非空（route 全集，含核心的 All）∧ 回填深度 ≠ 0 → 持久订阅建立任务；起点 = live_from − 回填深度，或 Origin（仅 backfill_from_origin 的流）；through = 起点
  BF --> BF : 读结论 covered_to 推进 through；Unavailable 不推进
  BF --> CL : joinable_venue_seq 的流：covered_to 连到 live_from（序号连续，边界闭合；frontier 越过）
  BF --> RE : 只有事件时间的流：covered_to 连到 live_from（已取得 [起点, live_from)，衔接未证明；frontier 越过，Gap{Source} 不视为闭合）
  BF --> INC : 历史穷尽或 Refused 而未达 live_from → Gap{Source, backfill_incomplete}，frontier 跳过未覆盖区间
  note right of BF
    核心判定、持久订阅 append 健康观察（与读结论 / backfill_incomplete 同事务）
    只请求 < live_from 的窗口；实时记录同时照常到达
    新 epoch 开新任务，旧任务的终态不沿用；深度、起点、主体集在建立时取定
    进度值带所属 epoch；health 只列当前 epoch 的非 None 值
  end note
```

读法：readiness 由集成推送（`Disconnected` 由集成会话），只说实时供给；回填进度由核心凭读结论记录判定、持久订阅 append，只说当前流 epoch 的 `[起点, live_from)` 补齐到哪。二者都是派生健康观察（不需确认），经 `health` 读模型对解释层可见，再由它翻成下游的连接状态；它们不改变记录接受条件——接受只看 `session_epoch`。

核出：readiness 原把 `Backfilling` 放在 `Live` 之前，而回填窗口的终点 `live_from` 要到 `Live` 才有，`through` 又是核心凭读结论记录才证明得了的——已拆为集成推送的 readiness 与核心判定的回填进度（§8.4）。

## D3.3 回填与实时边界

对照：§8.2 `backfill`、§8.4 回填、实时边界；Q14。

```mermaid
flowchart LR
  subgraph EPOCH["同一流 epoch"]
    direction LR
    G["Seq 1<br/>Gap{Source}"] --- BF1["窗口 1 的记录<br/>backfilled + 读结论（covered_to）"] --- BF2["窗口 2 的记录<br/>backfilled + 读结论（covered_to）"] --- LF["live_from<br/>实时供给起点"] --- L1["实时 …"]
  end
  CORE["核心按回填任务切窗口<br/>backfill(stream, window, subjects)；坐标：joinable_venue_seq 用 venue 序号，否则事件时间<br/>首窗口 from = 起点（live_from − 回填深度，或 Origin）；subjects = 任务建立时被路由的主体集，各窗口不变；上游分页与 pacing 在适配器内"] -->|"只请求 < live_from"| BF1
  BF1 -->|"下一窗口从 covered_to 续（崩溃重启同样）"| BF2
  BF2 -->|"covered_to 连到 live_from ∧ joinable_venue_seq"| CLOSE["Closed：边界闭合，frontier 越过<br/>回填与实时之间无重叠无洞（Q14）"]
  BF2 -->|"covered_to 连到 live_from ∧ 只有事件时间"| REACH["Reached：衔接未证明，frontier 越过<br/>Gap{Source} 仍列在 gaps，不满足成交完整性条件 2"]
  CLOSE -.->|"仅当起点 = Origin ∧ subjects = All"| COMPLETE["成交完整性条件 2 成立（§8.1）<br/>有限起点的 Closed 不闭合 epoch 开头的 Gap{Source}"]
  BF2 -->|"上游历史穷尽（covered_to 未达）或 Refused"| INC["append Gap{Source, backfill_incomplete}<br/>frontier 跳过未覆盖区间（不伪造连续）"]
  CORE -->|"Unavailable（任一页失败即整体失败）"| CH["Gap{Channel, backfill}<br/>可再发"]
```

读法：回填记录与实时记录同形、同 epoch，只多一个 `backfilled` 标记；回填窗口都在 `live_from` 之前。有可衔接序号的流上二者按序号不重叠也不留洞，不需要逐条去重。只有事件时间的流上，实时订阅确认前上游已发出的记录可能两边都不在，迟到的实时记录也可能与回填的同一对象并存，所以只称“到达”。续点是读结论记录里的覆盖边界，不需要任何一方保存游标。

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
  C->>SUB: subscribe(selector, mode, from?)（selector = 观察流：一组 (来源, 流, 主体集?, 供给 | 只投递) 项，可跨来源；或 执行事实 (来源, 作用域?)）
  alt 执行事实：mode ≠ ordered，或作用域键不在该来源任何声明版本里
    SUB-->>C: 拒绝
  else 观察流：逐项判定（各项独立）
    Note over SUB: 每项：来源未登记 / 流不在最近声明 / 配额池里不带主体集的供给项 → 该项拒绝；from < 保留边界 → 该项 BeyondRetention；供给项超配额 → 该项 QuotaExceeded（集成不收到超限主体）；来源从未有声明 → 该项待接纳；只投递项不进需求、不占配额
    alt 没有任何一项被接纳
      SUB-->>C: Rejected{items}
    else 至少一项被接纳
      SUB->>SUB: 写订阅表，cursor 每条选中流一个位置 = from（缺省 = 当前流末，不补历史）；订阅归属 principal，持久
      SUB-->>C: Subscription{id, items}（逐项结果；整体状态由逐项派生）
    end
  else 执行事实：接受
    SUB->>SUB: 写订阅表；selector 是成员规则：此后出现的 lane 流自动加入，从其首条记录起；from 可为起点
    SUB-->>C: Subscription{id, items}
  end
  opt 某流的需求（各供给项主体之并 ∪ 核心自己的需求；只投递项与挂起项不计）变了，且该集成会话已建立
    SUB->>I: route(stream, 主体全集 | All | 空集)
    alt Routed{refused}
      I-->>SUB: Routed{refused}
      SUB->>J: 同事务 append 路由结论记录（增减、refused 及原因）；refused 主体在所属各项内列为“来源拒绝”
    else Unavailable
      I-->>SUB: Unavailable（不 append；按 pacing 重发其时最新的全集；同一流至多一次 route 在途）
    end
  end
  loop 记录到达
    J->>DL: 新记录 pos（观察流订阅）
    EJ->>DL: 新记录 pos（执行事实订阅：存储按位置交出已提交的字节，投递不读读模型、不解析）
    DL->>C: 投递 pos（数据记录按信封 subject ∈ 项的主体集过滤，整条流的项不过滤；Gap / 读结论 / 路由结论投给该流每一项；同一订阅每条记录至多一次；每条流各自有序，流与流之间不定序；已投未确认 = 消费者内存里的事）
    C->>SUB: ack(subscription, cursor = pos)（确认 = 已处理）
    SUB->>SUB: cursor 推进（单写者，串行化）
  end
  alt 慢消费者：缓冲耗尽
    DL->>J: append Gap{Delivery, slow_consumer, from..to}
    DL->>C: 停投，投递 gap（需显式确认）
    C->>SUB: ack(subscription, cursor = 该 gap 的位置)（显式确认 gap）
    DL->>C: 从已确认 cursor 之后恢复投递
  end
  alt 断连 / 崩溃
    C--xDL: 连接断
    Note over SUB: 订阅 owner 是核心，订阅与 cursor 保留
    C->>SUB: 重连：同一 principal 重新 handshake → 自动挂接其持久订阅（不需再 subscribe）
    DL->>C: 从 cursor 之后重投；未确认区间可能重复，按 LogPosition 去重
  end
  opt 重新握手使某流进入配额池
    SUB->>SUB: 该流上整条流的供给项挂起（WholeStreamInPool），核心自己的 All 撤去；离开所有配额池时恢复
    SUB->>I: route(stream, 重算后的全集)
  end
  Note over SUB,I: 集成每次会话建立后，SUB 对每条需求非空的流重发 route；会话内第一次 route 之前集成不推送该流
```

读法：

- 确认语义唯一：`ack` = 已处理。已投未确认的记录在崩溃后会再见一次；已确认的永不重投（退回只经控制面 `rewind_cursor`）。
- 程序是同一种订阅者：它的 cursor 与 `Checkpoint` 同事务持久化（D4.1），所以程序永远不会看到已折入状态的记录。
- 需求按流的全集下发：多个订阅对同一主体只路由一次，配额在核心计量；一个主体的覆盖从加入它的路由结论记录之后开始。
- 投递按主体：数据记录只看信封上的 `subject`，不论来自推送、回填、一次性读还是回执；控制记录投给该流每一项。只投递项不改变需求与配额，一次性读得 `Pending{from, instance_id}` 后从 `from` 订阅一个只投递项，即收到那条结论或 gap（同一核心实例内）。
- 执行事实订阅走同一套 cursor 与 ack，但只能 `ordered`：执行事实不压缩、不合并，所以慢消费者只背压自己，不出现 `Gap{Delivery}`；投递经存储按位置读出已提交的记录、原样搬运，不经读模型、不解析，所以观察侧的订阅与投递元素不依赖效应侧类型（§7.3 uses 图）。它不经 `route`，不受声明与会话影响。

核出：无。

## D3.5 三种消费方式

对照：§4.2 消费方式表。

| 方式 | 触发依据 | 是否跳过 | 损失记法 |
|---|---|---|---|
| await-all | 所有输入流的**完备进度**到达要求位置 | 否（等待） | — |
| ordered | 单流顺序 | 否（背压） | — |
| latest / conflated | 最新值 | 是 | `Gap{Delivery, conflated}` 或消费者声明的窗口界 |

读法：

- `await-all` 看 frontier 不看 cursor——读到 Seq 60 不等于 60 之前的事件时间不再迟到。
- 损失语义由消费者显式声明：UI 报价显示可接受 conflation，依赖完整状态路径的阈值策略不能默认接受。

核出：无。

## D3.6 gap 来源判定

对照：§4.2 gap 三来源、§8.3 映射表、§6.7 两故障面。

```mermaid
flowchart TB
  Q{"缺口出在哪一段？"}
  Q -->|"来源流本身断代<br/>断线 / 配额 / 溢出 / 换凭据 / 载荷换版 / 回填穷尽 / 程序升级"| S["Gap{origin: Source, reason}<br/>新 epoch 首条或流内记录<br/>写者：集成推送入口 / 核心"]
  Q -->|"核心到消费者的投递<br/>慢消费者 / 订阅位置被压缩 / conflated"| D["Gap{origin: Delivery, reason}<br/>需消费者显式确认<br/>写者：投递调度"]
  Q -->|"核心发起的读渠道不可用<br/>取证 / 回填 / 一次性读 返回 Unavailable"| C["Gap{origin: Channel, channel}<br/>可再发<br/>写者：IO 壳 / 回填 / 一次性读"]
  Q -->|"submit 无业务回执"| U["不是 gap：Undetermined<br/>写边界 in-doubt（D6.1）"]
```

读法：集成一个进程崩溃可能同时产生 `Gap{Source}`（它的流）和 `Undetermined`（它在途的 `submit`）；两面各自收敛，互不替代。

核出：`Gap{Channel}` 的落点（取证 → 执行事实侧属该 Attempt；回填 / 一次性读 → 观察侧该流）原文未写——已并入 §4.2。
