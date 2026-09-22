# 03 观察入库、readiness、回填、订阅与 gap

对照：§3.1、§3.2、§6.3.6、§6.3.8、§6.3.10、§6.4 订阅组、W6。索引见 `README.md`。

## D3.1 一次推送的入库时序

对照：§6.3.6 推送表、§6.3.8 时间权威与归因、§2.2 处理器、§3.3、§3.2 投递、§5.2 偏离是状态。

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
  V-->>I: 上游事件（成交 / 报价 / 余额）
  I->>B: 推送观察记录{session_epoch, StreamId, received_at, venue seq, attribution?, 载荷 + payload_schema}
  alt 缺锚点
    B-->>I: 畸形记录，拒绝
  else session_epoch != 当前
    B-->>I: 边界丢弃，不 append
  else 接受
    B->>J: 分配 LogPosition（按到达顺序）；venue seq 倒退/重复 → 照常 append 并打 replayed / out_of_order
    J->>H: 字段出现 → 触发
    H->>J: occurred_at → 推进完备进度（无 occurred_at 按 received_at − 滞后界）
    opt attribution: FromAttempt(r) 或 idempotency_key 经登记解析到腿 r，且 r 处于 Undetermined 未终结
      H->>EJ: 同事务 append ResolutionEvidence{r, Attributed, Found{observation: 该记录}}
    end
    J->>D: 受影响节点增量重算（cutoff）；派生记录写回观察 J
    J->>T: 引用该流的单据重算 basis_validity / alignment
    J->>S: 按订阅表投递给已确认 cursor 在该记录之前的订阅者（记录位于 cursor 之后）
  end
```

读法：

- 核心是时间权威：`LogPosition` 按到达顺序分配，venue seq 只是证据；乱序与重复不被核心修正，由派生侧 fold 按 venue seq 处理。
- 同一条推送可能同时是三件事：订阅者的一条记录、某单据偏离状态的触发、某 `Undetermined` Attempt 的决议证据。
- 迟到回执走的就是这条路：旧 epoch 的被丢在第二个分支；新会话重送的在 `opt` 分支并入原 Attempt。

核出：无。

## D3.2 集成 × 流的 readiness 与流 epoch 决定

对照：§6.3.10 readiness 状态机、§6.3.5 `handshake` 行、§3.2 `Gap{Source}` 原因集、§6.7.3 `rotate_credential`。

```mermaid
stateDiagram-v2
  state "Live（非 Degraded）" as LN
  [*] --> Starting : 会话建立（新 session_seq）
  Starting --> Backfilling : 集成声明 Backfilling{through: Seq}
  Starting --> Live : 无需回填 / 能力不支持回填
  Backfilling --> Live : 进入 Live 时集成声明 live_from（首条实时记录的 venue seq）
  state Live {
    [*] --> LN
    LN --> Degraded : 能力收紧 / 配额受限（子态，接受条件不变）
    Degraded --> LN : 恢复
  }
  note left of Live
    进入 Live 后核心只请求 < live_from 的回填范围；
    回填页覆盖到 live_from 之前 → 边界闭合，frontier 才允许越过；
    回填穷尽未达 live_from → Gap{Source, backfill_incomplete}，frontier 跳过
  end note
  Starting --> Disconnected : 断线 / 集成进程退出
  Backfilling --> Disconnected : 断线 / 集成进程退出
  Live --> Disconnected : 断线 / 集成进程退出
  Disconnected --> Starting : 重连（新 session_seq）
  note right of Starting
    握手时每条流决定 epoch：
    能以 venue 游标证明续接 → 续原 epoch，Seq 接续
    否则 → 新 epoch，首条 Gap{Source, disconnect / start / quota / ingress_overflow}
    rotate_credential → 强制新 epoch，reason credential_rotated
    载荷版本变化 → 新 epoch，reason schema_change
  end note
