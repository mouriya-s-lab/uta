# 04 程序宿主：`Advance` 循环、生命周期、`EffectRequest` 分派

对照：§4.3、§6.1、§8.6、§7.5 程序状态、W10、W17、§9.2 #16/#21。索引见 `README.md`。

## D4.1 一轮 `Advance`

对照：§8.6 宿主协议、§4.3 输入 = 位置推进、§6.1 出口、§7.5 `Checkpoint` 与 cursor 同事务、§4.2 程序订阅。

```mermaid
sequenceDiagram
  participant J as 观察 Journal
  participant C as 核心（程序订阅 + 宿主协议）
  participant H as 程序宿主进程
  participant DB as SQLite（同一事务）
  participant O as 出站请求处理器
  participant I as 集成
  J->>C: 程序 cursor 之后有新记录
  C->>H: Advance(records, to = cursor')
  Note over H: 解释①：nodes 增量 DAG，cutoff<br/>解释②：rules fold → On / Require / Expire / Emit
  H-->>C: Output{effects, derivations, checkpoint{bytes, state_version}}
  alt Output 超预算（意图速率 / 状态大小）
    C->>DB: 同事务 append ProgramHalted{Budget(kind)}（执行 J）+ ProgramFailed{Budget(kind)}（观察 J）
    C->>H: 提交之后终止宿主进程；OS 确认退出后清除登记
    Note over C: 程序进入失败抑制，本批输出不落
  else checkpoint.state_version 不在本成员 Applied 所记的接受集合内（违反契约，视同 trap）
    C->>DB: 同事务 append ProgramHalted{Trap}（执行 J）+ ProgramFailed{Trap}（观察 J）
    C->>H: 提交之后终止宿主进程；OS 确认退出后清除登记
    Note over C: 程序进入失败抑制，本批输出不落；所以持久化的 Checkpoint 总被本成员接受（§8.6 状态迁移）
  else 正常
    C->>DB: BEGIN
    C->>DB: append EffectRequest 记录{member = 开始本成员的 Applied 位置}（执行 J，该程序的请求流）× effects
    C->>DB: append 派生记录（观察 J，程序流）× derivations
    C->>DB: 写 Checkpoint + 程序 cursor = cursor'；登记 cursor 引用
    C->>DB: COMMIT
    Note over C,DB: 崩在 COMMIT 前：整批不存在，重启重放同一批（#16）
    C->>O: 逐条分派 EffectRequest（事务之后）
    alt 读处理器（一次执行，不自行重试）
      alt 未调用集成（按 §8.2 read 的判定顺序：来源未登记 / 从未有声明 / 来源无会话 / 流不在会话有效声明里 / 会话有效声明的 read 为 Unsupported 或 Unknown / 请求不合 schema）
        O->>DB: EffectResponse{request: pos, NotCalled(reason)}（不调用集成、不 append 观察记录）
        Note over C: 程序的解释②在自己的请求流上看到这条 EffectResponse（§6.1）
      else 调用
        O->>I: read(stream, request, range)
        alt Answered（含空）或 Refused
          I-->>O: Answered(items) / Refused(reason)
          O->>DB: 同事务：item 观察记录 × N（one_shot）+ 读结论记录，provenance OneShot{origins ∋ Request(pos), request}（观察 J）+ EffectResponse{Concluded(结论)}（执行 J）
          J->>C: 程序按 cursor 看到它们 → 下一轮 Advance（闭环走观察侧）
        else Unavailable
          I-->>O: Unavailable
          O->>DB: 同事务：Gap{Channel}（观察 J，该流）+ EffectResponse{Unavailable(gap)}
          J->>C: 程序看到 gap，自行决定是否再 Emit
        end
      end
    else 写处理器（读 EffectRequest.member 所指 Applied：发出成员的装载 principal 与执行事实输入，不读当前成员）
      alt 意图的锚点构造不出（D5.2）
        O->>DB: EffectResponse{NotDrafted(Malformed{reason})}（请求流；不开单，不做作用域判定）
      else 构造成功，但意图的 (来源, 作用域) 不在发出成员 Applied 所记的执行事实输入之内
        O->>DB: EffectResponse{NotDrafted(ScopeNotObserved)}（请求流；不开单）
      else 构造成功且在其内
        O->>DB: 同事务 Draft{by: 发出成员的装载 principal, basis ∋ pos}（lane 流）+ SubmitForDecision + EffectResponse{Drafted(ticket)}（请求流）（D5.1）
        Note over C: 单据与尝试的记录经程序声明的执行事实输入投给它，解释②按 ticket_id 与 p 认出自己的
      end
    else 未注册
      Note over O: Unhandled：记录留在日志，无 EffectResponse，不重派
    end
  end
```

