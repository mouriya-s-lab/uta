# 6 效应宇宙

效应宇宙是可写的一半：形成意图、经决策代数放行、受控执行一次、用证据确认。它的机制都为写操作存在（§3.2）；效应侧对观察侧的引用只经 `basis`（§5）。

## 6.1 `EffectRequest`、处理器、程序解释②

> 图：D4.3 `EffectRequest` 分派与重派、D5.2 意图来源（`design/diagrams/04-program-host.md`、`05-ticket-and-sts.md`）。

程序的决策半边是**解释②（决策）**：`rules` → 对日志的 fold。其纯性由数据结构本身保证，而非开发约定。程序作为共享抽象见 §4.3。[证据：fp-01 M7 Mercury Workflow]

```rust
enum DecisionStep { On(Pattern, Box<DecisionStep>), Emit(EffectRequest), Require(Guard, OnFail), Expire(Deadline, Box<DecisionStep>) }
Emit(EffectRequest { effect_kind: EffectKind, payload: Bytes, basis: Basis, key: Option<Key> })
```

### 规则

程序需要请求核心不认识的副作用：发通知、拉一次历史 K 线、调外部模型、下单。程序的唯一出口是一个 `EffectRequest`，不是对每种副作用各加一个构造子。

**请求是值，不是调用**（`IO a` 的纪律，§6.5）。它被 append 为记录；核心不解释 `payload`。

**响应由处理器决定。** 注册了该 `EffectKind` 的处理器接手。未注册则记录留在日志里为 `Unhandled`：有请求无处理器不是错误，与字段无处理器不触发同理（§2.1）。

**处理器在注册时声明读 / 写。**

**读处理器：**

- 立即执行一次核心→集成的 `read`（§8.2）。
- 结果作为观察记录 append：带 `LogPosition`，`provenance: OneShot{origin: Request(该 EffectRequest 记录的 LogPosition)}`（§3.4）。
- 程序按位置推进看到它：闭环走观察侧。
- `Unavailable` → 观察侧 `Gap{origin: Channel}`。流未声明或能力不支持 → 不调用集成。
- 读处理器**不自行重试**：一条请求一次执行，是否再请求由程序看到结果 / gap 后决定。读可重试的主体是发起者（§3.4）。

**写处理器：**

- 请求被当作 Intent，进入单据 → STS → IO 壳的完整效应路径；结果是执行事实与决议记录。
- **负责人**：写处理器以程序的**装载 principal**（装载该程序的人或服务账户）为 `responsible` 开单（§6.2）。程序本身不是 principal。
- 该 principal 的授权范围决定单据能否不经人工直接放行（授权步，§6.3）。
- 开单的 `Draft.basis` 含该 `EffectRequest` 记录的位置：执行事实侧位置作因果依据（§5.1）。
- 程序意图没有编辑期：写处理器在同一事务 `Draft` 并 `SubmitForDecision`，单据直接进入 `AwaitingDecision`。
- 之后的退回、改写、移交由装载 principal 经单据操作进行（§8.5），与人起的单据无异。

**完成事实在执行事实侧** [设计]。

- 每条被处理的 `EffectRequest` 恰有一条执行事实记录 `EffectResponse{request: LogPosition, outcome}`。
- `outcome ∈ {Observed(observation: LogPosition), Unavailable(gap: LogPosition), Unsupported, Drafted(ticket_id)}`。
- `EffectResponse` 与产生它的观察记录 / `Gap` / `Draft` 同一事务 append。
- 理由：观察记录可压缩（§2.4），`EffectRequest` 永存（§7.5）；“是否已处理”必须能从与请求同寿命的事实重建。
- `Observed` 引用的观察记录落到保留边界下后，`EffectResponse` 仍成立，不钉住保留。
- `Unhandled` 请求没有 `EffectResponse`，也不重派。

**请求与响应的关联是引用，不是事务。** `EffectRequest` 记录随程序 `Advance` 输出持久化（§8.6），处理器在其后执行。核心重启时 fold 出**无 `EffectResponse`** 的已注册请求（§9.2 #21）：

- 读处理器重新执行一次；
- 写处理器重新开单。`Drafted` 与 `Draft` 同事务，所以“有 `Draft` 无 `EffectResponse`”不可达。

核心只保证：写类走两阶段，读类可重试，两类都被记录、都带 `basis`。

### 不变量

- `Emit(EffectRequest)` 是唯一出口；副作用的种类是注册表的事，程序代数不膨胀。由单一出口构造子保证。
- 对程序与核心同形：下单 `trade.place`（写）、发通知 `notify.telegram`（写，对外部世界也是写）、拉历史 `fetch.bars`（读），三者对程序是同一构造子。由 `EffectKind` 注册区分保证。
- 两个注册表对称：入站字段 → 处理器（§2.1）；出站请求 → 处理器（本节）。都是“出现了什么，则做什么”。
- **写处理器不绕过效应路径**：注册为写处理器即意味着经过规则链（至少授权与预算），不因“只是发个消息”就直通。否则程序拿到一条不经审批的外部写通道，H2 的不可信程序前提被破坏。由处理器注册语义保证。[域 H2]

### 为什么

若为每种副作用各加一个构造子，程序的封闭代数就随副作用种类膨胀，突破“构造子膨胀”红线（§4.3）。把副作用的种类推到注册表，`DerivationNode`/`DecisionStep` 就不动。

### 不选

- **对每种副作用各加一个 `DecisionStep` 构造子**：代数随副作用膨胀，退化为 Pine-with-limits（§4.3）。
- **让写处理器“轻量”消息直通、不走规则链**：破坏不可信程序前提。

### 程序隔离运行时的前提

程序无写能力，预算依赖宿主进程隔离而非类型系统。宿主 = **受监督子进程**（§8.6）：

- 每程序一个宿主进程，预算由 OS 进程机制限制。
- 状态经宿主协议以 `Checkpoint` 值显式序列化，不依赖运行时快照。
- 封闭值代数与两种解释不变（§4.3）：宿主仅是解释器的宿主，不进入设计中心。
- **运行时状态要求**：程序状态只经 `Checkpoint{bytes, state_version}` 序列化，与程序 cursor 同事务持久化（§7.5）。

**为何不是 Wasm**（§1.6.2）：

- Wasmtime 缺乏 live `Store` 快照 / 恢复 API；Component Model async ABI 尚未完成；fuel 虽能提供确定性指令预算，但无法限制阻塞性 host 调用。
- 三 OS 开箱即用未证：作为普通 Rust 依赖即可构建运行、预算与 trap 语义一致、独立二进制无平台差异。
- 这些是能力缺口，足以不选它作初始宿主。它可作实现阶段的替代宿主，走同一宿主协议（§8.6），不改设计。

## 6.2 单据 `Ticket`

> 图：D5.1 单据状态机、D5.5 两层对账重算（`design/diagrams/05-ticket-and-sts.md`）。

单据是意图形成期的抽象，它的锁 = 责任持有。

- 单据锁**不驱动 IO 壳，IO 壳不知道单据的存在**。
- 它不是“事务发起时加的锁”，而是在事务之前就存在，覆盖整个意图形成期：起单 → 编辑 → 送审 → 决定 → 进入 `Prepared` 关闭。
- IO 壳（§6.5）的模型是 Haskell `IO`；单据锁的模型是**责任持有**。
- 单据**单向读取** IO 壳 append 的记录（能力证据、`VenueAccepted`、归因后的订单观察）作为依据，不存在反向耦合。

术语：本节的“偏离 / fit”回答“我的意图还对不对”；决议 / resolution（§6.6）回答“我的动作发生了没有”。两者不同名，不共用状态（§11.3）。

### 类型

