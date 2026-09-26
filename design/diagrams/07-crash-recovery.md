# 07 崩溃与恢复

对照：§6.7 崩溃恢复、§7.2 启动第 1–5 步、§8.6 崩溃恢复、§9.2 崩溃矩阵 #1–#21、W2 脑裂变体、W3、W9。索引见 `README.md`。

## D7.1 重启时每次尝试的恢复判定

对照：§6.7“恢复”（判定顺序固定：先 fold 等待是否已结束，再看有无 `SendBarrier`）、§6.5 发出前门与转移表、§6.6 放弃跟踪；§7.2 第 2/4 步；§9.2 #1–#5、#7、#8。

```mermaid
flowchart TB
  START["第 2 步：对执行 J 每条 Prepared 做尝试的 fold（不接触集成）"]
  START --> Q0{"等待已结束？<br/>VenueAccepted / VenueRejected / NotSent / 已有 Found 或 Absent / Expired / Abandoned"}
  Q0 -->|"是"| Q4{"Abandoned，且最近一次重开是它之后的 Manual、该轮渠道未穷尽？"}
  Q4 -->|"否"| DONE["无动作（已 Expired 的尝试不再过发出前门；Abandoned 不自动重开）"]
  Q4 -->|"是"| MANR["第 4 步（会话建立后）：续跑该轮，渠道穷尽即停；只补结果，等待仍 Abandoned"]
  Q0 -->|"否"| Q1{"有 SendBarrier？"}
  Q1 -->|"无"| SAFE["确未发出（不变量 §6.9-8）"]
  SAFE --> G{"第 4 步：发出前门<br/>deadline 未过？该集成会话已建立？意图对会话有效能力可执行？"}
  G -->|"三者皆是"| SEND["durable append SendBarrier(p) → 写调用（submit / cancel）"]
  G -->|"deadline 未过，但会话未建立或能力不可执行"| WAIT["不 append 记录，保持 Prepared；会话建立 / 能力变化 / deadline 到时重新求值"]
  WAIT --> G
  G -->|"deadline 已过"| EXP["append Expired(p, deadline)：等待结束，未交出；不误升 Undetermined，不补偿（#2）"]
  Q1 -->|"有"| Q2{"SendBarrier(p) 有后继？"}
  Q2 -->|"无"| UD["第 2 步即 append Undetermined(p, CrashWindow)（#3/#4/#5）<br/>同事务回查已到达、归因到 p 的观察"]
  UD --> DRV["第 4 步（该集成会话已建立后）：启动对账驱动（D6.2）；本轮进度 = round == 当前轮 的 ResolutionEvidence（#7）<br/>停等且等待 Active 者因会话建立自动 append ReconciliationReopened{SessionRestored}"]
  Q2 -->|"Undetermined，等待 Active"| DRV
```

读法：判定只用记录；先问"等待是否已结束"，把已结束的尝试（含只有 `Expired` 的、已 `Abandoned` 的）挡在自动驱动之外；其余归入"重发 / 过期 / 对账"三个桶。"重发"桶在集成会话建立前停在发出前门等待，不产生记录；只有 `deadline` 能把它变成 `Expired`。崩溃前进行中的 `abandon` 若未 append `Abandoned`，日志里没有痕迹，尝试照常落进对账桶（#8）。

核出：无。

## D7.2 崩溃窗口在 W1 时序上的位置

对照：§9.2 #1–#6、#14、#16、#21；D6.5。

```mermaid
sequenceDiagram
  participant H as 程序宿主
  participant C as 核心
  participant DB as SQLite
  participant I as 集成
  participant A as 订阅者
  H-->>C: Advance Output
  Note over C,DB: ✕16 输出持久化前：整批不存在，从同一组已提交 cursor 重新推进
  C->>DB: COMMIT EffectRequest + 派生 + Checkpoint + cursor
  Note over C,DB: ✕21 EffectRequest 已提交、EffectResponse 未提交：重启重派（写：重新开单；读：重新执行一次）
  C->>DB: COMMIT Draft + SubmitForDecision + EffectResponse{Drafted}
  C->>DB: STS 各步 Outcome（带 checked_as_of）；规则状态由记录 fold，不另写
  Note over C,DB: ✕1 Prepared + Close(Prepared) 同事务中途：皆无，单据仍 AwaitingDecision
  C->>DB: COMMIT Prepared + Close(Prepared)
  Note over C,DB: ✕2 Prepared 有、SendBarrier 无：确未发出 → 过发出前门后发送（会话未建立则等待），或 deadline 已过 → Expired
  C->>DB: SendBarrier fsync
  Note over C,I: ✕3 SendBarrier 有、写调用未发：可能已发出 → Undetermined(CrashWindow)
  C->>I: submit
  Note over C,I: ✕4 写调用已发、回执未到：同 ✕3；✕14 集成在此崩溃：会话结束，集成会话恰完成该调用一次 → NoResponse → Undetermined
  I-->>C: Ack 或 NotSent
  Note over C,DB: ✕5 回执或 NotSent 已到、未 append：同 ✕3，by-key 取证重得同一状态（Evidence 落执行 J）；未交出的在唯一期内得 Absent
  alt Ack
    C->>DB: COMMIT VenueAccepted（含 Evidence）+ 该回应的观察记录 + 计数健康观察
  else NotSent
    C->>DB: COMMIT NotSent{reason, evidence} + 计数健康观察（没有交给上游，不写该回应的观察记录）
  end
  Note over C,A: ✕6 记录已提交、cursor 未推进：从已确认 cursor 重投，按 LogPosition 去重
  C->>A: 投递
```

