# 04 程序宿主：`Advance` 循环、生命周期、`EffectRequest` 分派

对照：§4.3、§6.1、§8.6、§7.5 程序状态、W10、W17、§9.2 #16/#21。索引见 `README.md`。

## D4.1 一轮 `Advance`

对照：§8.6 宿主协议、§4.3 输入 = 位置推进、§6.1 出口、§7.5 `Checkpoint` 与 cursor 同事务、§4.2 程序订阅。

```mermaid
sequenceDiagram
  participant J as 观察 Journal
  participant DL as 投递调度
  participant C as 核心（程序宿主元素 + 持久订阅的程序订阅）
  participant H as 程序宿主进程
  participant DB as SQLite（同一事务）
  participant O as 出站请求处理器
  participant I as 集成
  J->>DL: 程序 cursor 之后有新记录
  C->>DL: 取程序订阅的投递事件（cursor 之后的记录、未确认 Gap{Delivery}、覆盖推进）
  DL-->>C: 投递事件
  C->>H: Advance(events, to = cursor')：每条流上按位置先后的投递事件——cursor 为 Start{from} 的流先交出前导（该流此刻在 from 之下留下的记录）、程序订阅上未确认的 Gap{Delivery}{流, from, to, reason}（排在该流 to 之后的记录之前）、各输入流 cursor 之后的记录、await-all 输入的覆盖推进{流, through}（每次 Load 之后的第一次 Advance 先在每个 await-all 输入的起始位置给出 through：At{p} 为 fold 到 p 为止，Start{from} 为 from 之下的严格前缀，排在前导与第一条不低于起始位置的记录之前；每个 Gap{Delivery} 之后给出到它末位为止的 through；每条 through 只在它 fold 到的位置在当前流 epoch 里、且不低于该 epoch 覆盖检查点的 folded_below 前一位置时给出，恰在该位置时就是检查点的 through，所以起始位置低于 folded_below 时没有初始事件，末位更低的开头缺口之后也没有，由此后第一条可算的 through 取代初始事件；不存进 Checkpoint）；cursor' 不确认任何未交出的留存记录或缺口：At 流不越过这批交出的最后一个位置，Start{from} 流取交出的最后一个位置与 from 前一位置中较大者
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
    C->>DB: append 派生记录（观察 J：outputs 里每项所指节点的值按 §4.3 写在程序流 (Program(id), name) 上：本流 epoch 首值一条正贡献，值变一条撤回前一贡献并加入新值，值相等不写；每条 basis = 本事务提交的输入 cursor；未声明为输出的节点不落流）× 值有变化的输出
    C->>DB: 写 Checkpoint（它自己的表）；持久订阅元素写程序订阅上的程序 cursor = cursor'，同一次写删去 cursor' 覆盖的 Gap{Delivery}（cursor' 不确认未交出的缺口，所以这些都是这批交出过的）；登记 cursor 引用
    C->>DB: COMMIT
    Note over C,DB: 崩在 COMMIT 前：整批不存在，重启从同一组已提交 cursor 重新推进（#16）
    C->>O: 逐条分派 EffectRequest（事务之后）
    alt 读处理器（一次执行，不自行重试）
      alt 未调用集成（按 §8.2 read 的判定顺序：来源未登记或不是集成来源 / 从未有声明 / 来源无会话 / 流不在会话有效声明里 / 会话有效声明的 read 为 Unsupported 或 Unknown / 请求不合 schema）
        O->>DB: EffectResponse{request: pos, NotCalled(reason)}（不调用集成、不 append 观察记录）
        Note over C: 程序的解释②在自己的请求流上看到这条 EffectResponse（§6.1）
      else 调用
        O->>I: read(stream, request, range)
        alt Answered（含空）或 Refused
          I-->>O: Answered(items) / Refused(reason)
          O->>DB: 同事务：item 观察记录 × N（one_shot）+ 读结论记录，provenance OneShot{origins ∋ Request(pos), request}（观察 J）+ EffectResponse{Concluded(结论)}（执行 J）
          J->>DL: 程序 cursor 之后的新记录 → 经投递调度交给下一轮 Advance（闭环走观察侧）
        else Unavailable
          I-->>O: Unavailable
          O->>DB: 同事务：Gap{Channel}（观察 J，该流）+ EffectResponse{Unavailable(gap)}
          J->>DL: 这条 gap 经投递调度交给下一轮 Advance，程序看到它，自行决定是否再 Emit
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

- 程序看到的只有 `Advance` 交出的投递事件：cursor 之后的记录（cursor 为 `Start{from}` 的流先是前导）、程序订阅上未确认的投递缺口与 `await-all` 输入的覆盖推进；提交的 `Advance` 只确认它交出的这些，不确认任何未交出的留存记录或缺口。它的输出是值（`EffectRequest`、派生记录、`Checkpoint`），不是调用。
- 事务边界在 `COMMIT`：`Emit` 是否"发生"以 `EffectRequest` 记录是否持久为准；处理器执行在其后，通过位置引用与请求关联（D4.3）。
- `fetch.bars`（读）与 `trade.place`（写）对程序是同一构造子；差别在注册表。

核出：读处理器的每种完成结果（含空结果、上游拒绝、`Unavailable`、未调用集成）都需要与请求同寿命的完成事实——已并入 §6.1（`EffectResponse`）；空结果在观察侧由读结论记录表示——已并入 §8.2。

## D4.2 程序生命周期

对照：§8.6 装载期校验、`Load`/`Reset`/`Unload`、程序的活动集合与失败抑制、卸载与替换、程序流的 epoch、预算语义、状态迁移、运维冷启动；§8.5 `load_program`/`unload_program`；§8.7 原生 op 的执行；§6.1 发出成员；§4.2 `start`、`program_upgrade`；§7.2 生命周期表与受控停止。

```mermaid
stateDiagram-v2
  state "等待所引用集成来源的声明版本" as WAIT
  state "结束宿主执行中（装载中的成员先等已发出装载步骤的结论）" as DRAIN
  state "装载中：有最终被拒的项即失败（不读文件）；否则核对程序值与 Applied 所钉的内容 hash，再做声明校验，成立后核对所引用原生 op 的制品 hash，再把核对过的内容交给子系统，被接受才拉起宿主、以同一份值 Load" as Loading
  [*] --> Loading : load_program 读到的程序值通过结构校验（不通过只得控制记录 Rejected，没有 Applied，不进入本图）之后 append 的 Applied（id 不在活动集合里：以值记下成员事实：装载 principal、内容 hash、预算、接受的 state_version 集合、facts 声明的执行事实输入、值树引用的原生 op 名集合、输出契约（outputs 各项的 (name, 值类型)）；同事务建立程序订阅（按各输入声明的起点的全部 cursor：Tail 为 At，Origin 为 Start{from}；与观察输入的订阅项），并为新成员的每条程序流开新 epoch：Gap{Source, start}）/ 启动第 5 步（活动集合中未被抑制的程序；不开 epoch）；按“等待的先后”：有观察项初次接纳已被最终拒绝，或所引用的集成来源都已有声明版本
  [*] --> WAIT : 同上，但没有最终被拒的项，而所引用的某个集成来源还没有任何声明版本：不拉起宿主、不 ProgramHalted，留在活动集合里
  WAIT --> Loading : 该集成来源第一次握手成功、append 声明版本，待接纳的项随之转为接纳或被拒；有项被拒（最终的拒绝），或所引用的集成来源都已有声明版本
  WAIT --> Unloaded : unload_program 的 Applied（没有宿主，跳过第 1–3 步，直接 append）
  WAIT --> Loading : 替换的 Applied（没有宿主，跳过第 1–3 步，直接 append；沿用规则同 DRAIN→Loading），新成员有最终被拒的项或所引用的集成来源都已有声明版本
  WAIT --> WAIT : 替换的 Applied（同上），但新成员没有最终被拒的项，所引用的某个集成来源还没有声明版本：新成员等待，不拉起宿主
  Loading --> Running : hash 相符、声明校验成立、所引用原生 op 的制品与其安装 Applied 所记 hash 相符，核对过的内容交给子系统并被接受（只为这次宿主执行）→ Load(program, checkpoint?, budget) → Loaded{state_version}；program 就是核对过、校验过的那一份值；checkpoint 是本成员可交回的（Applied 沿用的，或其后持久化的最近一个），其 state_version 总在本成员接受的集合内，Load 不比对；宿主进程的解释器不再校验
  Loading --> Halted : 依次判定：有观察项初次接纳已被最终拒绝（如来源未登记或 QuotaExceeded，即使另有来源还没有声明版本）→ LoadRejected，不读程序值文件；否则程序值内容 hash 与 Applied 所钉不符（load_program 生效时即结构校验读到的那一份，启动第 5 步与等待之后从文件读）→ ContentUnavailable，不做声明校验；否则声明校验不成立（各输入声明与所引用集成来源的最近声明不符（流、字段与类型标签、await-all 的覆盖证据）；执行事实输入的作用域不成立；含 Pooled 或原生 op 而本实例没有子系统、原生 op 引用所指名的 op 不在已安装 op 集合里或所声明的签名与之不符、或前置条件不满足）→ LoadRejected(reason)；否则某个所引用原生 op 的制品文件读不到或与其安装 Applied 所记 hash 不符 → NativeArtifactUnavailable（不是 Trap，宿主未拉起）；否则核对过的内容交给子系统而未被接受（子系统不在、联系不上或拒绝）→ LoadRejected(NativeHandoverFailed)（宿主未拉起）；否则拉起宿主时 OS 没有给出进程 → LoadRejected(HostSpawnFailed)（不是 Trap，没有宿主可终止，这次宿主执行与交出随之结束）；都是同事务 ProgramHalted{…} + ProgramFailed。受控停止第 1 步之前已发出的步骤（未回答的交出、在进行的核对或声明校验）在实例结束锚点之前得出这些失败的，同样走这条转移：停止不吞掉装载失败
  Running --> Running : Advance 循环（D4.1）
  Running --> Halted : 超预算 / Output 的 checkpoint 版本不在本成员接受的集合内（视同 trap）/ trap（宿主异常退出）→ Output 不落，同事务 ProgramHalted + ProgramFailed，提交后终止宿主、OS 确认退出后清除登记
  Running --> Stopped : 受控停止第 2 步 → Unload；受控停止本身不改变成员（停止之前已在执行的卸载或替换照常完成，走 DRAIN 的停止转移），cursor 与引用不变，下一实例第 5 步重新 Load
  Loading --> Stopped : 受控停止第 1 步：不再开始新的装载步骤，已发出的步骤在实例结束锚点之前等到结论（不挡第 2–4 步）；交出已被接受（含停止之后才到的接受）而宿主还没有拉起的不再拉起，这次宿主执行就此结束（核心确认）；已发出的步骤成立而下一步尚未开始的不再往下走；受控停止本身不 append ProgramHalted（已发出的步骤得出失败的走 Loading → Halted），也不改变成员，cursor 与引用不变，下一实例第 5 步重新判定装载。第 2–4 步都已完成时交出仍没有回答的不走本转移：停止以失败报告（不写结束锚点、不释放 fence、不因没有回答而 append ProgramHalted，核心不替子系统判定超时），实例被外力结束之后，下一实例第 5 步照常判定装载
  Running --> DRAIN : unload_program，或替换（对该 id 再 load_program）生效：停止调度 Advance，等在途输出事务提交或确知不提交，Unload，OS 确认退出、清除行；此时还没有 Applied
  Loading --> DRAIN : unload_program，或替换（对该 id 再 load_program）生效时装载步骤已发出（交出还没有回答，或核对、声明校验在进行）：不再开始新的装载步骤，等每个已发出步骤的结论，核心不设超时；交出已被接受而宿主还没有拉起的不再拉起，这次宿主执行就此结束（核心确认）；交出未被接受或别的装载失败照常同事务 ProgramHalted + ProgramFailed，在 Applied 之前；已发出的步骤成立而下一步尚未开始的不再往下走；宿主已拉起（Load 已发出）的，照 Running → DRAIN 结束它。此时还没有 Applied，控制动作尚未完成，到后续 Applied 提交之后才完成
  DRAIN --> Unloaded : 然后 append unload_program 的 Applied：离开活动集合，程序订阅（全部 cursor 与观察输入的订阅项）结束，Checkpoint cursor 引用解除（Checkpoint 只作记录保留）；程序流不写记录，epoch 不结束
  DRAIN --> Loading : 然后 append 替换的 Applied（新程序值已通过结构校验）：同一条结束旧成员、开始新成员，以值记下新成员的输出契约；旧成员在“结束宿主执行中”得出装载失败的，这条 Applied 另以位置引用那条 ProgramHalted（同 Halted → Loading）；程序订阅留下，principal 换成新成员的装载 principal。沿用（非 cold_start、开始旧成员的 Applied 所记的输出契约与新成员的相同，且无旧 Checkpoint 或新程序接受其 state_version）：共有输入的 cursor 与引用原样沿用，订阅项只在主体集与用途也相同、且不是被拒的项时沿用、否则同事务结束旧项并建立与接纳新项（被拒的项从不沿用，任何替换都重新接纳它）；执行事实流按流：新旧 facts 都选中的流 cursor 沿用，只有新成员选中的按其 FactDecl 的起点建立，只有旧成员选中的结束；未确认的投递缺口随 cursor 沿用，Applied 记下沿用的 Checkpoint，程序流接着原 epoch；不沿用：同事务 ProgramReset{cold_start 为 Operator，否则 Replace}、新成员的每条程序流开新 epoch（Gap{Source, program_upgrade}）、项与 cursor 按起点重建（缺口随重建的 cursor 删除）、旧引用解除。新成员有最终被拒的项，或所引用的集成来源都已有声明版本
  DRAIN --> WAIT : 同上，但新成员没有最终被拒的项，所引用的某个集成来源还没有声明版本：不拉起宿主
  DRAIN --> Unloaded : 受控停止开始时正在进行的 unload_program 照常完成：已发出的装载步骤照停止第 1 步等到结论，在途 Advance 在停止第 2 步提交或确知不提交、然后 Unload，宿主 OS 确认退出之后、实例结束锚点之前 append unload_program 的 Applied（效果同上文 DRAIN → Unloaded 那一条）
  DRAIN --> Stopped : 受控停止开始时正在进行的替换照常完成：内层结论同上等到、宿主 OS 确认退出之后，在实例结束锚点之前 append 替换的 Applied（效果同 DRAIN → Loading，新成员的事实照常记下），但停止中不为新成员开始任何装载步骤、也不判定是否等待；下一实例第 5 步照常判定装载新成员
  note right of DRAIN
    受控停止中，停止之前已发出的交出始终没有回答时：
    停止以失败报告，卸载或替换都没有 Applied，
    不走上面两条停止转移；这个控制动作随实例结束、不生效，
    旧成员仍在活动集合里，继任实例第 5 步照常判定装载
  end note
  Halted --> Loading : 替换的 Applied（宿主已 OS 确认退出之后，跳过第 1–3 步），以位置引用 ProgramHalted（记下新成员事实；沿用规则同上，被拒的项在这里重新接纳，所以腾出配额或采纳来源之后以同一程序值重新装载即可；状态本身致 trap 时用 cold_start），新成员有最终被拒的项或所引用的集成来源都已有声明版本
  Halted --> WAIT : 同上，但新成员没有最终被拒的项，所引用的某个集成来源还没有声明版本
  Halted --> Halted : 核心重启：ProgramHalted 未被解除，第 5 步不装载
  Halted --> Unloaded : unload_program 的 Applied（宿主已 OS 确认退出之后）：程序订阅结束；程序流 epoch 不结束
  Unloaded --> Loading : 再 load_program（通过结构校验）：新成员与新的程序订阅，cursor 在其 Applied 同事务按起点建立，不交回旧 Checkpoint；同事务新成员的每条程序流开新 epoch：Gap{Source, start}；该 id 仍在失败抑制中时（卸载之前的 ProgramHalted 未被解除，含在“结束宿主执行中”得出的装载失败），这条 Applied 另以位置引用那条 ProgramHalted、解除抑制
  Unloaded --> WAIT : 同上，但新成员没有最终被拒的项，所引用的某个集成来源还没有声明版本
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
- 宿主执行在程序成员与核心实例两者之内：受控停止本身与崩溃都只结束宿主执行，不改变成员，cursor 与 `Checkpoint` 引用不变；`unload_program` 与替换先经“结束宿主执行中”结束它，再写结束成员的 `Applied`。装载步骤已发出的成员（`Loading → DRAIN`）在这里先等每个已发出步骤的结论、不开始新的步骤：交出被接受而宿主还没有拉起的不再拉起，这次宿主执行由核心确认结束；得出装载失败的，`ProgramHalted` + `ProgramFailed` 在 `Applied` 之前，替换的 `Applied` 以位置引用它（§8.6 装载中的成员）。没有宿主、也没有已发出装载步骤的成员（等待所引用集成来源的声明版本、`Halted`）跳过这一步。受控停止开始时已在“结束宿主执行中”的卸载或替换照常完成，`Applied` 在实例结束锚点之前提交（停止下的 `DRAIN → Unloaded`、`DRAIN → Stopped`）；替换的新成员在停止中不开始任何装载步骤，由下一实例第 5 步判定；交出始终没有回答的，停止失败，这个动作没有 `Applied`、不生效（§7.2 受控停止、§8.6 卸载与替换）。
- 装载期校验分两段（§8.6 装载期校验）：结构校验只看程序值，在 `load_program` 读到它时、任何 `Applied` 之前由程序宿主元素做，不成立只有控制记录 `Rejected`（输入来源是 `Program(_)`、两个输入同在一条流上、`facts` 里同一来源的起点不同即在此被拒），本图没有这个成员；声明校验在 `Applied` 之后、`Load` 之前做（“装载中”），原生 op 按控制流上的已安装 op 集合核对（§8.7 原生 op 的安装）。等待声明版本不是失败：没有 `ProgramHalted`。先后是：有观察项初次接纳已被最终拒绝（它以“被拒”留在程序订阅里，重启后结论不变）即失败，不等，也不读程序值文件；否则某个所引用的集成来源（含只被 `facts` 引用的来源）在采纳集合里而还没有声明版本就等待，不拉起宿主（本图各转移里的“所引用的集成来源还没有声明版本”都指这种来源）；都不是才核对程序值的内容 hash（不符即 `ContentUnavailable`），再对这一份值做声明校验；成立之后按各 op 的安装 `Applied` 重读、核对所引用原生 op 的制品（不符即 `NativeArtifactUnavailable`，它在任何代码执行之前，不是 trap；§8.7 原生 op 的执行），都相符才把核对过的制品内容交给子系统，只为这次宿主执行（交出未被接受即 `LoadRejected(NativeHandoverFailed)`，同样不拉起宿主），被接受才拉起宿主（OS 没有给出进程即 `LoadRejected(HostSpawnFailed)`，没有代码运行过，不是 trap；§8.6 失败抑制），以这一份程序值 `Load`。任何开始新成员的 `Applied`（首次装载、替换、卸载后再装载）以及启动第 5 步的每次装载都按这一先后。程序订阅跨替换留下，由 `unload_program` 的 `Applied` 结束。
- 没有装载时的 `Reset`：本成员可交回的 `Checkpoint` 总被本成员接受（替换的 `Applied` 比对沿用的，`Output` 检查其后持久化的，§8.6 状态迁移）；`ProgramReset` 只在不沿用的替换 `Applied` 事务里出现。
- 程序流的 epoch 由开始不沿用旧状态之成员的 `Applied` 开出，与该 `Applied` 同一事务，只开在这个新成员的程序流上：让 id 进入活动集合的 `load_program`（`[*]` 与 `Unloaded` 出发的那条）为 `start`，不沿用的替换为 `program_upgrade`；该 id 下第一次被声明的流，这条 gap 无前驱，不论原因是哪一个。每条程序流的 epoch 到该流下一条开 epoch 的 `Gap{Source}` 为止，即同一 id 此后第一个声明该流、不沿用旧状态的成员开始时；开始的成员不声明的流不写记录，epoch 照旧开着。卸载、沿用的替换（要求输出契约相同）、`Stopped` 与崩溃之后的重新 `Load` 都不开也不结束它。装载开 epoch 而不是 `Reset`，没有 `ProgramReset`（§8.6 程序流的 epoch）。
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
  RD -->|"未调用集成"| R3["EffectResponse{pos, NotCalled(reason)}（不支持 / 未确认 / 无会话（含从未有声明）/ 请求不合法 / 来源未登记或不是集成来源）"]
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