```rust
/// 一张单据 = 一把锁 + 一条线性版本链 + 一个对账钩子。锁的持有者是单据的负责人。
struct Ticket<Intent> {
    id: TicketId,
    responsible: Principal,      // 当前负责人；锁 = 这个字段的存在
    current_version: Hash,       // 版本链末端（内容寻址，每版带父 hash）
    versions: Vec<Version<Intent>>, // 只追加；Intent = 意图类型（下单/改单/撤单…）
    basis: Basis,               // §5：位置集，可含派生侧（观察）与执行事实侧位置
    basis_validity: BasisValidity, // 第一层：依据有效性（§5.2 定义 basis_valid / BasisValidity；单据只应用此门）
    alignment: IntentAlignment,   // 第二层：意图专属对账，逐项状态（可能全部 InputMissing）
    latest_revision: Option<Revision<Intent>>,  // 最近一次 Revise 的结构差（派生；见编辑 diff）
    state: Drafting | AwaitingDecision(Hash) | Closed(Outcome),
}

/// 第二层：意图专属对账。一组检查，每项声明它需要哪些观察输入（required_inputs 见 §2.5）。
/// 由 (意图类型 × 该 venue 当前能力证据) 在评估时解析，不存在单据上；握手变了它就变。
struct AlignmentCheck<Intent> { name: CheckName, required_inputs: Set<StreamKind>, eval: fn(&Intent, &Observed) -> CheckResult }
type AlignmentChecks<Intent> = Vec<AlignmentCheck<Intent>>;
enum CheckResult { Aligned, Diverged(Divergence), Undecidable(Gap) }        // 输入齐全时的三种结果
type IntentAlignment = Map<CheckName, Aligned | Diverged(Divergence) | Undecidable(Gap) | InputMissing(Set<StreamKind>)>;
//                                                                   ^ 该项需要的输入观察侧没有

enum TicketAction<Intent> {
    Draft   { by: Principal, initial: Intent, basis: Basis },  // 建立单据 = 取得锁 = 声明负责
    Revise  { by: Principal, next: Intent, basis: Basis },     // 仅 responsible；追加版本，current_version 前进
    Transfer{ by: Principal, from: Principal, to: Principal },  // 显式移交，记录，不静默；by = from（自愿）或持有控制授权的 principal（强制，§8.5）
    SubmitForDecision { by: Principal, at: Hash },           // 送审：冻结 current_version，Drafting → AwaitingDecision
    SendBack { by: Principal, reason },                      // 审批退回：AwaitingDecision → Drafting，responsible 不变
    Close   { outcome: Prepared(LogPosition) | Withdrawn | DecisionRejected | Expired },  // 锁消失；Withdrawn 仅 responsible，DecisionRejected 仅决定者，Expired 仅过期规则，Prepared 仅放行链
}
```

### 锁与转移

- **锁的本质是 `responsible` 字段**：`Draft` 即取锁，`Close` 即释放。持锁期间仅 `responsible` 可 `Revise`/`SubmitForDecision`；其他 principal 的 `Revise` 被拒绝，不排队、不产生分支。
- **`TicketAction` 均为 append 记录**：每条带 principal 与依据 `LogPosition`。`Ticket` 自身是这些记录的 fold（`fold_state`，§4.1），不是原地修改的对象。
- “锁”因此也是记录的解释：`responsible` 由最近一次 `Draft`/`Transfer` 决定。

**穷尽转移表：**

| 源状态 | 动作 | 目标状态 |
|---|---|---|
| `Drafting` | `Revise` | `Drafting` |
| `Drafting` | `SubmitForDecision` | `AwaitingDecision` |
| `AwaitingDecision` | `SendBack` | `Drafting` |
| `AwaitingDecision` | `Close(Prepared)` | `Closed` |
| `Drafting` \| `AwaitingDecision` | `Close(Withdrawn)` | `Closed` |
| `AwaitingDecision` | `Close(Expired \| DecisionRejected)` | `Closed` |
| `Drafting` \| `AwaitingDecision` | `Transfer` | 同态 |

- `Closed` 无出边：之后任何 `TicketAction` 被拒，含再次 `Close`。`Prepared` 之后的命运属 Attempt 链（§6.5），不再经单据。
- `AwaitingDecision` 期间 `Revise` 被拒：决定绑定的 current_version 不能变（C11）。

**触发者：**

- `Withdrawn` 由 `responsible` 发起。
- `DecisionRejected` 由决定者。
- `Expired` 由 STS 过期步（H6），只对 `AwaitingDecision` 触发（§6.3）。

**`Drafting` 没有过期。** 草稿不占 lane、不产生外部动作、不在 STS 链上。

- `deadline` 已过的草稿，在送审后立即由过期步关闭。
- 失联的草稿，由持有控制授权的 principal 强制 `Transfer` 后 `Withdrawn` 关闭（§8.5）。

**与写边界的接口。** 决策链（§6.3）对 `AwaitingDecision(current_version)` 放行后 append `Prepared`，并在同一事务中 `Close(Prepared(position))`。单据与 IO 壳之间有两条都经记录中介、方向相反的流：

- **写向**：单据 → `Prepared` 记录 → IO 壳（唯一的交出点，§5.3）。
- **读向**：IO 壳 append 的记录 → 单据的 `basis` / 检查项。

IO 壳不知道单据的存在。

### 两层对账：偏离是状态

**偏离是状态，不是动作。** `basis_validity` 与 `alignment` 均为单据 fold 的一部分，在**效应侧**评估。评估用组合子（`AlignmentCheck.eval` 是 `Comb<Observed, CheckResult>`，§2.5）与 `fold_state` 机制（§4.1）。

两层输入不同：

- **第一层（`basis_validity`）**：只读 `basis` 位置集与各流的完备位置 / 撤回 / 保留边界（§5.2）。
- **第二层（`alignment`）**：读 `required_inputs` 各流**当前流末**的 `fold_state`，经单向边“钩子读观察值”（§5.1）。
  - 当前流末 = 该 `StreamId` 已提交的全部记录，含 `one_shot` 与 `backfilled`，各带质量标记。
  - 不以完备位置截断：一次性读不推进完备进度，截断就看不见为补齐输入而读来的观察（§3.4）。
  - 不读 `basis` 位置处的旧值，否则世界变了单据不会变。

**`checked_as_of`。** 每次评估把实际消费的位置集记为 `checked_as_of: Set<LogPosition>`，随 `CheckResult` 进入单据 fold。它还写进依据它放行或否决的 Decision/`Outcome`/`Rejection` 记录（§6.3）。

- `basis` 回答“拟单时看到了什么”；`checked_as_of` 回答“这次判断看到了什么”。
- 二者都不复制观察值。

**重算触发**（本身不是 `TicketAction`）：

- `basis` 引用的流或 `required_inputs` 各流推进、被撤回、出现 gap；
- 能力证据变化；
- 保留边界推进；
- 策略必要项集 / `Lag` 变化。

因此**单据是否偏离是状态字段，不需要外部触发对账**；单据始终知道自己与世界的关系。

- 只读校验门（§5.2）在提交时退化为读取 `basis_validity == Fresh` 及策略要求的必要项。
- `AwaitingDecision` 期间世界变了，审批人看到的就是一张 `Diverged` 单据，不需要另一套失效逻辑。

**两层分开的原因。** 第一层只依赖 `LogPosition`，对所有单据均可计算；第二层依赖意图类型与 venue 提供的观察。混成一层，会让“不能做意图对账”的单据连新鲜度都丢掉。

### 门只看必要项

放行策略按 `(WriteLaneKey, OperationKind)` 声明哪些检查项是**必要项**（§7.6）。

- 仅必要项的 `Diverged`、`Undecidable` 或 `InputMissing` 触发 fail-closed（C12）。
- 其余检查项为 **advisory**：结果对审批人可见并写入依据，但不参与门。
- 不存在“所有 `Undecidable` 均阻断”的总门：那会让 advisory 在语义上重新变成 guard。

**能力项恒为必要项**，是唯一不由策略声明的必要项。

- `(WriteLaneKey, OperationKind)` 的能力证据为 `Supported` 才 `Aligned`；`Unsupported`/`Unknown` 即 `Diverged`（§2.2）。
- 策略不能把它降为 advisory：IO 壳对无能力的操作没有转移可走（§6.5）。

### 钩子

**钩子不一定存在，不一定能对账。** 第二层是多项独立检查的乘积，而非单一函数。

- 限价买单要对价格（报价流）、资金（余额流）、持仓（持仓流）、可交易性（能力证据）。
- venue 给报价不给持仓，就是“价格能对、持仓不能对”，不是整个钩子消失。
- 因此 `IntentAlignment` 逐项记 `InputMissing(缺哪些输入)`，不用 `Aligned` 冒充，也不设全局 `NoHook`。

**`InputMissing` 是可行动的。** 缺的输入若 venue 有一次性查询能力，发一次只读查询就产生一条观察记录（§3.4），该项随即可算。读是安全的、可批处理的（§2.2）。

- 策略可选“先查后判”“无该项对账则人工”“无该项对账则不发”。
- 真正的“不能对账” = 缺的输入没有任何渠道可得，这由能力证据说了算。

**钩子的输入是观察值及其出处**：派生 `Journal` 的 `fold_state`、能力证据、归因后的订单观察。

- 执行事实记录只作**身份与因果依据**（`basis` 中的 `VenueAccepted`/`SendBarrier` 位置），不进钩子的 `eval`。
- 归因后的订单观察是**记录**（集成或 IO 壳产出并带出处），不是读模型。“规则不引用读模型”不放宽。

**检查是纯函数、按 `Intent` 分派。** 下单看价格 / 资金 / 持仓 / 能力；撤单看能力（可按目标身份撤）+ advisory 仍在；`Replace` 看撤单项 + 新单项。新意图类型 = 新的检查集，核心不变。

### 编辑 diff 与偏离