读法：每个 ✕ 后的持久状态是 §9.2 对应行的"崩溃后持久状态"列；恢复动作由 D7.1 判定。fixture 验收：✕2 venue 调用 0；✕3/✕4/✕5 venue 调用 ≤ 1。

核出：无。

## D7.3 脑裂 / 孤儿接管（W2 变体、W9、#15）

对照：§7.2 第 1 步（fence、`instance_id`、进程表回收）、第 3 步（会话 epoch）；§8.3 边界拒绝；§6.6；W2 步 4。

```mermaid
sequenceDiagram
  participant OLD as 旧核心（instance_id = n）
  participant IA as 集成进程 A（epoch (n, k)）
  participant V as venue
  participant NEW as 新核心（instance_id = n+1）
  participant IB as 集成进程 B（epoch (n+1, 1)）
  participant J as 执行 J / 观察 J
  OLD->>J: SendBarrier(p)
  OLD->>IA: submit(attempt p)
  Note over OLD: 旧核心死亡（锁随进程释放）
  IA->>V: 上游下单仍可能送达
  NEW->>J: 取 fence；instance_id = n+1（旧会话 epoch 全部作废）
  NEW->>IA: 按进程表回收：请求退出 → 超时强制终止
  V-->>IA: 迟到回执（A 在回收完成之前仍可能收到）
  IA--xNEW: A 的通道只通向已退出的旧核心，回执不进入新核心
  Note over NEW,IA: OS 确认 A 已退出 → 清除 A 的进程表行；第 1 步到此完成，之后才有第 2 步
  NEW->>J: SendBarrier(p) 无后继 → Undetermined(p, CrashWindow)（第 2 步，不接触集成）
  Note over NEW: 第 3 步：拉起 B 时核心为它的通道分配 SessionEpoch (n+1, 1)（核心内部，集成不知道也不回填）
  NEW->>IB: 拉起并 handshake()
  par 两条并入路径（先到者确立结果；后到的取证结果只 append 作审计、不改结果，后到的归因记录不再 append ResolutionEvidence）
    NEW->>IB: query_by_key(K(p), key_role, scope, barrier_at) → Found → ResolutionEvidence{p, ByKey, Found}
  and
    V-->>IB: 订单状态推送（集成在声明的键作用域与唯一期内填 attribution FromAttempt(p)）
    IB->>J: 观察记录 → p 结果仍未知 → 同事务 ResolutionEvidence{p, Attributed, Found}
  end
  Note over NEW,J: 该意图恰一条 SendBarrier；fixture venue 调用 ≤ 1；不产生双写
```

读法：安全不依赖 A 的回执到达；三条不变量（`SendBarrier` 可能已发出、阻塞头集合非空时同 lane 无新普通写、对账经 B 独立取证）共同保证。同一集成任何时刻至多一个进程：A 的 OS 确认退出与进程表行的清除是第 1 步的结束，之后才有第 2 步的恢复与第 3 步拉起 B；A 在回收完成之前收到的迟到回执只能送进通向旧核心的通道。`SessionEpoch` 由核心在拉起 B 时为通道分配，`handshake()` 不带参数，集成从不知道它。核心重启必然换 `instance_id`；同核心内重连只换 `session_seq`。

核出：无。

## D7.4 程序侧、派生侧与快照崩溃（#10、#11、#16、#21、宿主 trap）

对照：§8.6 崩溃恢复、预算语义；§6.1 `EffectResponse`、请求与响应的关联；§4.1/§4.3 派生可重算；§7.4/§7.5 快照仅加速；§9.2 #10/#11/#16/#21。

