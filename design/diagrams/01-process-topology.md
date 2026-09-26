# 01 进程拓扑、启动、会话 epoch、持久化归属

对照：§0.1、§7.1、§7.2、§7.4、§7.5、§8.3。索引见 `README.md`。

## D1.1 进程拓扑与信道

对照：§0.1（权威在上游，上游只在集成内被消费；三段：清洗、抽象、清洗）、§7.1（进程、信任边界、凭据链、传输）、§7.3（核心→集成调用全部经集成会话的调用通道）、§8.5（principal、核心↔解释层）、§7.4（单写者）、§7.6（配置文件）。

```mermaid
flowchart TB
  subgraph USER["信任边界 = 一个 OS 用户 H7"]
    subgraph CORE["UTA 核心进程（单实例，H10）"]
      DB[("SQLite 单文件 WAL<br/>观察 J · 执行 J · 订阅表<br/>能力证据 · Checkpoint · 进程表 · 实例表 · 快照")]
      LOCK["OS 文件锁 + fence"]
    end
    INT1["集成进程 A<br/>上游的唯一消费点；venue SDK 语言不限<br/>凭据终点"]
    INT2["集成进程 B"]
    HOST1["程序宿主 1<br/>值树解释器<br/>rlimit / job object"]
    HOST2["程序宿主 2"]
    FILES["OPENALICE_HOME 统一路径（写者都是 Alice，核心只读）<br/>封存信封 · 密钥引用 · 集成登记 · 策略规则 · 装载清单 · 运行期参数 · 原生计算制品目录"]
  end
  ALICE["下游：Alice · CLI 使用者 · 外部客户程序<br/>独立生命周期"]
  IL["解释层（落点由实现定：CLI 自身或核心内部）<br/>对外概念 · 状态翻译 · 不持有状态"]
  V1["venue A（外部权威，F1）"]
  V2["venue B"]
  ALICE <-->|"一次性命令 / 双向长连接（对外面，跨仓库契约）<br/>对端凭据 → 非本用户拒绝；下游自报 actor"| IL
  IL <-->|"核心↔解释层 IDL（本仓库内部）<br/>每个下游连接一个会话；principal = (os_user, actor)"| CORE
  CORE <-->|"核心拉起时创建、经句柄继承交给该进程的通道（一个进程一个会话）<br/>核心→请求（IDL 10 操作，全部经集成会话的调用通道）<br/>集成→推送（观察 · Gap · 能力变更 · readiness）"| INT1
  CORE <-->|"同一 IDL"| INT2
  CORE <-->|"宿主协议 Load / Advance / Reset / Unload"| HOST1
  CORE <-->|"宿主协议"| HOST2
  INT1 <-->|"上游协议（REST / WS / FIX）"| V1
  INT2 <-->|"上游协议"| V2
  FILES -.->|"核心在启动、控制动作生效、拉起集成与装载程序时读（程序值与原生计算制品都按 Applied 所记的 hash 核对）；凭据解封后经继承句柄交给集成"| CORE
  ALICE -.->|"Alice 写配置文件（原子替换）"| FILES
  DB -.->|"独占；集成与宿主不接触"| CORE
```

读法（假想运行时）：

- 两类子进程（集成、程序宿主）都由核心拉起、登记在进程表，各自可以独立崩溃：集成崩了只影响它的流与在途写调用（D7.1）；宿主崩了只影响该程序（D4.2）。核心受控停止时先结束它们、OS 确认退出之后才写实例结束锚点（D1.6）；核心崩了子进程成孤儿，下次启动按进程表回收（D1.2）。
- 核心↔集成的通道由核心在拉起时创建、只交给该子进程；会话就是这条通道的一次化身，通道关闭之后读不到它的任何消息（§7.1、§7.2）。核心↔解释层的 Windows 信道退化为回环 + 令牌或命名管道，不换 IDL。下游只见解释层的对外面，不见这份 IDL。
- 凭据只走 `文件 → 核心 → 集成` 一条链，拉起时交付，副本随集成进程结束；下游、解释层、程序、宿主从不见凭据本体。
- 解释层不持有状态：它或下游崩溃，对核心只是会话断开，订阅与确认进度仍在核心。

核出：无。

## D1.2 启动五步

对照：§7.2 生命周期表与启动第 1–5 步；§6.7 恢复；§9.2 #15/#21。

