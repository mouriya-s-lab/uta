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

- 核心是时间权威：`LogPosition` 按到达顺序分配，venue seq 只是证据；乱序与重复不被核心修正，由派生侧 fold 按种类处理：成交按 execution_id 与修订计数（§8.1），其余种类按 venue seq 等证据。
- 同一条推送可能同时是三件事：订阅者的一条记录、某单据偏离状态的触发、某 `Undetermined` Attempt 的决议证据。
- 迟到回执走的就是这条路：旧 epoch 的被丢在第二个分支；新会话重送的在 `opt` 分支并入原 Attempt。

核出：无。

## D3.2 集成 × 流的 readiness、回填进度与流 epoch 决定

对照：§8.4 readiness 状态机与回填进度、§8.2 `handshake`、§4.2 `Gap{Source}` 原因集、§7.6 `rotate_credential`。

```mermaid
stateDiagram-v2
  state "Live{live_from}（非 Degraded）" as LN
  [*] --> Starting : 会话建立（新 session_seq）；集成推送
  Starting --> Live : 集成声明 live_from：上游给出可衔接 venue 序号时于订阅确认即声明（下一个期望序号，不等首条记录）；否则于首条实时记录到达（有 venue 序号用序号，否则用事件时间）
  state Live {
    [*] --> LN
    LN --> Degraded : 集成上报：上游降级 / 上游限流 / 能力收紧（子态，接受条件不变；与核心配额挂起订阅互相独立）
    Degraded --> LN : 恢复
  }
  Starting --> Disconnected : 断线 / 集成进程退出（核心 append）
  Live --> Disconnected : 断线 / 集成进程退出（核心 append）
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
  state "无回填任务（尚无 live_from / 续接原 epoch / 能力不支持 / 无要历史的订阅）" as NONE
  state "Backfilling{through}" as BF
  state "Closed" as CL
  state "Incomplete{through}" as INC
  [*] --> NONE : 流 epoch 开始
  NONE --> BF : 新 epoch 以 Gap{Source} 开始 ∧ backfill Supported ∧ 有要历史的订阅 ∧ 已有 live_from → 核心建立任务，through = 起点
  BF --> BF : 读结论 covered_to 推进 through；Unavailable 不推进
  BF --> CL : covered_to 连到 live_from（边界闭合，frontier 才允许越过）
  BF --> INC : 历史穷尽或 Refused 而未达 live_from → Gap{Source, backfill_incomplete}，frontier 跳过未覆盖区间
  note right of BF
    核心判定、核心 append 健康观察（与读结论 / backfill_incomplete 同事务）
    只请求 < live_from 的窗口；实时记录同时照常到达
    新 epoch 开新任务，旧任务的 Closed 不沿用
  end note
```

读法：readiness 由集成推送（`Disconnected` 由核心），只说实时供给；回填进度由核心凭读结论记录判定，只说这一个流 epoch 的 `[起点, live_from)` 补齐到哪。二者都是派生健康观察（不需确认），经 `health` 读模型对解释层可见，再由它翻成下游的连接状态；它们不改变记录接受条件——接受只看 `session_epoch`。

核出：readiness 原把 `Backfilling` 放在 `Live` 之前，而回填窗口的终点 `live_from` 要到 `Live` 才有，`through` 又是核心凭读结论记录才证明得了的——已拆为集成推送的 readiness 与核心判定的回填进度（§8.4）。

## D3.3 回填与实时边界

对照：§8.2 `backfill`、§8.4 回填、实时边界；Q14。

```mermaid
flowchart LR
  subgraph EPOCH["同一流 epoch"]
    direction LR
    G["Seq 1<br/>Gap{Source}"] --- BF1["窗口 1 的记录<br/>backfilled + 读结论（covered_to）"] --- BF2["窗口 2 的记录<br/>backfilled + 读结论（covered_to）"] --- LF["live_from<br/>实时供给起点"] --- L1["实时 …"]
  end
  CORE["核心按订阅需求切窗口<br/>backfill(stream, window)；上游分页与 pacing 在适配器内"] -->|"只请求 < live_from"| BF1
  BF1 -->|"下一窗口从 covered_to 续（崩溃重启同样）"| BF2
  BF2 -->|"covered_to 连到 live_from"| CLOSE["边界闭合：frontier 允许越过"]
  BF2 -->|"上游历史穷尽（covered_to 未达）或 Refused"| INC["append Gap{Source, backfill_incomplete}<br/>frontier 跳过未覆盖区间（不伪造连续）"]
  CORE -->|"Unavailable（任一页失败即整体失败）"| CH["Gap{Channel, backfill}<br/>可再发"]
```

读法：回填记录与实时记录同形、同 epoch，只多一个 `backfilled` 标记；按范围不重叠，所以不需要逐条去重。续点是读结论记录里的覆盖边界，不需要任何一方保存游标。

核出：无。

## D3.4 订阅、cursor、ack、慢消费者与重连

对照：§4.2 cursor 与确认、§8.5 订阅组（两种 selector、配额、订阅状态）、§7.5 订阅表、W6 步 3–4、W14、W20。

```mermaid
sequenceDiagram
  participant C as 消费方（解释层代下游，或程序）
  participant SUB as 持久订阅元素
  participant DL as 投递调度
  participant J as 观察 Journal
  participant EJ as 执行 Journal（经存储按位置读，不解析）
  C->>SUB: subscribe(selector, mode, from?)（selector = 观察流 (来源, 流, 主体集?) 或 执行事实 (来源, 作用域?)）
  alt 来源未登记 / 来源已有声明而 selector 引用其最近声明里没有的流
    SUB-->>C: 拒绝
  else 执行事实订阅而 mode ≠ ordered
    SUB-->>C: 拒绝
  else 观察流订阅 from < 保留边界
    SUB-->>C: BeyondRetention
  else 配额池里的流上不带主体集 / 超过配额池上限
    SUB-->>C: 拒绝 / QuotaExceeded{quota, limit}（集成不收到该订阅）
  else 接受
    SUB->>SUB: 写订阅表，cursor = from（缺省 = 各选中流当前流末，不补历史；执行事实可从起点）；订阅归属 principal，持久
    Note over SUB: 状态：来源有声明 → 活（与有无会话无关）；来源从未有声明 → 待接纳，首次握手成功后转活或被拒
    SUB-->>C: Subscription
  end
  loop 记录到达
    J->>DL: 新记录 pos（观察流订阅）
    EJ->>DL: 新记录 pos（执行事实订阅：存储按位置交出已提交的字节，投递不读读模型、不解析）
    DL->>C: 投递 pos（已投未确认 = 消费者内存里的事）
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
```

读法：

- 确认语义唯一：`ack` = 已处理。已投未确认的记录在崩溃后会再见一次；已确认的永不重投（退回只经控制面 `rewind_cursor`）。
- 程序是同一种订阅者：它的 cursor 与 `Checkpoint` 同事务持久化（D4.1），所以程序永远不会看到已折入状态的记录。
- 执行事实订阅走同一套 cursor 与 ack，但只能 `ordered`：执行事实不压缩、不合并，所以慢消费者只背压自己，不出现 `Gap{Delivery}`；投递经存储按位置读出已提交的记录、原样搬运，不经读模型、不解析，所以观察侧的订阅与投递元素不依赖效应侧类型（§7.3 uses 图）。

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
