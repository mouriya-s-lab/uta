# 6 效应宇宙

效应宇宙是可写的一半：形成意图、经决策代数放行、受控执行一次、用证据确认。它的机制都为写操作存在（§3.2）；效应侧对观察侧的引用只沿那条单向边，以位置引用承载，意图的那一份是 `basis`（§5）。

## 6.1 `EffectRequest`、处理器、程序解释②

> 图：D4.3 `EffectRequest` 分派与重派、D5.2 意图来源（`design/diagrams/04-program-host.md`、`05-ticket-and-sts.md`）。

程序的决策半边是**解释②（决策）**：`rules` → 对日志的 fold。其纯性由数据结构本身保证，而非开发约定。程序作为共享抽象见 §4.3。[证据：fp-01 M7 Mercury Workflow]

```rust
enum DecisionStep { On(Pattern, Box<DecisionStep>), Emit(EffectRequest), Require(Guard, OnFail), Expire(Deadline, Box<DecisionStep>) }
Emit(EffectRequest { effect_kind: EffectKind, payload: Bytes, basis: Basis })
```

### 规则

程序需要请求核心不认识的副作用：发通知、拉一次历史 K 线、调外部模型、下单。程序的唯一出口是一个 `EffectRequest`，不是对每种副作用各加一个构造子。

**请求是值，不是调用**（`IO a` 的纪律，§6.5）。它被 append 为记录；核心不解释 `payload`。

**响应由处理器决定。** 注册了该 `EffectKind` 的处理器接手。未注册则记录留在日志里为 `Unhandled`：有请求无处理器不是错误，与字段无处理器不触发同理（§2.1）。

**处理器在注册时声明读 / 写。**

**读处理器：**

- 立即经一次性读元素（§7.3）执行一次核心→集成的 `read`（§8.2）；同 identity 的在途调用照常并入（§2.2）。
- 集成作答（含上游明确拒绝）时，该流上同一事务 append 作答的 N 条观察记录与一条读结论记录（§8.2），出处 `OneShot{origins ∋ Request(该 EffectRequest 记录的 LogPosition), request}`（§3.4）。
- 程序按位置推进看到作答的记录：闭环走观察侧。
- `Unavailable` → 观察侧 `Gap{origin: Channel}`。核心未调用集成的情形（来源未登记或不是集成来源、来源从未有过声明版本、来源无当前会话、流不在会话有效声明里、会话有效声明对该流为 `Unsupported` 或 `Unknown`、请求不合 schema，判定顺序见 §8.2 `read`、§8.5 一次性读）不调用、不 append 观察记录。
- 读处理器**不自行重试**：一条请求一次执行，是否再请求由程序看到结果 / gap 后决定。读可重试的主体是发起者（§3.4）。

**写处理器：**

- 请求被当作 Intent，进入单据 → STS → IO 壳的完整效应路径；结果是执行事实与决议记录。
- **构造**：写处理器从请求载荷构造意图，只要求意图的锚点（§8.1）：`WriteLaneKey`、交易协议的操作种类之一、`basis`，撤单 / 改单还要 `target: VenueRef | IdemKey`；平仓的 `target: PositionRef` 由核心从 `basis` 所指的持仓观察记录构造（§6.2）。锚点构造不出（缺锚点、操作种类不在交易协议的封闭集合内、`IdemKey` 不是该作用域内某次尝试的 `SendBarrier` 记为订单键的键、平仓所指的持仓观察记录取不出 `PositionRef`，§6.2）即不是意图，不开单，记 `NotDrafted(Malformed{reason})`（见下）。参数（守卫字段与载荷）不在构造时判定：参数是否合规是单据 fold 的状态，送审时由输入约束步否决（§6.2、§6.3），所以程序发出的非法参数与人起的非法参数得到同一对记录（意图 + 否决，Q8）。[设计]
- **负责人**：写处理器以**发出成员的装载 principal**（装载该程序成员的人或服务账户，即开始该成员的 `Applied` 所记的 principal，见下文“发出成员”）为 `responsible` 开单（§6.2）。程序本身不是 principal。
- 该 principal 的授权范围决定单据能否不经人工直接放行（授权步，§6.3）。
- 开单的 `Draft.basis` 含该 `EffectRequest` 记录的位置：执行事实侧位置作因果依据（§5.1）。
- 程序意图没有编辑期：写处理器在同一事务 `Draft` 并 `SubmitForDecision`，单据直接进入 `AwaitingDecision`。
- 之后的退回、改写、移交由该 principal 经单据操作进行（§8.5），与人起的单据无异。

**完成事实在执行事实侧** [设计]。

- 每条被处理的 `EffectRequest` 恰有一条执行事实记录 `EffectResponse{request: LogPosition, outcome}`。
- `outcome` 按处理器分：
  - 读处理器：`Concluded(conclusion: LogPosition)`（读结论记录，含零条结果与上游拒绝）| `Unavailable(gap: LogPosition)` | `NotCalled(reason)`，`reason ∈ {Unsupported, Unconfirmed, NoSession, InvalidRequest, UnknownTarget}`（§8.5 一次性读；来源从未有过声明版本与此刻无已建立会话同为 `NoSession`，对应 §8.5 的 `Unavailable{source_state}`）；
  - 写处理器：`Drafted(ticket_id)` | `NotDrafted(Malformed{reason} | ScopeNotObserved)`。
- `EffectResponse` 与产生它的读结论记录 / `Gap` / `Draft` 同一事务 append；`NotCalled` 与 `NotDrafted` 没有伴随记录，`EffectResponse` 单独 append。
- **落点**：`EffectRequest` 与它的 `EffectResponse`（全部结果，含 `NotCalled` 与 `NotDrafted`）都在该程序的**请求流**上：按程序 id 各成一条的执行事实流。它们是 UTA 关于这个程序自己生命周期的事实，不属任何来源、lane 或作用域，所以没有有效 lane 或未登记来源的请求也有落点（§8.5 执行事实流）。
- **发出成员**：每条 `EffectRequest` 记下发出它的程序成员 `member`，即开始该成员的 `load_program` 或替换 `Applied` 在控制流上的位置。核心在 `Advance` 的输出事务里写下它：哪个成员在运行是核心自己的控制事实（§8.6）。
  - 该 `Applied` 以值记下成员的事实：装载 principal（该控制动作的 principal）、声明的执行事实输入集合、程序流集合、钉住的内容 hash 与接受的 `state_version` 集合（§8.5 `load_program`）。
  - 写处理器与重启重派只读发出成员的这些事实，不读当前成员的，也不重读程序值文件。请求可以在发出成员结束之后才被处理（§8.6 卸载与替换）；`Applied` 是控制流上的执行事实，成员结束、程序值文件被改或删去之后照样可读。
  - 不选：**按处理时的当前成员判定**：替换或卸载之后分派的请求会以另一成员的 principal 开单、按另一份声明判定作用域，卸载之后则无成员可读；**按请求与 `Applied` 的位置先后推断成员**：请求流与控制流之间没有可比的序（§2.3）；**结束成员之前处理完它的全部请求**：`Unhandled` 请求永远没有响应，在途读没有上界。
- 理由：观察记录可压缩（§2.4），`EffectRequest` 永存（§7.5）；“是否已处理”必须能从与请求同寿命的事实重建。
- `Concluded` 引用的读结论记录落到保留边界下后，`EffectResponse` 仍成立，不钉住保留。
- `Unhandled` 请求没有 `EffectResponse`，也不重派。
- **程序看得到自己的请求的结果** [设计]：程序只经它声明的输入得知结果，与其他输入同一套 cursor（§4.2、§8.6）：
  - 它的请求流是隐含的输入：同一条流上先有它自己的 `EffectRequest`（位置由此得知；程序按自己放进载荷的内容认出它们），后有指回这些位置的 `EffectResponse`，同流有序；
  - 要看写请求之后的单据与尝试，程序在值树里声明执行事实输入 `(来源, WriteScope?)`，与执行事实订阅同一个 selector（§8.5 订阅组）：该作用域各 lane 流上的单据记录（含 `Close` 的结局；放行时 `Close(Prepared(p))` 给出尝试位置 `p`）与 `p` 这次尝试的记录（`SendBarrier`、回执、`NotSent`、`Undetermined`、`ResolutionEvidence`、`ReconciliationReopened`、`Expired`、`Abandoned`，§6.5）按位置投给它；
  - 解释②在程序内按自己的锚点（`Drafted(ticket_id)` 所指单据、`Close(Prepared(p))` 所给的 `p`）挑出属于自己的事实；宿主与投递不做关联，只按位置搬运。请求流与 lane 流之间不定投递顺序（§8.5）：`Draft` 与 `Drafted` 同事务提交，程序可能先看到单据记录、后看到指向它的 `Drafted`，解释②按 `ticket_id` 两种次序都能匹配。解释①的节点不消费这些事实。
  - **构造规则**：写请求意图的 `WriteLaneKey` 所属的 `(来源, 作用域)` 不被发出成员的 `Applied` 所记的任何执行事实输入选中时，写处理器不开单，记 `NotDrafted(ScopeNotObserved)`（在 `Malformed` 判定之后）。它只由 UTA 自己的事实（发出成员的 `Applied` 与请求锚点）判定，所以程序不会写进一个自己看不到结果的作用域。
  - 理由：未调用与 `NotDrafted` 都不产生观察记录，程序只有经响应才看得到它们，而伪造观察记录去承载它们会把效应侧结论放进观察宇宙；程序若要组合多次写（例如先撤单、确认后再下单，§6.2 改单），它自己的尝试有没有结局只能从这些事实得知。撤单的回执或 `Found` 只说明撤单请求到达，不说明目标订单已结束：目标订单的状态要从订单状态观察读（§6.6 撤单的取证）。
  - 代价：声明的作用域里其他 principal 的单据与尝试同样投给程序，每批都唤起一次 `Advance`、计入它的预算；这是接受的代价。
  - 不选：**宿主按锚点动态关联、只投递程序自己的单据与尝试**：宿主要按已看到的 `EffectResponse` 维护一份可达集合决定投递资格，是一个新的关联索引机制（§0.1“机制即信号”）；**为程序另设结果订阅种类**：同一事实两条投递路径。

**请求与响应的关联是引用，不是事务。** `EffectRequest` 记录随程序 `Advance` 输出持久化（§8.6），处理器在其后执行。核心重启时 fold 出**无 `EffectResponse`** 的已注册请求（§9.2 #21）：

- 读处理器重新执行一次；
- 写处理器重新开单。`Drafted` 与 `Draft` 同事务，所以“有 `Draft` 无 `EffectResponse`”不可达；`NotDrafted` 已是响应，不重派。
- 重派同首次分派一样按发出成员的 `Applied` 所记事实开单与判定作用域；发出成员此后已被替换或卸载时亦然。

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

> 图：D5.1 单据状态机、D5.2 意图构造与参数合规、D5.3 顺序固定链（参数合规在输入约束步）、D5.5 两层对账与检查目录重算（`design/diagrams/05-ticket-and-sts.md`）。

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
    versions: Vec<Version<Intent>>, // 只追加；Intent = 意图类型（交易协议：下单 / 撤单 / 改单 / 平仓，见“交易协议”）；每版带其参数所依据的意图参数 schema 身份
    basis: Basis,               // §5：位置集，可含派生侧（观察）与执行事实侧位置
    parameter_validity: ParameterValidity, // 参数合规：当前版本的参数是否合目标来源声明的意图参数 schema（见“参数合规”）；不读观察，不属两层对账；输入约束步读取
    basis_validity: BasisValidity, // 第一层：依据有效性（§5.2 定义 basis_valid / BasisValidity；单据只应用此门）
    alignment: IntentAlignment,   // 第二层：意图专属对账，逐项状态（可能全部 InputMissing）
    latest_revision: Option<Revision<Intent>>,  // 最近一次 Revise 的结构差（派生；见编辑 diff）
    state: Drafting | AwaitingDecision(Hash) | Closed(Outcome),
}

/// 第二层：意图专属对账。一组检查，每项声明它需要哪些观察输入（required_inputs 见 §2.5）。
/// 由 (意图类型 × 该 venue 当前能力证据) 在评估时解析，不存在单据上；握手变了它就变。
struct AlignmentCheck<Intent> { name: CheckName, required_inputs: Set<StreamKind>, eval: fn(&Intent, &Observed) -> CheckResult }
type AlignmentChecks<Intent> = Vec<AlignmentCheck<Intent>>;
enum CheckResult { Aligned, Diverged(Divergence), Undecidable(Reason) }     // 输入齐全时的三种结果；Undecidable 的原因：范围内有 gap，或该流没有本项主体（该 instrument / 该持仓）的观察
type IntentAlignment = Map<CheckName, Aligned | Diverged(Divergence) | Undecidable(Reason) | InputMissing(Set<StreamKind>)>;
//                                                                   ^ 该项需要的输入观察侧没有（集成不提供该流）
enum ParameterValidity { Valid, Invalid(NonEmpty<Violation>), CapabilityNotEstablished(Unknown | NoSession), NotSupported, SchemaMismatch, TargetNotAccepted }  // 含义见“参数合规”