```mermaid
sequenceDiagram
  participant N as 新核心实例
  participant DB as SQLite
  participant O as 旧实例孤儿进程
  participant I as 集成进程
  participant H as 程序宿主
  participant A as 解释层（代下游）
  Note over N,DB: 第 1 步 取 fence
  N->>DB: OS 文件锁 + SQLite 独占
  alt 取不到
    N-->>N: 专用退出码退出（W9）
  end
  N->>DB: 同事务在实例表写新一行，instance_id += 1（上一实例若无结束锚点，此即其失权点）
  N->>DB: 读进程表（旧 instance_id 名下；上一实例受控停止则为空）
  loop 每个 (pid, start_time) 仍匹配的行
    N->>O: 请求退出，超时后强制终止
    O-->>N: OS 确认退出
    N->>DB: 清除该行
  end
  Note over N,DB: 第 2 步 从记录重建（不接触任何集成）
  N->>DB: 校验格式版本（C14，失败即拒绝启动）
  N->>DB: 快照 + 记录 fold_state：lane 上各尝试、单据、订阅表、程序活动集合
  N->>DB: 每个尝试先 fold 等待是否已结束；SendBarrier 无后继 → append Undetermined(CrashWindow)
  Note over N,I: 第 3 步 采纳集成登记，建立会话（读统一路径配置失败即拒绝启动；各集成独立推进）
  N->>DB: 从控制流 fold 各集成的 Halted：最近的 IntegrationHalted 未被某条 Applied 以位置引用解除者保持 Halted，不拉起
  N->>DB: 同一事务：append 登记的采纳记录（控制流；文件内容 hash、集成 id；发起方为本实例 instance_id）+ 每个采纳的集成的初始会话健康观察（保持 Halted 者为 Halted{cause, since}，其余为 Connecting，即本实例运行的开始锚点）
  N->>I: 读封存文件，拉起进程：创建通道并经句柄继承交出，凭据经继承句柄交付；写进程表 (instance_id, pid, start_time, role)；分配 session_seq
  N->>I: 在这条通道上 handshake()
  alt 合法 Projection
    I-->>N: Projection（scopes / streams / capabilities）
    N->>DB: 集成会话编排的同一事务：声明版本 + Established{epoch} 健康观察；开新 epoch 的流由集成会话 append Gap{Source}，并请持久订阅同事务写 None{epoch}；required_inputs 比对；集成会话请持久订阅把待接纳的订阅项（含等待该来源首个声明版本的程序订阅的项）按新声明转为接纳或被拒，那些程序随之按“等待的先后”判定（D4.2）；既有订阅按新声明重算路由
  else 投影不合法 / 契约版本不兼容 / Refused(reason)
    N->>DB: Halted{cause}：同一事务 append IntegrationHalted（P14，控制流；开始 Halted 抑制）与健康观察
    N->>I: 提交之后：结束会话、关闭通道、请求进程退出（超时强制终止）
    I-->>N: OS 确认退出 → 清除进程表的行：本次集成运行到此结束；不自动重试（等 restart_integration；Refused 另可 rotate_credential）
  else Unavailable
    N->>I: 保持 Connecting，按 pacing 在同一通道上重握手；本步不等它
  else 通道断开 / 进程退出
    N->>I: 结束该会话；OS 确认退出之后按 pacing 拉起新进程与新会话（D1.3）
  end
  Note over N,I: 第 4 步 恢复效应侧（只为已建立会话的集成发送与取证；之后建立会话者届时补做）
  N->>DB: 其集成已建立新会话、停等且结果未知、未被放弃的 Undetermined → append ReconciliationReopened{SessionRestored}
  N->>I: 每条结果未知、未被放弃的 Undetermined 启动对账驱动（读，D6.2）
  N->>N: 无 SendBarrier 者过发出前门（D7.1）：deadline 已过 → Expired；会话未建立或会话有效声明下不可执行 → 等待
  N->>I: 过门者 SendBarrier → 该尝试的写调用（submit / cancel）
  N->>N: fold 出无 EffectResponse 的 EffectRequest 重派（D4.3）
  N->>N: STS 链按记录重新评估待决单据（RuleState 由记录 fold 出）；按 deadline 重装过期计时器
  Note over N,H: 第 5 步 恢复观察侧与消费面
  N->>I: 为已建立会话的集成，持久订阅按订阅表与核心自己的需求（非配额池作用域的订单状态流与成交流恒为 All）合成全集经 route 下发，按需 backfill（不等回填完成）
  N->>H: 活动集合（控制流的 fold）中未被失败抑制的程序，按 §8.6 装载期校验的“等待的先后”：有观察项初次接纳已被最终拒绝的 → 同事务 ProgramHalted{LoadRejected} + ProgramFailed，不读程序值文件；否则所引用的集成来源（含只被 facts 引用的）在采纳集合里而还没有声明版本的留在活动集合里等待，不拉起、不 ProgramHalted；其余先从清单所指的文件读程序值、核对 Applied 所钉的内容 hash，不符 → ProgramHalted{ContentUnavailable} + ProgramFailed；相符的对这一份值做声明校验（最近声明取自集成会话，接纳结果取自持久订阅，原生 op 的名与签名取自控制流上的已安装 op 集合），不成立 → ProgramHalted{LoadRejected} + ProgramFailed；成立的再按各 op 的安装 Applied 所记引用读原生计算制品、核对内容 hash，不符 → ProgramHalted{NativeArtifactUnavailable} + ProgramFailed；都相符的把核对过的制品内容交给子系统（只为这次宿主执行），未被接受 → ProgramHalted{LoadRejected(NativeHandoverFailed)} + ProgramFailed，不拉起；被接受的拉起宿主并登记（OS 没有给出进程 → ProgramHalted{LoadRejected(HostSpawnFailed)} + ProgramFailed，不是 Trap），交回本成员可交回的最近 checkpoint（Applied 沿用的或之后持久化的）→ Load(program = 同一份值, checkpoint?, budget)
  H-->>N: Loaded；交回的 checkpoint 的 state_version 总在本成员接受的集合内（替换 Applied 比对或 Output 检查，§8.6 状态迁移），Load 不比对版本
  N->>A: 开放下游会话；此前 health() 返回 Starting
```

读法：

- 第 2 步的结论只来自记录：`Undetermined(CrashWindow)` 在没有任何集成在线时就已 append；随后第 4 步才去问 venue。上一实例受控停止时，第 1 步没有要回收的行，第 2 步没有无后继的 `SendBarrier`。
- 第 4 步先取证后发送：等待已结束的尝试先移出阻塞头集合，再放未发出的 `Prepared`；发送与取证都依赖该集成已建立的会话，尚无会话的集成其尝试在发出前门等会话建立（或 `deadline` 到期）。
- 失败分两级：取不到 fence、格式版本 / 迁移 / 重建失败、统一路径配置读不出，整体拒绝启动，不进入部分运行态；单个集成 `Connecting` 或 `Halted`、单个程序装载失败、被失败抑制或等待所引用集成来源的首个声明版本，只使该单元不可用，其余照常启动。`Halted` 与程序的失败抑制跨核心重启保持：它们来自控制流上的执行事实 `IntegrationHalted`、`ProgramHalted`，不来自可压缩的观察记录。消费方在第 5 步之前只看到 `Starting`。
- 旧实例的 readiness 不另写：第 3 步的 `Connecting` 记录之后，readiness 由 fold 派生为 `Disconnected`，直到新会话 `Established`（§8.4）。

核出：第 4 步原文只写了取证与发送，没写 `EffectRequest` 重派与待决单据的计时器重装——已并入 §7.2 第 4 步。

## D1.3 集成的会话状态与会话的通道

对照：§7.1 核心创建的通道、§7.2 第 3 步（会话状态、转移表、会话 epoch）与受控停止、§7.3 集成会话、§8.2 `handshake`、§8.3（推送、会话中吊销身份）、§8.4 readiness 与健康、§7.6 `rotate_credential`。

