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
  J->>C: 程序 cursor 之后有新记录 / frontier 推进
  C->>H: Advance(records, to = cursor')
  Note over H: 解释①：nodes 增量 DAG，cutoff<br/>解释②：rules fold → On / Require / Expire / Emit
  H-->>C: Output{effects, derivations, checkpoint{bytes, state_version}}
  alt Output 超预算（意图速率 / 状态大小）
    C->>DB: 同事务 append ProgramHalted{Budget(kind)}（执行 J）+ ProgramFailed{Budget(kind)}（观察 J）
    C->>H: 提交之后终止宿主进程；OS 确认退出后清除登记
    Note over C: 程序进入失败抑制，本批输出不落
  else 正常
    C->>DB: BEGIN
    C->>DB: append EffectRequest 记录（执行 J）× effects
    C->>DB: append 派生记录（观察 J，程序流）× derivations
    C->>DB: 写 Checkpoint + 程序 cursor = cursor'；登记 cursor 引用
    C->>DB: COMMIT
    Note over C,DB: 崩在 COMMIT 前：整批不存在，重启重放同一批（#16）
    C->>O: 逐条分派 EffectRequest（事务之后）
    alt 读处理器（一次执行，不自行重试）
      alt 未调用集成（按 §8.2 read 的判定顺序：来源未登记 / 从未有声明 / 流未声明 / 最近声明 read 为 Unsupported 或 Unknown / 请求不合 schema / 来源无会话）
        O->>DB: EffectResponse{request: pos, NotCalled(reason)}（不调用集成、不 append 观察记录）
        Note over C: 程序决策半边从自己请求的 EffectResponse 看到它（§6.1）
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
    else 写处理器
      O->>DB: 同事务 Draft{by: 装载 principal, basis ∋ pos} + SubmitForDecision + EffectResponse{Drafted(ticket)}（D5.1）
    else 未注册
      Note over O: Unhandled：记录留在日志，无 EffectResponse，不重派
    end
  end
```

读法：

- 程序看到的只有位置之后的记录与 frontier；它的输出是值（`EffectRequest`、派生记录、`Checkpoint`），不是调用。
- 事务边界在 `COMMIT`：`Emit` 是否"发生"以 `EffectRequest` 记录是否持久为准；处理器执行在其后，通过位置引用与请求关联（D4.3）。
- `fetch.bars`（读）与 `trade.place`（写）对程序是同一构造子；差别在注册表。

核出：读处理器的每种完成结果（含空结果、上游拒绝、`Unavailable`、未调用集成）都需要与请求同寿命的完成事实——已并入 §6.1（`EffectResponse`）；空结果在观察侧由读结论记录表示——已并入 §8.2。

## D4.2 程序生命周期

对照：§8.6 `Load`/`Reset`/`Unload`、程序的活动集合与失败抑制、预算语义、状态迁移、运维冷启动；§8.5 `load_program`/`unload_program`；§4.2 `program_upgrade`；§7.2 生命周期表与受控停止。

```mermaid
stateDiagram-v2
  [*] --> Loading : load_program(manifest_ref, cold_start?) 的 Applied（钉内容 hash，进入活动集合）或 启动第 5 步（活动集合中未被抑制的程序）
  Loading --> Running : Load(program, checkpoint?, budget) → Loaded{state_version}
  Loading --> Halted : 程序值内容 hash 与 Applied 所钉不符 / LoadRejected（Id 越界/环、required_inputs 缺、含 Pooled 而子系统未装）→ 同事务 ProgramHalted{ContentUnavailable 或 LoadRejected(reason)} + ProgramFailed
  state "Reset 处理中" as RST
  Loading --> RST : 核心 Load 前比对：checkpoint 的 state_version 不被程序接受 → 不携带装载，Reset(StateVersionMismatch)；替换时 Reset(Replace)
  Loading --> RST : load_program(…, cold_start = true) → 不携带 checkpoint 装载，Reset(Operator)
  RST --> Running : append ProgramReset；程序流新 epoch Gap{Source, program_upgrade}；按 H9 回填
  Running --> Running : Advance 循环（D4.1）
  Running --> Halted : 超预算 / trap（宿主异常退出）→ 同事务 ProgramHalted + ProgramFailed，提交后终止宿主、OS 确认退出后清除登记
  Running --> Stopped : 受控停止第 2 步 → Unload；活动集合不变，下一实例第 5 步重新 Load
  Running --> Unloaded : unload_program 的 Applied → Unload，离开活动集合，Checkpoint cursor 引用解除（Checkpoint 本身保留）
  Halted --> Loading : load_program 的 Applied 以位置引用 ProgramHalted（重新钉内容 hash；携最近 Checkpoint，状态本身致 trap 时用 cold_start）
  Halted --> Halted : 核心重启：ProgramHalted 未被解除，第 5 步不装载
  Unloaded --> Loading : 再 load_program；替换程序 = unload_program 旧 + load_program 新（携旧 checkpoint；新程序不接受旧版本 → Reset(Replace)）
  Stopped --> [*]
  note right of Running
    核心崩溃：重启后从与 cursor 同事务持久化的
    最近 Checkpoint 重新 Load，重放 cursor 之后的记录，
    不重复 Emit（#16）
  end note
```

读法：

- `Halted` 是失败抑制：它由执行事实 `ProgramHalted` 承载，跨核心重启保持，不自动恢复——超预算是程序作者的问题，由控制面 principal 决定是否重装；其他程序、账户、核心不受影响。`ProgramFailed` 只供展示。
- 活动集合是 `load_program`/`unload_program` 的 `Applied` 的 fold；改装载清单或程序值文件不改变它，内容与所钉 hash 不符时不装载。
- 宿主执行在程序成员与核心实例两者之内：`Stopped`（受控停止）与崩溃都只结束宿主执行，成员与 `Checkpoint` 引用不变。

核出：无。

## D4.3 `EffectRequest` 分派与重启重派

对照：§6.1（读/写处理器、`EffectResponse`、请求与响应的关联是引用）、§3.4 读即观察记录、§7.3 出站请求处理器、§9.2 #21、§7.2 第 4 步。

```mermaid
flowchart TB
  ER[("EffectRequest 记录 @pos（执行 J，永存）<br/>effect_kind · basis · key · 载荷")]
  REG{"effect_kind 注册为？"}
  ER --> REG
  REG -->|"读处理器"| RD["一次执行：按 §8.2 read 的判定顺序"]
  RD -->|"Answered（含空）/ Refused"| R1["同事务：item 观察记录 × N + 读结论记录，OneShot{origins ∋ Request(pos), request}（观察 J，可压缩）<br/>+ EffectResponse{pos, Concluded(结论)}（执行 J）"]
  RD -->|"Unavailable"| R2["同事务：Gap{Channel}（观察 J）<br/>+ EffectResponse{pos, Unavailable(gap)}"]
  RD -->|"未调用集成"| R3["EffectResponse{pos, NotCalled(reason)}（不支持 / 未确认 / 无会话（含从未有声明）/ 请求不合法 / 来源未登记）"]
  REG -->|"写处理器"| WR["同事务 Draft{responsible = 装载 principal, basis ∋ pos}<br/>+ SubmitForDecision + EffectResponse{pos, Drafted(ticket)}"]
  REG -->|"未注册"| UH["Unhandled：留在日志，无 EffectResponse"]
  subgraph RESTART["重启（§7.2 第 4 步）：fold 出已注册且无 EffectResponse 的 EffectRequest"]
    Q1{"有 EffectResponse{request = pos}？"}
    Q1 -->|"无，读处理器"| RD2["重新执行一次"]
    Q1 -->|"无，写处理器"| WR2["重新开单（Draft 与 Drafted 同事务，'有 Draft 无响应'不可达）"]
    Q1 -->|"有"| SKIP["不重派（读结论等观察记录是否已被压缩无关）"]
  end
  ER -.-> Q1
```

读法：请求与响应是引用关系不是事务：请求先持久，完成事实稍后以位置指回它。完成事实与请求同在执行 J、同寿命，重派判定不依赖可压缩的观察记录；读的结果最终恰一条 `EffectResponse`、写至多一张单据。读处理器不自行重试——是否再请求由程序看到结果/gap 后决定。

核出：见 D4.1。
