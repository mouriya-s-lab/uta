# 07 崩溃与恢复

对照：§5.4 崩溃恢复、§6.1 启动第 1–5 步、§6.5 崩溃恢复、§7.2 崩溃矩阵 #1–#21、W2 脑裂变体、W3、W9。索引见 `README.md`。

## D7.1 重启时每条 Attempt 链的恢复判定

对照：§5.4"恢复"、发出前门、转移表；§6.1 第 2/4 步；§7.2 #1–#5、#7、#8。

```mermaid
flowchart TB
  START["第 2 步：对执行 J 每条 Prepared 做 fold（不接触集成）"]
  START --> Q1{"有 SendBarrier？"}
  Q1 -->|"无"| SAFE["确未发出（不变量 §2.5-8）<br/>仍是可安全发送的 Prepared"]
  SAFE --> G{"第 4 步：发出前门<br/>deadline 未过？"}
  G -->|"是"| SEND["durable append SendBarrier → submit（需第 3 步会话）"]
  G -->|"否"| EXP["append Expired(deadline)：终，不误升 Undetermined，不补偿（#2）"]
  Q1 -->|"有"| Q2{"SendBarrier 有后继？"}
  Q2 -->|"无"| UD["第 2 步即 append Undetermined(CrashWindow)（#3/#4/#5）<br/>同事务回查已到达的归因观察"]
  UD --> DRV["第 4 步：启动对账驱动（D6.2），下一渠道由已 append 的 ResolutionEvidence 决定（#7）"]
  Q2 -->|"VenueAccepted / VenueRejected / Expired"| DONE["已终结：无动作"]
  Q2 -->|"Undetermined 且无 Found/Absent"| DRV
  Q2 -->|"Undetermined 且已 Found/Absent"| DONE
  subgraph REPL["复合链腿间（#8）"]
    C1{"撤单腿终态？"}
    C1 -->|"VenueAccepted / Found，无新腿 SendBarrier"| C2["读目标终态观察 → 算量 → 新腿过发出前门（同 SAFE 语义）"]
    C1 -->|"VenueRejected / Absent / Expired"| C3["链已 Resolved：无新腿"]
    C1 -->|"Undetermined"| C4["同 UD"]
  end
  Q1 -.->|"Prepared 是 Replace"| C1
```

读法：判定只用记录，两个二分点（有无 `SendBarrier`、有无后继）把每条链归入"重发 / 过期 / 对账 / 已完"四个桶，没有第五个桶。

核出：无。

## D7.2 崩溃窗口在 W1 时序上的位置

对照：§7.2 #1–#6、#21；D6.5。

```mermaid
sequenceDiagram
  participant H as 程序宿主
  participant C as 核心
  participant DB as SQLite
  participant I as 集成
  participant A as 订阅者
  H-->>C: Advance Output
  Note over C,DB: ✕16 输出持久化前：整批不存在，重放同一批
  C->>DB: COMMIT EffectRequest + 派生 + Checkpoint + cursor
  Note over C,DB: ✕21 EffectRequest 已提交、Draft 未提交：重启重派开单
  C->>DB: Draft + SubmitForDecision
  C->>DB: STS 各步 Outcome + RuleState
  Note over C,DB: ✕1 Prepared + Close(Prepared) 同事务中途：皆无，单据仍 AwaitingDecision
  C->>DB: COMMIT Prepared + Close(Prepared)
  Note over C,DB: ✕2 Prepared 有、SendBarrier 无：确未发出 → 过门后重发或 Expired
  C->>DB: SendBarrier fsync
  Note over C,I: ✕3 SendBarrier 有、submit 未发：可能已发出 → Undetermined(CrashWindow)
  C->>I: submit
  Note over C,I: ✕4 submit 已发、回执未到：同 ✕3；✕14 集成在此崩溃：NoResponse → Undetermined
  I-->>C: Ack
  Note over C,DB: ✕5 回执已到、未 append：同 ✕3，by-key 取证重得同一回执
  C->>DB: COMMIT 回执观察记录 + VenueAccepted
  Note over C,A: ✕6 记录已提交、cursor 未推进：从已确认 cursor 重投，按 LogPosition 去重
  C->>A: 投递
```

读法：每个 ✕ 后的持久状态是 §7.2 对应行的"崩溃后持久状态"列；恢复动作由 D7.1 判定。fixture 验收：✕2 venue 调用 0；✕3/✕4/✕5 venue 调用 ≤ 1。

核出：无。

## D7.3 脑裂 / 孤儿接管（W2 变体、W9、#15）