```mermaid
stateDiagram-v2
  state "Connecting：本实例的运行在进行，尚无已建立的会话；在当前通道上 handshake()" as HS
  state "Established(SessionEpoch)：只读本会话的通道" as EST
  state "Halted{ProjectionInvalid | ContractIncompatible}" as REJ
  state "Halted{Refused(reason)}" as REF
  [*] --> HS : 采纳的登记：核心启动（控制流上没有未解除的 IntegrationHalted；与采纳记录同事务），或 restart_integration 采纳本实例尚未运行的 id（与其 Applied 同事务）；append Connecting 健康观察（运行的开始锚点）。受控停止开始时 Applied 还没有提交的得 Rejected(Stopping)，不采纳，见 D1.6
  [*] --> REJ : 核心启动（最近的 IntegrationHalted 未解除，不拉起）
  [*] --> REF : 核心启动（最近的 IntegrationHalted 未解除，不拉起）
  HS --> EST : handshake 返回合法 Projection（同事务 append 声明版本与 Established 健康观察；有未兑现的轮换时各流开新 epoch（credential_rotated），同事务在控制流上 append 轮换兑现记录）
  HS --> REJ : 投影不合法（含表外写操作 / 键角色 / 目标种类）/ 契约版本不兼容（同事务 append IntegrationHalted 与健康观察，不降级；提交后结束会话、终止进程，OS 确认退出、清除行时本次运行结束）
  HS --> REF : handshake 返回 Refused（上游明确拒绝身份或配置；同上）
  HS --> HS : handshake 返回 Unavailable → 按 pacing 在同一通道上重握手
  HS --> HS : 通道断开 / 进程退出 → 结束该会话；OS 确认退出后拉起新进程、新通道（session_seq += 1）
  HS --> HS : rotate_credential / restart_integration → 结束会话、终止进程，OS 确认退出后拉起新进程（session_seq += 1）
  EST --> HS : 通道断开 / 集成进程退出（含集成因上游吊销身份而关闭通道退出）；在途调用恰好完成一次（写 NoResponse、读 Unavailable），然后关闭通道
  EST --> HS : rotate_credential / restart_integration（同上）
  EST --> EST : 从本会话通道读入的推送 → 盖上 SessionEpoch、分配 LogPosition
  REF --> HS : rotate_credential / restart_integration（运维 principal；上一次运行结束（旧进程 OS 确认退出、行已清除）之后才提交 Applied，它以位置引用该 IntegrationHalted、与 Connecting 健康观察同事务，开始新一次运行；提交后才拉起。受控停止开始时还没有提交的得 Rejected(Stopping)，仍 Halted，见 D1.6）
  REJ --> HS : restart_integration（同上）
  REJ --> REJ : rotate_credential → 控制记录 Rejected
  note right of EST
    SessionEpoch = (instance_id, session_seq)：核心给通道化身起的名字，
    盖在核心 append 的记录上；集成不回填。
    旧会话的通道已关闭，它的迟到回执、推送与握手结果读不到；
    venue 已受理的写由新会话观察或对账并入原尝试（D6.6 步 4）
  end note
  note left of HS
    只有 Established 时 IO 壳才发写与取证（发出前门，D6.1）
    全部调用经集成会话的调用通道；每次状态改变（HS→HS 不算）集成会话 append 一条会话健康观察；
    readiness 不另写：最近的会话健康观察不是 Established 时，fold 派生 Disconnected
  end note
```

读法：

- 一个集成进程恰有一个会话（一条通道）；会话在进程之内结束，进程在 OS 确认退出之后才让位给下一个。上游暂时不可达只在同一通道上重握手，不换进程。
- 会话 epoch 与流 epoch 独立：重连后集成若能以 venue 游标证明续接，流 epoch 不变、`Seq` 接续；证明不了才开新流 epoch（D3.2）。
- `rotate_credential` 留下未兑现的轮换（控制流的 fold）：该集成此后第一次成功的握手，不论在本实例还是之后的实例，强制新流 epoch（`credential_rotated`），不允许续接，并在同一事务 append 以位置引用它的轮换兑现记录（§7.6）；`Unavailable`、`Refused` 与投影不合法都不兑现。
- `Connecting` 会自己恢复，两种 `Halted` 只由运维动作解除，核心重启不解除（它来自执行事实 `IntegrationHalted`，解除的 `Applied` 以位置引用它）；健康面把二者分开给出。`Halted` 记的是核心不再自动握手的决定，进程的结束另由 OS 确认。
- 会话结束时在途调用先恰好完成一次，再关闭通道，已完成的调用不会被迟到回应再完成，也不再计数（§7.2 第 3 步）。
- 重握手后不再声明的流：选中它的订阅对该流挂起（原因 `StreamUndeclared`），流再被声明时恢复；断连与 `Halted` 不使订阅挂起（§8.2）。

核出：无。

## D1.4 持久化归属：谁写哪张表

对照：§7.5 持久化归属表；§7.3“执行事实 append 链的唯一写入口”。