单据是不可变量的线性版本链，每次 `Revise` 都是一个 diff（`version_n → version_{n+1}`），而这个 diff 本身触发副作用。它与对账产生的偏离方向相反，必须分开收束：

| | 编辑 diff | 偏离（对账 diff） |
|---|---|---|
| 变的是 | 单据（意图）变了，世界没变 | 世界变了，单据没变 |
| 来源 | `TicketAction::Revise`（负责人主动） | 观察侧推进 → `basis_validity`/`alignment` 重算 |
| 类型 | `Revision<Intent>`：两版意图的结构差（价格改了 / 数量改了 / 目标换了） | `IntentAlignment` 的变化：`Aligned → Diverged` |
| 触发 | 变更通知（审批人看到“改了什么”）、守卫字段变更 → `AwaitingDecision` 自动 `SendBack`、限额差额校验、审计记录 | 偏离警告、自动 `SendBack`、fail-closed |

- `Revision<Intent>` 是派生字段：`revision(versions[n], versions[n+1])` 由意图类型定义（[交易协议] 提供 `Revision<PlaceOrder>`）。核心不解释，与 `IntentAlignment` 同为单据 fold 的派生结果。
- 编辑 diff 的处理器是字段处理器（§2.1）。它属效应抽象但**不进入 IO 壳**：发生在 `Prepared` 之前，与两阶段无关。它触发的对外写（通知）自己作为新请求走完整路径。
- 它与撤回代数 `RetractableDelta`（§4.1）不是一回事：那是观察侧的撤回代数（有逆元）；这里是意图版本间的结构差，无逆元需求，不会“撤回一次编辑”，只会再编辑一次。
- 不收束的后果：审批人分不清“我要重看”（意图改了）与“市场跑了”（世界变了）。

### 撤单与改单 [交易协议]

**目标身份是构造前提，“仍在”是 advisory。**

撤单 / 改单意图类型在构造时**必须**携带目标身份 `target: VenueRef | IdemKey`（parse-don't-validate）。无目标即构造不出意图，不需要事后规则。

身份来源三种。身份进意图的 `target`，其来源记录的位置进 `basis`：

1. 本地 `VenueAccepted(venue_order_id)`；
2. 本地 `SendBarrier` 的幂等键（无回执的提交）；
3. 归因观察记录中的 venue 身份（外部订单，F9/P11）。

- 构造期只保证“目标存在且与账户作用域匹配”，**不**保证 venue 此刻支持按该身份撤单。那是运行期能力检查项：有幂等键 ≠ 有 cancel-by-key（F6）。
- “原单仍在”来自观察侧 listing，按 F10 只能是 advisory，永不作为撤单放行的必要项。否则最安全的动作在 listing 滞后时被 fail-closed。

**改单是单一意图类型 `Replace`，不是两张单据。**

- venue 有原子 cancel/replace 能力，就是一个操作。
- 没有，IO 壳在**同一条 Attempt 链**里解释为 `SendBarrier(cancel) → 目标订单终态证据 → SendBarrier(new)`。
- 新单数量按意图声明的口径（剩余量或绝对量），从目标订单的终态观察（含累计成交量）算出。这是 `>>=`：第二腿读第一腿的结果，发生在 IO 壳内，不是单据层的两次起单。
- 撤单腿 `Undetermined` 时整条链停在对账，新单腿不发。
- 撤单腿收敛后，链等待目标终态（转移表，§6.5）。新单腿仍要过发出前门；意图的 `deadline` 已过则记 `Expired(deadline)`，新单腿永不发出。

**`ResolutionEvidence` 指尝试，终态与成交量是观察记录。**

- 执行侧 `ResolutionEvidence` 的 found/absent 回答“我的提交到达了吗”。
- 派生侧观察记录反映目标订单的终态与累计成交量。
- 二者来自同一次 venue 交互：`Found` 时同一事务落两侧各一条记录，`Found` 以位置引用那条观察记录，原始证据字节留在执行侧（记录模型，§6.5）。

### 意义与产品规则

- **锁最大的意义不是互斥，是入口**：它是 UTA 表达“**现在有一张单据，我是负责人**”的唯一方式。取得单据 = 单据存在 + 某 principal 从此负责；后续编辑、送审都以这个身份记账。没有负责人的单据不存在。
- **一份单据只有一种交易意图**：不分叉不合并，订单不能“同时想买又想卖两个价”。版本链线性；`Revise` 在锁内追加，不需要 hash 期望比较。
- **产品层代价与协作边界**：两个 AI 不能同时处理同一张单据。合理，但不好用。核心有意接受这个代价，不用分叉 / 合并修补。协作在核心之外：`Transfer` 移交；第二个 AI 另起单据，由决定者二选一；或把建议发给负责人。
- **决定绑定 current_version**：Decision 引用 `AwaitingDecision(current_version)` 的 hash；进入 `Prepared` 要求被决定的版本 = 当前 current_version。`SendBack` 后的 `Revise` 使 current_version 前进，旧 Decision 自然失效。
- **一张单据至多一次 `Close(Prepared)`**：之后的改动是新单据（改单 / 撤单各自起单），各走各的写边界。

### 不变量

- 一张单据**至多一个负责人**（`responsible` 存在即锁），持锁期间仅 `responsible` 可 `Revise`。由单据锁保证。
- 一张单据**至多一次** `Close(Prepared)`；`AwaitingDecision` 期间 `Revise` 被拒。由状态机穷尽转移保证。
- 门只对必要项 fail-closed，advisory 不参与门。由放行策略声明保证。
- `basis_validity == Fresh` 且必要项 `Aligned` 才进 prepare。由单据应用只读校验门保证（门定义在 §5.2）。

### 为什么

意图在 `Prepared` 之前是脚本 / 值。两个 AI 对同一个暂存订单编辑，这时根本不是一般数据库事务形态下的锁：等于事务还没提交，外面有人想改 SQL 脚本。草稿是否自己就是个锁，这才是 UTA 里加锁的地方；单据本身就是锁。

### 不选

- **互斥原语作锁**：死锁、超时释放复杂；`responsible` 字段无这些（代价见权衡）。
- **分叉 / 合并（git 式）修补协作**：一次订单草稿理应只有一种交易意图，借线性历史不借分叉合并（C11）。
- **外部触发对账**：单据统一对账钩子使偏离成为状态问题，不需外部触发。

### 权衡

- **`responsible` 字段替代互斥原语**：无死锁、无超时释放的复杂性。代价是“负责人失联”必须由策略层通过 `Expired` 或带 principal 的强制 `Transfer` 处理（§6.3）：责任归于规则而非锁机制。
- **单据锁不驱动 IO 壳**：意图形成期可以任意长、任意多次退回，IO 壳完全不感知；单据只单向读 IO 壳的记录作依据。唯一耦合点是 `Close(Prepared)` 与 `Prepared` 的 append 必须在同一 SQLite 事务内（§7.4）。
- **保护边界限于意图形成期**：这把锁保护的是意图形成期的线性与责任归属，不是任何 venue 域事实；执行阶段与观察侧仍然没有锁（§6.8）。

## 6.3 STS 规则与顺序固定链

> 图：D5.3 顺序固定链、D5.6 人工审批时序（`design/diagrams/05-ticket-and-sts.md`）。

决策是 State Transition System 规则，不是订单对象：

```rust
trait Rule {
    type Context;    // 本次判断依据：时钟、授权策略、能力证据
    type RuleState;  // 规则必须记住的状态：冷却、待决集合版本、lane 阻塞头、过期
    type Input;      // 本次输入：意图、决定、超时、回执、对账证据
    type Rejection;  // 规则层具名 enum，包装组合子层的失败 kind；无 catch-all
    type Outcome;    // 接受转换产生的事实
    fn step(ctx: &Context, st: RuleState, input: Input) -> Result<(RuleState, Vec<Outcome>), NonEmpty<Rejection>>;
}
```

### 规则

**规则独立、组合具名。** [证据：fp-01 M8 cardano STS；fp-04 命题 5]

- 授权、输入约束、审批、期限、lane、fail-closed 各为独立规则，各有自己的 `RuleState`/`Outcome`/`Rejection`，不共同修改单一全局对象。
- 组合状态是**具名 struct**；组合 rejection 是**具名 enum**，每个变体 `From` 一条子规则的 rejection。
- 子规则内部的守卫失败是组合子层的 kind enum（§2.5），由规则层包装，两层各自封闭。
- 扩展轴是 venue 与协议，不是规则。加规则改这两个具名类型，显式接受。

**执行解耦。** 规则计算“允许执行” ≠ 调用 venue。所有决定先持久化为记录，再由 IO 壳执行（§6.5）。

