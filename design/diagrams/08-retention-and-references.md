# 08 保留边界、引用登记、`basis_validity`

对照：§2.4 保留语义、§5.2、§8.5 `advance_retention`、§7.5 保留边界与引用登记、W13。索引见 `README.md`。

## D8.1 引用登记的生命周期（三种持有者）

对照：§2.4 登记方与解除时机；§7.5 保留边界与引用登记。

```mermaid
stateDiagram-v2
  state "意图 basis（派生侧位置）" as B {
    state "未登记" as B0
    state "已登记" as B1
    state "已解除" as B2
    [*] --> B0 : Draft / Revise 携带 basis（Drafting / AwaitingDecision 期间）
    B0 --> B1 : Prepared append（核心自动登记）
    B1 --> B2 : 该尝试等待结束（结果确立、Expired、Abandoned）
    B0 --> B0 : 边界越过 → basis_validity BeyondRetention（单据 Diverged，fail-closed）
  }
  state "程序 Checkpoint 的 cursor" as C {
    state "已登记" as C1
    state "已解除" as C2
    [*] --> C1 : 程序进入活动集合后的第一个 Checkpoint 与 cursor 同事务持久化（此前没有登记；登记时可能已在边界之下，照样阻止推进）
    C1 --> C1 : 下一个 Checkpoint 持久化即替换；沿用旧状态的替换原样转给新成员；宿主 Unload、受控停止、失败抑制都不解除
    C1 --> C2 : unload_program 的 Applied，或不沿用旧状态的替换的 Applied（都在宿主 OS 确认退出之后 append；最近 Checkpoint 保留作审计）
  }
  state "取证 ResolutionEvidence 引用的观察位置" as R {
    state "已登记" as R1
    state "已解除" as R2
    [*] --> R1 : 等待仍 Active 时 append 的 ResolutionEvidence
    R1 --> R2 : 该尝试等待结束（结果确立、Expired、Abandoned）
  }
```

读法：

- 登记方是核心、消费者不手工登记；解除随持有者生命周期自动发生。
- 解除后的历史引用仍可读作审计；已解除的引用落到边界之下读得 `BeyondRetention`，不阻止压缩。未解除的引用即使已在边界之下（程序第一个 `Checkpoint` 登记的 cursor 可能如此）也照样阻止边界推进，直到被取代或解除（§2.4 不变量）。
- `Undetermined` 停等时，它的 `basis` 与取证引用钉住保留边界；解除随该尝试等待结束自动发生——任一入口（`Attributed Found`、`ReconciliationReopened` 后的自动取证、`abandon` 之后 IO 壳 append 的 `Abandoned`，D6.2）都行，没有绕过记录的旁路。

核出：无。

## D8.2 `advance_retention` 与压缩

对照：§2.4 保留语义、§8.5 控制组、§7.4 `compact_below_retention`、§7.5 保留边界与引用登记、§4.1、W13。

```mermaid
flowchart TB
  OP["控制面 principal：advance_retention(to: Set&lt;LogPosition&gt;)<br/>每条要推进的观察流一个新边界"] --> AUTH{"(principal, 动作种类) 授权？"}
  AUTH -->|"否"| U["Rejected(Unauthorized)"]
  AUTH -->|"是"| EACH["逐流判定 to(s)；任一流不通过即整体拒绝，无流被推进"]
  EACH --> MONO{"to(s) > 该流当前边界？（边界只前进）"}
  MONO -->|"否"| RJ0["Rejected(NotForward)"]
  MONO -->|"是"| MIN["核心算该流已登记引用的最早位置 min(s)"]
  MIN --> CMP{"to(s) ≤ min(s)？"}
  CMP -->|"否"| RJ["Rejected(ReferencedBelow{min})<br/>要越过只能先让持有者解除：该尝试等待结束（结果确立、Expired、Abandoned）/ 程序推进 checkpoint、unload_program 或不沿用旧状态的替换"]
  CMP -->|"是"| WIN{"to(s) ≤ 该流配置窗口下界？"}
  WIN -->|"否"| RJ2["Rejected(InsideWindow{bound})"]
  WIN -->|"是，且各流都通过"| APPLY["写各流新边界；控制记录 Applied(position)"]
  APPLY --> COMPACT["compact_below_retention：仅观察侧 RetractableDelta 表，逐流按该流保留规则删边界之下，只删不改<br/>一般观察流：边界之下全部删去<br/>健康流：只删被同键后续记录取代的，每键最新一条留作基线<br/>程序流：只删被同一流 epoch 里后续值记录取代的值记录，每个流 epoch 最新一条值记录原样留作基线（撤回部分照留），开 epoch 的 Gap{Source} 留下<br/>执行 J 没有保留边界：不压缩、不删除"]
  COMPACT --> INV["不变量 §6.9-3：边界推进不越过所在观察流已登记引用的最早位置；程序 Checkpoint 的 cursor 引用登记时可能已在边界之下（第一次提交的 cursor 例如为 from 的前一位置或一段被删位置的末位），此后阻止推进直到下一个 Checkpoint 取代它；未登记者得 BeyondRetention"]
```