```mermaid
flowchart LR
  subgraph W["写者（都在核心进程内）"]
    PUSH["集成推送入口<br/>（位置由核心分配）"]
    RD["一次性读元素：除回填外的 read（发起方：读处理器 · 钩子先查后判 · 消费方 read）<br/>（结果项 + 读结论记录或 Gap{Channel} + 计数观察；并入的读处理器的 EffectResponse 由其 owner 同事务 append）"]
    IOR["IO 壳：回执 / 取证的观察记录（订单状态；每笔可识别执行一条成交记录）"]
    TK["单据（TicketAction）"]
    STS["STS 规则链（含授权步否决时的安全事件）"]
    IOE["IO 壳：SendBarrier（写操作 · 键 · 键角色）/ VenueAccepted / VenueRejected / NotSent / Undetermined / Expired / Abandoned /<br/>ResolutionEvidence / ReconciliationReopened{SessionRestored} / CapabilityObserved / 取证 Gap{Channel}"]
    ATTR["效应侧归因处理器：ResolutionEvidence{Attributed}"]
    CTL["控制面：控制记录（bypass_lane 随其 lane 流，其余都在控制流；解除 Halted 的 Applied 引用 IntegrationHalted；restart_integration 的 Applied 带文件 hash 与 instance_id；load_program 的 Applied 钉内容 hash、以值记下执行事实输入、引用的原生 op 名集合与输出契约、记下沿用的 Checkpoint，解除失败抑制时引用 ProgramHalted；install_native_op 的 Applied 以值记下 op 名、签名、artifact 引用与内容 hash，remove_native_op 的记下 op 名）· 越权安全事件 · ReconciliationReopened{Manual}"]
    SESS["会话入口：未完成握手请求的安全事件"]
    OUT["出站请求处理器：EffectRequest · EffectResponse"]
    SUBEL["持久订阅元素"]
    ISESS["集成会话"]
    HOSTP["宿主协议 / 程序宿主元素"]
    FENCE["fence"]
    RET["保留协议"]
  end
  subgraph T["SQLite 表"]
    OJ[("观察 Journal<br/>RetractableDelta，可压缩")]
    EJ[("执行事实 Journal<br/>纯 append；lane 流 · 声明流 · 请求流 · 控制流")]
    SUB[("订阅表 / cursor")]
    CAP[("能力证据")]
    CK[("Checkpoint")]
    PT[("进程表 (instance_id, pid, start_time, role)：OS 进程的索引")]
    RB[("保留边界 + 引用登记")]
    SN[("快照")]
    IID[("实例表：instance_id 与结束锚点")]
  end
  PUSH --> OJ
  RD --> OJ
  IOR --> OJ
  TK --> EJ
  STS --> EJ
  IOE --> EJ
  ATTR --> EJ
  CTL --> EJ
  CTL -->|"程序流开 epoch 的 Gap{Source}（start / program_upgrade），与开始不沿用旧状态之成员的 Applied 同事务"| OJ
  SESS --> EJ
  OUT --> EJ
  SUBEL -->|"需求 · cursor · 逐项状态 · 投递缺口 Gap{Delivery}；程序订阅：开始 / 替换 / 卸载成员的 Applied 同事务建立、沿用、重建或结束它及其项，Advance 事务里写它的 cursor 并删被覆盖的缺口"| SUB
  SUBEL -->|"回填的结果项与读结论记录 · 回填进度健康观察 · 覆盖检查点 · 路由结论记录"| OJ
  ISESS --> CAP
  ISESS -->|"登记的采纳记录 · IntegrationHalted · 轮换兑现记录（控制流）· 声明版本"| EJ
  ISESS -->|"集成进程的行"| PT
  ISESS -->|"会话健康观察 · 调用计数（计数由发起方同事务提交）· 握手时新流 epoch 的 Gap{Source}（与持久订阅的 None{epoch} 同事务）"| OJ
  HOSTP -->|"程序宿主进程的行"| PT
  IOE --> CAP
  HOSTP -->|"Checkpoint（与持久订阅写在程序订阅上的 cursor 同一 Advance 事务）"| CK
  HOSTP -->|"Advance 输出的派生记录（解释①）· 程序观察 ProgramReset / ProgramFailed"| OJ
  HOSTP -->|"ProgramHalted（控制流，与 ProgramFailed 同事务）"| EJ
  FENCE -->|"新实例一行、instance_id += 1（与 fence 同事务）；受控停止的结束锚点"| IID
  RET --> RB
  TK -.->|"Prepared 同事务自动登记 basis"| RB
  IOE -.->|"等待仍 Active 时的 ResolutionEvidence 自动登记引用"| RB
  ATTR -.->|"等待仍 Active 时的 ResolutionEvidence 自动登记引用"| RB
  CTL -.->|"unload_program 与不沿用旧状态的替换的 Applied 解除程序 cursor 引用（在宿主 OS 确认退出之后 append）"| RB
  HOSTP -.->|"进入活动集合后每个 Checkpoint 自动登记 cursor"| RB
```

读法：

- 观察 J 有七类写者（含一次性读元素的读结果、集成会话的会话健康观察与握手时的新 epoch `Gap{Source}`、持久订阅的回填结果、回填进度与路由结论、程序宿主元素写的派生记录与程序观察、控制面的程序流开 epoch 的 `Gap{Source}`），执行 J 有九类（含会话入口的安全事件、集成会话的采纳记录、声明版本、`IntegrationHalted` 与轮换兑现记录、程序宿主元素的 `ProgramHalted`）；两侧共享存储原语但类型宇宙不共享（§4.1）。采纳记录、`IntegrationHalted`、轮换兑现记录、`ProgramHalted` 与除 `bypass_lane` 外的控制记录都在一条控制流上（§7.3），采纳集合、`Halted`、未兑现的轮换、活动集合与失败抑制、已安装 op 集合都只按这条流上的位置 fold。
- 进程表有两个写者，按 `role` 分行：集成会话写集成进程的行，程序宿主元素写宿主进程的行（§7.5）。订阅表只有持久订阅元素一个写者：消费方订阅与程序订阅都在其中，程序的 cursor 只存在这里；程序宿主元素编排 `Advance` 事务、写 `Checkpoint` 表，控制面的 `Applied` 事务里的程序订阅变更也由持久订阅元素写（§7.5 订阅表）。
- 读模型不写任何表：它是按种类对执行 J、观察 J 的只读 fold（`subscriptions` 读订阅表当前态，§8.5），供 `read_model` 读取。
- 引用登记不是独立写者动作：随 `Prepared`、`Checkpoint` 与等待仍 `Active` 时的 `ResolutionEvidence` 的 append 自动写入，随尝试等待结束（结果确立、`Expired`、`Abandoned`）、下一 checkpoint、`unload_program` 或不沿用旧状态的替换自动解除（D8.1）。
- 核心不写统一路径下的任何文件（§7.6）。

核出：无。

## D1.5 同事务集合

对照：§7.4 事务原子性；§6.1 请求完成事实；§6.2 与写边界的接口；§6.5 记录模型；§8.6 `Advance` 与失败抑制；§7.2 第 1、3 步；§7.6 轮换强制的新流 epoch。