**核心状态最小化。** 核心只持有规则运行所需的状态。订单读模型、持仓读模型仅是消费侧对执行事实 `Journal` 的 fold，可表现为具名数据结构，但**不作权威、不被规则引用**（§4.4）。[证据：fp-03 命题 1]

**记录类型按 stream 参数化。** 每条 lane / 账户 stream 拥有独立的 Input 集与 decider（Equinox `Category`/`StreamId` 模式），核心不定义全局 effect enum。[证据：fp-03 条目 7]

**失败类型结构。** `NonEmpty<Rejection>` 保留依赖结构；`Validated` 式累积仅用于相互独立的校验项。[证据：fp-04 命题 8]

### 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期

规则组合分两类：

- **可交换集**：相互独立的 guard，以交换性测试保证。
- **顺序固定链**：由代码固定执行顺序并测试。

不存在“任意装配顺序均不改变结果”的默认前提。[证据：fp-03 命题 6]

分类 [设计]：

- 五步之间是顺序固定链：每步读前一步 append 的记录（§2.1 推论 3）。
- 可交换集只出现在**一步之内**：输入约束步的各守卫字段校验、授权步的各 scope 判定彼此独立，以 `Validated` 累积并要求交换性测试。
- 跨步的守卫一律不可交换。

| 步 | 读什么 / 做什么 | 依据 |
|---|---|---|
| 授权 | 以单据 `responsible` 为主体查 `(principal, WriteLaneKey, OperationKind)` scope | §6.8；C11 |
| 输入约束 | 守卫字段校验：instrument 属目标账户、数量/名义有限为正、子账户已枚举、side/名义金额阈值 | C9/C10；守卫字段处理器（§2.1） |
| 审批 | 策略要求人工则送审，等待带版本的 Decision；不要求人工则以 `rule_version` 为依据直接通过 | C3；H6；C11 |
| lane | 读该 `WriteLaneKey` 的阻塞头集合；集合非空则停在本步，集合为空后放行 | H4 |
| 过期 | `deadline` 过期规则，以 `Input::超时` 触发 | H6 |

**授权步。** 查询的是：哪些 principal 的记录足以让写进入 prepare。写处理器的装载 principal 授权范围，决定能否不经人工直接放行。

**审批步。**

- 决定者按 `(principal, 动作种类)` 授权。
- 另一笔过期未决独立处理。
- **一个 `current_version` 至多一条 Decision** [设计]：`decide` 的接受判据是“该 `(ticket, current_version)` 尚无 Decision 记录”（C11 的待决集合版本即此）；已有 → `Conflict(AlreadyDecided)`。
- 理由：决定之后单据可能仍停在 lane 步而版本不变，靠 `Closed` 挡不住同版本的第二条决定。

**lane 步。** 阻塞头集合的定义与语义见 §6.4。

- 集合非空时本笔停在此步，单据保持 `AwaitingDecision`，不产生 `Prepared`。
- 唯一不等待的写：`target` 为阻塞头 Attempt 幂等键的撤单意图（§6.4）。

**过期步。** `AwaitingDecision` 期间到期 = `Close(Expired)` 否决记录，不补偿。这包括停在审批步或 lane 步时。

### 放行门

进入 `Prepared` 前，链读取单据 fold 的三项状态：

1. `basis_validity == Fresh`（§5.2）；
2. 必要项 `alignment` 为 `Aligned`（§6.2）；能力项恒在必要项内，不由策略声明；
3. `AwaitingDecision(current_version)` 与决定绑定的版本一致。

任一不满足 → `PredicateFailure`（fail-closed，C12），不发出。

- 它与链上其他步的否决同形：`Rejection` 记录带 `rule_version`，单据 `Close(DecisionRejected)`（W18），负责人按当前世界另起单据。
- 等待期间已呈 `Diverged` 的单据，可由审批人在放行前 `SendBack`（§6.2）。
- 规则**不自己算**这三项：它们是单据 fold 已经算好的状态字段。
- 此门即只读校验边界（§5.2）在 STS 链上的读取点，不参与两阶段。

**审计字段。** 每条 Decision/`Outcome`/`Rejection` 记录携带它所依据的 `checked_as_of`（该次 `alignment` 评估实际消费的位置集，§6.2）与 `rule_version`。审计由此重放这次判断。

### 规则版本变更 [设计]

链每次评估读当时生效的规则版本（§8.5），规则不冻结进单据。

待决单据放行时，若当前 `rule_version` 与它此前各步通过时所依据的不同，链从授权步起按当前规则重过全部五步：

- 已有 Decision 仍绑定 `current_version`，仍是“这一版批不批”的回答。一版至多一条 Decision，不因规则变更再要一条。
- 决定者对该动作的授权与“是否需人工”按当前规则重判：
  - 原单自动通过而新规则要求人工 → 单据停在审批步等 Decision；
  - 原决定者在新规则下已无该授权 → 审批步否决。
- 任一步在新规则下否决即 `Rejection`（带新 `rule_version`），单据 `Close(DecisionRejected)`。

### 不变量

- 组合状态是具名 struct、组合 rejection 是具名 enum，每变体 `From` 一条子规则 rejection。
- 组合子层失败与规则层 `Rejection` **两层各自封闭**，由两层各自 enum + `From` 保证（§2.5）。
- 核心不定义全局 effect enum；记录类型按 stream 参数化。由 decider per stream 保证。

### 不选

- **cardano 式泛型 `Embed` 组合规则**：Rust 无类型级和 / 积自动构造，泛型嵌套是 O(N²) 样板或退化到 `BoxError` catch-all；规则集本就闭合，具名 struct/enum 更直接。
- **全局 effect enum**：按 stream 参数化（Equinox 模式）避免全局耦合。
- **“任意装配顺序不改变结果”作默认前提**：fp-03 命题 6 表明不成立，须区分可交换集与顺序固定链。

## 6.4 lane

> 图：D5.4 lane 阻塞头（`design/diagrams/05-ticket-and-sts.md`）。

lane 是对 venue 写入通道的有序队列，= unknown 阻塞半径。

核心只要求写通道作用域键 `WriteLaneKey` 是意图链路的锚点，取自集成握手声明的 `WriteScope`（§2.2）。核心不知道它对应上游的什么结构。

### 键与粒度 [交易协议]

- **对齐**：集成通常把 `WriteLaneKey` 对齐为 `(账户, 子账户)`；无子账户的 venue 退化为账户。枚举子账户是集成做契约对齐时的义务（C10、§2.1），核心不要求、也不知道。
- **粒度权衡**：此粒度恰为 H4 要求的写全序范围。
  - 细于该粒度（如按 instrument）虽能防重复投放，却无法阻止“资金状态未知时继续加仓”。H1 本质：unknown 占用的 buying power 不按 instrument 隔离，且 venue 限流按账户生效。
  - 粗于该粒度（整账户）则超出 H4 范围，导致单笔卡住的订单冻结无关子账户。

### 队首阻塞是通讯协议

**队首阻塞是通讯协议的一部分，不是 UTA 选择的锁。** 提交属于消费动作，仅在消费者自身有权消费时才存在加锁概念；UTA 无权决定是否消费，消费方始终是 venue。

**等待机理。** lane 上有未终结 Attempt（最长的情形是队首 `Undetermined`）时，后续意图必须等待。

- 原因：**后续写入的语义依赖队首结果**，即同账户 buying power、待撤订单是否存在、venue 侧顺序。这是与 venue 的通讯协议语义，而非数据库层面的并发互斥。
- **等待发生在 `Prepared` 之前**：等待者是 `AwaitingDecision` 的单据，不是已放行的记录。因此正常路径下同 lane 至多一条未终结 Attempt；`Prepared` 一旦 append 即由 IO 壳紧接执行。
- 等待期间单据的 `basis_validity`/`alignment` 照常重算（偏离是状态，§6.2）。放行时链先过过期步，再过依据有效性门。
- 等待超过 `deadline` 由过期步 `Close(Expired)` 终结。

**阻塞头集合。** 阻塞头是该 `WriteLaneKey` 上**未终结 Attempt 的集合**，存在 `RuleState` 中。未终结 = 存在 `Prepared` 无 `SendBarrier`、`SendBarrier` 无后继、`Undetermined` 未 `Resolved`，或复合链未完（§6.5）。

- 正常路径下集合至多一条。
- 两个例外使集合扩大：撤阻塞头的撤单意图、显式绕过（见下）。
- 集合内每条由各自的对账驱动独立收敛（§6.6）。任何一条的终结只把它移出集合；**集合为空**才解除普通写的等待，不存在“部分解除”。
- 集合非空时仍可继续加入获准的例外（再一笔撤阻塞头、再一次绕过）。

### 无第二类越顶队列

队首取证期间 lane 上只发生决议动作。读侧的决议动作（按键查询、listing、成交 / 持仓对账）属于 IO 壳的对账协议，不是队列项。