对照：§6.1 第 1 步（fence、`instance_id`、进程表回收）、第 3 步（会话 epoch）；§6.3.6 边界拒绝；§5.4；W2.4。

```mermaid
sequenceDiagram
  participant OLD as 旧核心
  participant IA as 集成进程 A（旧 epoch）
  participant V as venue
  participant NEW as 新核心
  participant IB as 集成进程 B（新 epoch）
  participant J as 执行 J / 观察 J
  OLD->>J: SendBarrier @p
  OLD->>IA: submit(attempt @p)
  Note over OLD: 旧核心死亡（锁随进程释放）
  IA->>V: 上游下单仍可能送达
  NEW->>J: 取 fence；instance_id += 1（旧会话 epoch 作废）
  NEW->>IA: 按进程表回收：请求退出 → 超时强制终止
  NEW->>J: SendBarrier @p 无后继 → Undetermined(CrashWindow)
  NEW->>IB: 拉起并 handshake(新 epoch)
  V-->>IA: 迟到回执（若 A 尚存）
  IA--xNEW: 旧 epoch 回执在边界丢弃
  par 两条并入路径
    NEW->>IB: query_by_key(key) → Found → ResolutionEvidence ByKey Found
  and
    V-->>IB: 订单状态推送（attribution FromAttempt(p) 或 idempotency_key）
    IB->>J: 观察记录 → 同事务 ResolutionEvidence Attributed Found
  end
  Note over NEW,J: 该意图恰一条 SendBarrier；fixture venue 调用 ≤ 1；不产生双写
```

读法：安全不依赖 A 的回执到达；三条不变量（`SendBarrier` 可能已发出、同 lane 无新 Attempt、对账经 B 独立取证）共同保证。

核出：无。

## D7.4 程序侧崩溃（#16、#21、宿主 trap）

对照：§6.5 崩溃恢复、预算语义；§5.1 请求与响应的关联；§7.2 #16/#21。

```mermaid
flowchart TB
  Q{"崩在哪？"}
  Q -->|"宿主进程异常退出（trap）"| T1["核心终止宿主，append ProgramFailed{Trap}<br/>程序 Failed，等 load_program"]
  Q -->|"核心在 Advance 输出 COMMIT 前"| T2["整批不存在；重启从最近 Checkpoint Load<br/>重放 cursor 之后同一批记录，不重复 Emit（#16）"]
  Q -->|"核心在 COMMIT 后、处理器响应持久化前"| T3["fold 出无响应引用的 EffectRequest（D4.3）<br/>读：重发 read；写：无 Draft 引用则开单（#21）"]
  Q -->|"Load 时 state_version 不符"| T4["Reset(StateVersionMismatch)：ProgramReset 记录<br/>程序流新 epoch Gap{Source, program_upgrade}，按 H9 回填"]
```

读法：程序状态只有一条持久边界（`Checkpoint` 与 cursor 同事务）；一切恢复都从它开始重放。

核出：无。

## D7.5 崩溃矩阵 → 图元素对照

| §7.2 行 | 图 | 落点 |
|---|---|---|
| #1 | D7.2 ✕1、D1.5 | 同事务集合 |
| #2 | D7.1 SAFE→G、D7.2 ✕2 | 发出前门 |
| #3 | D7.1 UD、D7.2 ✕3 | `Undetermined(CrashWindow)` |
| #4 | D7.2 ✕4、D6.2 | 取证循环 |
| #5 | D7.2 ✕5、D6.3 | 回执即观察记录 |
| #6 | D7.2 ✕6、D3.4 | cursor 语义 |
| #7 | D7.1 DRV、D6.2 | 下一渠道由 fold 重建 |
| #8 | D7.1 REPL、D6.4 | 复合链续跑 |
| #9 | D1.5 末行 | 半写不可见 |
| #10 | D2.4 OBS | 派生侧可重算 |
| #11 | D1.4 快照 | 快照仅加速 |
| #12 | D9.4 | 原子替换 |
| #13 | D3.6 S、D3.2 | `Gap{Source}` |
| #14 | D7.2 ✕14、D3.6 U | 写投放侧 |
| #15 | D7.3 | 脑裂 / 孤儿 |
| #16 | D7.4 T2、D4.1 | `Checkpoint` + cursor |
| #17–#19 | — | hpc 子系统，当前阶段不画 |
| #20 | D9.1 | Alice 重连 |
| #21 | D7.4 T3、D4.3 | `EffectRequest` 重派 |
