# 01 进程拓扑、启动、会话 epoch、持久化归属

对照：§0.1、§7.1、§7.2、§7.4、§7.5、§8.3。索引见 `README.md`。

## D1.1 进程拓扑与信道

对照：§0.1（权威在上游，上游只在集成内被消费）、§7.1（进程、信任边界、凭据链、传输）、§8.5（principal）、§7.4（单写者）、§7.6（配置文件与运行期登记）。

```mermaid
flowchart TB
  subgraph USER["信任边界 = 一个 OS 用户 H7"]
    subgraph CORE["UTA 核心进程（单实例，H10）"]
      DB[("SQLite 单文件 WAL<br/>观察 J · 执行 J · RuleState · 订阅表<br/>能力证据 · Checkpoint · 进程表 · 快照 · instance_id")]
      LOCK["OS 文件锁 + fence"]
    end
    INT1["集成进程 A<br/>上游的唯一消费点；venue SDK 语言不限<br/>凭据终点"]
    INT2["集成进程 B"]
    HOST1["程序宿主 1<br/>值树解释器<br/>rlimit / job object"]
    HOST2["程序宿主 2"]
    FILES["OPENALICE_HOME 统一路径<br/>Alice 写：封存信封 · 密钥引用 · 集成登记 · 策略规则 · 装载清单 · 运行期参数<br/>核心写：运行期登记（instance_id · 格式版本 · 最近快照位置）"]
  end
  ALICE["Alice 进程<br/>消费方 + 控制方<br/>独立生命周期"]
  V1["venue A（外部权威，F1）"]
  V2["venue B"]
  ALICE <-->|"JSON-RPC over UDS / 命名管道<br/>对端凭据 → principal = (os_user, actor)"| CORE
  CORE <-->|"JSON-RPC 双向：核心→请求（IDL 9 操作）<br/>集成→推送（观察 · Gap · 能力变更 · readiness）"| INT1
  CORE <-->|"同一 IDL"| INT2
  CORE <-->|"宿主协议 Load / Advance / Reset / Unload"| HOST1
  CORE <-->|"宿主协议"| HOST2
  INT1 <-->|"上游协议（REST / WS / FIX）"| V1
  INT2 <-->|"上游协议"| V2
  FILES -.->|"核心读 Alice 写的文件；凭据解封后注入集成"| CORE
  CORE -.->|"写运行期登记（Alice 只读）"| FILES
  ALICE -.->|"写配置文件（原子替换）"| FILES
  DB -.->|"独占；集成与宿主不接触"| CORE
```

读法（假想运行时）：

- 两类子进程（集成、程序宿主）都由核心拉起、登记在进程表，各自可以独立崩溃：集成崩了只影响它的流与在途 `submit`（D7.1）；宿主崩了只影响该程序（D4.2）；核心崩了子进程成孤儿，下次启动按进程表回收（D1.2）。
- 所有 RPC 语义在一份 IDL；Windows 只换信道（回环 + 令牌或命名管道），不换 IDL。
- 凭据只走 `文件 → 核心 → 集成` 一条链；Alice、程序、宿主从不见凭据本体。

核出：无。

## D1.2 启动五步

对照：§7.2 启动与接管顺序第 1–5 步；§6.7 恢复；§9.2 #15/#21。

```mermaid
sequenceDiagram
  participant N as 新核心实例
  participant DB as SQLite
  participant O as 旧实例孤儿进程
  participant I as 集成进程
  participant H as 程序宿主
  participant A as Alice
  Note over N,DB: 第 1 步 取 fence
  N->>DB: OS 文件锁 + SQLite 独占
  alt 取不到
    N-->>N: 专用退出码退出（W9）
  end
  N->>DB: 同事务 instance_id += 1（旧会话 epoch 全部作废）
  N->>DB: 读进程表（旧 instance_id 名下）
  loop 每个 (pid, start_time) 仍匹配的登记
    N->>O: 请求退出，超时后强制终止
    N->>DB: 清除登记
  end
  Note over N,DB: 第 2 步 从记录重建（不接触任何集成）
  N->>DB: 校验格式版本（C14，失败即拒绝启动）
  N->>DB: 快照 + 记录 fold_state：lane 链、单据、RuleState、订阅表
  N->>DB: 每条链先 fold 是否已 Resolved；未完链的当前腿 SendBarrier 无后继 → append Undetermined(CrashWindow)
  Note over N,I: 第 3 步 握手
  N->>I: 按集成登记拉起进程，写进程表 (instance_id, pid, start_time, role)
  N->>I: handshake(session_epoch = (instance_id, session_seq))
  I-->>N: Projection（scopes / streams / capabilities）
  N->>DB: append 一版能力证据；required_inputs 比对；各流决定续 epoch 或新 epoch + Gap{Source}
  Note over N,I: 第 4 步 恢复效应侧
  N->>DB: 停等的 Undetermined → append ReconciliationReopened{SessionRestored}
  N->>I: 每条未终结的 Undetermined 启动对账驱动（读，D6.2）；AwaitingTargetTerminal 的链继续读目标
  N->>N: 当前腿无 SendBarrier 者过发出前门（D7.1）
  N->>I: 未过期者 SendBarrier → submit
  N->>N: fold 出无 EffectResponse 的 EffectRequest 重派（D4.3）
  N->>N: STS 链按 RuleState 续跑待决单据；按 deadline 重装过期计时器
  Note over N,H: 第 5 步 恢复观察侧与消费面
  N->>I: 按订阅表重建路由，按需 backfill
  N->>H: 拉起宿主并登记；比对 checkpoint 的 state_version → Load(program, checkpoint?, budget)
  H-->>N: Loaded 或 LoadRejected（版本不被接受不是 LoadRejected：不携带 checkpoint 装载并 Reset，D4.2）
  N->>A: 开放会话；此前 health() 返回 Starting
```