**唯一能越过阻塞头的写**：**以阻塞头 Attempt 的幂等键为 `target` 的撤单意图** [设计]（目标身份来源之二，§6.2）。

- 它经单据与 STS 全链；lane 步对它不施加阻塞头等待。
- 理由：等待的依据是“后续写的语义依赖队首结果”，而这笔撤单不依赖队首结果，它存在的目的就是让队首结果可判定。
- 放行后它成为该 lane 阻塞头集合的第二个成员，与阻塞头各自独立收敛。
- 它不是对协议的违反，不需要 `bypass_lane`。

**它自身的回执不直接决议阻塞头。** 把“撤单被拒”解释成“原单不存在”或“已成交”是 heuristic，禁止（§6.6）。阻塞头仍只由取证渠道收敛；撤单只是把 venue 侧推到一个读得出的终态：

- 撤单腿终结于 `VenueAccepted`/`Found` 后，阻塞头的取证重开一轮（`ReconciliationReopened{CancelLegTerminal}`，§6.6）。
- 撤单腿终结于 `VenueRejected`/`Absent`/`Expired`，不重开；阻塞头照常按本轮进度收敛。
- 重开后，by-key 读到目标（已撤或任何状态）即 `Found`；by-key 明确否定即 `Absent`。
- listing 未见仍是 `Inconclusive`（F10）：撤单不提升任何渠道的证明力。

### 显式绕过

带 principal 的显式绕过仍可存在，但必须记录为 Decision，并记为**对协议的自觉违反**，不是队列的一种模式。

绕过的语义 [设计]：

- 绕过使该单据越过 lane 步进入 `Prepared`，于是该 lane 同时存在多条未终结 Attempt，按阻塞头集合的规则收敛。
- 绕过只影响本 lane。
- 绕过记录带 principal 与被绕过的阻塞头位置集，是审计事实。

lane 的有序与队首阻塞来自通讯协议，不是 UTA 抢占通道的锁；无锁定位见 §6.8。

### 不变量

- 同 lane 的**阻塞头集合**在正常路径下至多一条未终结 Attempt；任一 `Undetermined` 记录在 `Resolved` 前，同 lane 无新普通写。例外只有两个，且二者都扩大阻塞头集合：以阻塞头幂等键为 `target` 的撤单意图；显式绕过（以 Decision 记录在案）。由 STS lane 步在 `Prepared` 之前等待 + IO 壳按序推进保证。
- 显式绕过必须记录为 Decision。由绕过语义保证。

### 为什么

提交是消费动作，消费方始终是 venue，UTA 从头到尾自己都不能决定是否消费。所以“队首阻塞”只能是与 venue 的通讯协议语义，而非 UTA 的锁。

lane 的键取自握手 `WriteScope`，因为核心不知道也不该知道上游账户结构（§2.2）。[域 H4/H1；证据：fp-03 条目 7]

## 6.5 IO 壳：两阶段、腿与链、转移表

> 图：D6.1 腿的状态机、D6.3 一次交互的记录矩阵、D6.4 `Replace` 复合链（`design/diagrams/06-io-shell-attempt.md`）。

### 两阶段协议与它的位置

unknown 并非孤立的类型问题，其实质是一个**对账系统**。写边界协议即为**两阶段事务**：

| 两阶段 | UTA |
|---|---|
| prepare | `Prepared` 持久化 |
| commit | 发出并取得业务回执 |
| in-doubt | `Undetermined` |
| resolution | 对账 |

in-doubt 作为一等持久状态有直接先例：MongoDB `UnknownTransactionCommitResult` error label、Oracle `in-doubt` + `DBA_2PC_PENDING`、PostgreSQL/MySQL `PREPARED`、Kafka `PREPARE_COMMIT`、Seata `PhaseOne_Timeout`。它不是 `Option`、字符串或普通 pending 状态。[证据：fp-06 命题 1、修正 1]

**“两阶段只在写边界”窄义成立、广义不成立。**

- **窄义成立**：prepare 记录的归属、持久化及协议驱动只在写边界（IO 壳），9 个先例案例完全一致。
- **广义不成立**：in-doubt **状态**无一例外泄漏到四个面：
  - 串行化阻塞：PostgreSQL 持锁、Oracle `ORA-01591`、Kafka LSO；
  - 对账发起端：Oracle RECO、外部 TM、客户端 worker；
  - 审计读模型：`DBA_2PC_PENDING`、`pg_prepared_xacts`、`serverStatus`；
  - 上游超时语义：`ORA-02050`、MongoDB label 直达 driver、Binance `-1007`。

因此：**协议逻辑只在 IO 壳**。`Prepared`/`SendBarrier`/`Undetermined` 作为记录，对 lane 队列（§6.4）、读模型、对账发起及审批人视图合法可见，且各方均须按“可能已发生”解释。这些面只是把状态读出去的通道，不把 prepare 逻辑复制至读 / 编排 / 对账层。[证据：fp-06 修正 6]

**不锁 ≠ 不要两阶段。** 两阶段协议的价值不在锁，而在**已经定义了中间态的副作用语义**。

- prepare 之后、resolution 之前，效果被定义为“可能已发生”：既非成功亦非未发生，不能重发，也不能当作没做过，只能由 resolution 用证据收敛。
- 这正是 F5 所需的定义。故 `Prepared → SendBarrier → (VenueAccepted | VenueRejected | Undetermined) → ResolutionEvidence* → Resolved` 保持为两阶段协议，而非退化为“发一次、超时算失败”的单阶段调用。[证据：fp-06 命题 1]

**在 2PC 中的位置。** UTA 是协调者；venue 是**总是单方面决定的参与者**：不 prepare、不等协调者裁决，收到请求即自行 commit 或 reject，协调者事后只能发现结果。[证据：fp-06 命题 5 与案例——Stripe/Binance/Temporal 均无 participant prepare]

- 把这一情形类比为 2PC 中“参与者单方面完成、协调者进入 in-doubt 并以查询 / 日志决议”的分支（XA heuristic outcome），是**设计类比** [设计]。
- venue 从未参加 prepare，也没有 commit/rollback 可下达；UTA 这一侧缺少 XA 的 participant 契约（prepare/commit/rollback），只借用它对中间态与决议出口（found / absent / inconclusive）的定义。
- 名字保持“两阶段”，因为中间态语义与决议流程都来自它；只是 UTA 永远处在参与者已单方面决定的那一支。[证据：fp-06 命题 1（in-doubt 一等化）]

### IO 壳是效应侧的解释器

IO 壳不是“调用 venue 的那个函数”，而是效应侧的解释器：`Prepared` 之后、决议之前发生的一切都在它里面。它与 Haskell 的 `IO` 同构，先定义，再运行：

| Haskell | UTA |
|---|---|
| `IO a` 值（纯，可构造、可组合、可检查） | `Prepared` 记录：已持久化的“将对 venue 做什么”的描述；构造过程（草稿 → 决策 → append）不接触 venue |
| `>>=` 组合：后一步依赖前一步的结果 | lane 全序：同 lane 后一个 Attempt 的语义依赖前一个的终态，故 lane 步在前一个未终结时不放行下一个（§6.4）；复合链的第二腿读第一腿的终态（§6.2） |
| runtime 运行 `main` | IO 壳解释 `Prepared` 链：系统中唯一产生外部副作用的环节 |
| 效果发生后只剩结果值与外部状态变更 | 效果发生后只剩 append 的记录（`VenueAccepted`/`VenueRejected`/`Undetermined`/`ResolutionEvidence`；未发出即到期则 `Expired`）与 venue 侧变更 |
| `unsafePerformIO` 是禁忌 | 规则、程序、集成、消费方直接调用 venue 写接口是禁忌 |
| 纯代码可随意重算，`IO` 不能 | 派生侧与决策可以重放重算，`Prepared` 的实际执行不能重放 |

二者唯一差异正是 F5：Haskell runtime 总能知道 `IO` 动作做完没有；UTA runtime 面对 venue 时可能不知道。因此 UTA 的“运行”多出 `Undetermined` 结果与配套对账，其余纪律与 `IO` 相同。

“先定义再运行”是 IO 壳的全部设计原则：任何“运行”的东西先是一条记录；任何记录都不是运行。

**位置。** 单据（§6.2）与 STS 决策（§6.3）位于上游，产出 `Prepared` 记录；读模型与消费方位于下游，只读记录。IO 壳是核心中**唯一**把记录变成 venue 动作、把 venue 响应变成记录的地方；核心内不存在第二处接触集成进程写接口的路径。

### 输出

执行事实侧只有 append：

- `SendBarrier`、`VenueAccepted`、`VenueRejected(reason)`、`Undetermined(reason)`、`Expired(deadline)`；
- `ResolutionEvidence`、`ReconciliationReopened`；
- `CapabilityObserved`；
- `Gap{origin: Channel}`。