读法：

- 程序看到的只有 cursor 之后的记录；它的输出是值（`EffectRequest`、派生记录、`Checkpoint`），不是调用。
- 事务边界在 `COMMIT`：`Emit` 是否"发生"以 `EffectRequest` 记录是否持久为准；处理器执行在其后，通过位置引用与请求关联（D4.3）。
- `fetch.bars`（读）与 `trade.place`（写）对程序是同一构造子；差别在注册表。

核出：读处理器的每种完成结果（含空结果、上游拒绝、`Unavailable`、未调用集成）都需要与请求同寿命的完成事实——已并入 §6.1（`EffectResponse`）；空结果在观察侧由读结论记录表示——已并入 §8.2。

## D4.2 程序生命周期

对照：§8.6 装载期校验、`Load`/`Reset`/`Unload`、程序的活动集合与失败抑制、卸载与替换、程序流的 epoch、预算语义、状态迁移、运维冷启动；§8.5 `load_program`/`unload_program`；§6.1 发出成员；§4.2 `start`、`program_upgrade`；§7.2 生命周期表与受控停止。

```mermaid
stateDiagram-v2
  state "等待来源的声明" as WAIT
  state "结束宿主执行中" as DRAIN
  [*] --> Loading : load_program 的 Applied（id 不在活动集合里：以值记下成员事实：装载 principal、内容 hash、预算、接受的 state_version 集合、执行事实输入；同事务按各输入声明的起点建立全部 cursor，并为新成员的每条程序流开新 epoch：Gap{Source, start}）/ 启动第 5 步（活动集合中未被抑制的程序；不开 epoch）
  [*] --> WAIT : 同上，但所引用的某个来源还没有任何声明版本：不拉起宿主、不 ProgramHalted，留在活动集合里
  WAIT --> Loading : 该来源第一次握手成功（append 声明版本）→ 做装载期校验
  WAIT --> Unloaded : unload_program 的 Applied（没有宿主，跳过第 1–3 步，直接 append）
  WAIT --> Loading : 替换的 Applied（没有宿主，跳过第 1–3 步，直接 append；沿用规则同 DRAIN→Loading），新成员所引用的来源都已有声明版本
  WAIT --> WAIT : 替换的 Applied（同上），但新成员所引用的某个来源还没有声明版本：新成员等待，不拉起宿主
  Loading --> Running : Load(program, checkpoint?, budget) → Loaded{state_version}；checkpoint 是本成员可交回的（Applied 沿用的，或其后持久化的最近一个），其 state_version 总在本成员接受的集合内，Load 不比对
  Loading --> Halted : 程序值内容 hash 与 Applied 所钉不符 / LoadRejected（Id 越界/环、required_inputs 与最近声明不符、含 Pooled 而子系统未装）→ 同事务 ProgramHalted{ContentUnavailable 或 LoadRejected(reason)} + ProgramFailed
  Running --> Running : Advance 循环（D4.1）
  Running --> Halted : 超预算 / Output 的 checkpoint 版本不在本成员接受的集合内（视同 trap）/ trap（宿主异常退出）→ Output 不落，同事务 ProgramHalted + ProgramFailed，提交后终止宿主、OS 确认退出后清除登记
  Running --> Stopped : 受控停止第 2 步 → Unload；活动集合、cursor 与引用不变，下一实例第 5 步重新 Load
  Running --> DRAIN : unload_program，或替换（对该 id 再 load_program）生效：停止调度 Advance，等在途输出事务提交或确知不提交，Unload，OS 确认退出、清除行；此时还没有 Applied
  DRAIN --> Unloaded : 然后 append unload_program 的 Applied：离开活动集合，全部 cursor 结束，Checkpoint cursor 引用解除（Checkpoint 只作记录保留）；程序流不写记录，epoch 不结束
  DRAIN --> Loading : 然后 append 替换的 Applied：同一条结束旧成员、开始新成员。沿用（非 cold_start、新旧成员的程序流集合相同，且无旧 Checkpoint 或新程序接受其 state_version）：共有输入 cursor 与引用原样沿用，Applied 记下沿用的 Checkpoint，程序流接着原 epoch；不沿用：同事务 ProgramReset{cold_start 为 Operator，否则 Replace}、新成员的每条程序流开新 epoch（Gap{Source, program_upgrade}）、cursor 按起点重建、旧引用解除。新成员所引用的来源都已有声明版本
  DRAIN --> WAIT : 同上，但新成员所引用的某个来源还没有声明版本：不拉起宿主
  Halted --> Loading : 替换的 Applied（宿主已 OS 确认退出之后，跳过第 1–3 步），以位置引用 ProgramHalted（记下新成员事实；沿用规则同上，状态本身致 trap 时用 cold_start），新成员所引用的来源都已有声明版本
  Halted --> WAIT : 同上，但新成员所引用的某个来源还没有声明版本
  Halted --> Halted : 核心重启：ProgramHalted 未被解除，第 5 步不装载
  Halted --> Unloaded : unload_program 的 Applied（宿主已 OS 确认退出之后）；程序流 epoch 不结束
  Unloaded --> Loading : 再 load_program：新成员，cursor 在其 Applied 同事务按起点建立，不交回旧 Checkpoint；同事务新成员的每条程序流开新 epoch：Gap{Source, start}
  Unloaded --> WAIT : 同上，但新成员所引用的某个来源还没有声明版本
  Stopped --> [*]
  note right of Running
    核心崩溃：重启后从本成员可交回的最近 Checkpoint
    （与 cursor 同事务持久化）重新 Load，重放 cursor 之后的记录，
    不重复 Emit（#16）
  end note
```