| 同一 SQLite 事务内必须一起提交 | 依据 | 崩在中途的后果（§9.2） |
|---|---|---|
| `Close(Prepared(position))` + `Prepared` | §6.2、§7.4 | #1：二者皆无，单据仍 `AwaitingDecision` |
| Decision / `Outcome` / `Rejection` 记录 | §7.4 | 链步未发生，重启按记录重新求值该步（`RuleState` 由记录 fold 出，§6.3） |
| 程序写处理器的 `Draft` + `SubmitForDecision` + `EffectResponse{Drafted}` | §6.1 | #21：无 `EffectResponse` → 重派开单 |
| 读处理器的结果项与读结论记录 / `Gap{Channel}` + `EffectResponse{Concluded / Unavailable}` | §6.1 | #21：无 `EffectResponse` → 重新执行一次 |
| 写调用（`submit` / `cancel`）的 `Ack`：`VenueAccepted`（含 `Evidence`）+ 该回应的观察记录（订单状态；每笔可识别执行一条成交记录）+ 该调用的计数健康观察 | §6.5 记录模型、§8.4 | #5：视为无后继 → `Undetermined` → by-key 取证重得同一状态 |
| 写调用返回 `NotSent`：`NotSent` 记录 + 该调用的计数健康观察 | §6.5、§8.4 | 视为无后继 → `Undetermined(CrashWindow)`（保守） |
| 一次取证命中：`ResolutionEvidence{Found}`（含 `Evidence`）+ `provenance: Reconciliation` 的该回应观察记录 | §6.5 | #7：该次取证不存在；fold 显示本轮该渠道未取证，重做（读可重试） |
| 归因命中：带 `FromAttempt` 的观察记录 + `ResolutionEvidence{Attributed}`（记录来自回执、取证响应或一次性读时，与该回应的其余记录同一事务） | §6.6、§8.1 | 推送带来的未 append：核心崩溃即集成成孤儿被回收，重启握手后该流续接（集成以 venue 游标证明）或新 epoch + `Gap{Source}`；续接则记录重到，仍处 `Undetermined` 且结果未知的尝试照常归因。来自回执、取证响应的，随该回应一起未落，按上面对应的行恢复。来自一次性读的按发起方：读处理器的结果项按上面读处理器那一行恢复（#21 重新执行一次）；消费方 `read` 与“先查后判”的读随实例结束、没有结论，核心不重发（§7.2 生命周期表、§8.5 一次性读的 `Pending`、§10.5 #50）。两种情形下该尝试都仍处 `Undetermined`，经它自己的渠道或此后的记录收敛 |
| `Undetermined` append + 回查已到达的归因观察 | §6.6 | 二者同事务，不存在"归因已到但未匹配"的持久态 |
| `Advance` 输出：`EffectRequest` 记录 + 派生记录 + `Checkpoint` + 程序订阅上的程序 cursor（持久订阅元素写）+ 新 cursor 覆盖的 `Gap{Delivery}` 的删除 | §8.6 | #16：整批不存在，从同一组已提交 cursor 重新推进，未确认的缺口仍在 |
| fence 取得 + 实例表新一行（`instance_id += 1`） | §7.2 第 1 步 | 未提交则旧 `instance_id` 仍有效，重来 |
| 单条观察 append + `LogPosition` 分配 | §7.4 | #9：半写不可见 |
| 调用结果的记录（回执、取证、读结论、`Gap{Channel}`、`Undetermined`）+ 该调用的计数健康观察 | §7.3、§8.4 | 二者皆无：调用结果未落，计数不前进；不存在“计了数却无结果”的持久态 |
| 握手成功（集成会话编排）：声明版本 + `Established` 健康观察 + 开新 epoch 的流的 `Gap{Source}` 与各自的 `None{epoch}` 回填进度（集成会话请持久订阅写）+ 待接纳订阅项（含程序订阅的项）按新声明转为接纳或被拒；有未兑现的轮换时，各流 `Gap{Source, credential_rotated}` + 控制流上以位置引用被兑现的 `rotate_credential` `Applied` 的轮换兑现记录 | §7.2 第 3 步、§7.6、§8.2 `handshake`、§8.4 | 二者皆无：会话未建立，轮换仍未兑现，重启后在新进程的新通道上再握手，那次成功的握手照样开新 epoch 并兑现 |
| 会话内上报开新 epoch（集成会话编排）：集成推送入口 append 的 `Gap{Source}` + 持久订阅为该逻辑流写的 `None{epoch}` | §8.3、§8.4 会话内开的新流 epoch | 二者皆无：这条 gap 未被接受，重启后该流在新会话的握手里按续接或新 epoch 决定 |
| 启动第 3 步：登记的采纳记录（控制流）+ 每个采纳的集成的初始会话健康观察（`Connecting`，或恢复的 `Halted{cause, since}`） | §7.2 第 3 步、§8.4 健康不留旧值 | 二者皆无：本实例还没有采纳记录，重启后由继任实例第 3 步重来；不存在列出采纳成员而没有本实例会话值的健康切面 |
| `IntegrationHalted` + `Halted` 健康观察（进程的终止在提交之后，另由 OS 确认） | §7.2 第 3 步 | 二者皆无：仍在上一状态，重启按执行事实重判；已提交而进程未确认退出：继任实例第 1 步回收 |
| 解除 `Halted` 的控制记录 `Applied`（引用 `IntegrationHalted`）+ `Connecting` 健康观察；只在上一次集成运行结束（进程 OS 确认退出、行已清除）之后提交 | §7.2 第 3 步 | 二者皆无：仍 `Halted`，没有握手发生 |
| `restart_integration` 采纳本实例尚未运行的 id：`Applied`（带文件 hash 与 `instance_id`）+ `Connecting` 健康观察 | §7.2 第 3 步、§8.5 | 二者皆无：该 id 不在采纳集合里，没有运行、没有进程 |
| `load_program` 的 `Applied`（钉内容 hash、以值记下输出契约、记下沿用的 `Checkpoint`）+ 程序订阅上全部输入的 cursor 与观察输入的订阅项；让 id 进入活动集合的（首次装载、卸载之后再装载）另加程序订阅本身的建立与新成员每条程序流新 epoch 的 `Gap{Source, start}`；替换保留程序订阅，共有输入的项只在主体集与用途也相同、且不是被拒的项时沿用，否则同事务结束旧项、建立并接纳新项；替换不沿用旧状态时（`cold_start`、不接受旧 `state_version`，或开始旧成员的 `Applied` 所记的输出契约与新成员的不同）另加 `ProgramReset`、新成员每条程序流新 epoch 的 `Gap{Source, program_upgrade}`、项与 cursor 的重建（重建的 cursor 上的投递缺口随之删除）、旧保留引用的解除；新成员不声明的流不写记录。结构校验不成立的程序值只有控制记录 `Rejected`，不在这个集合里 | §8.5、§8.6 卸载与替换、程序流的 epoch、装载期校验 | 二者皆无：id 不在活动集合里的，程序仍不在集合里，没有程序订阅，程序流没有新 epoch；替换的，旧成员照旧（替换时旧宿主已结束，重启照常判定装载旧成员，失败抑制中的仍被抑制）；已提交：按 `Applied` 所记沿用或不携带装载，不重复 `Reset`，不再开 epoch |
| `unload_program` 的 `Applied` + 程序订阅（cursor 与观察输入订阅项）的结束 + 保留引用的解除（宿主已 OS 确认退出、已发出的装载步骤都已得出结论之后）；程序流上不写记录，它的 epoch 不结束 | §8.6 卸载与替换 | 二者皆无：程序仍在活动集合里，重启照常判定装载（等待中得出装载失败、已在它之前提交 `ProgramHalted` 的，仍在失败抑制里） |
| `ProgramHalted` + `ProgramFailed` 观察（宿主的终止在提交之后） | §8.6 | 二者皆无：重启时程序仍在活动集合里且未被抑制，按 `Checkpoint` 重新装载；超预算或 trap 若再发生，再记一次 |

