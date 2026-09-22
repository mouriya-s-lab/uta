# 04 程序宿主：`Advance` 循环、生命周期、`EffectRequest` 分派

对照：§3.3、§5.1、§6.5、§6.7.2 程序状态、W10、W17、§7.2 #16/#21。索引见 `README.md`。

## D4.1 一轮 `Advance`

对照：§6.5 宿主协议、§3.3 输入 = 位置推进、§5.1 出口、§6.7.2 `Checkpoint` 与 cursor 同事务、§3.2 程序订阅。

```mermaid
sequenceDiagram
  participant J as 观察 Journal
  participant C as 核心（程序订阅 + 宿主协议）
  participant H as 程序宿主进程
  participant DB as SQLite（同一事务）
  participant O as 出站请求处理器
  participant I as 集成
  J->>C: 程序 cursor 之后有新记录 / frontier 推进
  C->>H: Advance(records, to = cursor')
  Note over H: 解释①：nodes 增量 DAG，cutoff<br/>解释②：rules fold → On / Require / Expire / Emit
  H-->>C: Output{effects, derivations, checkpoint{bytes, state_version}}
  alt Output 超预算（意图速率 / 状态大小）
    C->>H: 终止宿主进程
    C->>J: append ProgramFailed{Budget(kind)}；程序 Failed
  else 正常
    C->>DB: BEGIN
    C->>DB: append EffectRequest 记录（执行 J）× effects
    C->>DB: append 派生记录（观察 J，程序流）× derivations
    C->>DB: 写 Checkpoint + 程序 cursor = cursor'；登记 cursor 引用
    C->>DB: COMMIT
    Note over C,DB: 崩在 COMMIT 前：整批不存在，重启重放同一批（#16）
    C->>O: 逐条分派 EffectRequest（事务之后）
    alt 读处理器
      O->>I: read(stream, selector, range)
      I-->>O: Records 或 Unavailable
      O->>J: append 观察记录 provenance OneShot{Request(pos)}, one_shot（或 Gap{Channel}）
      J->>C: 程序按 cursor 看到它 → 下一轮 Advance（闭环走观察侧）
    else 写处理器
      O->>DB: 同事务 Draft{by: 装载 principal, basis ∋ pos} + SubmitForDecision（D5.1）
    else 未注册
      Note over O: Unhandled：记录留在日志，不重派
    end
  end
```

读法：

- 程序看到的只有位置之后的记录与 frontier；它的输出是值（`EffectRequest`、派生记录、`Checkpoint`），不是调用。
- 事务边界在 `COMMIT`：`Emit` 是否"发生"以 `EffectRequest` 记录是否持久为准；处理器执行在其后，通过位置引用与请求关联（D4.3）。
- `fetch.bars`（读）与 `trade.place`（写）对程序是同一构造子；差别在注册表。

核出：无（#21 的窗口已在上一轮并入 §5.1/§7.2）。

## D4.2 程序生命周期

对照：§6.5 `Load`/`Reset`/`Unload`、预算语义、状态迁移；§6.4 `load_program`/`unload_program`；§3.2 `program_upgrade`。

```mermaid
stateDiagram-v2
  [*] --> Loading : load_program(manifest_ref) 或 启动第 5 步
  Loading --> Running : Load(program, checkpoint?, budget) → Loaded{state_version}
  Loading --> Rejected : LoadRejected（Id 越界/环、required_inputs 缺、含 Pooled 而子系统未装、state_version 不符且不接受 Reset）
  Loading --> ResetThenRunning : state_version 不符 → Reset(StateVersionMismatch)
  ResetThenRunning --> Running : append ProgramReset；程序流新 epoch Gap{Source, program_upgrade}；按 H9 回填
  Running --> Running : Advance 循环（D4.1）
  Running --> Failed : 超预算 / trap（宿主异常退出） → 终止宿主，append ProgramFailed
  Running --> Unloaded : unload_program → Unload，清除进程登记，最近 Checkpoint 保留
  Failed --> Loading : load_program 重新装载（携最近 Checkpoint）
  Unloaded --> Loading : 替换程序 = Unload 旧 + Load 新（携旧 checkpoint；新程序不接受旧版本 → Reset(Replace)）
  Rejected --> [*]
  note right of Running
    核心崩溃：重启后从与 cursor 同事务持久化的
    最近 Checkpoint 重新 Load，重放 cursor 之后的记录，
    不重复 Emit（#16）
  end note
```

读法：`Failed` 不自动恢复——超预算是程序作者的问题，由控制面 principal 决定是否重装；其他程序、账户、核心不受影响。

核出：无。

## D4.3 `EffectRequest` 分派与重启重派

对照：§5.1（读/写处理器、请求与响应的关联是引用）、§6.2 出站请求处理器、§7.2 #21、§6.1 第 4 步。

```mermaid
flowchart TB
  ER[("EffectRequest 记录 @pos<br/>effect_kind · basis · key · 载荷")]
  REG{"effect_kind 注册为？"}
  ER --> REG
  REG -->|"读处理器"| RD["立即执行：read(...)<br/>结果 → 观察记录 provenance OneShot{Request(pos)}"]
  REG -->|"写处理器"| WR["同事务 Draft{responsible = 装载 principal, basis ∋ pos}<br/>+ SubmitForDecision"]
  REG -->|"未注册"| UH["Unhandled：留在日志"]
  RD --> OBS[("观察 J")]
  WR --> EJ[("执行 J：TicketAction")]
  subgraph RESTART["重启（§6.1 第 4 步）：fold 出无响应引用的 EffectRequest"]
    Q1{"有观察记录 provenance 引用 pos？"}
    Q2{"有 Draft.basis 引用 pos？"}
    Q1 -->|"无，且是读处理器"| RD2["重发 read（读可重试）"]
    Q2 -->|"无，且是写处理器"| WR2["开单"]
    Q2 -->|"有"| SKIP["不再开单"]
    Q1 -->|"有"| SKIP2["不重发"]
  end
  ER -.-> Q1
  ER -.-> Q2
```

读法：请求与响应是引用关系不是事务：请求先持久，响应稍后以位置指回它。重派的判定是纯 fold，读的结果最终恰一条、写至多一张单据。

核出：无。