读法：

- `Halted` 是失败抑制：它由控制流上的执行事实 `ProgramHalted` 承载，跨核心重启保持，不自动恢复——超预算是程序作者的问题，由控制面 principal 决定是否重装；其他程序、账户、核心不受影响。`ProgramFailed` 只供展示。
- 活动集合是控制流上 `load_program`/`unload_program` 的 `Applied` 的 fold；改装载清单或程序值文件不改变它，内容与所钉 hash 不符时不装载。替换是一个动作、一条 `Applied`，中间没有程序不在集合里的时刻。
- 宿主执行在程序成员与核心实例两者之内：`Stopped`（受控停止）与崩溃都只结束宿主执行，成员、cursor 与 `Checkpoint` 引用不变；`unload_program` 与替换先经“结束宿主执行中”结束它，再写结束成员的 `Applied`；没有宿主的成员（等待来源的声明、`Halted`）跳过这一步。
- 等待来源的声明不是失败：没有 `ProgramHalted`，来源一有声明版本就照常校验与装载（§8.6）。任何开始新成员的 `Applied`（首次装载、替换、卸载后再装载），只要新成员所引用的某个来源还没有声明版本，就进入等待，不拉起宿主。
- 没有装载时的 `Reset`：本成员可交回的 `Checkpoint` 总被本成员接受（替换的 `Applied` 比对沿用的，`Output` 检查其后持久化的，§8.6 状态迁移）；`ProgramReset` 只在不沿用的替换 `Applied` 事务里出现。
- 程序流的 epoch 由开始不沿用旧状态之成员的 `Applied` 开出，与该 `Applied` 同一事务，只开在这个新成员的程序流上：让 id 进入活动集合的 `load_program`（`[*]` 与 `Unloaded` 出发的那条）为 `start`，不沿用的替换为 `program_upgrade`；该 id 下第一次被声明的流，这条 gap 无前驱，不论原因是哪一个。每条程序流的 epoch 到该流下一条开 epoch 的 `Gap{Source}` 为止，即同一 id 此后第一个声明该流、不沿用旧状态的成员开始时；开始的成员不声明的流不写记录，epoch 照旧开着。卸载、沿用的替换（要求程序流集合相同）、`Stopped` 与崩溃之后的重新 `Load` 都不开也不结束它。装载开 epoch 而不是 `Reset`，没有 `ProgramReset`（§8.6 程序流的 epoch）。
- 成员结束之后，它已提交的 `EffectRequest` 仍按开始它的 `Applied` 所记事实分派与重派（D4.3）。