读法：`SendBarrier` 不在任何集合里——它单独 durable append（fsync）后才允许该尝试的写调用，这正是把崩溃窗口二分的屏障（D6.1）。

核出：无。

## D1.6 生命周期嵌套与受控停止

对照：§7.2 生命周期表、会话结束时在途调用恰好完成一次、受控停止；§7.1 凭据链与核心创建的通道；§7.6 轮换强制的新流 epoch；§8.2 `route` 的 `generation` 与谁调、何时调，§8.3 上报 gap 之前先完成已收到的调用、§8.4 会话内开的新流 epoch；§8.5 会话的生命周期、控制组；§8.6 程序的活动集合与失败抑制、卸载与替换、程序流的 epoch；§8.7 原生 op 的安装、原生 op 的执行。

```mermaid
flowchart TB
  subgraph INST["核心实例：fence 事务开始（OS、SQLite）· 结束锚点或继任者的 fence 结束"]
    CS["消费方会话：handshake 开始（OS 对端凭据）· 传输关闭结束"]
    subgraph RUN["集成运行（每个采纳的登记）：让它进入初始状态的 Connecting 健康观察（第 3 步，或 restart_integration 采纳新 id）或解除 Applied（上一次运行结束之后才提交）开始；受控停止开始时还没有提交的采纳或解除得 Rejected(Stopping)，不开始运行 · Halted 路径：IntegrationHalted 之后会话结束、最后一个进程 OS 确认退出清除行时结束（OS，或继任者第 1 步）；或实例结束（受控停止时没有进程的运行同样以实例结束锚点结束）。Connecting 中换进程不结束运行"]
      subgraph PROC["集成进程：拉起 + 进程表行开始（OS）· OS 确认退出、清除行结束；同一集成同时至多一个"]
        CRED["凭据副本：拉起时经继承句柄交付 · 随进程退出结束"]
        subgraph SESS["会话 = 通道化身：拉起时核心创建、分配 SessionEpoch · 在途调用完成、丢弃未了的 route 义务后核心关闭通道"]
          EST["Established：声明版本 + 健康观察开始 · 会话结束"]
          ROBL["route 义务（每条流至多一项，内存，持久订阅确认）：会话建立时为需求非空的流起；已建立期间接受会话内开 epoch 的 Gap{Source} 或需求变化时起，已有则并入 · 第一次 Routed 了结（发出全集已非目标全集时随即另起，§8.2），或会话结束时在途调用完成之后、关闭通道之前丢弃，不再重发；没有已建立的会话就没有义务"]
          CALLR["在途读侧调用（read / backfill / route）：发出开始 · 封闭返回值（集成在上报 Gap{Source} 之前以 Unavailable 作答已收到的），或会话结束时的强制完成；只嵌在会话里"]
          CALLW["在途写与取证调用：发出开始 · 封闭返回值或会话结束时的强制完成；只嵌在会话里"]
          ROBL -.->|"逐次发出 route（同一时刻至多一次在途）"| CALLR
        end
      end
    end
    subgraph HEXEC["程序宿主执行：成员引用原生 op 时子系统接受为它交出的内容开始，然后拉起 + 行 + Load（不引用的从拉起开始）· Unload 或终止后 OS 确认退出结束；拉起未成（OS 没有给出进程）即结束，成员同事务 ProgramHalted{LoadRejected(HostSpawnFailed)} + ProgramFailed；交出已被接受而宿主还没有拉起时：成员被 unload_program 或替换结束、或受控停止，都不再拉起即结束（核心确认），崩溃随实例结束"]
      HOVER["交给子系统的原生 op 内容：每次宿主执行一次交出、只属于这次执行，每份以 (op 名, 内容 hash) 标识 · 子系统接受即开始，未被接受则这次执行不开始、不拉起宿主（LoadRejected(NativeHandoverFailed)）· 随这次执行结束；子系统对这次执行只运行为它交出的，(op 名, 内容 hash) 相同的执行可共用一份已载入的"]
    end
  end
  subgraph MEMBER["程序成员（跨实例）：load_program Applied 开始（钉内容 hash，以值记下输出契约）· unload_program 或替换的 Applied 结束，只在宿主执行结束之后 append；装载步骤已发出的成员先等每个已发出步骤的结论（被接受而未拉起的不再拉起；装载失败的 ProgramHalted + ProgramFailed 在 Applied 之前），不设超时；替换的同一个 Applied 开始新成员，沿用判定比较开始旧成员的 Applied 所记输出契约"]
    CKREF0["程序订阅的项在本成员内的部分：开始成员的 Applied 同事务建立或沿用 · 替换的 Applied 里，新程序不再声明的输入：项与 cursor 都结束；共有输入里主体集或用途变了的项，以及被拒的项（从不沿用）：项结束、同事务建立新项并重新接纳，cursor 接着走；不沿用时全部重建"]
    CKREF["Checkpoint 的保留引用：进入活动集合后第一个 Checkpoint 登记 · unload_program 或不沿用旧状态的替换 Applied 解除；Unload 与失败抑制不解除"]
  end
  PSUB["程序订阅（每个活动程序 id 一个；程序的 cursor 只在这里）：让 id 进入活动集合的 Applied 同事务建立（持久订阅元素）· unload_program 的 Applied 结束；替换保留它，principal 换成新成员的装载 principal；共有输入的 cursor 原样沿用，项只在主体集与用途也相同、且不是被拒的项时沿用；未确认的投递缺口随 cursor，不随项；提交的 Advance 是它的确认，只确认交出的投递事件；Origin 输入的 cursor 建为 Start{from}，前导在第一次提交的 Advance 时确认、cursor 成为 At，在它之前崩溃则前导重交"]
  PSUB -.->|"跨替换延续，其项在各成员里建立、沿用或重建"| CKREF0
  MEMBER -.->|"每个实例一个宿主执行（未被失败抑制，且按“等待的先后”不等待、程序值的内容 hash 相符、声明校验成立、所引用原生 op 的制品 hash 相符时）"| HEXEC
  EPOCH["流 epoch（集成来源的每条逻辑流，不嵌在实例或会话里）：该流开 epoch 的 Gap{Source} 开始（握手时集成会话 append，或会话内集成上报）· 下一条开 epoch 的 Gap{Source} 结束（backfill_incomplete 是 epoch 内的记录，不结束也不开 epoch）；边界由集成确认，身份由核心分配；握手以游标续接时跨会话、跨实例延续"]
  PEPOCH["流 epoch（程序产出的每条派生流；不嵌在实例或程序成员里）：开始不沿用旧状态之成员的 Applied 事务里、该成员每条程序流上的 Gap{Source} 开始（让 id 进入活动集合的 load_program：start；不沿用旧状态的替换：program_upgrade；该 id 下第一次被声明的流无前驱）· 该流下一条开 epoch 的 Gap{Source} 结束，它写在同一 id 此后第一个声明该流、不沿用旧状态的成员开始的 Applied 里；unload_program、不声明该流的成员开始、沿用旧状态的替换（输出契约相同）与实例更替都不结束它；控制面确认，身份由核心分配"]
  CALLR -.->|"问的是 · 结果按它准入（不是嵌套）：发出 epoch 是调用的属性（read 由 dispatch_end 记下，backfill 为任务所在 epoch，route 由 generation 指名）；read / backfill 的结果到达时它已结束，核心记 Unavailable；route 带旧 generation，集成答 Unavailable、供给不变"| EPOCH
  HALT["不嵌套的持久事实（控制流上）：IntegrationHalted 起的 Halted 抑制 / ProgramHalted 起的失败抑制（以位置引用它的 Applied 解除）、登记的采纳、rotate_credential Applied 起的未兑现轮换（以位置引用它的轮换兑现记录结束）；以及订阅、单据"]
  INSTOP["已安装的原生 op（每个 op 名，跨实例；不嵌在实例里）：install_native_op 的 Applied 开始（以值记下签名、artifact 引用与内容 hash）· 同名 remove_native_op 的 Applied 结束；控制面确认；制品文件由 Alice 管理，每次拉起引用它的宿主之前由核心重读、核对 hash"]
  INSTOP -.->|"remove 在活动成员的 Applied 列出该名时 Rejected(InUse)"| MEMBER
```