读法：边界按观察流分别维持、只前进（`LogPosition` 只在同一 `StreamId` 内有序）；执行事实永不删除；派生历史的 `DELETE` 只在各流边界之下，留下的记录一条也不改写。健康流与程序流是状态值的流，按键 / 按流 epoch 留基线，所以任一 `as_of ≥ 边界` 的 fold 与压缩前相等（健康读另要求 `as_of` 的控制流前缀不落后于健康流基线，落后的得 `BeyondRetention`，§2.4、§8.4）；从边界订阅的消费者 cursor 为 `Start{边界}`，每次挂接先收到边界之下留下的记录（前导），第一次覆盖整段前导的确认之前断连或崩溃就再收一遍，确认之后成为 `At`，前导之下被删的段不是它的损失（§4.2 cursor 与确认）；cursor 为 `At` 而落在边界之下的订阅者得到的 `Gap{Delivery, compacted}` 只覆盖被删去的位置（每一段连续被删、且不在未确认缺口里的位置一项），各段之间的基线照常按位置投递（§2.4、§4.2）。程序流的 fold 取最新一条值记录的正贡献，撤回部分只指名被取代的位置，所以基线指名已删记录不影响结果：新 fold、已折入被删记录的订阅者与收到压缩缺口的订阅者，收到基线之后都持有同一个值（§4.1 程序流的 fold）。

核出：边界是逐流的位置集、越过留存窗口的拒绝名 `InsideWindow{bound}`、执行事实侧没有边界，原文未写——已并入 §2.4、§8.5、§7.5。

## D8.3 `basis_validity` 判定

对照：§5.2 `basis_valid`（`BeyondRetention` 只对观察侧位置）、默认窗口 `Lag = 0`、执行事实侧引用不因年龄变假；§6.2 门。

```mermaid
flowchart TB
  IN["basis: Set<LogPosition>；操作的 Lag（策略声明，缺省 0）"]
  IN --> EACH["对每个位置（逐位置顺序：边界 → 撤回 → 滞后，后一项以前一项通过为前提，§5.2）"]
  EACH --> SIDE{"位置在哪侧？"}
  SIDE -->|"执行事实侧（VenueAccepted / SendBarrier / EffectRequest 位置）"| EOK["有效（执行事实侧没有保留边界，不因年龄变假）"]
  SIDE -->|"派生侧（观察，含归因观察）"| D1{"≥ 该流保留边界？"}
  D1 -->|"否"| BR["BeyondRetention(pos)"]
  D1 -->|"是"| D2{"该位置的贡献被撤回？"}
  D2 -->|"是"| RT["Retracted(pos)"]
  D2 -->|"否"| D3{"同 epoch 且依据之后该流已提交的记录数 ≤ Lag？<br/>Lag 以同一 StreamId 上的 Seq 距离计；只比较核心自己的位置<br/>Lag = 0：依据位置就是该流当前流末；epoch 早于当前 → Stale"}
  D3 -->|"否"| ST["Stale(Lag)"]
  D3 -->|"是"| DOK["有效"]
  EOK --> AGG
  DOK --> AGG
  AGG{"全部有效？"} -->|"是"| FRESH["Fresh"]
  AGG -->|"否"| NOT["取最严重一类作 BasisValidity：BeyondRetention > Retracted > Stale（§5.2）<br/>门 fail-closed；全部失败位置及原因给审批人"]
  BR --> AGG
  RT --> AGG
  ST --> AGG
```

读法：`Lag = 0`（缺省）时 `Fresh` 是"决定看到了核心此刻已有的该流全部记录"；策略声明正 `Lag` 时允许依据之后该流再提交至多 `Lag` 条记录。判定不读完备进度，也不读收到时间：完备是来源的性质，本门不用它（§5.2）。空 `basis` 空真为 `Fresh`，此时能不能放行由第二层必要项决定（D5.5）。

核出：默认窗口语义为"依据位置就是当前流末"（§5.2，`Lag = 0` 限定）；逐位置顺序与集合值的严重度取法原文未写——已并入 §5.2。
