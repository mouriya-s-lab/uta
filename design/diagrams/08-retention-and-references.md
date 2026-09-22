# 08 保留边界、引用登记、`basis_validity`

对照：§2.3 保留语义、§4、§6.4 `advance_retention`、§6.7.2 保留边界与引用登记、W13。索引见 `README.md`。

## D8.1 引用登记的生命周期（三种持有者）

对照：§2.3"权责与周期归属"；§6.7.2 引用登记行。

```mermaid
stateDiagram-v2
  state "意图 basis（派生侧位置）" as B {
    state "未登记" as B0
    state "已登记" as B1
    state "已解除" as B2
    [*] --> B0 : Draft / Revise 携带 basis（Drafting / AwaitingDecision 期间）
    B0 --> B1 : Prepared append（核心自动登记）
    B1 --> B2 : 该 Attempt 链 Resolved
    B0 --> B0 : 边界越过 → basis_validity BeyondRetention（单据 Diverged，fail-closed）
  }
  state "程序 Checkpoint 的 cursor" as C {
    state "已登记" as C1
    state "已解除" as C2
    [*] --> C1 : Checkpoint 与 cursor 同事务持久化
    C1 --> C1 : 下一个 Checkpoint 持久化即替换
    C1 --> C2 : Unload（最近 Checkpoint 保留但程序不再消费）
  }
  state "取证 ResolutionEvidence 引用的观察位置" as R {
    state "已登记" as R1
    state "已解除" as R2
    [*] --> R1 : ResolutionEvidence append
    R1 --> R2 : Attempt Resolved
  }
```

读法：

- 登记方是核心、消费者不手工登记；解除随持有者生命周期自动发生。
- 解除后的历史引用仍可读作审计；落到边界之下读得 `BeyondRetention`，不阻止压缩。
- `Undetermined` 停等时，它的 `basis` 与取证引用钉住保留边界；解除随链 `Resolved` 自动发生——任一收敛入口（`resolve`、`Attributed Found`、`ReconciliationReopened` 后的自动取证，D6.2）都行，没有绕过链的旁路。

核出：无。

## D8.2 `advance_retention` 与压缩

对照：§2.3 保留语义、§6.4 控制组、§6.7.1 `compact_below_retention`、§3.1、W13。

```mermaid
flowchart TB
  OP["控制面 principal：advance_retention(to)"] --> AUTH{"(principal, 动作种类) 授权？"}
  AUTH -->|"否"| U["Rejected(Unauthorized)"]
  AUTH -->|"是"| MONO{"to > 当前边界？（边界只前进）"}
  MONO -->|"否"| RJ0["Rejected(NotForward)"]
  MONO -->|"是"| MIN["核心算 min(当前登记引用)"]
  MIN --> CMP{"to ≤ min？"}
  CMP -->|"否"| RJ["Rejected(ReferencedBelow{min})<br/>要越过只能先让持有者终结：关闭单据 / 链 Resolved / unload_program"]
  CMP -->|"是"| WIN{"to ≤ 配置窗口下界？"}
  WIN -->|"否"| RJ2["Rejected(reason)：边界 = min(配置窗口下界, 最早登记引用)"]
  WIN -->|"是"| APPLY["写新边界；控制记录 Applied(position)"]
  APPLY --> COMPACT["compact_below_retention：仅 RetractableDelta 表<br/>执行 J 不压缩、不删除（只可能落到边界下不再精确重建）"]
  COMPACT --> INV["不变量 §2.5-3：所有已登记引用 ≥ 边界"]
```

读法：边界只前进；执行事实永不删除；派生历史的 `DELETE` 只在边界之下。

核出：无。

## D8.3 `basis_validity` 判定

对照：§4 `basis_valid`、默认窗口 `Lag = 0`、执行事实侧引用不因年龄变假；§5.2 门。

```mermaid
flowchart TB
  IN["basis: Set<LogPosition>；操作的 Lag（策略声明，缺省 0）"]
  IN --> EACH["对每个位置（逐位置顺序：边界 → 撤回 → 滞后，后一项以前一项通过为前提，§4）"]
  EACH --> SIDE{"位置在哪侧？"}
  SIDE -->|"执行事实侧（VenueAccepted / SendBarrier / EffectRequest 位置）"| E1{"≥ 保留边界？"}
  E1 -->|"是"| EOK["有效（不因年龄变假）"]
  E1 -->|"否"| BR["BeyondRetention(pos)"]
  SIDE -->|"派生侧（观察）"| D1{"≥ 保留边界？"}
  D1 -->|"否"| BR
  D1 -->|"是"| D2{"该位置的贡献被撤回？"}
  D2 -->|"是"| RT["Retracted(pos)"]
  D2 -->|"否"| D3{"同 epoch 且 Seq ≥ 该流完备位置 − Lag？<br/>完备位置 = 核心最近一次推进该流完备进度时的流末位置；Lag 以 Seq 距离计<br/>Lag = 0：不早于最近一次完备位置；epoch 早于当前 → Stale"}
  D3 -->|"否"| ST["Stale(Lag)"]
  D3 -->|"是"| DOK["有效"]
  EOK --> AGG
  DOK --> AGG
  AGG{"全部有效？"} -->|"是"| FRESH["Fresh"]
  AGG -->|"否"| NOT["取最严重一类作 BasisValidity：BeyondRetention > Retracted > Stale（§4）<br/>门 fail-closed；全部失败位置及原因给审批人"]
  BR --> AGG
  RT --> AGG
  ST --> AGG
```

读法：`Lag = 0`（缺省）时 `Fresh` 是"决定至少看到了所有之前不再变的记录"；策略声明正 `Lag` 时允许落后完备位置至多 `Lag` 个 `Seq`。空 `basis` 空真为 `Fresh`，此时能不能放行由第二层必要项决定（D5.5）。

核出：默认窗口语义已改为"不早于最近一次完备位置"（§4，`Lag = 0` 限定）；逐位置顺序与集合值的严重度取法原文未写——已并入 §4。