关联身份：

- 腿级记录与取证渠道的 `Gap{origin: Channel}` 都带 `AttemptRef`（见下“代数”）。
- `CapabilityObserved` 是能力证据，带 `(WriteLaneKey, OperationKind)`，不属于任何 Attempt（§7.5）。

`Undetermined` 的 `reason` 封闭为两种，二者进入同一对账驱动，只是审计出处不同：

- `NoResponse`：`submit` 无业务回执（§8.2）；
- `CrashWindow`：重启时 `SendBarrier` 无后继（§6.7）。

IO 壳**不修改**任何记录，不持有权威状态；重启后其全部状态由 `fold_state` 重建。

### 回执与取证的记录模型 [设计]

venue 对我方写的响应是执行事实：C13 原始负载完整保留，执行事实永不删除（§3.1）。同一响应里的订单 / 成交状态又是观察（§3.4）。

响应经集成消费后到达核心，由两部分组成（§2.1）：集成的结论（契约载荷 + `payload_schema`）与所消费的上游原文（原始负载）。执行事实侧永存的证据值 `Evidence = {payload, payload_schema, raw}` 同时保存两者：前者是 UTA 据以行动的结论，后者是结论的出处。[设计]

因此一次 venue 交互在**同一 SQLite 事务**内落两条记录：

- **执行事实侧**一条，**持有 `Evidence`**（永存）。
- **观察 `Journal`** 上一条同内容的观察记录，可压缩。它带 `provenance` 与归因 `attribution: FromAttempt(AttemptRef)`，由 IO 壳填（§8.3）。它供复合链读终态、供单据钩子与读模型消费、供订阅者与推送观察同形地看到。

具体：

| 交互 | 执行事实侧 | 观察侧 |
|---|---|---|
| `submit` 返回 `Ack` | `VenueAccepted{venue_order_id, receipt: Evidence, observation: LogPosition}` | `provenance: Receipt{attempt}` 的观察记录 |
| 取证命中 | `ResolutionEvidence{attempt, channel, round, outcome: Found{observation: LogPosition, evidence: Evidence}}` | `provenance: Reconciliation{attempt, channel}` 的观察记录 |
| 取证 `Absent` / `Inconclusive` | `ResolutionEvidence` | 无（没有订单状态可记） |
| `Unavailable` | 只落 `Gap{origin: Channel}`，不是取证结果（§6.6） | 无 |
| `submit` 返回 `Reject` | `VenueRejected`，`reason` 保留 `Unmapped(raw)`（§2.6） | 无（venue 侧不存在订单） |

**`channel ∈ {ByKey, Listing, Fills, Replay, Attributed, Manual}`：**

- 前四种是 IO 壳依序取证的渠道（§6.6）。
- `Attributed`：集成推送的归因观察命中处于 `Undetermined` 的腿，由效应侧归因处理器 append（§8.1）。
- `Manual`：带 principal 的人工决议，经 `resolve` 进入（§8.5），记录 `{principal, outcome, note}`。

**`outcome ∈ {Found{observation, evidence}, Absent, Inconclusive}`。**

**每条 `Found` 都带 `evidence`**，C13 对六种渠道一视同仁：

- 主动取证取集成返回的该次响应；
- `Attributed` 取该推送记录的载荷与原始负载；
- `Manual` 取被引用观察记录在 `resolve` 时的载荷与原始负载。被引用的观察记录可压缩，执行侧的 `evidence` 不可。

**`round`** 是该次取证**发起时**所属的轮次：最近一条 `ReconciliationReopened` 的位置，首轮为空。`Attributed`/`Manual` 取 append 时的当前轮。

观察侧那条记录落到保留边界下后，执行侧的 `Evidence` 仍在。审计读执行事实，不依赖观察副本。

### 代数：Attempt、腿、`AttemptRef`

**Attempt** = 一条 `Prepared` 记录及其后继阶段链，身份 = `attempt_position`（该 `Prepared` 的 `LogPosition`）。

链由**腿**组成，腿身份 `AttemptRef = (attempt_position, leg)` [设计]：

- 单腿操作只有 `leg = 1`。
- venue 无原子能力时，`Replace` 是同一链上两条连续的腿：`leg = 1` 撤单、`leg = 2` 新单（§6.2）。

所有腿级记录（`SendBarrier`、`VenueAccepted`、`VenueRejected`、`Undetermined`、`Expired`、`ResolutionEvidence`、`ReconciliationReopened`）与观察侧的 `FromAttempt`/`provenance` 都以 `AttemptRef` 关联。`idempotency_key ↔ AttemptRef` 登记（§8.1）。两条腿的回执、归因、取证进度、恢复判定互不混用：撤单腿的回执观察不能终结新单腿的 `Undetermined`。

每条腿是线性阶段链：

```
[发出前门] → (SendBarrier → (VenueAccepted | VenueRejected | Undetermined → ResolutionEvidence*)) | Expired(deadline)
```

- 链的 `Resolved` 由各腿状态与“是否还有下一腿”fold 出。
- IO 壳是链的驱动器。每一步转移由 `venue 回应类型 × 该 (venue, op) 的能力证据 × 超时参数 × deadline` 决定。
- 链是闭合 sum，转移表穷尽（见下），没有“其他”分支。

**`Resolved` 是 fold 状态，不是记录。**

- 腿达终态即该腿终结：`VenueAccepted`、`VenueRejected`、`Expired` 直接终结。
- `Undetermined` 在出现 `Found`/`Absent` 的 `ResolutionEvidence` 后终结；任一渠道、任一轮次，含 `Attributed` 与 `Manual`。
- 腿终结后**重算整条链**：链 `Resolved` = 当前腿终结且（按转移表）不再有下一腿。
- 撤单腿终结于 `VenueAccepted`/`Found`，则链进入 `AwaitingTargetTerminal`，仍未完。无论该终结来自哪个渠道，含 `Manual`。
- 取证渠道集取自该 (venue, op) 当前能力证据（§7.5）；渠道顺序固定（§6.6）。
- 本轮已取证渠道 = `round` 等于当前轮（最近一条 `ReconciliationReopened` 的位置）的 `ResolutionEvidence`。因此“渠道穷尽”与“下一渠道”都由 fold 重建（§9.2 #7）。

**`SendBarrier` 是发送屏障。** durable append（fsync）之后才允许调用 `submit`。它把崩溃窗口二分：

- `Prepared` 无 `SendBarrier` = **确未发出**；
- `SendBarrier` 无后继 = **可能已发出**。

先例：PostgreSQL `EndPrepare` 先 `XLogFlush` 再 `MarkAsPrepared`。[证据：fp-06 修正 5]

### 转移表（穷尽）

以下按腿状态 × 事件列出全部转移；未列出的组合在 fold 中不可达（sum 穷尽，C13 风格：无“其他”分支）。

**单腿**（`leg = 1`，或 `Replace` 有原子能力）：

| 腿状态 | 事件 | 结果 |
|---|---|---|
| `Prepared` | 过发出前门失败 | `Expired(deadline)`（终） |
| `SendBarrier` | `submit` 返回 `Ack` | `VenueAccepted`（终） |
| `SendBarrier` | `submit` 返回 `Reject` | `VenueRejected`（终） |
| `SendBarrier` | `submit` 返回 `NoResponse` | `Undetermined` |
| `Undetermined` | `ResolutionEvidence{Found}` | 腿终结 |
| `Undetermined` | `ResolutionEvidence{Absent}` | 腿终结（本次提交未发生） |
| `Undetermined` | `ResolutionEvidence{Inconclusive}` | 下一渠道；渠道穷尽 → 停，等 `Manual` 或 `ReconciliationReopened` |
| `Undetermined` | `Attributed`（任一时刻到达） | 与 `Found` 同效 |

`Found` 时 Attempt 只回答“到达了”；观察记录里 venue 已受理 / 已拒 / 已成交的状态，由读模型与钩子 fold。

**复合链**（`Replace` 无原子能力）在单腿之上多一个链级 fold 状态 **`AwaitingTargetTerminal`** [设计]：

| 撤单腿（`leg = 1`）终结于 | 链 |
|---|---|
| `VenueAccepted` 或 `Found` | 进入 `AwaitingTargetTerminal` |
| `VenueRejected`、`Absent` 或 `Expired` | `Resolved`，新腿永不发 |

撤单腿以拒绝 / 不存在 / 过期终结时新腿永不发，原因：

- IO 壳不解释拒绝原因：“已成交”与“不存在”在线缆上不可靠区分，永不 heuristic。
- 目标可能仍在时发新腿等于加仓（H1）。
- 负责人按观察记录另起单据。

**`AwaitingTargetTerminal` 的事件与出边：**