```

读法：readiness 变化是派生健康观察（不需确认），经 `health` 读模型对 Alice 可见；它不改变记录接受条件——接受只看 `session_epoch`。

核出：无。

## D3.3 回填与实时边界

对照：§6.3.10 回填、实时边界；Q14。

```mermaid
flowchart LR
  subgraph EPOCH["同一流 epoch"]
    direction LR
    G["Seq 1<br/>Gap{Source}"] --- BF1["回填页 1<br/>backfilled"] --- BF2["回填页 2<br/>backfilled"] --- LF["live_from<br/>首条实时记录"] --- L1["实时 …"]
  end
  CORE["核心按订阅需求与 pacing<br/>backfill(stream, from, page)"] -->|"只请求 < live_from"| BF1
  BF2 -->|"next_cursor 续页"| BF2
  BF2 -->|"覆盖到 live_from 之前"| CLOSE["边界闭合：frontier 允许越过"]
  BF2 -->|"穷尽仍未达"| INC["append Gap{Source, backfill_incomplete}<br/>frontier 跳过该区间（不伪造连续）"]
  CORE -->|"Unavailable"| CH["Gap{Channel, backfill}<br/>可再发"]
```

读法：回填记录与实时记录同形、同 epoch，只多一个 `backfilled` 标记；按范围不重叠，所以不需要逐条去重。

核出：无。

## D3.4 订阅、cursor、ack、慢消费者与重连

对照：§3.2 cursor 与确认、§6.4 订阅组、§6.7.2 订阅表、W6 步 3–4、W14。

```mermaid
sequenceDiagram
  participant C as 消费方（Alice 或程序）
  participant SUB as 持久订阅元素
  participant DL as 投递调度
  participant J as 观察 Journal
  C->>SUB: subscribe(selector, mode, from?)
  alt selector 引用未声明流
    SUB-->>C: 拒绝
  else from < 保留边界
    SUB-->>C: BeyondRetention
  else 超过投影配额
    SUB-->>C: QuotaExceeded{scope, limit}（集成不收到该订阅）
  else 接受
    SUB->>SUB: 写订阅表，cursor = from（缺省 = 各选中流当前流末，不补历史）；订阅归属 principal，持久
    SUB-->>C: Subscription
  end
  loop 记录到达
    J->>DL: 新记录 pos
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

核出：无。

## D3.5 三种消费方式

对照：§3.2 消费方式表。

| 方式 | 触发依据 | 是否跳过 | 损失记法 | 典型消费者 |
|---|---|---|---|---|
| await-all | 所有输入流的**完备进度**到达要求位置 | 否（等待） | — | 依赖完整状态路径的阈值策略、程序节点的 `Join` |
| ordered | 单流顺序 | 否（背压） | — | 程序 cursor、读模型 fold |
| latest / conflated | 最新值 | 是 | `Gap{Delivery, conflated}` 或消费者声明的窗口界 | UI 报价显示 |

读法：`await-all` 看 frontier 不看 cursor——读到 Seq 60 不等于 60 之前的事件时间不再迟到。

## D3.6 gap 来源判定

对照：§3.2 gap 三来源、§6.3.7 映射表、§5.4 两故障面。

```mermaid
flowchart TB
  Q{"缺口出在哪一段？"}
  Q -->|"来源流本身断代<br/>断线 / 配额 / 溢出 / 换凭据 / 载荷换版 / 回填穷尽 / 程序升级"| S["Gap{origin: Source, reason}<br/>新 epoch 首条或流内记录<br/>写者：集成推送入口 / 核心"]
  Q -->|"核心到消费者的投递<br/>慢消费者 / 订阅位置被压缩 / conflated"| D["Gap{origin: Delivery, reason}<br/>需消费者显式确认<br/>写者：投递调度"]
  Q -->|"核心发起的读渠道不可用<br/>取证 / 回填 / 一次性读 返回 Unavailable"| C["Gap{origin: Channel, channel}<br/>可再发<br/>写者：IO 壳 / 回填 / 一次性读"]
  Q -->|"submit 无业务回执"| U["不是 gap：Undetermined<br/>写边界 in-doubt（D6.1）"]
```

读法：集成一个进程崩溃可能同时产生 `Gap{Source}`（它的流）和 `Undetermined`（它在途的 `submit`）；两面各自收敛，互不替代。

核出：`Gap{Channel}` 的落点（取证 → 执行事实侧属该 Attempt；回填 / 一次性读 → 观察侧该流）原文未写——已并入 §3.2。