```mermaid
sequenceDiagram
  participant OS as OS（停止请求）
  participant C as 核心实例
  participant L as 解释层（消费方会话）
  participant H as 程序宿主
  participant I as 集成进程
  participant DB as SQLite
  OS->>C: 停止请求
  Note over C: 1 关闭新工作的入口
  C->>L: 不再接受会话，关闭全部消费方会话
  C->>C: 不再接受新的控制动作；停止之前已在执行的控制动作照常完成：unload_program / 替换等已发出装载步骤的结论与在途 Advance，abandon 等在途取证调用；Applied 还没有提交的 restart_integration / rotate_credential 不等
  C->>DB: 停止之前已在执行、Applied 还没有提交的 restart_integration / rotate_credential（在等旧集成进程退出，或要采纳本实例尚未运行的 id）：append Rejected(Stopping)，不采纳、不解除 Halted、不留下未兑现的轮换
  C->>C: 不再 append SendBarrier、不再取证、不再 route / backfill / 一次性读 / 分派；不再拉起集成进程（Connecting 按 pacing 的重拉，与停止之前已提交的 restart_integration / rotate_credential 之后的拉起都没有；那次运行没有进程，以实例结束锚点结束）、不再发新的握手；已在途的调用照常等结果
  C->>C: 不再开始新的装载步骤，等待中的成员不再判定装载；已发出的装载步骤（未回答的交出、在进行的核对或声明校验）等到结论，都在第 5 步之前；这一等待不挡第 2–4 步
  C->>DB: 核对不符、声明校验不成立或交出未被接受：照常同事务 ProgramHalted + ProgramFailed（停止不吞掉装载失败）
  C->>C: 交出已被接受（含停止之后才到的接受）而宿主还没有拉起：不再拉起，这次宿主执行就此结束（核心确认），交出随之结束
  Note over C,H: 2 程序
  C->>DB: 等在途 Advance 的事务提交（或确知不提交）
  C->>H: Unload（受控停止本身不改变成员）
  Note over C,I: 3 集成会话
  C->>DB: 在途写 NoResponse → Undetermined，在途读 Unavailable（各与计数同事务）
  C->>C: 丢弃未了的 route 义务，不再重发
  C->>DB: 停止之前已在执行的 abandon：在途取证调用都已完成（含刚被强制完成为 Unavailable 的），一个事务里重查：结果仍未知 append Abandoned，否则不写（Rejected(NotUndetermined)）
  C->>I: 关闭通道
  Note over C,I: 4 进程
  C->>H: 请求退出，超时强制终止
  C->>I: 请求退出，超时强制终止
  H-->>C: OS 确认退出
  I-->>C: OS 确认退出
  C->>DB: 清除进程表的行
  C->>DB: 停止之前已在执行的 unload_program / 替换：内层结论都已到、宿主 OS 确认退出之后 append 它的 Applied（替换的新成员不开始装载步骤，由下一实例第 5 步判定）
  alt 全部确认，且停止之前已发出的交出都已有回答
    Note over C,DB: 5 实例结束锚点（本实例最后一次写）
    C->>DB: 实例表本行写结束锚点
    Note over C,DB: 6 释放 fence
    C->>DB: 关闭 SQLite，释放 OS 文件锁
  else 有进程得不到确认，或第 2–4 步都已完成时交出仍没有回答
    C-->>OS: 停止失败：不写结束锚点、不释放 fence；不因交出没有回答而 append ProgramHalted（核心不替子系统判定超时）；等这次交出或那个宿主进程退出的 unload_program / 替换没有 Applied，随实例结束、不生效；外力结束即崩溃路径，D1.2 第 1 步回收
  end
```

读法：