| # | 事件 | 结果 |
|---|---|---|
| (i) | 目标订单终态观察到达 | 按意图口径算新腿数量：> 0 → 新腿（`leg = 2`）过发出前门，之后同单腿；= 0（口径为剩余量且目标已全部成交）→ 链 `Resolved`，无新腿记录 |
| (ii) | 读返回目标存在但非终态；状态映射为 `unknown`/`Unmapped(raw)`；或未见目标 | 保持等待，按 pacing 再读 |
| (iii) | 读返回 `Unavailable` | `Gap{origin: Channel}`，再读 |
| (iv) | 意图 `deadline` 到期 | append `Expired(deadline)`（`leg = 2`），链 `Resolved`；新腿永不发、不补偿（H6） |

- (i) 的目标终态观察有三种来源：撤单腿回执 / 取证观察本身已含目标终态（`cumulative_filled_quantity`）；带 `attribution` 指向目标的推送观察；IO 壳按目标身份的一次性读。一次性读在 `target` 为 `IdemKey` 时用 `query_by_key`，为 `VenueRef` 时用 `read(orders, venue_order_id)`；均为读、可重试、有 pacing。
- (ii) 中 `unknown`/`Unmapped(raw)` 不冒充终态（C13）；listing / 一次性读未命中不证明不存在（F10）。
- (iv) 的到期在等待期间由发出前门的同一时钟检查。

因此该状态**有界**：出口是目标终态或 `deadline`，没有人工路径，也不会永久停留。负责人可在 `Expired` 后按目标的最新观察另起单据。`AwaitingTargetTerminal` 期间链未完，仍是 lane 阻塞头集合的成员（§6.4）。

### 与集成操作集的关系

核心对集成的操作集（IDL，小且闭合：`handshake`/`submit`/`query_by_key`/`list_open`/`list_fills`/`cancel`/`replay_by_key`/`backfill`/`read`）是核心↔集成契约，完整操作集与返回值见 §8.2。

- IO 壳只用其中的写与取证操作。
- `backfill` 由订阅侧发起；`read` 由读处理器、钩子取证与消费方发起（§3.4）。
- `NoResponse` 与 `Unavailable` 是一等返回值而非异常。

### 不变量

- IO 壳是核心中唯一把记录变成 venue 动作的地方，不修改记录、不持权威。由关系表（§3.3）与 `fold_state` 重建保证。
- `Prepared` 无 `SendBarrier` = 确未发出；`SendBarrier` 无后继 = 可能已发出；IO 壳不 `submit` 已有 `SendBarrier` 的腿。由发送屏障 durable append（fsync）保证。
- 每条腿发出前过 `deadline` 门；过期即 `Expired(deadline)` 终结，永不发出。由发出前门保证。
- 链的每个 `Prepared` **至多**达一个终态，且在证据充分（`Found`/`Absent`/回执/`deadline`）时必达。由闭合 sum 与转移表保证。
- `Undetermined` 的收敛不承诺时限（C2），只承诺停等可被推进：新证据（任一时刻到达的 `Attributed`、`ReconciliationReopened` 重开后的取证），或带 principal 的 `Manual`。
- `VenueAccepted` 只认业务回执，其余落 `Undetermined`。由 `submit` 返回契约保证。

## 6.6 对账驱动与决议

> 图：D6.2 取证循环（`design/diagrams/06-io-shell-attempt.md`）。

### 渠道顺序

进入 `Undetermined` 后，IO 壳按该 (venue, op) 能力证据声明的渠道**自动**依次取证：

```
by-key → listing + venue 身份 → fills/positions → 保留期内 replay-by-key
```

### 取证结果与记录的对应（固定矩阵）

| 取证结果 | 记录 | 腿 |
|---|---|---|
| 命中带归因身份的订单 / 成交 / 原响应 | 同事务：观察记录 + `ResolutionEvidence{Found}` | 终结 |
| 渠道给出明确否定 | `ResolutionEvidence{Absent}` | 终结 |
| 未命中 | `ResolutionEvidence{Inconclusive}` | 转下一渠道 |
| `Unavailable` | 只落 `Gap{origin: Channel}` | 不变；同渠道按 pacing 再发 |

- 明确否定只有 by-key 的 `Absent`；listing/fills 没有这一语义（F10）。
- `Unavailable` **不算取证**、不换渠道。一个不可用的渠道不是证据；跳过它会把“没查到”伪装成“查过了”。

### 停等

渠道穷尽仍 `Inconclusive`，append 后**停下**。停等可由两种事件推进：

- 带 principal 的人工 `ResolutionEvidence`，经 `resolve`（§8.5）；
- 一条 `ReconciliationReopened{attempt, cause}` [设计] 重开一轮。

另有被动渠道 `Attributed` 可在任一时刻到达（见下）。

IO 壳**永不 heuristic**。取证是读副作用，可以重试；`submit` 是写副作用，永不重试（§3.4）。

### 重开与轮次

- 新一轮的已取证渠道集从空开始。
- `ResolutionEvidence.round` 在**发起读**时取当前轮，并随记录落盘。
- 旧轮在途读的迟到响应带旧 `round`，不计入新轮：响应归属其发起轮次，不按到达顺序冒充。它只在腿仍未终结时 append。
- `Found`/`Absent` 已终结的腿不受重开影响：重开只重置读进度，不撤销终态。

`cause ∈ {CancelLegTerminal(AttemptRef), SessionRestored, Manual(principal)}`：

| cause | 谁 append | 何时 |
|---|---|---|
| `CancelLegTerminal(AttemptRef)` | IO 壳自动 | 以该阻塞头为 `target` 的撤单腿终结于 `VenueAccepted`/`Found` 时（§6.4） |
| `SessionRestored` | IO 壳自动 | 该 venue 的集成会话重建（新 `session_seq`）时，对其所有停等的 `Undetermined` |
| `Manual(principal)` | 运维 principal | 经 `retry_reconciliation`（§8.5） |

- `CancelLegTerminal` 表示 venue 已处理撤单请求，是新的取证机会，**不是**目标已到终态的证据。
- `SessionRestored` 的理由：渠道穷尽常因 venue 当时不可达。
- 三种触发在 append 前都重查目标腿仍处于 `Undetermined` 未终结，否则不写。

### 被动渠道 `Attributed`

顺序之外还有一条被动渠道。

- 集成推送的带 `attribution: FromAttempt(r)` 的观察记录到达时（或只带 `idempotency_key`、经登记解析到 r，§8.1），若腿 r 处于 `Undetermined` 且未终结，效应侧归因处理器在同一事务 append `ResolutionEvidence{r, Attributed, Found{observation: 该记录, evidence: 该记录的载荷与原始负载}}`。
- append `Undetermined(r)` 的事务内，同样检查已到达的、归因到 **r** 的观察。腿身份精确匹配：同链另一腿的观察不算。
- r 仍在 `SendBarrier` 等回执时不产生该记录：链保持线性，回执由 `submit` 返回值落 `VenueAccepted`/`VenueRejected`。
- 迟到回执（W2 步 4）由此并入同一腿，与取证 `Found` 同效。

[证据：fp-06 修正 7；域 C1/C2/C12]

### 先例谱系

- 能全自动收敛的系统，同时拥有权威结果源与防重身份：Oracle RECO、Kafka coordinator log、Seata TC state。
- 权威源在系统外的系统（PostgreSQL/MySQL 外部 TM、Stripe、Binance、Temporal），自动化止于“重建状态 / 触发查询 / 一次安全重放”，决议交外部。
- UTA 因 F1 权威恒在外，属于后者。

### `replay_by_key`

形式上似写，但保留期内同幂等键重放在语义上是查询：Stripe 同 key 拿回原响应。保留期声明错误即重复下单（Longbridge 10 分钟缓存、IBKR 无键）。

因此它**默认关闭，按 venue 显式开启，且只在能力证据声明的保留期内调用**，渠道顺序里排最后。

### 不变量

- 取证（读副作用）可重试，`submit`（写副作用）永不重试。由两类副作用的区分保证（§3.4）。
- `inconclusive` 停人工，IO 壳永不 heuristic。由对账驱动保证。

## 6.7 崩溃恢复与集成崩溃两故障面

> 图：D7.1 恢复判定（`design/diagrams/07-crash-recovery.md`）。

### 恢复

恢复时 IO 壳从日志重建各 lane 的链状态。判定顺序固定 [设计]：

1. **先 fold 链是否已 `Resolved`**：任一腿以 `Expired`/`VenueRejected` 终结，或末腿以 `VenueAccepted`/`Found`/`Absent` 终结且无下一腿。已 `Resolved` 者无动作；`Expired` 不会被再过一次发出前门。
2. **未 `Resolved` 者先看链级状态**：复合链处于 `AwaitingTargetTerminal` 者，继续按目标身份读。
3. **其余看当前腿：**
   - 无 `SendBarrier` 者仍是可安全发送的腿，过发出前门后发送或 `Expired`；
   - `SendBarrier` 无后继者，append `Undetermined(CrashWindow)`，进入对账驱动；
   - `Undetermined` 未终结者，按 `round == 当前轮` 的 `ResolutionEvidence` 重建本轮进度（发起轮次归属，不按 append 先后），续跑下一渠道或停等。