读法：

- 第 2 步的结论只来自记录：`Undetermined(CrashWindow)` 在没有任何集成在线时就已 append；随后第 4 步才去问 venue。
- 第 4 步先取证后发送：已能 `Resolved` 的 lane 先解除，再放未发出的 `Prepared`；发送依赖第 3 步的会话 epoch。
- 任一步失败整体拒绝启动，不进入部分运行态；消费方在第 5 步之前只看到 `Starting`。

核出：第 4 步原文只写了取证与发送，没写 `EffectRequest` 重派与待决单据的计时器重装——已并入 §7.2 第 4 步。

## D1.3 会话 epoch 与边界接受

对照：§7.2 第 3 步、§8.3（推送错误）、§8.4 readiness、§7.6 `rotate_credential`。

```mermaid
stateDiagram-v2
  state "握手中：session_seq += 1，以 (instance_id, session_seq) 发起 handshake" as HS
  state "会话已建立（当前 epoch 生效）" as EST
  state "已拒绝" as REJ
  [*] --> HS : 核心拉起集成 / 重连 / rotate_credential / restart_integration
  HS --> EST : handshake 返回合法 Projection，能力证据 append
  HS --> REJ : 投影不合法 / 契约版本不兼容（记 P14，不降级）
  HS --> HS : 传输失败 → 重连（再次 += 1）
  EST --> HS : 传输断开 / 集成进程退出
  EST --> EST : 推送 epoch == 当前 → 接受并分配 LogPosition
  EST --> EST : 推送 epoch != 当前 → 边界拒绝，不 append
  REJ --> [*]
  note right of EST
    SessionEpoch = (instance_id, session_seq)
    集成把它回填到每条推送与 submit 回执
    旧 epoch 的迟到回执不进核心；
    venue 已受理的写由新会话观察或对账并入原腿（D6.6 步 4）
  end note
```

读法：

- 会话 epoch 与流 epoch 独立：重连后集成若能以 venue 游标证明续接，流 epoch 不变、`Seq` 接续；证明不了才开新流 epoch（D3.2）。
- `rotate_credential` 强制新流 epoch（`credential_rotated`），不允许续接。

核出：无。

## D1.4 持久化归属：谁写哪张表

对照：§7.5 持久化归属表；§7.6 运行期登记文件；§7.3“执行事实 append 链的唯一写入口”。