核出：无。

## D4.3 `EffectRequest` 分派与重启重派

对照：§6.1（读/写处理器、发出成员、`EffectResponse`、请求与响应的关联是引用）、§8.6 卸载与替换、§3.4 读即观察记录、§7.3 出站请求处理器、§9.2 #21、§7.2 第 4 步。

```mermaid
flowchart TB
  ER[("EffectRequest 记录 @pos（执行 J，该程序的请求流，永存）<br/>member（→ 开始发出成员的 Applied，控制流）· effect_kind · basis · 载荷")]
  REG{"effect_kind 注册为？"}
  ER --> REG
  REG -->|"读处理器"| RD["一次执行：按 §8.2 read 的判定顺序"]
  RD -->|"Answered（含空）/ Refused"| R1["同事务：item 观察记录 × N + 读结论记录，OneShot{origins ∋ Request(pos), request}（观察 J，可压缩）<br/>+ EffectResponse{pos, Concluded(结论)}（执行 J）"]
  RD -->|"Unavailable"| R2["同事务：Gap{Channel}（观察 J）<br/>+ EffectResponse{pos, Unavailable(gap)}"]
  RD -->|"未调用集成"| R3["EffectResponse{pos, NotCalled(reason)}（不支持 / 未确认 / 无会话（含从未有声明）/ 请求不合法 / 来源未登记）"]
  REG -->|"写处理器"| WM["读发出成员的事实：member 所指 Applied 记下的装载 principal 与执行事实输入<br/>（不读当前成员；成员已结束、程序值文件已改或删去照样可读）"]
  WM --> WCON{"意图构造：锚点构造得出？（D5.2）"}
  WCON -->|"否"| WMAL["EffectResponse{pos, NotDrafted(Malformed{reason})}（请求流；不开单，不做作用域判定）"]
  WCON -->|"是"| WSC{"意图的 (来源, 作用域) 在发出成员 Applied 所记的执行事实输入之内？"}
  WSC -->|"否"| WN["EffectResponse{pos, NotDrafted(ScopeNotObserved)}（请求流；不开单）"]
  WSC -->|"是"| WR["同事务 Draft{responsible = 发出成员的装载 principal, basis ∋ pos}<br/>+ SubmitForDecision + EffectResponse{pos, Drafted(ticket)}（请求流）"]
  REG -->|"未注册"| UH["Unhandled：留在日志，无 EffectResponse"]
  subgraph RESTART["重启（§7.2 第 4 步）：fold 出已注册且无 EffectResponse 的 EffectRequest"]
    Q1{"有 EffectResponse{request = pos}？"}
    Q1 -->|"无，读处理器"| RD2["重新执行一次"]
    Q1 -->|"无，写处理器"| WR2["重新开单：同首次分派，按发出成员的事实（发出成员已被替换或卸载亦然）；Draft 与 Drafted 同事务，'有 Draft 无响应'不可达"]
    Q1 -->|"有"| SKIP["不重派（读结论等观察记录是否已被压缩无关）"]
  end
  ER -.-> Q1
  WR2 --> WM
```

读法：请求与响应是引用关系不是事务：请求先持久，完成事实稍后以位置指回它。完成事实与请求同在执行 J、同寿命，重派判定不依赖可压缩的观察记录；读的结果最终恰一条 `EffectResponse`、写至多一张单据。读处理器不自行重试——是否再请求由程序看到结果/gap 后决定。写处理器先构造意图，构造不出即 `Malformed`，构造成功才判定作用域；负责人与作用域都取发出成员的 `Applied` 所记事实（请求的 `member`），所以替换或卸载之前提交、之后才分派或在重启后重派的请求，仍以旧成员的 principal 开单、按旧成员的执行事实输入判定。

核出：见 D4.1。