IO 壳不 `submit` 任何已有 `SendBarrier` 的腿。

### 集成崩溃两故障面

集成崩溃按崩溃发生在哪条路径分为两个面。判别边界唯一：**崩溃是否落在 IO 壳的 `submit` 写投放路径上**。

| 故障面 | 判别边界 | 领域状态 | 收敛 |
|---|---|---|---|
| 观察流侧崩溃 | 崩溃在订阅/推送路径上（无在途 `submit`） | `Gap{origin: Source}`（§4.2） | 续传 / 重连补齐 |
| 写投放侧崩溃 | 崩溃在 `submit` 路径上（已 `SendBarrier`、等业务回执） | `NoResponse` = `Undetermined` | 对账驱动取证 |

- 两面**可以并发**：同一集成进程既跑观察流又有在途 `submit` 时崩溃，观察流记 `Gap{origin: Source}`，在途 Attempt 记 `Undetermined`。它们是同一进程的两个不同职责面，各自独立收敛，互不替代。
- 前者是流内记录，可续、可标 gap；后者是一次可能已发生的写，只能由对账收敛。
- 写投放侧不区分“集成挂了”和“venue 没回”；区分靠后续证据（`ResolutionEvidence`），不靠猜。

### 为什么（写边界整体）

F1 使权威恒在 venue，UTA 面对 venue 时可能不知道动作是否完成（F5）。所以必须有 in-doubt 一等状态与对账收敛，而非把 timeout 当终态；这与 9 个一手先例一致。[证据：fp-06 命题 1–命题 6；域 F1/F5/C1/C2/C12]

### 不选

- **把 IO 壳做薄（“就是个 HTTP 调用”）**：会让两阶段、unknown 与队列语义散落至规则与集成里。
- **用泛型 typestate 保证发送屏障**：泛型 typestate 在崩溃恢复路径失效，从记录重建不能产回不同类型。故用 move-semantics token：`SendBarrier` 无 `Copy`、构造器私有且内含 durable append，`submit` 只接受 `SendBarrier` 值。静态保证只覆盖首执一次的调用栈，恢复路径全部是运行期 enum。
- **把 timeout 当收敛终态（单阶段“发一次超时算失败”）**：无一先例把 timeout 当终态，会造成重复投放或漏单。
- **宣称“unknown 的可组合代数”**：9 个一手案例未见“多个 unknown 组合成新 unknown”的运算，不写成行业无先例。

[证据：fp-06 修正 2/4/5]

崩溃窗口的恢复协议以 fsync 崩溃注入验收：`Prepared`/`SendBarrier`/`submit` 各窗口不重复投放（§10.5 #8）。

### 权衡

- IO 壳是效应侧**最大**的组件而非最薄的。代价是内部转移表与渠道策略是设计重点，由走查（§9）与验收 §10.5 #8/#17 覆盖。
- 它的一切状态都是记录：每一步取证都写日志（体积与噪音），换来重启零丢失与审计完整。留存由保留语义（§2.4）与运行期参数（§7.6）约束。
- 它与集成之间是 JSON-RPC（§7.1、§8.1）：集成在 `submit` 中途崩溃 = `NoResponse` = `Undetermined`。
- **单据锁在它之外**（属于意图形成期，§6.2）。因此“UTA 唯一的锁”与“IO 壳内没有锁”两句同时成立。

## 6.8 无锁定位

UTA 唯一的锁是单据；执行阶段与观察侧没有锁。本节说明为什么其余地方都不需要锁。

### 规则

- **唯一的锁是单据（§6.2）**：提交是消费动作，只有有权消费者才谈得上锁。意图形成期的单据确实有多个编辑者争同一份东西，单据本身就是锁。过期读由 C11 的依据版本**可见**，不需要锁让它不可能。
- **venue 域事实无锁（F1）**：账户、持仓、订单、成交、价格由 venue 消费与裁决，UTA 无权决定是否消费，因此对它们**不存在 UTA 的锁**。
- **UTA 自身记录的权威不是锁**：审批、意图、尝试、队列顺序、程序装载、订阅表是 UTA 说出的话，UTA 对它们有权威。但 append-only 日志上顺序就是位置本身：单写者（H10）、每次 append 原子、没有第二个写者争同一位置。
- **对 venue 的写入通道**：每 `WriteLaneKey` 一条有序队列（H4；交易协议下键为 (账户, 子账户)）。其有序与队首阻塞来自**通讯协议**（后续写的含义依赖队首结果，§6.4），不是 UTA 抢占通道的锁。UTA 不能通过“先拿到通道”改变 venue 消费什么，只能决定自己以什么顺序把请求交给协议。
- **记录追加与因果引用**：执行事实侧只 append，位置即顺序；SQLite 单写者是记录器的实现事实而非域语义。
  - C11 的“待决集合版本期望”是决定对 UTA 自身单据版本的**因果引用**：Decision 绑定 `(ticket, current_version)`（审批步，§6.3）。
  - 期望版本不等于 `current_version`，或该版本已有 Decision → `Conflict`，不执行、不改状态（§8.5）。
  - 这是 UTA 对自己记录的版本判定，不是对 venue 状态的锁。决定所依据的观察是否已过时，另由依据有效性门判定（§5.2）。
- **对外部状态无锁**：“依据有效性”只是对证据新鲜度的有界赌注，不阻止世界在 prepare 与 venue 执行之间变化（那是 F5 / 对账的领域）。
  - venue 若提供真正的锁（cancel/replace 引原单身份、条件单、expected-version 改单、幂等键唯一性、保证金预占），那把锁是 **venue 的**。
  - 它经能力证据声明（§2.2）后由 IO 壳使用；未声明则不存在，不得假装。

**结论。** 悲观锁由消费者自身持有、不侵入外部；乐观锁要比较的版本住在被消费状态的拥有者那里。UTA 不是消费者，既做不了悲观锁，做乐观校验时锁也不在它这里。DB 隔离级别的词汇（可重复读、串行化、悲观 / 乐观锁）不用于描述 UTA 自身。

### 读触发的写同形；授权是规则不是记录种类

AI 发请求、程序满足规则、审批人作决定，都是“读的副作用消费为写”，记录形状相同：带 principal、带依据 `LogPosition`。

- 差别只在**哪些 principal 的记录足以让写进入 prepare**，这由授权规则（顺序固定链，§6.3）决定。
- “谁、何时、依据什么”由记录上的 principal 与依据字段满足，不需要为审批另设记录种类。

### 不变量

UTA 对 venue 域事实无锁；DB 隔离词汇不描述 UTA 自身。由 F1 + 本节定位保证。

### 不选

- **给 UTA 自身造悲观 / 乐观锁**：UTA 不是消费者，锁不在它这里；过期读用依据版本可见即可（§5.2）。
- **为审批另设记录种类**：读触发的写同形，principal + 依据字段已足够；授权是规则不是记录种类。

## 6.9 不变量索引

以下不变量在任何时刻成立。完整表述与保证者在所有者节。

| # | 不变量 | 所有者 |
|---|---|---|
| 1 | 同 lane 阻塞头集合正常路径下至多一条未终结 Attempt；`Undetermined` 在 `Resolved` 前同 lane 无新普通写；两个例外扩大集合，集合为空才解除等待 | §6.4 |
| 2 | 执行事实侧只 append，永不改写为“从未发生” | §3.1 |
| 3 | 压缩后所有已登记引用的 `LogPosition` ≥ 其所在观察流的保留边界；未登记者得 `BeyondRetention` | §2.4 |
| 4 | 一张单据至多一个负责人 | §6.2 |
| 5 | 写处理器不绕过效应路径 | §6.1 |
| 6 | 观察侧不引用效应侧（不透明出处值除外） | §3.2 |
| 7 | 一张单据至多一次 `Close(Prepared)` | §6.2 |
| 8 | `Prepared` 无 `SendBarrier` = 确未发出；`SendBarrier` 无后继 = 可能已发出 | §6.5 |
| 9 | 组合子层失败与规则层 `Rejection` 两层各自封闭 | §2.5、§6.3 |
| 10 | 引用了没有集成提供字段的树 fail-closed | §2.5 |
| 11 | 锚点缺失 = 畸形记录；处理器字段缺失 = 不触发 | §2.1 |
| 12 | venue 状态映射保留 `Unmapped(raw)`，不伪造穷尽映射 | §2.2、§2.6 |