```mermaid
flowchart LR
  subgraph W["写者（都在核心进程内）"]
    PUSH["集成推送入口<br/>（位置由核心分配）"]
    DAG["派生 DAG 解释①"]
    RD["一次性读：读处理器 · 钩子取证 · 消费方 read"]
    IOR["IO 壳：回执 / 取证观察"]
    TK["单据（TicketAction）"]
    STS["STS 规则链"]
    IOE["IO 壳：SendBarrier / VenueAccepted / VenueRejected / Undetermined / Expired /<br/>ResolutionEvidence / ReconciliationReopened{CancelLegTerminal|SessionRestored} / CapabilityObserved / 取证 Gap{Channel}"]
    ATTR["效应侧归因处理器：ResolutionEvidence{Attributed}"]
    CTL["控制面：控制记录 · 安全事件 · ResolutionEvidence{Manual} · ReconciliationReopened{Manual}"]
    OUT["出站请求处理器：EffectRequest · EffectResponse"]
    SUBEL["持久订阅元素"]
    HS["握手"]
    HOSTP["宿主协议"]
    FENCE["fence / 进程登记"]
    RET["保留协议"]
  end
  subgraph T["SQLite 表"]
    OJ[("观察 Journal<br/>RetractableDelta，可压缩")]
    EJ[("执行事实 Journal<br/>纯 append")]
    RS[("RuleState")]
    SUB[("订阅表 / cursor")]
    CAP[("能力证据")]
    CK[("Checkpoint + 程序 cursor")]
    PT[("进程表 (instance_id, pid, start_time, role)")]
    RB[("保留边界 + 引用登记")]
    SN[("快照")]
    IID[("instance_id")]
  end
  subgraph F["统一路径文件（核心写、Alice 只读）"]
    RUN["运行期登记：instance_id · 格式版本 · 最近快照位置"]
  end
  PUSH --> OJ
  DAG --> OJ
  RD --> OJ
  IOR --> OJ
  TK --> EJ
  STS --> EJ
  STS --> RS
  IOE --> EJ
  ATTR --> EJ
  CTL --> EJ
  OUT --> EJ
  SUBEL --> SUB
  HS --> CAP
  HS -->|"握手版能力证据"| EJ
  IOE --> CAP
  HOSTP --> CK
  FENCE --> PT
  FENCE -->|"instance_id += 1（与 fence 同事务）"| IID
  FENCE --> RUN
  RET --> RB
  TK -.->|"Prepared 同事务自动登记 basis"| RB
  IOE -.->|"ResolutionEvidence 自动登记引用"| RB
  ATTR -.->|"ResolutionEvidence 自动登记引用"| RB
  CTL -.->|"ResolutionEvidence 自动登记引用"| RB
  HOSTP -.->|"Checkpoint 自动登记 cursor"| RB
```

读法：

- 观察 J 有四类写者，执行 J 有七类；两侧共享存储原语但类型宇宙不共享（§4.1）。
- 读模型不写任何表：它是执行 J（+ 归因观察）的只读 fold，随请求或订阅计算。
- 引用登记不是独立写者动作：随 `Prepared`/`Checkpoint`/`ResolutionEvidence` 的 append 自动写入，随 `Resolved`/下一 checkpoint 自动解除（D8.1）。

核出：无。

## D1.5 同事务集合

对照：§7.4 事务原子性；§6.1 请求完成事实；§6.2 与写边界的接口；§6.5 记录模型；§8.6 `Advance`；§7.2 第 1 步。

| 同一 SQLite 事务内必须一起提交 | 依据 | 崩在中途的后果（§9.2） |
|---|---|---|
| `Close(Prepared(position))` + `Prepared` | §6.2、§7.4 | #1：二者皆无，单据仍 `AwaitingDecision` |
| Decision / `Outcome` / `Rejection` 记录 + `RuleState` 更新 | §7.4 | 链步未发生，重启按 `RuleState` 重跑该步 |
| 程序写处理器的 `Draft` + `SubmitForDecision` + `EffectResponse{Drafted}` | §6.1 | #21：无 `EffectResponse` → 重派开单 |
| 读处理器的观察记录 / `Gap{Channel}` + `EffectResponse{Observed / Unavailable}` | §6.1 | #21：无 `EffectResponse` → 重新执行一次 |
| `submit` 的 `Ack`：`VenueAccepted`（含 `Evidence`）+ 回执观察副本 | §6.5 记录模型 | #5：视为无后继 → `Undetermined` → by-key 取证重得同一状态 |
| 一次取证命中：`ResolutionEvidence{Found}`（含 `Evidence`）+ `provenance: Reconciliation` 观察副本 | §6.5 | #7：该次取证不存在；fold 显示本轮该渠道未取证，重做（读可重试） |
| 推送归因命中：观察记录 + `ResolutionEvidence{Attributed}` | §6.6、§8.1 | 推送未 append：核心崩溃即集成成孤儿被回收，重启握手后该流续接（集成以 venue 游标证明）或新 epoch + `Gap{Source}`；续接则记录重到，仍处 `Undetermined` 的腿照常归因 |
| `Undetermined` append + 回查已到达的归因观察 | §6.6 | 二者同事务，不存在"归因已到但未匹配"的持久态 |
| `Advance` 输出：`EffectRequest` 记录 + 派生记录 + `Checkpoint` + 程序 cursor | §8.6 | #16：整批不存在，重放同一批记录 |
| fence 取得 + `instance_id += 1` | §7.2 第 1 步 | 未提交则旧 `instance_id` 仍有效，重来 |
| 单条观察 append + `LogPosition` 分配 | §7.4 | #9：半写不可见 |

读法：`SendBarrier` 不在任何集合里——它单独 durable append（fsync）后才允许 `submit`，这正是把崩溃窗口二分的屏障（D6.1）。

核出：无。