enum TicketAction<Intent> {
    Draft   { by: Principal, initial: Intent, basis: Basis },  // 建立单据 = 取得锁 = 声明负责
    Revise  { by: Principal, next: Intent, basis: Basis },     // 仅 responsible；追加版本，current_version 前进
    Transfer{ by: Principal, from: Principal, to: Principal },  // 显式移交，记录，不静默；by = from（自愿）或持有控制授权的 principal（强制，§8.5）
    SubmitForDecision { by: Principal, at: Hash },           // 送审：冻结 current_version，Drafting → AwaitingDecision
    SendBack { by: Principal, reason },                      // 审批退回：AwaitingDecision → Drafting，responsible 不变
    Close   { outcome: Prepared(LogPosition) | Withdrawn | DecisionRejected | Expired },  // 锁消失；Withdrawn 仅 responsible，DecisionRejected 仅决定者或 STS 链的否决，Expired 仅过期规则，Prepared 仅放行链
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

- `Closed` 无出边：之后任何 `TicketAction` 被拒，含再次 `Close`。`Prepared` 之后的命运属尝试（§6.5），不再经单据。
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

### 参数合规：意图参数 schema [设计]

每个订单意图，无论来自解释层代开的会话还是程序的写处理器，都按**目标来源为该操作声明的意图参数 schema** 判定参数是否合规，在任何 `Prepared` 之前。

- **schema 从哪来。** 每个 `Supported` 的写能力 `(scope, OperationKind)` 在其 `CapabilityProof` 里声明它接受的意图参数 schema 身份 `(schema_id, schema_version)`（§2.2、§8.1）：交易协议该操作种类的公共意图 schema（随 IDL 发布），或该集成以它为基础只增加字段与约束的扩展 schema，因而合扩展者必合公共者。schema 覆盖意图的全部参数：已注册的守卫字段与载荷（订单类型、time-in-force、来源专有选项等没有处理器读的参数）。判定读的是该来源的**会话有效声明**（§7.3 集成会话：只在该来源会话 `Established` 时存在，等于这一会话的声明版本及其 `CapabilityObserved` 的 fold），不读离线时留下的最近声明。
- **schema 语言。** JSON Schema，与配置文件的 schema 同一语言（§7.6），钉在一个 draft 版本与一个关键字子集上，`format` 只作注解不作断言；schema 与一个参考校验器随 IDL 发布，两个实现对同一参数得出同一结论由此可测（§10.5 #30、#31）。类型相关的必填与互斥（限价单要限价、追踪单的偏移量与百分比二选一、某来源只对市价单接受名义金额、某来源不支持的选项被禁止）写成 schema 自身的条件约束；合规与否只由该语言的校验语义决定，没有第二道由实现各自补写的语义校验。来源相关的限制由该来源的 schema 选定，不是交易协议的全局事实。
- **意图带身份。** 每版意图带它的参数所依据的 schema 身份，版本 hash 覆盖它。下游的参数在构建期按某个 schema 版本生成（`design/downstream/design.md` 第 4 节），程序按它读到的声明写参数；带身份使“按哪一版写的”不靠猜。
- **守卫字段的数量规则**（交易协议注册的字段，不随来源变）：下单与改单的新单部分，`quantity` 与 `notional` 恰有一个，有限且为正；平仓只可带 `quantity`（有限且为正），不带 `notional`；撤单两者都不带。C10 的“数量 / 名义有限且为正”即此。改单另带新单的**数量口径**：绝对量（改后的订单按意图所带的 `quantity` 或 `notional`）或剩余量（只与 `quantity` 合法：改后的订单按意图的 `quantity` 减去原单已成交的量）。口径的含义由上游在改单的那一次写里按它自己的成交记录落实（见“交易协议”改单）；核心只按 schema 校验口径是否被接受，不据它计算任何数量。
- **结果是单据 fold 的状态** `parameter_validity`：
  - `Valid`；
  - `Invalid(violations)`：参数不合该 schema 或违反数量规则，逐项列出违反之处；
  - `CapabilityNotEstablished(Unknown | NoSession)`：可执行性此刻无从判定：会话有效声明对该 `(scope, OperationKind)` 为 `Unknown`，或该来源此刻没有已建立的会话（没有会话有效声明）。它不是来源的否定，是非终结状态：单据等待（§6.3 输入约束步），会话建立或能力证据变化时重算，出口是等来确立的能力，或 `deadline` 到期（过期步）；
  - `NotSupported`、`SchemaMismatch`、`TargetNotAccepted`：会话有效声明已确立，而该版本对它不可执行，三者依次是下文“可执行性”的三个条件不成立，原因可区分：来源声明不提供这个操作（`Unsupported`，没有补救）；按新版 schema 重写参数即可；换一种目标身份（例如按 venue 订单身份）另起或改写即可。
  - 从 `Draft` 起即求值，每次 `Revise` 对完整的新版本重算（不只校验 diff），能力证据变化与该来源会话进入或离开 `Established` 时重算；负责人与审批人经读模型 `tickets` 看得到（§8.5）。
- **在哪里否决。** 输入约束步（§6.3）无条件读取它：`Invalid`、`NotSupported`、`SchemaMismatch`、`TargetNotAccepted` 即 `Rejection`（带违反项或原因）+ `Close(DecisionRejected)`；`CapabilityNotEstablished` 使单据停在输入约束步等待，不产生记录。它不是检查项，策略不能把它降为 advisory。草稿可以带着不合规的参数保存与修改，送审之前没有任何外部写；已确立为不合规的版本一经送审必得一对记录：意图与否决（Q8）。放行之后到发送之间可执行性若变，发出前门再核对一次（§6.5）。
- **核心仍不解释载荷。** 这是协议输入边界上的形状校验：核心按 schema 判定合不合规，不读取载荷里的值参与任何计算；没有处理器读的参数校验之后原样交给集成（§2.1）。意图参数 schema 是 UTA 的契约，不是上游请求格式。
- **构造不出的不是意图。** 缺锚点（`WriteLaneKey`、操作种类、`basis`；撤单 / 改单的 `target`）、操作种类不在交易协议的封闭集合内、`IdemKey` 目标不是该作用域内某次尝试的 `SendBarrier` 记为订单键的键（键由核心按尝试单射铸造，至多对应一次尝试，§6.5；见“交易协议”订单身份来源之二），或平仓所指的持仓观察记录取不出合规的 `PositionRef`（平仓的 `target` 由核心从该记录构造，见“交易协议”），意图构造不出，不开单：会话的 `draft` 得 `Rejected(Malformed)`、不 append 任何记录（与畸形记录在入口被拒同理，§2.1）；程序的请求得 `EffectResponse{NotDrafted(Malformed)}`（§6.1）。`revise` 交来的下一版构造不出时同样得 `Rejected(Malformed)`：不 append，单据与其 `current_version` 不变。

理由：程序不可信、会输出非法值（H2），只在解释层校验就留下程序这条绕过口；而参数到了集成才被发现不合规，集成只能在 `SendBarrier` 之后不发，返回 `NotSent`（§8.3）：审批人已批准、`SendBarrier` 已在、冷却已计时，负责人却要到那时才知道这一版注定发不出。参数合规作为单据状态，既让负责人在草稿期就看到问题，又让所有来源在同一步得到同一对记录。能力未确立单列为等待而不是 `NotSupported`：来源没有说“不提供”，把 `Unknown` 或离线记成来源的否定，就是替来源写了一条它没说过的结论（§0.1 权威源头与生命周期）。

不选：

- **只在解释层按生成的参数校验**：程序发出的意图绕过它（H2）。
- **作为一项检查（`AlignmentCheck`）**：检查项可被策略设为 advisory，且检查读观察、到放行门才生效，审批人会先批准一张注定不能发的单。
- **在 `draft` / `revise` 时拒绝参数不合规的版本**：留不下 Q8 要求的意图与否决记录；schema 声明在草稿与放行之间还会随握手变化，构造期的判定本就要重算。
- **交给集成拒绝**：见理由，`SendBarrier` 之后的本地拒绝只能是 `NotSent`，批准、屏障与冷却都已发生。
- **能力未确立即否决（`Unknown` 视同 `Unsupported`，或离线时按最近声明判定）**：来源没有给出否定，否决记录却说“来源不提供”；瞬断就关闭送审中的单据。
- **把参数约束写成组合子值树**（与规则、检查同一表示，§2.5）：值树不能同时充当下游在构建期生成参数的来源（`design/downstream/design.md` 第 4 节），同一份约束就要写两遍；JSON Schema 既是生成来源又是校验依据，钉住版本与参考校验器后两实现结论相同。

### 两层对账：偏离是状态

**偏离是状态，不是动作。** `basis_validity` 与 `alignment` 均为单据 fold 的一部分，在**效应侧**评估。评估用组合子（`AlignmentCheck.eval` 是 `Comb<Observed, CheckResult>`，§2.5）与 `fold_state` 机制（§4.1）。

两层输入不同：

- **第一层（`basis_validity`）**：只读 `basis` 位置集与各流当前已提交的流末、撤回、保留边界（§5.2）。
- **第二层（`alignment`）**：读 `required_inputs` 各流**当前流末**的 `fold_state`，经单向边“钩子读观察值”（§5.1）。
  - 当前流末 = 该 `StreamId` 已提交的全部记录，含 `one_shot` 与 `backfilled`，各带质量标记。
  - 不按任何完备进度截断：一次性读不推进完备进度（§4.2），截断就看不见为补齐输入而读来的观察（§3.4）。
  - 不读 `basis` 位置处的旧值，否则世界变了单据不会变。

**`checked_as_of`。** 每次评估把实际消费的位置集记为 `checked_as_of: Set<LogPosition>`，随 `CheckResult` 进入单据 fold。它还写进依据它放行或否决的 Decision/`Outcome`/`Rejection` 记录（§6.3）。

- `basis` 回答“拟单时看到了什么”；`checked_as_of` 回答“这次判断看到了什么”。
- 二者都不复制观察值。

**重算触发**（本身不是 `TicketAction`）：

- `basis` 引用的流或 `required_inputs` 各流推进、被撤回、出现 gap；
- 能力证据变化，该来源会话进入或离开 `Established`（也重算参数合规）；
- 保留边界推进；
- 策略必要项集、检查参数 / `Lag` 变化。

因此**单据是否偏离是状态字段，不需要外部触发对账**；单据始终知道自己与世界的关系。

- 只读校验门（§5.2）在提交时退化为读取 `basis_validity == Fresh` 及策略要求的必要项。
- `AwaitingDecision` 期间世界变了，审批人看到的就是一张 `Diverged` 单据，不需要另一套失效逻辑。

**两层分开的原因。** 第一层只依赖 `LogPosition`，对所有单据均可计算；第二层依赖意图类型与 venue 提供的观察。混成一层，会让“不能做意图对账”的单据连新鲜度都丢掉。

### 门只看必要项

放行策略按 `(WriteLaneKey, OperationKind)` 为交易协议检查目录（见“交易协议”）中的每项声明取 **必要**、**advisory** 或不列，并给出该项的参数（§7.6）。

- 仅必要项的 `Diverged`、`Undecidable` 或 `InputMissing` 触发 fail-closed（C12）。
- 其余检查项为 **advisory**：结果对审批人可见并写入依据，但不参与门。
- 不存在“所有 `Undecidable` 均阻断”的总门：那会让 advisory 在语义上重新变成 guard。
- 策略没有列出的项不求值；列出了需要参数的项却没给参数，或把只能 advisory 的项列为必要，该规则文件不合法（重载 `Rejected`，启动期按读配置失败处理，§7.6）。

**能力项恒为必要项**，是唯一不由策略声明、恒求值的必要项。

- 意图对会话有效声明**可执行**（交易协议“可执行性”）才 `Aligned`；已确立而不可执行即 `Diverged`，偏离原因即不成立的那个条件（`Unsupported`、声明的 schema 已不是意图所带的那个、目标种类不被接受，§2.2）。
- 能力未确立（会话有效声明为 `Unknown`，或该来源没有已建立的会话，即参数合规的 `CapabilityNotEstablished`）时，能力项不给结论：它不是 `Diverged`，单据停在放行门前等待（§6.3 放行门），会话建立或能力证据变化时重算，`deadline` 到期由过期步关闭。
- 策略不能把它降为 advisory：IO 壳对无能力的操作没有转移可走（§6.5）。

### 钩子

**钩子不一定存在，不一定能对账。** 第二层是多项独立检查的乘积，而非单一函数。

- 限价买单要对价格（报价流）、资金（余额流）、持仓（持仓流）、能力（能力证据）、可交易性（目录流）。
- venue 给报价不给持仓，就是“价格能对、持仓不能对”，不是整个钩子消失。
- 因此 `IntentAlignment` 逐项记 `InputMissing(缺哪些输入)`，不用 `Aligned` 冒充，也不设全局 `NoHook`。

**缺观察是可行动的。** 流在但没有本项主体的观察（`Undecidable`，如从没见过该 instrument 的持仓）时，若该流可一次性读，发一次只读查询就产生一条观察记录（§3.4），该项随即可算。读是安全的、可批处理的（§2.2）。

- 策略可对一项声明“先查后判”：该版本送审时，此项若缺主体的观察，核心为它发一次 `read`（出处 `OneShot{origins ∋ Ticket(TicketId)}`，§8.2），结果到达即重算；每个送审的版本至多一次，不自动重试。
- 真正的“不能对账” = `InputMissing`：该集成不提供这条流，没有任何渠道可得。

**钩子的输入是观察值及其出处**：派生 `Journal` 的 `fold_state`、能力证据、归因后的订单观察。

- 执行事实记录只作**身份与因果依据**（`basis` 中的 `VenueAccepted`/`SendBarrier` 位置），不进钩子的 `eval`。
- 归因后的订单观察是**记录**（集成或 IO 壳产出并带出处），不是读模型。“规则不引用读模型”不放宽。

**检查是纯函数、按 `Intent` 分派。** 每种操作种类的检查集是交易协议检查目录的一部分（见“交易协议”）；撤单的“原单仍在”只能是 advisory。新意图类型 = 新的检查集，核心不变。

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

### 交易协议：操作种类、目标、检查目录 [交易协议]

> 本节是预置写侧基本类型“订单”的协议内容（§0.1、§2.2），不属于核心代数：核心只按锚点、已注册字段与本节列出的检查求值。阈值、比例、允许集合、间隔等取值是业务，在规则文件（§7.6）。

**操作种类是封闭集合** [设计]：`Place`（下单）、`Cancel`（撤单）、`Replace`（改单）、`Close`（平仓）。`OperationKind` 是锚点（§8.1），集合外的值构造不出意图（见“参数合规”）。加一种是轴 B（§8.3）。

| 操作种类 | `target` | 守卫字段 | 写操作（一次上游写，§2.2） |
|---|---|---|---|
| `Place` | 无 | `side`、`instrument`、`quantity` 与 `notional` 恰有一个 | `submit` |
| `Cancel` | 订单身份 `VenueRef \| IdemKey` | 无 | `cancel` |
| `Replace` | 订单身份 `VenueRef \| IdemKey` | 新单部分同 `Place`；数量口径 | `submit`，上游以这一次写原子地完成改单，见下 |
| `Close` | 持仓身份 `PositionRef`（含 instrument） | `instrument` 由 `PositionRef` 给出，不单独填；`quantity` 可有可无 | `submit` |

一个操作种类恰是一次上游写：一次意图放行得一次尝试，一次尝试至多一次写调用（§6.5、§8.1）。声明表外的写操作使握手投影不合法；撤单的键角色只能是 `None` 或 `RequestKey`，不带键的写不能声明 by-key 与 replay-by-key 渠道，撤单不能声明 listing 与成交 / 持仓对账渠道（它们返回的记录不会归因到撤单请求，§6.6 撤单的取证）。

**目标身份是构造前提，“仍在”是 advisory。** 带 `target` 的意图类型在构造时**必须**携带目标身份（parse-don't-validate）。无目标即构造不出意图，不需要事后规则。身份进意图的 `target`，其来源记录的位置进 `basis`（§2.3）。

订单身份来源三种：

1. 本地 `VenueAccepted(venue_order_id)`；
2. 本地 `SendBarrier` 记为订单键（`OrderKey`，§2.2）的调用方键：无回执的提交。记为请求键的键只标识一次请求，不是订单身份；
3. 归因观察记录中的 venue 身份（外部订单，F9/P11）。

持仓身份只有一种来源：持仓观察记录里作用域内稳定的持仓身份（公共持仓 schema，§8.1；区分同一 instrument 的多空分仓与 venue 自有的持仓身份）。`PositionRef` 是含持仓身份与 instrument 的不透明值，由核心在构造平仓意图时从那条记录取得：调用方只指出持仓观察记录的位置（它进 `basis`），记录不存在、已落到保留边界之下、不是持仓记录或不属目标作用域，意图即构造不出（`Malformed`）。平仓意图的 `instrument` 就是 `PositionRef` 的 instrument，不另填，所以“目标持仓与 instrument 不一致”构造不出来，这由接纳边界保证，不靠调用方自律。

- 构造期只保证“目标存在且与账户作用域匹配”，**不**保证 venue 此刻能按该身份撤单、改单或平仓。撤单与改单的那一半是下文的可执行性：有调用方键 ≠ 能按键撤单（F6）。
- “原单仍在”来自观察侧 listing，按 F10 只能是 advisory，永不作为撤单放行的必要项。否则最安全的动作在 listing 滞后时被 fail-closed。

**可执行性** [设计]。一版意图对目标来源的写能力**可执行**，当且仅当在该来源的会话有效声明（§7.3 集成会话；只在会话 `Established` 时存在）中：

1. 该 `(WriteLaneKey, OperationKind)` 为 `Supported`；
2. 其 `CapabilityProof` 声明的意图参数 schema 身份等于意图所带的那个；
3. 意图以订单身份为 `target`（`Cancel`、`Replace`）时，`target` 的种类（`VenueRef` 或 `IdemKey`）在声明的 `target_kinds` 里。

该来源没有会话有效声明（此刻没有已建立的会话），或第 1 条的判定为 `Unknown`，可执行性**未确立**：它不成立也不被否定，读它的三处都等待而不否决。

这是一个谓词，三处读它，只是求值时刻不同：参数合规（依次得 `NotSupported`、`SchemaMismatch`、`TargetNotAccepted`，送审即在输入约束步否决；未确立得 `CapabilityNotEstablished`，在输入约束步等待，见“参数合规”）；能力项（放行门，§6.3）；发出前门的能力条件（§6.5）。

- 理由：只有调用方键、没有 venue 订单身份的撤单（例如撤一次结果未知的尝试投放的订单，§6.4）落在只能按 venue 订单身份撤单的来源上，集成只能不发、返回 `NotSent`（§8.3）：审批与屏障都已发生，撤单才被发现发不出。目标种类与参数 schema 一样只回答“这一版能不能被这个来源执行”，与世界状态无关，所以与 `NotSupported` 同在参数合规里：负责人从起单就看得到，送审即关闭，不会有人先批准一张注定发不出的单。
- 同一 `(scope, OperationKind)` 的各目标种类共用一个写声明与一组取证渠道；上游按目标种类走不同执行路径的，集成只声明它能按所声明写操作执行的那些种类（会推翻它的观测见 §10.4 #20）。
- 不选：
  - **只放在能力项**：审批人先批准，再在放行门失败；
  - **写进意图参数 schema**：`target` 是锚点，不是参数；
  - **交给集成判断**：只能在屏障之后得 `NotSent`，正是要消除的缺陷；
  - **按订单或 instrument 细分**：没有证据需要，§10.4 #20 是会推翻它的观测。

**平仓是有 venue 锁的操作种类** [设计]。

- `(scope, Close)` 的 `Supported` 断言的是上游自己给出的保证（§6.8 的 venue 锁）：对该来源意图参数 schema 接受的**每一个**平仓请求，执行它**不会增大目标持仓的绝对数量，也不会开出反方向持仓**，例如上游原生的平仓接口，或上游强制执行的 reduce-only 单。只能用普通反向单模拟的来源声明 `Unsupported`；只对部分品种有此保证的来源，要么以 schema 把接受域收窄到有保证的请求，要么声明 `Unsupported`。
- 数量口径由该来源的意图参数 schema 定（见“参数合规”）：上游原生支持“按执行时的整个持仓平掉”，schema 才允许省略 `quantity`（全平）；上游只能下有界的 reduce-only 单，schema 要求 `quantity`（至多减少这么多）。在 `submit` 里先读持仓再按读到的数量下 reduce-only 单，保证不增仓但不是“全平”：读与执行之间持仓可能变大。
- 集成在一次 `submit` 内可以先读后写，改变上游状态的调用至多一次（§8.1）。
- 下游仍可以自己组装一张反向 `Place`：那是业务，UTA 对它不作不增仓的任何保证。
- 理由：P6 与既有入口 A31/A34 把平仓列为操作；有的上游按持仓身份平仓，反向单根本不是平仓；而“平仓穿过零变成反向开仓”是加仓一类的伤害。UTA 自己的持仓观察只是对新鲜度的有界赌注（§6.8），不能担保不增仓；担保只能来自上游的锁。
- 不选：
  - **不设平仓操作种类，平仓一律是下游组装的反向下单**：按持仓身份平仓的上游从此不可达，不增仓的保证对 UTA 与下游都不可见。
  - **反向下单 + UTA 发送前重查持仓**：把本地观察当作权威（§0.1），读与执行之间仍可穿过零。
  - **按 instrument 在目录观察上声明“此处平仓不增仓”**：保证只对部分品种成立的来源因此仍可平那些品种；但不增仓是安全断言，放在观察上就随观察的新鲜度成立，陈旧的观察会把一次普通反向单当成有锁的平仓。保证只对部分品种成立的来源以 schema 把接受域收窄到有上游保证的请求，否则声明 `Unsupported`；接受域之外的品种由下游自己组装平仓。

**改单是单一意图类型 `Replace`，一次上游写原子完成，否则不支持** [设计]。

- `(scope, Replace)` 的 `Supported` 断言的是上游自己的保证：该来源意图参数 schema 接受的每一个改单请求，上游以**一次写**完成撤旧下新（或原地改单），并按意图所带的数量口径的原义落实（剩余量口径下“原单已成交的量”由上游按它自己的成交记录在这次写里计入）。不能按原义执行的口径不进 schema；不能以一次写完成改单的来源声明 `Unsupported`。
- 这只保证执行的是意图声明的口径，不是不增仓的保证：改单本可以加量。
- 核心不从观察计算任何数量，也不在尝试之内编排第二次写。需要“先撤、确认原单已结束、再下”的调用方，自己把撤单与下单作为**两张单据**组合：各自走完整的单据、STS 与 IO 壳，各有自己的授权、审批、冷却与尝试。撤单的回执或 `Found` 只说明撤单请求到达了上游，**不**证明原单已结束（§6.6 撤单的取证）；原单是否结束、成交了多少，要由调用方从订单状态观察判断（程序经 §6.1 看得到自己的尝试结局与观察流），这是业务（§0.1 下单的组合方式）。
- 理由：一次写之内的原子改单，结果是上游对这一次写的回答；拆成两次写，第二次写的时机与数量就要由核心从它手边的观察（原单终态与累计成交量的某条副本）得出，而那是上游的状态，核心只能请求并等待，不能在副本上作出结论去授权一次不可逆的写（§0.1 权威源头与生命周期）。撤了再下的时机与数量本是业务，放在调用方，它按自己的风险承担去读、去等、再下新单；新单作为一张独立的 `Place` 照常过授权、审批与冷却。
- 不选：
  - **核心以两腿计划 `[cancel, submit]` 执行不能原子改单的来源**：新单的数量与放行只能来自核心对原单终态观察的选取，上游之后更正终态或累计量时已发出的新单收不回；新单还绕过了 `Place` 的授权、审批与冷却。会推翻这一取舍的观测见 §10.4 #19；
  - **把两种执行做成两个操作种类**：同上，非原子的那一种仍要核心编排第二次写；
  - **由集成在一次 `submit` 里自行先撤后下**：两次上游写（§8.1 禁止），`SendBarrier` 二分崩溃窗口的保证不再成立。

**`ResolutionEvidence` 指尝试，终态与成交量是观察记录。**

- 执行侧 `ResolutionEvidence` 的 found/absent 回答“我的这次写到达了吗”。
- 派生侧观察记录反映订单的状态与累计成交量。
- 二者来自同一次 venue 交互：`Found` 时同一事务落执行侧一条记录与观察侧该回应的观察记录（回应含订单状态时订单状态一条，及每笔可识别执行一条成交记录，§6.5、§8.1），`Found` 以位置引用命中的那条观察记录，`Evidence`（契约载荷与原始负载）留在执行侧（记录模型，§6.5）。

**检查目录** [设计]。交易协议的第二层检查项是下表这个闭合集合（`CheckName` 按操作种类闭合，§7.7）。每项对同一组记录与同一组参数给出同一结果；数值一律按精确有理数计算（§2.6），不做舍入。“最近观察”指该流当前流末的 `fold_state` 中该主体最近的记录（见“两层对账”）；订单状态与持仓的“最近”按 §8.1“订单身份与最近观察”的规则取（只按来源给出的定序证据排序）：最近观察**顺序未确立**（同一身份有两条以上内容不同、彼此不可比的记录），或订单、持仓的身份出现在两条流上（跨流冲突），依赖它的检查项为 `Undecidable`。

| 检查项 | 适用 | `required_inputs` | `Aligned` / `Diverged` / `Undecidable` | 规则文件参数 |
|---|---|---|---|---|
| 能力 | 全部 | ∅（读能力证据，执行事实，不是观察流） | 见“门只看必要项” | 无；恒为必要项 |
| 可交易性 | `Place`、`Replace`、`Close` | 目录 | 该作用域该 instrument 的最近目录观察声明本操作种类“可写” / 声明“不可写” / 没有该 instrument 的目录观察 | 无；它判断的是最近观察的声明，不承诺上游此刻可写；观察陈旧的后果是上游对一笔真写的回应（通常 `VenueRejected`），不越过写边界的任何安全不变量 |
| 敞口 | `Place`、`Replace` 的新单部分 | 持仓、余额、目录、报价 | 见下 / 情景值 > `ratio · E` / 见下 | `ratio`（> 0） |
| 持仓在 | `Close` | 持仓 | 目标持仓的最近观察为未平，且带 `quantity` 时其绝对数量 ≥ `quantity` / 已平或数量不足 / 没有目标持仓的观察，最近观察顺序未确立，或目标持仓身份为跨流冲突 | 无 |
| 原单仍在 | `Cancel`、`Replace` | 订单状态 | 目标订单的最近观察为非终态 / 为终态 / 没有目标订单的观察，最近观察顺序未确立，或目标订单身份为跨流冲突 | 无；只能 advisory（F10） |

**敞口的情景值。** 它回答“按声明数量全部成交之后，这个 instrument 在该作用域的名义敞口占权益多少”，是按合约价格的情景估值，不是风险模型：不算 delta、不做换汇、不算保证金。

- `s`：`side` 买为 +1、卖为 −1。
- `q0`：该作用域该 instrument 的持仓在其所在持仓流上的最近观察中的带符号数量；同一 instrument 有多于一个持仓身份（多空分仓），其最近观察顺序未确立，或其持仓身份为跨流冲突（§8.1“订单身份与最近观察”），时 `Undecidable`，不净额化，也不在不可比的记录或两条流之间任取其一。
- `p`：意图参数带限价（公共意图 schema 的限价字段）时取限价，否则取最近报价观察中公共报价 schema 指定的参考价字段。
- `m`：最近目录观察中的合约乘数。
- `V`：意图带 `notional` 时取它，否则 `quantity · p · m`。
- `E`：该作用域最近余额观察中的权益。
- 情景值 = `|q0 · p · m + s · V|`；`Aligned` 当且仅当情景值 ≤ `ratio · E`。
- `Undecidable`：任一所需主体没有观察；`p ≤ 0`、`m ≤ 0` 或 `E ≤ 0`；`p`、`V`、`E` 的币种不全相同。缺值与 gap 都不放行必要项；实际消费的观察进 `checked_as_of`。
- `Replace` 以意图声明的新单数量作全部成交的情景：剩余量口径下上游实际改成的数量不大于它；原单在改单之前的成交不在 `q0` 里，所以它不是对改后敞口的担保。

**目录之外的 guard 是下游的决定者。** 目录与 STS 参数（§6.3）之外的任何规则（按策略、按组合、按外部模型的判断）由下游代码实现：一个 principal 订阅待决单据并经 `decide` 作决定（§8.5），或程序在 `Emit` 之前自己把关。其边界：

- 它只在策略要求人工审批的单据上起作用，并占用该 `current_version` 唯一的一条 Decision（§6.3）：它批准后同一版本不能再由另一审批人决定，它否决则单据关闭。要“自动 guard + 人工审批”，它只作否决或 `SendBack`（`SendBack` 不是 Decision，不占这一版的决定），批准留给人工审批人；或由人工审批人参考它的输出作决定。它想改判一个已批准的版本，只能经 `SendBack` 退回草稿、由负责人 `Revise` 出新版本。
- 它的判断在作出时一次成立，不随世界推进重算；目录检查会重算。
- 它不能越过核心的否决：放行时依据有效性门与必要项照常生效（§6.3）。
- 程序在自己 `Emit` 之前的把关只管它自己发出的意图，不是对所有来源的 guard。
- 加一项目录检查是改交易协议（`CheckName` 加一个变体，编译器指出全部遗漏），不是改规则文件。

理由：放行与否是外部可观测的行为，同一个规则文件在两个实现里必须给出同一个放行 / 否决，所以每项检查的输入、判据与参数都在这里写定；阈值与比例本身仍是业务，由规则文件给出。不选：只在规则文件列检查名、判据留给实现（两实现对同一单给出不同结果）；把所有 guard 都做成下游决定者（它不能对所有来源生效，也不随世界重算）。

### 意义与产品规则

- **锁最大的意义不是互斥，是入口**：它是 UTA 表达“**现在有一张单据，我是负责人**”的唯一方式。取得单据 = 单据存在 + 某 principal 从此负责；后续编辑、送审都以这个身份记账。没有负责人的单据不存在。
- **一份单据只有一种交易意图**：不分叉不合并，订单不能“同时想买又想卖两个价”。版本链线性；`Revise` 在锁内追加，不需要 hash 期望比较。
- **产品层代价与协作边界**：两个 AI 不能同时处理同一张单据。合理，但不好用。核心有意接受这个代价，不用分叉 / 合并修补。协作在核心之外：`Transfer` 移交；第二个 AI 另起单据，由决定者二选一；或把建议发给负责人。
- **决定绑定 current_version**：Decision 引用 `AwaitingDecision(current_version)` 的 hash；进入 `Prepared` 要求被决定的版本 = 当前 current_version。`SendBack` 后的 `Revise` 使 current_version 前进，旧 Decision 自然失效。
- **一张单据至多一次 `Close(Prepared)`**：之后的改动是新单据（改单 / 撤单 / 平仓各自起单），各走各的写边界。
- **单据不记起单理由** [设计]：`Draft`/`Revise` 与批准都不带负责人或审批人撰写的自由文本。单据记录回答“谁、何时、依据哪些位置、哪一版”（S8）；自由文本只出现在对他人意图的判断上：`SendBack` 的原因、否决的原因、规则 `Rejection` 的违反项，读模型 `tickets` 按版本给出它们（§8.5）。“为什么想下这一单”是策略的业务上下文，由下游按它拿到的请求引用自己保存（§10.6）；它不能放进意图参数，意图参数会原样交给集成。不选：`Draft`/`Revise`/每条 Decision 都带备注，没有任何核心代数读它，而批准本就不带原因（P7）。

### 不变量

- 一张单据**至多一个负责人**（`responsible` 存在即锁），持锁期间仅 `responsible` 可 `Revise`。由单据锁保证。
- 一张单据**至多一次** `Close(Prepared)`；`AwaitingDecision` 期间 `Revise` 被拒。由状态机穷尽转移保证。
- 门只对必要项 fail-closed，advisory 不参与门。由放行策略声明保证。
- `basis_validity == Fresh` 且必要项 `Aligned` 才进 prepare。由单据应用只读校验门保证（门定义在 §5.2）。
- 参数不合目标来源声明的意图参数 schema 的版本送审即被否决，不进 prepare；这对所有来源成立。由输入约束步无条件读取 `parameter_validity` 保证（§6.3）。

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
    type RuleState;  // 规则判断要用的状态：冷却、待决集合版本、lane 阻塞头、过期；每次求值由记录 fold 出，不另存（见下）
    type Input;      // 本次输入：意图、决定、deadline 到时、回执、对账证据
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

**核心状态最小化。** 核心只持有规则运行所需的状态。订单、持仓等读模型只是对记录的非权威 fold（种类与输入见 §8.5），可表现为具名数据结构，但**不作权威、不被规则引用**（§4.4）。[证据：fp-03 命题 1]

**记录类型按 stream 参数化。** 每条 lane / 账户 stream 拥有独立的 Input 集与 decider（Equinox `Category`/`StreamId` 模式），核心不定义全局 effect enum。[证据：fp-03 条目 7]

**失败类型结构。** `NonEmpty<Rejection>` 保留依赖结构；`Validated` 式累积仅用于相互独立的校验项。[证据：fp-04 命题 8]

**`RuleState` 是记录的 fold，不另存** [设计]。`step` 读的 `RuleState` 是一个值，每次求值时从执行事实与规则文件求出；`step` 的输出是记录（Decision、`Outcome`、`Rejection`、`Close`、`Prepared`），不是对某张状态表的更新。每个成员的源头：

| 成员 | 源头（fold 的输入） |
|---|---|
| 冷却时钟 | 该 `(WriteLaneKey, instrument)` 上下单写的 `SendBarrier` 记录（见“冷却”），间隔取规则文件 |
| 待决集合版本 | 该单据的 `TicketAction` 与 Decision 记录：`(ticket, current_version)` 有无 Decision |
| lane 阻塞头 | 该 lane 执行事实流上各尝试的记录与 `bypass_lane` 控制记录（§6.4、§6.5） |
| 过期 | 单据当前版本所带的 `deadline` 与核心 UTC 时钟（§2.6） |

- **等待与重入。** 链的一步可以不产生记录而停下：单据保持 `AwaitingDecision`，停在该步。停下的点有三类：审批步（待 Decision）、lane 步（阻塞头集合非空）、能力未确立（输入约束步读到 `CapabilityNotEstablished`，或放行门的能力项未确立，§6.2）。停下不是一条记录，也没有等待标志；链由 STS 规则链在下列事件的事务提交之后对相关单据重新求值：
  - 停在 lane 步：该 lane 上 `SendBarrier`、`VenueAccepted`、`VenueRejected`、`NotSent`、`Undetermined`、`ResolutionEvidence`、`Expired`、`Abandoned` 与 `bypass_lane` 控制记录的 append；
  - 停在审批步：该单据的 Decision；
  - 能力未确立：该来源的声明版本、`CapabilityObserved`，会话进入或离开 `Established`；
  - 任一停下的点：该单据 `deadline` 的计时器到期（过期步对停在任一点的 `AwaitingDecision` 单据生效，见“过期步”），`reload_config(rules)`。
- **重入经过 lane 步** [设计]。从任一等待（能力或会话未确立、审批、lane）恢复的单据，重跑链时都依次重过 lane（阻塞头）→ 冷却 → 过期 → 放行门，每一步照常可以停下或否决：停在审批步、lane 步或放行门前的单据从 lane 步起重跑；停在输入约束步的单据还没有过审批与 lane，从输入约束步起重跑，同样经过 lane 步。单据从不由放行门前的等待直接进入 `Prepared`：append `Prepared` 的那次求值必定刚过了 lane 步与冷却。例外只有两个，都不产生 `Prepared` 的捷径：规则版本变更从授权步起重过全部五步（见“规则版本变更”）；`deadline` 计时器只求值过期步（见“过期步”）。
  - 理由：阻塞头只算已 `Prepared` 的尝试（§6.4），停在放行门前的单据不是阻塞头。同一 lane 的两张单据可以在离线期间都过了 lane 步、停在放行门前；会话恢复时若从门续跑，两张先后 `Prepared`，同 lane 出现两次等待中的尝试，冷却也被绕过。从 lane 步重跑，先放行的那张成为阻塞头，后一张在 lane 步看到它而停下。
- **原子性。** 一步产生的全部记录同一事务 append；放行时 `Prepared` 与 `Close(Prepared)` 同事务（§6.2）。没有“记录 + 状态表”的双写。
- **恢复。** 重启后对每张 `AwaitingDecision` 单据按记录重新求值（§7.2 第 4 步）：停在哪一步、等什么，都由记录与当时的会话、能力、时钟重新得出。
- 理由：这些成员都是 UTA 自己的记录的函数；另存一份就是在源头之外的副本，它的写者（STS）看不到改变它的全部输入（阻塞头由 IO 壳的记录改变），会陈旧而误放或误挡（§0.1 近处副本）。没有实测的性能需要，不加缓存；需要时按近处副本的纪律另加，写清派生、失效与刷新（会推翻它的观测见 §10.4 #28）。
- 不选：**持久化 `RuleState` 表并与决策同事务更新**：两个源头（表与记录的 fold）并存，IO 壳 append 的记录不经 STS 就改变阻塞头，表随之陈旧，还要一个没有归属的唤醒者。

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
| 输入约束 | 读单据 fold 的 `parameter_validity`（§6.2 参数合规）；守卫字段校验：子账户已枚举、instrument 在策略的允许集合内（允许集合只对带 instrument 的操作种类，`Cancel` 不适用） | C9/C10；守卫字段处理器（§2.1） |
| 审批 | 策略要求人工则送审，等待带版本的 Decision；不要求人工则以 `rule_version` 为依据直接通过 | C3；H6；C11 |
| lane | 读该 `WriteLaneKey` 的阻塞头集合；集合非空则停在本步；集合为空时判冷却，冷却期内否决，否则放行 | H4；C12 |
| 过期 | `deadline` 过期规则，由 `deadline` 到时触发 | H6 |

**授权步。** 查询的是：哪些 principal 的记录足以让写进入 prepare。程序意图的单据以发出成员的装载 principal 为负责人，它的授权范围决定能否不经人工直接放行。

**输入约束步** [设计]。以下各项彼此独立，以 `Validated` 累积，一次否决列出全部违反：

- `parameter_validity` 为 `Invalid`（带逐项违反）、`NotSupported`、`SchemaMismatch` 或 `TargetNotAccepted`，各自单列，四者原因可区分（§6.2）。它无条件生效，策略不能关闭。为 `CapabilityNotEstablished` 时本步不否决也不放行：单据停在本步等待（见“`RuleState` 是记录的 fold”），与其余各项无关；能力确立后重新求值全部各项。
- 目标子账户已枚举（C10）。
- **允许集合**：策略为该 `(principal, WriteLaneKey, OperationKind)` 给出 instrument 允许集合时，意图的 instrument 不在集合内即否决；不给出即不限制；给出空集即全部否决。
- **不在本步：instrument 是否属目标账户。** 这是 venue 的事实，UTA 没有它的源头，也没有可询问的契约操作；核心在写调用之前只否决由自己的记录与声明值判得出的不合规（C9）。instrument 能否在该账户交易，由上游对这次写的回答给出（`VenueRejected`）；策略要求提前挡住已知不可写的 instrument 时，列出可交易性检查（目录观察，§6.2 检查目录）。
- **各项的适用范围** [设计]：一项校验只对带它所读字段的操作种类求值（守卫字段表，§6.2 交易协议）。意图的 instrument 是 `Place` 与 `Replace` 新单部分的守卫字段，`Close` 取 `PositionRef` 的 instrument；`Cancel` 没有 instrument，允许集合对它不适用，它的目标订单属于目标作用域已由构造保证（§6.2 目标身份）。C10 按作用域而定，对全部操作种类照常适用。策略给 `Cancel` 配 instrument 允许集合，规则文件不合法（§7.6），不在运行期忽略。理由：`Cancel` 没有 instrument，规则无从比较；否决全部、放行全部或在运行期忽略，是三种都说得通、结果不同的做法，而写这条规则的人以为它在起作用。给文件判不合法与“给 `Close` 配冷却”同理。
- 否决 = `Rejection`（带违反项与 `rule_version`）+ `Close(DecisionRejected)`。

**审批步。**

- 是否需人工由策略按 `(principal, WriteLaneKey, OperationKind)` 给出：总是、从不，或“名义超过阈值 N 时”。第三种下，意图带 `notional` 且 ≤ N 则不需人工；带 `notional` 且 > N、或以 `quantity` 定量（不带 `notional`）则需人工。STS 不读观察，不估算以数量定量的单子值多少钱；估算属于敞口检查（§6.2），而不能比较时一律走人工是 fail-closed 的一侧。`Cancel` 既不带 `notional` 也不带 `quantity`，第三种条件对它无从判定，只能给“总是”或“从不”；给它第三种条件，规则文件不合法（§7.6），理由同输入约束步的适用范围。
- 决定者按 `(principal, 动作种类)` 授权。
- 另一笔过期未决独立处理。
- **一个 `current_version` 至多一条 Decision** [设计]：`decide` 的接受判据是“该 `(ticket, current_version)` 尚无 Decision 记录”（C11 的待决集合版本即此）；已有 → `Conflict(AlreadyDecided)`。
- 理由：决定之后单据可能仍停在 lane 步而版本不变，靠 `Closed` 挡不住同版本的第二条决定。

**lane 步。** 阻塞头集合的定义与语义见 §6.4。

- 集合非空时本笔停在此步，单据保持 `AwaitingDecision`，不产生 `Prepared`。
- 不等待阻塞头的只有两种（§6.4）：以阻塞头中某次尝试记下的订单键为 `target` 的撤单意图；有覆盖当前全部阻塞头的绕过控制记录的单据版本。

**冷却** [设计]。冷却是 lane 规则的判断，其时钟是执行事实的 fold：

- **键与时钟**：`(WriteLaneKey, instrument)` 上最近一次“下单写”的 `SendBarrier` 记录时间（UTC，§2.6）；instrument 经 `SendBarrier` 的 `AttemptRef` 回连其 `Prepared` 所载的意图取得。下单写 = `Place` 与 `Replace` 的尝试；撤单与平仓的尝试不设、也不受冷却。
- **间隔与键同轴**：间隔由策略按 `(WriteLaneKey, OperationKind)` 给出（只对 `Place`、`Replace` 可给），跨 principal 共享，不按 principal 分。理由：时钟本就跨 principal；若间隔按 principal 给，间隔短的 principal 不断刷新共享时钟，间隔长的 principal 永远等不到，结果是“最先用完的那个限制”而不是任何一行声明的限制。
- **更新点**：`SendBarrier` 持久化之时（可能已发出，§6.5）。`Prepared` 未发即 `Expired` 的不计；因而没有真正发往上游的写也可能计时（`SendBarrier` 之后、调用之前崩溃，或集成返回 `NotSent`），这是有意接受的保守代价。
- **判定点**：单据经 lane 步放行的那一刻判定，不是等待条件；单据从等待重入时重过 lane 步，冷却随之重判（见“等待与重入”）。该 `(WriteLaneKey, OperationKind)` 有间隔 `d` 时，当前时刻 < 该键时钟 + `d` 即否决：`Rejection::Cooldown{until}` + `Close(DecisionRejected)`；等于或晚于即通过。正常路径下放行时同 lane 前一次尝试已结束等待，它的 `SendBarrier` 已在记录里；同一键上若有已 `Prepared` 而尚无 `SendBarrier` 的下单写（只在绕过时出现），本次判定不放行（否决）；它不设时钟，那次尝试越过屏障或 `Expired` 后这一阻碍随之消失。间隔为 0 等同不设冷却。`bypass_lane` 只越过阻塞头等待，不越过冷却。
- **恢复**：时钟是对 `SendBarrier` 记录的 fold，重启后由记录重建，没有另存的状态。
- 理由：在检查通过时计时，放行后未发出也占冷却，且审批等待期间计时已开始（旧实现的缺陷，O11）；以业务回执计时，被拒或结果未知的发送不计冷却，丢掉了“可能已发出”的依据。放在 lane 步而不是输入约束步，因为单据可能在审批与 lane 上等很久，判定必须贴近放行。撤单与平仓是减少风险的动作，不应被冷却挡住。
- 不选：冷却作为一项检查（`AlignmentCheck`）：检查只读观察值，执行事实不进钩子的 `eval`（§6.2）；冷却让单据等待而不是否决：待决单据会在 lane 上堆积，与 C12 的“规则否决”不符。

**过期步。** `AwaitingDecision` 期间到期 = `Close(Expired)` 否决记录，不补偿。这包括停在审批步、lane 步，以及因能力未确立停在输入约束步或放行门前时：`deadline` 计时器到期即对该单据求值过期步，不先重跑它前面停下的那一步。

### 放行门

进入 `Prepared` 前，链读取单据 fold 的三项状态：

1. `basis_validity == Fresh`（§5.2）；
2. 必要项 `alignment` 为 `Aligned`（§6.2）；能力项恒在必要项内，不由策略声明，它按会话有效声明核对可执行性（含意图所带的参数 schema 身份仍是声明的那个）；
3. `AwaitingDecision(current_version)` 与决定绑定的版本一致。

能力项未确立（该来源此刻没有已建立的会话，或会话有效声明对该操作为 `Unknown`）时，门不求值：单据停在门前等待，能力确立后从 lane 步起重跑（见“等待与重入”），`deadline` 到期由过期步关闭。其余任一不满足 → `PredicateFailure`（fail-closed，C12），不发出。

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

**等待机理。** lane 上有等待中的尝试（最长的情形是队首 `Undetermined`）时，后续意图必须等待。

- 原因：**后续写入的语义依赖队首结果**，即同账户 buying power、待撤订单是否存在、venue 侧顺序。这是与 venue 的通讯协议语义，而非数据库层面的并发互斥。
- **lane 阻塞头等待发生在 `Prepared` 之前**：等待者是 `AwaitingDecision` 的单据，不是已放行的记录。停在放行门前等会话或能力的单据也还不是阻塞头，所以它恢复时从 lane 步重跑（§6.3 等待与重入）。因此正常路径下同 lane 至多一次等待中的尝试；`Prepared` 一旦 append 即交给 IO 壳，过发出前门即发；门的会话或能力条件不成立时，尝试在门前等待，仍是阻塞头（§6.5）。
- 等待期间单据的 `basis_validity`/`alignment` 照常重算（偏离是状态，§6.2）。放行时链先过过期步，再过依据有效性门。
- 等待超过 `deadline` 由过期步 `Close(Expired)` 终结。

**阻塞头集合。** 阻塞头是该 `WriteLaneKey` 上**等待中的尝试的集合**：UTA 仍在等它的结论、还没有结束等待的尝试（§6.5 等待与结果）。等待中 = `Prepared` 无 `SendBarrier` 且未 `Expired`、`SendBarrier` 无后继，或 `Undetermined` 既无 `Found`/`Absent` 也无 `Abandoned`。集合是该 lane 执行事实流上这些记录（与 `bypass_lane` 控制记录）的 fold，不另存（§6.3“`RuleState` 是记录的 fold”）。

- 正常路径下集合至多一条。
- 两个例外使集合扩大：撤阻塞头的撤单意图、显式绕过（见下）。
- 集合内每条独立结束等待：由各自的对账驱动得到结论（§6.6），或由 principal 放弃（`Abandoned`，§6.6）。任何一条结束只把它移出集合；**集合为空**才解除普通写的等待，不存在“部分解除”。
- 集合非空时仍可继续加入获准的例外（再一笔撤阻塞头、再一次绕过）。

### 无第二类越顶队列

队首取证期间 lane 上只发生决议动作。读侧的决议动作（按键查询、listing、成交 / 持仓对账）属于 IO 壳的对账协议，不是队列项。

**唯一不违反协议而越过阻塞头的写**：**以阻塞头中某次尝试记下的订单键为 `target` 的撤单意图** [设计]（目标身份来源之二，§6.2）。

- 键按尝试铸造与登记（§6.5、§8.1），所以资格按尝试判定：`target` 必须精确等于阻塞头集合中某次已有 `SendBarrier` 的尝试在其 `SendBarrier` 里记为订单键（`OrderKey`，§2.2）的键。键角色取自该尝试发出时的声明、随 `SendBarrier` 落盘，之后的声明不改写它。撤单尝试的键只标识一次撤单请求，不合格；下单、平仓与改单的尝试只有声明了订单键时才合格；尚无 `SendBarrier` 的尝试没有登记的键，也不合格。
- 该键能否被上游用来撤单，仍由可执行性判定（目标种类 `IdemKey` 在 `(scope, Cancel)` 的 `target_kinds` 里，§6.2），不因它越过等待而放宽：只能按 venue 订单身份撤单的来源上，这笔撤单送审即以 `TargetNotAccepted` 否决，不进阻塞头集合；那里的阻塞头只由取证渠道、`Attributed` 得到结论，或由 principal 放弃。
- 它经单据与 STS 全链；lane 步对它不施加阻塞头等待。
- 理由：等待的依据是“后续写的语义依赖队首结果”，而这笔撤单不依赖队首结果，它存在的目的就是把 venue 侧推到一个读得出的状态。
- 放行后它成为该 lane 阻塞头集合的第二个成员，与阻塞头各自独立结束等待。
- 它不是对协议的违反，不需要 `bypass_lane`。

**它自身的结果不决议阻塞头。** 撤单的回执与 `Found` 只说明撤单请求到达了上游；把“撤单被受理”解释成“原单已撤”，或把“撤单被拒”解释成“原单不存在”或“已成交”，都是 heuristic，禁止（§6.6）。阻塞头的结果只由它自己的取证渠道与 `Attributed` 给出，撤单不提升任何渠道的证明力（listing 未见仍是 `Inconclusive`，F10），也不自动重开阻塞头的取证：撤单有了结论之后要不要再问一次阻塞头，由 principal 经 `retry_reconciliation` 决定（§6.6）。

### 显式绕过

带 principal 的显式绕过仍可存在。它是控制动作 `bypass_lane(ticket)`（§8.5）的结果，记为控制记录 `Applied`，是**对协议的自觉违反**，不是队列的一种模式，也不是 Decision：它不回答“这一版批不批”，不占该 `current_version` 唯一的 Decision（§6.3），不出现在 P7 的裁决集合里。

绕过的语义 [设计]：

- **范围**：`Applied` 记录带 principal、单据、该单据的 `WriteLaneKey`（由单据取得，请求不另给）、该单据生效时的 `current_version`，以及生效时刻该 lane 阻塞头集合的位置集。它只对这一版本、只对这些阻塞头有效：`SendBack` 后 `Revise` 出的新版本不继承它；它生效之后才加入集合的阻塞头不在其内，单据仍为它们等待。它随该 lane 的执行事实流投递（§8.5），lane 的订阅者看得到每次绕过。
- **生效条件**：单据处于 `AwaitingDecision`；否则控制记录 `Rejected`。
- **作用**：lane 步在放行时读这条执行事实，只跳过它所列阻塞头的等待。它不越过授权、输入约束、审批、冷却、过期与放行门：未获批准的单据照常停在审批步。
- 放行后该 lane 同时存在多次等待中的尝试，按阻塞头集合的规则各自结束等待。
- 绕过只影响本 lane。
- 理由：绕过授权的是“不等这些阻塞头”，不是对意图版本的审批；把它记成 Decision，要么占掉批准所需的那一条，要么让一版有两条 Decision。

lane 的有序与队首阻塞来自通讯协议，不是 UTA 抢占通道的锁；无锁定位见 §6.8。

### 不变量

- 同 lane 的**阻塞头集合**在正常路径下至多一次等待中的尝试；任一 `Undetermined` 在结果确立或被 principal 放弃之前，同 lane 无新普通写。例外只有两个，且二者都扩大阻塞头集合：以阻塞头中某次尝试记下的订单键为 `target` 的撤单意图；显式绕过（`bypass_lane` 的控制记录在案，只对所记版本与所记阻塞头有效）。由 STS lane 步在 `Prepared` 之前等待、任何重入都重过 lane 步（§6.3）+ IO 壳按序推进保证。
- 显式绕过必须有带 principal 的 `bypass_lane` 控制记录。由绕过语义保证。

### 为什么

提交是消费动作，消费方始终是 venue，UTA 从头到尾自己都不能决定是否消费。所以“队首阻塞”只能是与 venue 的通讯协议语义，而非 UTA 的锁。

lane 的键取自握手 `WriteScope`，因为核心不知道也不该知道上游账户结构（§2.2）。[域 H4/H1；证据：fp-03 条目 7]

## 6.5 IO 壳：两阶段、尝试、转移表

> 图：D6.1 尝试的状态机、D6.3 一次交互的记录矩阵（`design/diagrams/06-io-shell-attempt.md`）；D7.1 恢复时的发出前门（`07-crash-recovery.md`）。

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
- 这正是 F5 所需的定义。故 `Prepared → SendBarrier → (VenueAccepted | VenueRejected | NotSent | Undetermined) → ResolutionEvidence*` 保持为两阶段协议，而非退化为“发一次、超时算失败”的单阶段调用。[证据：fp-06 命题 1]

**在 2PC 中的位置。** UTA 是协调者；venue 是**总是单方面决定的参与者**：不 prepare、不等协调者裁决，收到请求即自行 commit 或 reject，协调者事后只能发现结果。[证据：fp-06 命题 5 与案例——Stripe/Binance/Temporal 均无 participant prepare]

- 把这一情形类比为 2PC 中“参与者单方面完成、协调者进入 in-doubt 并以查询 / 日志决议”的分支（XA heuristic outcome），是**设计类比** [设计]。
- venue 从未参加 prepare，也没有 commit/rollback 可下达；UTA 这一侧缺少 XA 的 participant 契约（prepare/commit/rollback），只借用它对中间态与决议出口（found / absent / inconclusive）的定义。
- 名字保持“两阶段”，因为中间态语义与决议流程都来自它；只是 UTA 永远处在参与者已单方面决定的那一支。[证据：fp-06 命题 1（in-doubt 一等化）]

### IO 壳是效应侧的解释器

IO 壳不是“调用 venue 的那个函数”，而是效应侧的解释器：`Prepared` 之后、决议之前发生的一切都在它里面。它与 Haskell 的 `IO` 同构，先定义，再运行：

| Haskell | UTA |
|---|---|
| `IO a` 值（纯，可构造、可组合、可检查） | `Prepared` 记录：已持久化的“将对 venue 做什么”的描述；构造过程（草稿 → 决策 → append）不接触 venue |
| `>>=` 组合：后一步依赖前一步的结果 | lane 全序：同 lane 后一次尝试的语义依赖前一次的结果，故 lane 步在前一次仍在等待时不放行下一次（§6.4） |
| runtime 运行 `main` | IO 壳解释 `Prepared`：核心中唯一发出写调用的环节；调用在上游的落实由集成完成（§8.1） |
| 效果发生后只剩结果值与外部状态变更 | 效果发生后只剩 append 的记录（`VenueAccepted`/`VenueRejected`/`NotSent`/`Undetermined`/`ResolutionEvidence`；未发出即到期则 `Expired`）与 venue 侧变更 |
| `unsafePerformIO` 是禁忌 | 规则、程序、消费方直接调用 venue 写接口，或集成在 IO 壳的调用之外自行发起写，是禁忌 |
| 纯代码可随意重算，`IO` 不能 | 派生侧与决策可以重放重算，`Prepared` 的实际执行不能重放 |

二者唯一差异正是 F5：Haskell runtime 总能知道 `IO` 动作做完没有；UTA runtime 面对 venue 时可能不知道。因此 UTA 的“运行”多出 `Undetermined` 结果与配套对账，其余纪律与 `IO` 相同。

“先定义再运行”是 IO 壳的全部设计原则：任何“运行”的东西先是一条记录；任何记录都不是运行。

**位置。** 单据（§6.2）与 STS 决策（§6.3）位于上游，产出 `Prepared` 记录；读模型与消费方位于下游，只读记录。IO 壳是核心中**唯一**向集成发出写调用、把集成的返回值变成记录的地方；核心内不存在第二处接触集成进程写接口的路径。

### 输出

执行事实侧只有 append：

- `SendBarrier`、`VenueAccepted`、`VenueRejected(reason)`、`NotSent(reason)`、`Undetermined(reason)`、`Expired(deadline)`；
- `ResolutionEvidence`、`ReconciliationReopened`、`Abandoned{principal, note, rule_version}`；
- `CapabilityObserved`；
- `Gap{origin: Channel}`。

关联身份：

- 尝试的记录与取证渠道的 `Gap{origin: Channel}` 都带 `AttemptRef`（见下“代数”）。
- `CapabilityObserved` 是能力证据，带 `(WriteLaneKey, OperationKind)` 或逻辑流 `(source, stream)` 的读 / 回填能力，以及它所更新的声明版本（被接受的推送或回应所在的会话 epoch，由核心盖印），不属于任何尝试（§7.5）。

`Undetermined` 的 `reason` 封闭为两种，二者进入同一对账驱动，只是审计出处不同：

- `NoResponse`：写调用在可能已交出之后没有业务回执（§8.2）；
- `CrashWindow`：重启时 `SendBarrier` 无后继（§6.7）。

IO 壳**不修改**任何记录，不持有权威状态；重启后其全部状态由 `fold_state` 重建。

### 回执与取证的记录模型 [设计]

venue 对我方写的响应是执行事实：C13 原始负载完整保留，执行事实永不删除（§3.1）。同一响应里的订单 / 成交状态又是观察（§3.4）。

响应经集成消费后到达核心，由两部分组成（§2.1）：集成的结论（契约载荷 + `payload_schema`）与所消费的上游原文（原始负载）。执行事实侧永存的证据值 `Evidence = {payload, payload_schema, raw}` 同时保存两者：前者是 UTA 据以行动的结论，后者是结论的出处。集成为一次操作调用了多个上游接口时，`raw` 是全部上游响应的原文，按调用顺序（§8.1 操作的粒度）。[设计]

因此一次 venue 交互在**同一 SQLite 事务**内落两侧记录：

- **执行事实侧**一条，**持有 `Evidence`**（永存）。
- **观察 `Journal`** 上该回应的观察记录，可压缩：订单状态一条（回应含订单状态时），加回应所含每笔可识别执行各一条成交记录（§8.1）。各条的 `attribution` 只按该条自己的关联证据填写，不因同在一个回应里继承；`provenance` 相同，由 IO 壳填（§8.3）。它们供单据钩子与读模型消费、供订阅者与推送观察同形地看到。
- **`dispatch_end` 按落点的流记** [设计]：IO 壳在发出写调用或取证调用时，为这次调用可能写入的每条流（该作用域的订单状态流与成交流）各记下那一刻已提交的流末位置。append 回应的观察记录时，每条带它所在那条流的这个位置作 `dispatch_end`；同一回应的各条 `provenance` 相同，`dispatch_end` 各是自己那条流的（§8.1 来源顺序与 `dispatch_end`）。理由：`dispatch_end` 所在的流 epoch 是这条记录的发出 epoch，流末位置与流 epoch 都只对一条流有意义；拿订单状态流的位置去标成交记录，成交流上的来源顺序与发出 epoch 就都没有依据。

具体：

| 交互 | 执行事实侧 | 观察侧 |
|---|---|---|
| 写调用返回 `Ack` | `VenueAccepted{venue_order_id, receipt: Evidence, observation: LogPosition}`；`observation` 指订单状态记录 | `provenance: Receipt{attempt}` 的观察记录，各带所在流的 `dispatch_end` |
| 写调用返回 `Reject` | `VenueRejected`，`reason` 保留 `Unmapped(raw)`（§2.6） | 无（venue 侧不存在订单） |
| 写调用返回 `NotSent` | `NotSent{reason, evidence: Evidence}`；`raw` 是集成据以判定未交出的本地原文，可为空 | 无（没有交给上游） |
| 取证命中 | `ResolutionEvidence{attempt, channel, round, outcome: Found{observation: LogPosition, evidence: Evidence}}`；`observation` 指命中的那条记录（`list_fills` 命中多笔归因到该尝试的成交时，它们同在该作用域成交流上，取其中 `Seq` 最小者） | `provenance: Reconciliation{attempt, channel}` 的观察记录，各带所在流的 `dispatch_end`；`list_fills` 命中不造订单状态记录 |
| 取证 `Absent` / `Inconclusive` | `ResolutionEvidence` | 无（没有订单状态可记） |
| `Unavailable` | 只落 `Gap{origin: Channel}`，不是取证结果（§6.6） | 无 |

撤单尝试的回执里目标订单的状态是目标订单的记录：它的 `attribution` 按它自己的关联证据填写，不因出现在撤单回执里而成为 `FromAttempt(撤单尝试)`（§6.6 撤单尝试的取证）。

**`channel ∈ {ByKey, Listing, Fills, Replay, Attributed}`：**

- 前四种是 IO 壳依序取证的渠道（§6.6）。
- `Attributed`：集成推送的、归因到处于 `Undetermined` 的尝试的观察，由效应侧归因处理器 append（§6.6、§8.1）。

**`outcome ∈ {Found{observation, evidence}, Absent, Inconclusive}`。**

**每条 `Found` 都带 `evidence`**，C13 对五种渠道一视同仁：主动取证取集成返回的该次响应；`Attributed` 取该推送记录的载荷与原始负载（这类流必须保留原文，§8.1）。

**`round`** 是该次取证**发起时**所属的轮次：最近一条 `ReconciliationReopened` 的位置，首轮为空。`Attributed` 取 append 时的当前轮。

观察侧那些记录落到保留边界下后，执行侧的 `Evidence` 仍在。审计读执行事实，不依赖可压缩的观察记录。

### 代数：尝试与 `AttemptRef`

**尝试（Attempt）** = 一条 `Prepared` 记录及其后继记录 = **对上游的一次写** [设计]。身份 `AttemptRef = attempt_position`（该 `Prepared` 的 `LogPosition`）。一种操作种类只对应一次上游写（§6.2 协议表），所以一次尝试没有更细的内部结构。

所有尝试的记录（`SendBarrier`、`VenueAccepted`、`VenueRejected`、`NotSent`、`Undetermined`、`Expired`、`ResolutionEvidence`、`ReconciliationReopened`、`Abandoned`）与观察侧的 `FromAttempt`/`provenance` 都以 `AttemptRef` 关联。

**发出所依据的声明落进记录** [设计]。`SendBarrier` 记下这次尝试所用的写操作（`submit` / `cancel`）、所带的调用方键及其角色（订单键 / 请求键，§2.2），都取自它过发出前门时的会话有效声明（§7.5）。此后取证渠道与撤阻塞头的资格都按记录判定，不按之后变化的声明：键角色决定撤阻塞头的资格（§6.4），不能被后来的声明改写。

**调用方键由核心铸造** [设计]。发出前门选定的写证明（§2.2）键角色不是 `None` 时，IO 壳以固定的单射编码从 `AttemptRef` 生成调用方键 `K(AttemptRef)`，记进 `SendBarrier`。

- 编码随 IDL 发布（§8.2），是 `attempt_position` 的定长文本编码：不取哈希、不截断，所以不同尝试的键不同。
- 集成只在上游在该作用域逐字接受这一编码的每个值（字符集、长度）时声明订单键或请求键，否则声明 `None`（§8.3）。键装不进上游约束时不压缩、不改写，而是没有键。
- 键登记（§8.1）是 `SendBarrier` 所记键到 `AttemptRef` 的唯一索引。编码单射只保证 UTA 自己的尝试互不冲突，**不证明**上游一条带同样字节的记录属于这次尝试：同一作用域里别的写者可以发同样的字节，上游也可以在它自己的周期后复用键。核心因此从不凭键字节归因，也从不断定一条记录是 `External`；按键归因由集成在它声明的键作用域与唯一期内填 `FromAttempt`，其外是 `Unattributed`（§8.3）。同一作用域没有别的写者使用这一编码，是运维义务，UTA 观察不到它被违反（§10.4 #25）。
- 意图不带键：`EffectRequest` 没有键字段（§6.1），键只在发出时由核心产生。

尝试是线性阶段链：

```
[发出前门] → (SendBarrier → (VenueAccepted | VenueRejected | NotSent | Undetermined → ResolutionEvidence* [→ Abandoned])) | Expired(deadline)
```

- IO 壳是尝试的驱动器。每一步转移由 `集成的回应类型 × 该集成的会话状态 × 该 (WriteLaneKey, OperationKind) 的会话有效能力与 SendBarrier 所记的写操作和键角色 × deadline` 决定。
- 链是闭合 sum，转移表穷尽（见下），没有“其他”分支。

### 等待与结果：两根轴 [设计]

一次尝试有两个不同源头的问题，分开记、分开 fold：

| 轴 | 问题 | 源头 | 值 |
|---|---|---|---|
| **等待**（continuation） | UTA 还要不要为这次尝试等下去、问下去 | UTA 自己的记录 | `Active` \| `Finished` \| `Expired` \| `Abandoned` |
| **结果**（outcome） | 这次写在上游发生没有 | 上游（经集成的回执、取证与推送取回的副本）；`NotSent` 与 `Expired` 是 UTA 自己确知的“未交出” | `Accepted` \| `Rejected` \| `NotSent` \| `Unknown` → `Found` \| `Absent` |

**等待** 是执行事实的 fold，不另存：

- `Active`：`Prepared` 无 `SendBarrier` 且无 `Expired`；或 `SendBarrier` 无后继；或 `Undetermined` 之后既无 `Found`/`Absent` 的 `ResolutionEvidence`，也无 `Abandoned`。
- `Finished`：结果已确立，即 `VenueAccepted`、`VenueRejected`、`NotSent`，或 `Undetermined` 之后第一条 `Found`/`Absent`（任一渠道、任一轮次，含 `Attributed`）。
- `Expired`：发出前门判定 `deadline` 已过，尚未交出即终止。它的结果就是“未交出”，由 UTA 确知。
- `Abandoned`：principal 在结果仍未知时放弃跟踪（§6.6）。

`Expired` 与 `Abandoned` 是 UTA 拥有的两种出口；`Abandoned` 是唯一一种在可能已交出之后由 UTA 结束等待的出口。**`Abandoned` 是吸收态**：此后到达的 `Found`/`Absent` 只补上结果，不改变等待，不重新阻塞 lane，也不恢复保留钉。

**结果** 是上游事实的副本，从不作为门：

- `Undetermined` 的尝试，在等待是 `Active` 或 `Abandoned` 时都可以由第一条 `Found`/`Absent` 补上结果；此后的 `ResolutionEvidence` 照常 append 作审计，结果不再改变。
- `Found` 只回答“这次写到达了上游”；观察记录里 venue 已受理 / 已拒 / 已成交的状态，由读模型与钩子 fold。
- 放弃之后结果仍未知，对外显示“已放弃跟踪，结果未知”，不显示为已发生或未发生（§8.5）。

等待离开 `Active` 的那一刻，这次尝试移出 lane 阻塞头集合（§6.4），它对观察记录的保留钉随之释放（§2.4）。

**取证渠道与轮次。**

- 一次尝试的取证渠道集取自当前会话有效能力（§7.5）：会话有效声明的写证明与 `SendBarrier` 所记的写操作、键角色都相同时，是它声明的渠道；否则为空，尝试停等（§6.6）。渠道顺序固定（§6.6）。渠道集按当前能力求，所以某次尝试停等之后，能力变更使集合出现本轮尚未取证的渠道，它就是下一渠道。
- 本轮已取证渠道 = `round` 等于当前轮（最近一条 `ReconciliationReopened` 的位置）的 `ResolutionEvidence`。因此“渠道穷尽”与“下一渠道”都由 fold 重建（§9.2 #7）。
- IO 壳自动取证只为等待是 `Active` 的 `Undetermined`；等待是 `Abandoned` 的尝试只在 principal 的 `retry_reconciliation` 开出的那一轮里取证，渠道穷尽即停（§6.6）。

**`SendBarrier` 是发送屏障。** durable append（fsync）之后才允许调用写操作（`submit` / `cancel`）。它把崩溃窗口二分：

- `Prepared` 无 `SendBarrier` = **确未发出**；
- `SendBarrier` 无后继 = **可能已发出**。

先例：PostgreSQL `EndPrepare` 先 `XLogFlush` 再 `MarkAsPrepared`。[证据：fp-06 修正 5]

**发出前门** [设计]。IO 壳在 append `SendBarrier` 的那一刻对该尝试求值三个条件，全部成立才 durable append `SendBarrier`，并在同一会话上调用写操作：

1. 意图的 `deadline` 未过：核心 UTC 时钟早于 `deadline`（§2.6），记录的事件时间与收到时间不参与；
2. 该 `WriteLaneKey` 所属集成的会话已建立（会话有效声明为 `Established`，§7.2、§7.5），写调用将在这个会话上发出；
3. 意图对该 `(WriteLaneKey, OperationKind)` 的会话有效能力（§7.5）**可执行**（§6.2 可执行性：`Supported`、声明接受意图所带的参数 schema、`target` 的种类在接受之列）。

结果只有三种：

- **发送**：三条都成立。
- **过期**：`deadline` 已过 → `Expired(deadline)`，等待结束。
- **等待**：其余条件不成立。尝试保持 `Prepared` 无 `SendBarrier`，不 append 任何记录；它仍是确未发出，仍在 lane 阻塞头集合里（§6.4）。会话建立、能力证据变化、`deadline` 到时重新求值。等待永不变成 `Undetermined`；`deadline` 只终止尚无 `SendBarrier` 的尝试。

`SendBarrier` fsync 之后会话才断开时，写调用的结果取决于集成能否证明未交出：能证明得 `NotSent`，否则得 `NoResponse` → `Undetermined`（§8.2）；已有 `SendBarrier` 的尝试永不退回等待。

理由：

- **会话条件**：没有会话时写调用只能失败于传输，一笔核心明知未发出的写就成了结果未知，阻塞 lane 直到取证收敛；在没有按键回读渠道的 venue 上只能等被动证据或由 principal 放弃（§1.6.1）。会话状态是核心自己运行的状态机（§7.2），不是健康观察，所以本门不读读模型。
- **能力条件**：放行后单据已 `Closed`（§6.2），此后再没有别处按当前能力复查这次尝试。重握手或 `CapabilityObserved` 收紧、移除该操作或它接受的参数 schema 时，不能凭放行时的旧能力发出。
- **等待而不终止**：短暂断线或能力暂失不否决已放行的意图。放行时的批准在 `deadline` 内保持有效，这是有意的：时效由 `deadline` 界定（H6）。

不选：

- **在发出前门把无会话集成的能力当作 `Unsupported`**：离线时的声明副本只是最近声明，不是此刻的能力（§7.5）；一次瞬断就会让已放行的意图被当作不可执行。
- **被拒或需干预的集成由核心追加一版全 `Unknown` 能力证据**：那是核心替集成伪造声明；能力证据只记握手声明与能力观察（§7.5）。发送已由本门挡住，集成此刻的状态由健康面给出（§8.5）。
- **以健康观察作必要检查项的输入**：集成死了就不再推送健康，最新的健康观察恰在它失效时过时；规则也不引用读模型（§6.3）。
- **`Degraded` 阻断发送**：`Degraded` 是按流的 readiness 子态（§8.4），属观察侧；其中与写有关的能力收紧已经经 `CapabilityObserved` 进入能力证据与本门。

### `NotSent`：可证明的未交出 [设计]

`NotSent(reason)` 是写调用的封闭返回之一，只在集成能证明这次写**没有交给任何可能把它送出的部件**时返回（§8.3）。SDK 发送缓冲、重连后补发的队列都算已交出；证明不了就是 `NoResponse`。

- `reason` 封闭：`SchemaMismatch`（过屏障之后集成发现意图的参数 schema 身份已不是它此刻接受的，§8.2）与 `LocalRefusal(raw)`（集成在交出前自行拒绝，原文保留）。
- `NotSent` 结束等待（`Finished`），结果是“未交出”；不是 `Undetermined`，不进对账驱动，lane 随即解除。
- 冷却照常：冷却时钟由 `SendBarrier` 设起（§6.3），不因未交出而撤销。
- 健康计数按失败计（§8.4）：它说明集成没能完成这次写，不说明上游回答了。

理由：过屏障之后的 schema 漂移与本地拒绝都是集成确知“没交出去”的情形；记成 `NoResponse` 等于把一个确定的否定变成未知，让 lane 为一件没发生的事等取证。不选：**所有过屏障后的失败一律 `NoResponse`**：理由同上；**`NotSent` 允许按“大概没发出”返回**：交出之后的不确定正是 F5，只能是 `NoResponse`。

### 转移表（穷尽）

以下按尝试状态 × 事件列出全部转移；未列出的组合在 fold 中不可达（sum 穷尽，C13 风格：无“其他”分支）。

| 尝试状态 | 事件 | 结果 |
|---|---|---|
| `Prepared` | 过发出前门 | durable append `SendBarrier`，随后按所记写操作调用 `submit` 或 `cancel`（二者返回同一回执形态，§8.2） |
| `Prepared` | 发出前门：`deadline` 已过 | `Expired(deadline)`（等待结束，未交出） |
| `Prepared` | 发出前门：会话或能力条件不成立 | 保持 `Prepared`，不 append 记录；条件变化或 `deadline` 到时重新求值 |
| `SendBarrier` | 写调用返回 `Ack` | `VenueAccepted`（等待结束） |
| `SendBarrier` | 写调用返回 `Reject` | `VenueRejected`（等待结束） |
| `SendBarrier` | 写调用返回 `NotSent` | `NotSent`（等待结束，未交出） |
| `SendBarrier` | 写调用返回 `NoResponse` | `Undetermined` |
| `Undetermined`，等待 `Active` | `ResolutionEvidence{Found}` / `{Absent}` | 结果确立，等待结束 |
| `Undetermined`，等待 `Active` | `ResolutionEvidence{Inconclusive}` | 下一渠道；渠道穷尽 → 停等（§6.6） |
| `Undetermined`，等待 `Active` | `Attributed`（任一时刻到达） | 与 `Found` 同效 |
| `Undetermined`，等待 `Active` | `abandon` 在在途取证完成后结果仍未知 | `Abandoned`（等待结束，结果仍未知） |
| `Undetermined`，等待 `Abandoned` | `Found`/`Absent`（`Attributed`，或 principal 发起的 `retry_reconciliation`） | 补上结果；等待仍是 `Abandoned` |
| `Undetermined`，等待 `Abandoned` | `ResolutionEvidence{Inconclusive}`（principal 发起的 `retry_reconciliation` 那一轮） | 什么都不变：等待仍是 `Abandoned`，结果仍未知；本轮下一渠道，渠道穷尽即停（§6.6） |

### 与集成操作集的关系

核心对集成的操作集（IDL，小且闭合：`handshake`/`submit`/`query_by_key`/`list_open`/`list_fills`/`cancel`/`replay_by_key`/`backfill`/`read`/`route`）是核心↔集成契约，完整操作集与返回值见 §8.2。所有调用都经集成会话（§7.3）的调用通道。

- IO 壳只用其中的写操作（`submit`/`cancel`）与取证操作，且只在该集成会话已建立时调用（发出前门；无会话时的取证见 §6.6）。
- 每个写调用由集成在它自己的时限内给出封闭结果；可能已交出之后超出时限，结果就是 `NoResponse`（§8.3）。核心不另设写调用的超时。
- `backfill`、`route` 由订阅侧发起；`read` 由读处理器、钩子取证与消费方发起（§3.4）。
- `NotSent`、`NoResponse` 与 `Unavailable` 是一等返回值而非异常。

### 不变量

- IO 壳是核心中唯一向集成发出写调用的地方，不修改记录、不持权威。由关系表（§3.3）与 `fold_state` 重建保证。
- `Prepared` 无 `SendBarrier` = 确未发出；`SendBarrier` 无后继 = 可能已发出；IO 壳不对已有 `SendBarrier` 的尝试再调用写操作。由发送屏障 durable append（fsync）保证。
- 每次尝试发出前过发出前门：`deadline` 已过即 `Expired(deadline)`、永不发出；集成无已建立会话或会话有效能力不可执行时只等待，不 append `SendBarrier`。由发出前门保证。
- 取证渠道与撤阻塞头资格只按 `SendBarrier` 所记的写操作与键角色判定，不随之后的声明变化。由声明落进记录保证。
- 调用方键是 `AttemptRef` 的单射编码，由核心铸造；核心从不凭键字节归因，也从不断定 `External`。由键铸造与归因分工（§8.3）保证。
- 每次尝试的等待**至多**结束一次，且在证据充分（回执、`NotSent`、`Found`/`Absent`、`deadline`）或 principal 放弃时必结束；`Abandoned` 之后等待不再改变。由闭合 sum 与转移表保证。
- 结果只由上游证据（回执、`Found`/`Absent`）或集成确知的未交出（`NotSent`）与发出前门的到期（`Expired`）给出，至多确立一次；放弃不给出结果。由两根轴的分工保证。
- `Undetermined` 的收敛不承诺时限（C2），只承诺停等可被推进：新证据（任一时刻到达的 `Attributed`、`ReconciliationReopened` 重开后的取证），或 principal 的 `abandon`。
- `VenueAccepted` 只认业务回执；`NotSent` 只认集成可证明的未交出；其余落 `Undetermined`。由写操作（`submit`/`cancel`）的返回契约保证。

## 6.6 对账驱动与放弃跟踪

> 图：D6.2 取证循环、D6.4 等待与结果两根轴（`design/diagrams/06-io-shell-attempt.md`）。

### 渠道顺序

进入 `Undetermined` 后，只要等待是 `Active`，IO 壳按该尝试的取证渠道集（§6.5：取自当前会话有效能力中与 `SendBarrier` 所记一致的写证明）**自动**依次取证：

```
by-key → listing + venue 身份 → fills/positions → replay-by-key
```

`by-key` 与 `replay-by-key` 都以该尝试 `SendBarrier` 所记的调用方键发问，不以意图的 `target`；该尝试未带键时，这两条渠道不会出现在它的声明里（§6.2 协议表、§8.3）。两者都带 `barrier_at`（该 `SendBarrier` 的核心时刻）、作用域与键角色：这个键此刻是否仍在上游的保留期与唯一期内，由集成按 `barrier_at` 判断，窗口之外返回 `Unavailable`（§8.3），不返回 `Absent`。核心不持有上游的保留期或唯一期，也不自己比较。

**撤单尝试的取证** [设计]。撤单尝试问的是“这次撤单请求到达上游没有”，不是目标订单此刻处于什么状态。它的回执（`Ack` → `VenueAccepted`）是同一次调用的回应，直接给出结果，不经归因（§6.5）；回执里目标订单的状态照常是目标订单的记录。归因到撤单尝试的记录只有一种：上游以该尝试的请求键把某条记录关联到这次撤单请求，集成在它声明的键作用域与唯一期内据此填了 `FromAttempt(撤单尝试)`（§8.3）。目标订单自己的记录（订单状态、成交、listing 里的条目、带目标订单键的记录）归目标订单所属的尝试，或按集成的判定为 `External`/`Unattributed`，不因为它说的是目标就归因到撤单尝试，也就不是撤单尝试的 `Found`。各渠道因此是：

- by-key、replay-by-key：以撤单尝试自己的请求键发问，回答的是这次撤单请求（§8.3 集成义务），`Found`/`Absent`/`Inconclusive` 照常；只在该尝试带请求键时声明。
- listing、fills/positions：listing 列的是未结订单，成交对账列的是订单的执行，返回的记录都不是撤单请求，永远不会归因到撤单尝试。撤单写证明不声明这两条渠道（§6.2、§8.1 握手校验）。
- `Attributed` 照常。不带键的撤单尝试因此没有自动取证渠道，只由 `Attributed`（集成能以别的上游关联证据把记录归到这次撤单请求时）收敛，或由 principal 放弃跟踪。

撤单尝试有了结果，也**不证明目标已结束**：`VenueAccepted` 或 `Found` 只说明撤单请求到达了上游，目标可能已在此前成交、仍在撤单中，或上游随后拒绝了撤单。目标是否已结束只看目标订单自己的记录（§8.1 订单身份与最近观察）。把撤单与后续下单组合成改单的调用方，按目标的记录决定是否提交第二张单据（§6.2 `Replace`）。

理由：listing 上仍有目标，不证明撤单没到（可能滞后，F10）；不再有目标，也不证明撤单到了（目标可能已成交或被别人撤掉）。把目标的记录当作撤单尝试的 `Found`，一笔结果未知的撤单就被当成已送达，这是 heuristic。不选：**撤单写证明可以声明 listing 与成交对账，命中目标即 `Found`**：理由同上；**可以声明，但规定它们永不命中**：一条注定只得 `Inconclusive` 的声明渠道只增加调用与 pacing 负载，而声明应如实（§8.3）。

### 取证结果与记录的对应（固定矩阵）

| 取证结果 | 记录 | 尝试 |
|---|---|---|
| 命中归因到该尝试的订单 / 成交 / 原响应（撤单尝试见上） | 同事务：观察记录 + `ResolutionEvidence{Found}` | 结果确立 |
| 渠道给出明确否定 | `ResolutionEvidence{Absent}` | 结果确立 |
| 未命中 | `ResolutionEvidence{Inconclusive}` | 转下一渠道 |
| `Unavailable` | 只落 `Gap{origin: Channel}` | 不变；同渠道按 pacing 再发 |

- 明确否定只有 by-key 的 `Absent`，且只在集成判断该键仍在上游唯一期内时给出；listing/fills 没有这一语义（F10）。**空列表永远不是 `Absent`**：listing 或 fills 的空答只是 `Inconclusive`。listing 是否需要隔一段时间再确认一次、间隔多长，都由集成按上游的保证判断，保证之外返回 `Unavailable`（§8.3）。
- `Unavailable` **不算取证**、不换渠道。一个不可用的渠道不是证据；跳过它会把“没查到”伪装成“查过了”。
- **集成无已建立会话时不取证**：IO 壳不对它发任何取证读，也不因此记 `Gap{origin: Channel}`；没有发出的读不是渠道不可用。会话重新建立后，本轮进行中的驱动从本轮进度续跑（下一渠道由 fold 重建，§6.5）；已渠道穷尽而停等、等待仍是 `Active` 的尝试按下文 `SessionRestored` 重开一轮。[设计] 理由：无会话时的“调用”只会是核心自造的失败，把它记成 `Gap{Channel}` 等于伪造渠道证据的出处。

### 停等

渠道穷尽仍 `Inconclusive`，append 后**停下**。停等可由三种事件推进：

- 一条 `ReconciliationReopened{attempt, cause}` [设计] 重开一轮；
- 被动渠道 `Attributed` 在任一时刻到达（见下）；
- principal 经 `abandon` 放弃跟踪（见下）：它结束等待，不给出结果。

IO 壳**永不 heuristic**。取证是读副作用，可以重试；写调用是写副作用，永不重试（§3.4）。

### 重开与轮次

- 新一轮的已取证渠道集从空开始。
- `ResolutionEvidence.round` 在**发起读**时取当前轮，并随记录落盘。
- 旧轮在途读的迟到响应带旧 `round`，不计入新轮：响应归属其发起轮次，不按到达顺序冒充。
- 结果已确立的尝试不受重开影响：重开只重置读进度，不撤销结果。

`cause ∈ {SessionRestored, Manual(principal)}`：

| cause | 谁 append | 何时 | 对象 |
|---|---|---|---|
| `SessionRestored` | IO 壳自动 | 该 venue 的集成会话实际建立（新会话，§7.2）时 | 该集成所有停等、等待仍是 `Active` 的 `Undetermined` |
| `Manual(principal)` | 运维 principal | 经 `retry_reconciliation`（§8.5） | 一次结果仍未知的 `Undetermined`，等待是 `Active` 或 `Abandoned` |

- `SessionRestored` 的理由：渠道穷尽常因 venue 当时不可达。它不作用于 `Abandoned` 的尝试：放弃之后不再有自动的询问。
- `Manual` 重开的一轮对 `Abandoned` 的尝试也依序取证一遍、渠道穷尽即停；它只能补上结果，不改变等待。
- 两种触发在 append 前都重查目标尝试仍处于 `Undetermined` 且结果未知，否则不写。

### 放弃跟踪 `abandon` [设计]

`abandon(attempt, note)` 是带 principal 的控制动作（§8.5）。它回答的是 UTA 自己的问题：**还要不要为这次尝试等下去**；它不回答“这次写发生没有”。

- **资格**：该尝试处于 `Undetermined`、等待是 `Active`、结果未知。否则返回 `Rejected(NotUndetermined)`，不写记录。
- **次序**（放弃是这次尝试等待的结束锚点，在途的取证调用是它的内层，§0.1）：
  1. IO 壳不再为该尝试发起新的取证调用；
  2. 已在途的取证调用照常完成，各自按上表 append 自己的结果；
  3. 然后 IO 壳在一个事务里重查：结果仍未知则 append `Abandoned{attempt, principal, note, rule_version}` 并返回 `Abandoned(position)`；在途调用已给出结果则不写，返回 `Rejected(NotUndetermined)`。
- **不设持久的“放弃中”标志**：第 1 步只是 IO 壳进程内的停发，不是记录。第 3 步之前崩溃，日志里没有 `Abandoned`，恢复后该尝试照常 `Active`；调用方没有收到返回，重发 `abandon` 即可。
- **效果**：等待变为 `Abandoned`，这是吸收态。该尝试移出 lane 阻塞头集合（§6.4），它的保留钉释放（§2.4）；IO 壳不再自动取证，`SessionRestored` 不再重开它。
- **结果仍可补上**：被动的 `Attributed` 与 principal 的 `retry_reconciliation` 照常可以给出 `Found`/`Absent`；它们只补结果，不恢复等待，不重新阻塞 lane。
- **对外**：结果未补上时显示“已放弃跟踪，结果未知”，从不显示为已发生或未发生；补上之后显示补上的结果，并保留曾放弃跟踪的记录（§8.5）。
- **代价**：放弃之后 lane 放行后续写，而这次写可能已经发生，后续写所依据的 buying power 与持仓可能不含它（H1）。这是 principal 带名承担的取舍，记录在 `Abandoned` 上。

理由：尝试的结果是上游事实，UTA 只持有它的副本；能否继续是 UTA 自己的决定。principal 能拍板的只有后者。放弃因此只写 UTA 拥有的那一半，结果仍等上游证据。

不选：

- **principal 直接写 `Found`/`Absent`（人工决议）**：那是由 UTA 一侧写出一份上游事实，没有上游证据作源头；之后到达的真实证据与它矛盾时，无从裁决。
- **先持久化“放弃中”，再等在途调用**：多出一条需要与在途调用结果对账的记录；按生命周期，外层的结束锚点在内层全部结束之后才写，不需要中间态。
- **取消在途的取证调用**：它们的回答是证据，丢弃等于放弃一个可能的结果。
- **放弃后仍按会话恢复自动重开**：放弃就是不再为它自动询问；要再问一次由 principal 经 `retry_reconciliation` 显式发起。

### 被动渠道 `Attributed`

顺序之外还有一条被动渠道。

- 带 `attribution: FromAttempt(r)` 的观察记录到达时，若 r 处于 `Undetermined` 且结果未知（等待是 `Active` 或 `Abandoned`），效应侧归因处理器在同一事务 append `ResolutionEvidence{r, Attributed, Found{observation: 该记录, evidence: 该记录的载荷与原始负载}}`。
- `FromAttempt(r)` 由集成填写（§5.3、§8.3），核心不从键字节推出它：只带调用方键的记录是否属于 r，由集成在它声明的键作用域与唯一期内判断，其外集成填 `Unattributed`。核心也从不把一条记录判为 `External`。
- append `Undetermined(r)` 的事务内，同样检查已到达的、归因到 **r** 的观察。
- r 仍在 `SendBarrier` 等回执时不产生该记录：链保持线性，回执由写调用返回值落 `VenueAccepted`/`VenueRejected`/`NotSent`。
- 迟到回执（W2 步 4）由此并入同一尝试，与取证 `Found` 同效。

[证据：fp-06 修正 7；域 C1/C2/C12]

### 先例谱系

- 能全自动收敛的系统，同时拥有权威结果源与防重身份：Oracle RECO、Kafka coordinator log、Seata TC state。
- 权威源在系统外的系统（PostgreSQL/MySQL 外部 TM、Stripe、Binance、Temporal），自动化止于“重建状态 / 触发查询 / 一次安全重放”，决议交外部。
- UTA 因 F1 权威恒在外，属于后者。

### `replay_by_key`

形式上似写，但保留期内同幂等键重放在语义上是查询：Stripe 同 key 拿回原响应。保留期判断错误即重复下单（Longbridge 10 分钟缓存、IBKR 无键）。

因此它**默认关闭，按 venue 显式开启**，渠道顺序里排最后。IO 壳每次调用都带 `barrier_at`；这个键是否仍在上游保留期内由集成判断，保留期之外返回 `Unavailable`、不重放（§8.3）。上游的保留期是上游的性质，核心不持有它的副本。

### 不变量

- 取证（读副作用）可重试，写调用（写副作用）永不重试。由两类副作用的区分保证（§3.4）。
- `Inconclusive` 停等，IO 壳永不 heuristic；停等只由新证据推进，或由 principal 放弃跟踪结束。由对账驱动保证。
- 放弃跟踪从不给出结果；`Abandoned` 只在在途取证全部完成、结果仍未知时 append，之后等待不再改变，也不再自动取证。由 `abandon` 的次序保证。
- 按键的取证在上游保留期或唯一期之外返回 `Unavailable`，空列表不是 `Absent`。由集成义务（§8.3）保证。

## 6.7 崩溃恢复与集成崩溃两故障面

> 图：D7.1 恢复判定（`design/diagrams/07-crash-recovery.md`）。

### 恢复

恢复时 IO 壳从日志 fold 各 lane 上每次尝试的等待与结果（§6.5 两根轴）。判定顺序固定 [设计]：

1. **等待已结束者无动作**：`Finished`（`VenueAccepted`、`VenueRejected`、`NotSent`，或已有 `Found`/`Absent`）、`Expired` 或 `Abandoned`。`Expired` 不会被再过一次发出前门；`Abandoned` 的尝试只有在最近一次重开是它之后的 `Manual`、且该轮渠道尚未穷尽时，才在会话建立后续跑该轮。
2. **无 `SendBarrier` 者**仍是确未发出的尝试，过发出前门（§6.5）：发送、等待（该集成尚无已建立会话或会话有效能力不可执行）或 `Expired`。
3. **`SendBarrier` 无后继者**，append `Undetermined(CrashWindow)`，进入对账驱动。
4. **`Undetermined` 且等待 `Active` 者**，按 `round == 当前轮` 的 `ResolutionEvidence` 重建本轮进度（发起轮次归属，不按 append 先后），在该集成会话建立后续跑下一渠道或停等。

崩溃前正在进行的 `abandon` 若尚未 append `Abandoned`，日志里没有它的痕迹，尝试按第 4 条照常 `Active`（§6.6）。IO 壳不对任何已有 `SendBarrier` 的尝试再调用写操作。

### 集成崩溃两故障面

集成崩溃按崩溃发生在哪条路径分为两个面。判别边界唯一：**崩溃是否落在 IO 壳的写调用（`submit`/`cancel`）投放路径上**。

| 故障面 | 判别边界 | 领域状态 | 收敛 |
|---|---|---|---|
| 观察流侧崩溃 | 崩溃在订阅/推送路径上（无在途 `submit`） | `Gap{origin: Source}`（§4.2） | 续传 / 重连补齐 |
| 写投放侧崩溃 | 崩溃在写调用路径上（已 `SendBarrier`、等业务回执） | `NoResponse` = `Undetermined` | 对账驱动取证 |

- 两面**可以并发**：同一集成进程既跑观察流又有在途 `submit` 时崩溃，观察流记 `Gap{origin: Source}`，在途 Attempt 记 `Undetermined`。它们是同一进程的两个不同职责面，各自独立收敛，互不替代。
- 前者是流内记录，可续、可标 gap；后者是一次可能已发生的写，只能由对账收敛。
- 写投放侧不区分“集成挂了”和“venue 没回”；区分靠后续证据（`ResolutionEvidence`），不靠猜。

### 为什么（写边界整体）

F1 使权威恒在 venue，UTA 面对 venue 时可能不知道动作是否完成（F5）。所以必须有 in-doubt 一等状态与对账收敛，而非把 timeout 当终态；这与 9 个一手先例一致。[证据：fp-06 命题 1–命题 6；域 F1/F5/C1/C2/C12]

### 不选

- **把 IO 壳做薄（“就是个 HTTP 调用”）**：会让两阶段、unknown 与队列语义散落至规则与集成里。
- **用泛型 typestate 保证发送屏障**：泛型 typestate 在崩溃恢复路径失效，从记录重建不能产回不同类型。故用 move-semantics token：`SendBarrier` 无 `Copy`、构造器私有且内含 durable append，写调用（`submit` / `cancel`）只接受 `SendBarrier` 值。静态保证只覆盖首执一次的调用栈，恢复路径全部是运行期 enum。
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
| 1 | 同 lane 阻塞头集合正常路径下至多一次等待中的尝试；`Undetermined` 在结果确立或被 principal 放弃之前同 lane 无新普通写；两个例外扩大集合，集合为空才解除等待 | §6.4 |
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
| 13 | 一次尝试的等待至多结束一次，`Abandoned` 是吸收态；结果只来自上游证据、集成确知的未交出或发出前的到期，放弃从不给出结果 | §6.5、§6.6 |
| 14 | 调用方键由核心以 `AttemptRef` 的单射编码铸造；核心从不凭键字节归因，也从不断定 `External` | §6.5 |