```mermaid
flowchart TB
  Q{"崩在哪？"}
  Q -->|"宿主进程异常退出（trap），或 Output 的 checkpoint 版本不在本成员接受的集合内（视同 trap）"| T1["核心同一事务 append ProgramHalted{Trap}（执行 J）+ ProgramFailed{Trap}（观察 J，只供展示）；版本违例的 Output 不落<br/>提交之后终止宿主，OS 确认退出后清除登记；程序 Halted（失败抑制，跨重启保持），等引用该 ProgramHalted 的 load_program"]
  Q -->|"核心在 Advance 输出 COMMIT 前"| T2["整批不存在；重启从最近 Checkpoint Load<br/>从同一组已提交 cursor 重新推进，不重复 Emit（#16）"]
  Q -->|"核心在 COMMIT 后、EffectResponse 持久化前"| T3["fold 出无 EffectResponse 的已注册请求（D4.3）<br/>读：重新执行一次；写：按 member 所指发出成员的 Applied 重新开单，发出成员已被替换或卸载亦然（Draft 与 EffectResponse{Drafted} 同事务，不存在有 Draft 无响应）（#21）"]
  Q -->|"派生 DAG 重算中途（#10）"| T5["核心崩溃：这批 Advance 的输出事务未提交，派生记录、EffectRequest、Checkpoint 与 cursor 一条也没有落<br/>恢复：同 T2，从最近 Checkpoint Load，从已提交的 cursor 重新推进（#16）<br/>只有宿主进程崩溃：trap，同 T1 失败抑制"]
  Q -->|"快照写入中途（#11）"| T6["持久：快照部分写、原记录完整<br/>恢复：半写快照丢弃，从保留边界 fold_state 重建；只增加重启延迟"]
```

读法：程序状态只有一条持久边界（`Checkpoint` 与 cursor 同事务）；一切恢复都从它开始重放。本成员可交回的 `Checkpoint` 总被本成员接受（沿用的由替换的 `Applied` 比对，其后持久化的由 `Output` 检查，§8.6 状态迁移），所以重启 `Load` 不比对版本，也没有装载时的 `Reset` 可以重复。请求的完成事实在执行 J（`EffectResponse`），读结论等观察记录被压缩不影响重派判定。派生记录与 `EffectRequest`、`Checkpoint`、cursor 同批提交，没有提交的批次不留任何记录，所以 DAG 重算中途的崩溃（T5）不留半批；快照不是权威，只加速重建。trap 的顺序与 D4.2 相同：失败抑制的执行事实 `ProgramHalted` 先提交，核心才终止宿主；程序的状态是 `Halted`，`ProgramFailed` 是可压缩的展示观察，不承载状态（§8.6 失败抑制）。

核出：以可压缩观察记录判断"请求已响应"会在压缩后误重派——已并入 §6.1（`EffectResponse`）。

## D7.5 崩溃矩阵 → 图元素对照

| §9.2 行 | 图 | 落点 |
|---|---|---|
| #1 | D7.2 ✕1、D1.5 | 同事务集合 |
| #2 | D7.1 SAFE→G、D7.2 ✕2 | 发出前门 |
| #3 | D7.1 UD、D7.2 ✕3 | `Undetermined(CrashWindow)` |
| #4 | D7.2 ✕4、D6.2 | 取证循环 |
| #5 | D7.2 ✕5、D6.3 | `Evidence` 在执行 J |
| #6 | D7.2 ✕6、D3.4 | cursor 语义 |
| #7 | D7.1 DRV、D6.2 | 下一渠道由 fold 重建 |
| #8 | D7.1 Q0/Q4、D6.4 | `abandon` 没有中间记录：未 append `Abandoned` 则仍 `Active`，落进对账桶 |
| #9 | D1.5（单条观察 append 行） | 半写不可见 |
| #10 | D7.4 T5 | 批次未提交即不存在；核心崩溃同 #16 重放，宿主崩溃是 trap |
| #11 | D7.4 T6 | 半写快照丢弃，fold 重建 |
| #12 | D9.4 | 原子替换 |
| #13 | D3.6 S、D3.2 | `Gap{Source}` |
| #14 | D7.2 ✕14、D3.6 U | 写投放侧 |
| #15 | D7.3 | 脑裂 / 孤儿 |
| #16 | D7.4 T2、D4.1 | `Checkpoint` + cursor |
| #17–#19 | — | hpc 子系统，当前阶段不画 |
| #20 | D9.1 | 下游重连（经解释层） |
| #21 | D7.4 T3、D4.3 | `EffectResponse` 重派判定 |