- 每层都在外层之内开始、在外层之前结束；每个锚点都有确认者。持久事实（`Halted`、失败抑制、采纳、未兑现的轮换、订阅、单据）不嵌在实例里，由记录的 fold 跨实例恢复；前四者都在控制流上，按这条流上的位置 fold。已安装的原生 op 同样在控制流上、跨实例；它与程序成员不是嵌套：成员可以在 op 安装之前得到 `Applied`（之后声明校验不成立），而移除在有活动成员的 `Applied` 列出该名时被拒（`InUse`），所以 op 不先于引用它的成员结束（§7.2 生命周期表、§8.7 原生 op 的安装）。
- 流 epoch 也不嵌在实例或会话里：集成来源的流，握手以游标续接时它跨会话、跨实例延续，只由下一条开 epoch 的 `Gap{Source}` 结束，`backfill_incomplete` 不结束它；程序产出的派生流，由开始不沿用旧状态之成员的 `Applied` 在该成员的每条程序流上开出（让 id 进入活动集合的 `load_program` 带 `start`，不沿用旧状态的替换带 `program_upgrade`），到该流下一条开 epoch 的 `Gap{Source}` 为止，即同一 id 此后第一个声明该流、不沿用旧状态的成员开始时；`unload_program`、不声明该流的成员开始、沿用旧状态的替换（输出契约相同）与实例更替都不结束它。读侧调用与写、取证调用一样只嵌在会话里，会话结束时强制完成。`route` 义务也嵌在会话里：它跨过多次 `route` 调用（`Unavailable` 按 pacing 再发，gap 与需求变化并入），第一次 `Routed` 了结；会话结束时，在途调用完成之后、关闭通道之前丢弃，不带进下一个会话，下一个会话建立时按那时的需求重新起；会话结束只丢弃义务，不结束流 epoch。发出 epoch 是读侧调用的属性，不是外层：流 epoch 何时结束由集成决定，送出 `Gap{Source}` 时仍在通道上的调用只能在它之后结束，所以 CALLR 到 EPOCH 的虚线是“问的是、结果按它准入”，不是嵌套。集成在上报 `Gap{Source}` 之前以 `Unavailable` 作答它已收到的调用；`read`、`backfill` 的结果在发出 epoch 结束之后才到核心的，由核心完成为 `Unavailable`；带旧 `generation` 的 `route` 由集成答 `Unavailable`、供给不变，核心不另判。写与取证按作用域寻址，同样只嵌在会话里。
- 集成运行在 `Halted` 路径上的结束锚点是最后一个进程 OS 确认退出、清除进程表的行，不是 `IntegrationHalted`：后者先提交（§7.2 第 3 步“进入 `Halted`”），只开始 `Halted` 抑制，会话与进程在它之后才结束。`Connecting` 中换进程不结束运行。解除 `Halted` 的 `Applied` 只在上一次运行结束之后提交，所以同一集成的两次运行不重叠。受控停止第 1 步之后不再拉起任何集成进程；停止开始时还没有提交的解除与采纳新 id 的 `restart_integration` 得 `Rejected(Stopping)`，不开始运行；停止之前已提交而进程还没有拉起的运行没有进程，同样以实例结束锚点结束。
- 程序成员跨实例，宿主执行是成员与实例的共同内层；受控停止结束宿主执行而不改变成员。`unload_program` 与替换先结束宿主执行（停调度、等在途输出事务、OS 确认退出），再 append 结束成员的 `Applied`，cursor 与保留引用随之结束或转给新成员（§8.6 卸载与替换）。成员还在装载、已有装载步骤发出时，卸载与替换同样不开始新的装载步骤，在结束成员的 `Applied` 之前等每个已发出步骤的结论，核心不设超时：交出已被接受而宿主还没有拉起的不再拉起，这次执行由核心确认结束；交出未被接受或别的装载失败照常同事务 `ProgramHalted` + `ProgramFailed`，都在 `Applied` 之前，替换的 `Applied` 以位置引用这条 `ProgramHalted`（§8.6 装载中的成员）。成员引用原生 op 时，宿主执行从子系统接受这次交出开始，交出只属于这次执行、随它结束，所以移除之后以另一份内容重新安装的同名 op，下一次执行运行的是新交出的那一份；交出未被接受，这次执行就不开始，宿主不被拉起（§7.2 生命周期表、§8.7 原生 op 的执行）。交出已被接受之后 OS 没有给出宿主进程，这次执行就此结束，成员同事务 `ProgramHalted{LoadRejected(HostSpawnFailed)}` + `ProgramFailed`，不是 trap（§8.6 失败抑制）。受控停止第 1 步不再开始新的装载步骤，并在实例结束锚点之前等到已发出步骤的结论，这一等待不挡第 2–4 步：交出已被接受（含停止之后才到的接受）而宿主还没有拉起的不再拉起，这次执行由核心确认结束，停止本身不 append `ProgramHalted`；已发出的步骤得出核对不符、声明校验不成立或交出未被接受的，照常同事务 `ProgramHalted` + `ProgramFailed`（§7.2 受控停止、§8.7 失败语义）；第 2–4 步都已完成时交出仍没有回答的，停止以失败报告，不写结束锚点、不释放 fence，也不因没有回答而 append `ProgramHalted`：这次交出的结论只有子系统是源头，核心不替它判定超时；实例之后被外力结束即崩溃路径，由继任者的 fence 结束。
- 停止之前已在执行的控制动作照常完成，结论在实例结束锚点之前：`unload_program` 与替换等已发出装载步骤的结论与在途 `Advance`，宿主 OS 确认退出之后 append `Applied`（替换的新成员不开始装载步骤，由下一实例第 5 步判定）；`abandon` 等在途取证调用完成（第 3 步强制完成的为 `Unavailable`）之后重查，结果仍未知才 append `Abandoned`，否则不写；`Applied` 还没有提交的 `restart_integration` 与 `rotate_credential` 不等任何进程，在第 1 步得 `Rejected(Stopping)`：不采纳、不解除 `Halted`、不留下未兑现的轮换，发起方在服务再次启动之后重新发起。停止之前已提交的 `rotate_credential` `Applied` 留下的未兑现轮换，由该集成之后第一次成功的握手兑现：开新 epoch（`credential_rotated`），同一事务 append 轮换兑现记录。所以受控停止本身不改变成员，成员的变化只来自这些动作自己的 `Applied`。交出始终没有回答、或所等的宿主进程得不到 OS 确认退出时停止失败，等它的控制动作没有 `Applied`，随实例结束、不生效（§7.2 受控停止“停止之前已在执行的控制动作”）。
- 受控停止不为集成另写会话健康观察：实例结束锚点就是本实例各集成运行的结束，停止时没有进程的运行也一样；下一实例第 3 步写新的初始状态。

核出：无。
