# 8 接口契约

本章定义核心与集成、解释层、程序宿主、可选行情派生计算子系统之间的接口：每个操作做什么、返回什么、在核心内留下什么记录、怎样失败。下游只经解释层接触核心（§8.5）。

## 8.1 核心↔集成：握手、锚点、字段注册表、`payload_schema`

> 图：D2.1 一条记录进核心（`design/diagrams/02-record-model.md`）。

集成是独立 OS 进程，语言不限：venue SDK 是什么语言就用什么语言。它是上游被消费的唯一地方（§0.1）：怎样调用上游、怎样把上游记录落到契约的对齐点、怎样判定上游的回应，都在集成内完成，对核心只以契约值出现。因此核心↔集成的契约必须跨语言。

- 契约的语义在本章定义。
- 一份 IDL 是**实现阶段制品**：由本仓库拥有、随 release 发布，这是计划不是现状。任何语言按 IDL 实现即可接入。
- 传输按 OS 选择（§7.1），不改契约语义。
- 集成本身（为什么有这一层、做什么与不做什么、一致性测试）的设计在 `design/integration/design.md`；本章只拥有契约。

### 操作的粒度是 UTA 的意图 [设计]

契约里的每个操作是 UTA 需要的一件事：列某作用域的订单、按键查一笔、投放一笔。它的粒度是 UTA 意图的粒度，由写侧基本类型（§0.1）与读侧流决定，不照搬任何上游接口；下游业务只组合这些操作，不决定它们。

- 一个操作在上游对应几次调用、什么顺序、怎样传参、分页与重试，契约不表达、不约束。上游可能要连调多个过程式接口才凑出“账户 A 的订单”；这是集成对该意图的解释，与 IO 壳解释 `Prepared` 同构（§6.5）：UTA 给出意图值，集成负责在上游上运行它。
- 理由：上游接口的形状只在集成里被消费（§0.1）。契约若照搬上游接口，UTA 就得编排上游调用，等于在 UTA 内消费上游，且每接一个上游就要改契约。

**一个操作只有一个封闭结果。** 多次上游调用的中间状态不越过集成：

- 任一次调用失败，结果只能是该操作封闭返回值中的一个（读为 `Unavailable`），不以缺项的结果冒充请求范围内的完整结果。理由：缺掉的记录会被当成不存在（F10）。
- 多次调用不是原子的，调用之间上游状态可能变化。结果记录的 `received_at` 取末次上游响应的时刻；结果只断言“首次调用发出到末次响应之间观察到这些”，不断言它们在某一时刻同时成立。
- 结果作取证证据时，`Evidence` 的原始负载是这次操作全部上游响应的原文，按调用顺序，不只末次（§6.5）。

**写意图至多一次上游写。** 一次 `submit` / `cancel` 在上游可以有任意次读（查合约身份、查交易权限），改变上游状态的调用至多一次（§8.3）。

- 某上游要连发多次写才能完成一个意图（例如先建单再激活）时，不得装成一次 `submit`：要么作为新 `OperationKind`（轴 B，§8.3）在 IO 壳中按腿解释，每条腿各有 `SendBarrier`（同 `Replace`，§6.5）；要么该 `(scope, OperationKind)` 声明为 `Unsupported`。
- 理由：两次写之间崩溃时，第二次写是否发生对核心不可见，`SendBarrier` 把崩溃窗口二分的保证（§6.5）不再成立。

### 契约的三部分 [设计]

| 部分 | 内容 | 在哪里执行 | 核心怎么用 | 由谁保证 |
|---|---|---|---|---|
| **声明** | `Projection`（§2.2）中除 `mappings` 外的部分：作用域（含 `account_ref`）、流（含读侧能力、请求 schema、名义数据等级）、写能力与取证渠道、订阅配额、扩展 schema；锚点对齐（下表） | —（值） | 握手读取并 fold：路由、门、`required_inputs` 比对、取证渠道顺序；每次握手落一个声明版本（§7.5），经读模型 `sources` 交给解释层（§8.5） | 值；核心按 schema 校验 |
| **记录映射** | `Projection.mappings`：一条上游记录怎样落到对齐点：字段对齐（附换算）、进扩展、丢弃；枚举映射表（见下） | 集成进程内，由本仓库发布的解释器求值 | 随声明在握手交给核心：核心静态校验，并求出每条流实际提供的契约字段集；核心不执行它 | 值；静态 fold |
| **行为** | §8.2 操作集与 §8.3 推送：怎样拿到上游记录（调用编排、鉴权、分页、重连、pacing），以及需要上下文的判定（如键超出上游保证期限时查不到只能是 `Unavailable`） | 适配器代码 | 每次调用与推送 | 一致性测试（§8.3 集成义务） |

判据：不含调用、先后顺序、时间与跨记录状态的，写成值（声明或记录映射）；含其中任一的，是适配器代码，只以封闭返回值对核心可见。

### 记录映射 [设计]

记录映射把一条上游记录落到契约的**对齐点**上：锚点、已注册字段、公共或扩展载荷 schema 的字段。

**每个上游字段的处置**，三选一：

- **对齐**：对到某个契约字段，可附换算（单位、精度、时间格式、身份构造，身份见 §2.6）。
- **进扩展**：契约没有、但对该 venue 有意义，写进该集成声明的扩展 schema 的字段。
- **丢弃**：不写进映射的上游字段不进契约载荷；它只在该记录保留原始负载时随原文留存（见下）。

**枚举映射表**（如订单状态）：上游枚举的每个值对到契约词表的一个值；表外的值输出 `Unmapped(raw)`，这是唯一允许的缺省分支（C13）。

**表示。** 映射本身是一张处置表：上游字段名 → 处置（对齐到哪个契约字段并附换算 / 进扩展 / 丢弃），外加枚举映射表。上游形状只出现在表的键里。换算是纯函数值，用同一组合子值树表示（`Comb<上游值, 契约值>`，§2.5）：树的输入是表中该行列出的上游字段值，树里不出现上游字段名；所需算子按 §2.5 的扩展轴显式加构造子。它回答“映射是哪个代数的组合”：换算与规则、程序、检查项同一表示，只是在集成侧求值。

**握手时核心的静态校验**（失败即投影不合法，拒绝该集成，§8.2 `handshake`）：

- 声明为某公共 schema 的流，映射对齐了该 schema 的全部必填字段；
- 换算的输入与输出类型相符；
- 枚举映射表的缺省分支只有 `Unmapped(raw)`；
- 进扩展的字段都在声明的扩展 schema 里；
- 声明为成交种类的流，映射对齐了注册字段 `execution_id`（见下文“成交与订单状态的契约语义”）；
- 写路径的回执与取证响应、以及可带 `attribution` 的流（订单状态、成交），不带“不保留原始负载”的声明。
- 每条流声明的 `request_schema` 是公共请求 schema 或在该集成的扩展 schema 里；每个配额池只列本投影声明的流；每个作用域的 `streams` 只列本投影声明的流。

**每条流提供的字段集** = 被对齐的公共字段 ∪ 扩展字段。§2.5 的“引用了没有集成提供的字段即 fail-closed”对照的就是这个集合：适配器丢弃了某个可选字段，引用它的树在装载 / 启动期被拒。

**原始负载的保留**，按路径分：

- **写路径**（回执、取证的响应）与**可带 `attribution` 的流**（订单状态、成交）：上游原文必须完整随记录送达。前者进 `Evidence`（C13，§6.5）；后者的记录可能经 `Attributed` 渠道成为某条腿的证据（§6.6），同样进 `Evidence`。映射不能改变这一点。
- **其余观察流**（报价、盘口、K 线等）：映射对每条流声明原始负载保留与否。不保留时，该流记录只有契约载荷，丢弃的字段随之不可追溯。理由：C13 只约束写的证据；高频行情逐条留原文是存储代价，值不值得由适配器作者按该流的审计需要选择。

**在哪里求值。** 映射在集成进程内求值，核心只校验、不执行。解释器由本仓库随 IDL 发布，以库的形式嵌入适配器，各语言绑定是实现制品；不引入新的跨进程协议。理由：消费点在集成（§0.1），核心若执行映射，就要接收上游形状的记录。

### 握手：`Projection`

握手一次交换一个**投影**：集成对若干组合子的可解释性声明，带来源与观察时间，随握手变化。`Projection`、`WriteScope`、`StreamDecl`、`Quota`、`Capability`、`Verdict` 的类型与语义见 §2.2。

- 核心按投影路由，并把它的声明部分作为一个**声明版本**落盘（执行事实，§7.5）；解释层经读模型 `sources` 拿到它，并经执行事实订阅得知它变了，据此回答下游“此刻能不能”（§8.5；`design/downstream/design.md` 第 4 节）。下游只见解释层的对外概念，不见投影本身。
- `Verdict::Unknown` 是**能力未知**，约束启动阶段（能不能发）；它与写边界的**结果未知**（`Undetermined`，约束恢复阶段）分开（§6.5）。
- 写操作的 `CapabilityProof` 含 unknown 证据渠道声明，完备枚举为：
  - 按调用方键回读；
  - open-order listing + venue 订单身份（含“缺席需二次确认”的间隔）；
  - 成交或持仓对账；
  - 保留期内 `replay_by_key`；
  - 无。
- 这是对账渠道顺序的来源（P1、C2；§6.6）。
- 写操作的 `CapabilityProof` 还声明它接受的意图参数 schema 身份（§2.2）；意图按它在输入约束步校验（§6.3），发出前门再核对一次（§6.5）。
- 读侧的一次性读、回填能力按流声明在 `StreamDecl` 上，不进 `capabilities`（§2.2 读侧声明）。
- `account_ref` 与历史声明版本矛盾时只把该引用标为不可解析，不使投影不合法（§2.2）；其余静态校验失败仍按 `handshake` 的“投影不合法”处理（§8.2）。

### 锚点表 × 链路

锚点：没有它构不成链路；闭合、必填、入口即验。缺锚点是**畸形记录**，在集成边界拒绝（不是规则否决）。锚点只服务路由与关联，不服务业务判断（§2.1）。

| 链路 | 锚点 |
|---|---|
| 观察记录 | `session_epoch`（边界接受条件，§7.2）、`StreamId(source, stream, epoch)`、`received_at`；`LogPosition` 由核心在接受时分配（§8.3） |
| 意图 | `principal`、`WriteLaneKey`（不透明，由集成从上游账户结构对齐得出）、`OperationKind`、`basis`（可为空集，但必须存在） |
| 撤单/改单意图 | 上述 + `target: VenueRef \| IdemKey`（构造前提，§6.2） |
| 尝试/决议 | `AttemptRef = (attempt_position, leg)`（`Prepared` 的 `LogPosition` + 腿序号，§6.5）、`WriteLaneKey` |
| 订阅 | 选择器（观察流：来源、流、主体集；或执行事实：来源、作用域）、消费方式 |

### 处理器字段注册表

处理器的定义（触发条件、`required_inputs`、效果）与“字段不存在 → 不触发，不是错误”见 §2.1；`required_inputs` 的定义见 §2.5。注册表按两侧分开：

| 字段出现 | 侧 | 处理器 |
|---|---|---|
| `occurred_at` | 观察 | 事件时间完备进度；缺席则按 `received_at` 保守推导（时间权威见 §8.3） |
| `idempotency_key` | 效应 | key↔`AttemptRef` 登记；`replay_by_key` 渠道可用 |
| `attribution: FromAttempt(AttemptRef)` | 效应 | lane 决议匹配；驱动复合链第二腿；归因 |
| `cumulative_filled_quantity` | 效应[交易协议] | `Replace` 第二腿数量；`orders` 读模型的订单累计成交量 |
| `execution_id` | 效应[交易协议] | `orders` 读模型按执行计数的键（见下文“成交与订单状态的契约语义”） |
| `execution_revision` | 效应[交易协议] | 同一执行的修订取舍（同上） |
| `deadline` | 效应 | 过期规则 |
| 守卫字段（side / instrument / 名义金额） | 效应[交易协议] | 输入约束、审批阈值 |
| `venue_order_id` | 效应 | 按 id 撤单路径 |
| `payload_schema` | 观察 | 程序 / 钩子解释器选择 |

**`idempotency_key` 处理器：**

- 登记的是 `SendBarrier` 携带的键，按腿。
- 推送观察只带该键而无 `FromAttempt` 时，由此登记解析到腿，再按 `attribution` 行处理。

**`attribution: FromAttempt(AttemptRef)` 处理器（效应侧归因处理器）：**

- 目标腿处于 `Undetermined` 未终结时，append `ResolutionEvidence{AttemptRef, Attributed, Found{observation: 该记录, evidence: 该记录的载荷与原始负载}}`（§6.6）。
- 复合链处于 `AwaitingTargetTerminal` 且该记录是目标订单终态时，驱动第二腿（§6.5）。
- 归因由谁填见 §8.3。

### `payload_schema`

- 集成按记录映射（见上）把上游记录写成契约载荷，并打 `payload_schema` 标签；需要保留原文的记录同时附上原始负载（§2.1）。核心不解释两者，只路由字节、存输出。
- 程序（§6）与单据钩子按 `payload_schema` 选择解释器，解释契约载荷。原始负载不交给任何解释器，只作证据（C13）。
- 核心计算的精确类型（写侧基本类型的守卫字段、`cumulative_filled_quantity`、程序 / 钩子解释器用的 money/quantity）只在核心计算处出现；观察侧信封不含价格（§2.6）。

**schema 属于契约** [设计]：

- **公共 schema**：P2 所列跨 venue 共有的种类（quote、book、bar、余额、持仓、订单状态、成交等）各有一份，随 IDL 由本仓库发布。某种类有公共 schema 时，集成必须以它输出该种类的流。
  - 持仓公共 schema 的每条记录带作用域内稳定的持仓身份（区分同一 instrument 的多空分仓与 venue 自有持仓身份），平仓意图的 `target` 取自它；目录公共 schema 的记录按 (instrument, `OperationKind`) 给出写资格（可写 / 不可写），集成已知的不可写（停牌、退市、该类只读、无权限）以它发布（§6.2）。
  - 成交与订单状态两个种类的语义见下文“成交与订单状态的契约语义”。
- **公共意图 schema**：交易协议的每种操作种类（下单、撤单、改单、平仓，§6.2）各有一份意图参数 schema，随 IDL 发布，写成 JSON Schema，含类型相关的必填与互斥约束。集成在该 `(scope, OperationKind)` 的 `CapabilityProof` 里声明它接受的 schema 身份：公共意图 schema，或以它为基础只增加字段与约束的扩展 schema（§2.2）；意图按声明的 schema 在输入约束步校验（§6.3）。意图参数 schema 是 UTA 的契约，不是上游请求格式。
- **交易协议检查读的公共字段**（§6.2 检查目录）：持仓记录的带符号数量与作用域内稳定的持仓身份；报价记录的一个指定参考价字段；目录记录的合约乘数、计价币种与按 (instrument, OperationKind) 的写资格；余额记录的权益及其币种；公共意图 schema 的限价字段。
- **扩展 schema**：venue 特有、公共 schema 容纳不下的内容，由集成在声明中给出 schema 文本，以单独的流输出；需要与公共流关联时，程序按记录上的身份字段 `Join`（§2.5）。扩展 schema 同样属于契约，不是上游消息格式。
- 核心不解释 schema 内容：程序与钩子的解释器按 schema 注册；schema 身份随 `Projection` 交给解释层（§2.2）。
- 理由：载荷若是上游形状，程序就成了 UTA 内第二个消费上游的地方，只能按 venue 分别写，B2/B4 要求的跨渠道组合做不成（§0.1）。

**身份与版本** [设计]：`payload_schema = (schema_id, schema_version)`，随 `StreamDecl` 在握手声明，同一 `StreamId` 内不变（§2.2）。

- 集成要换载荷版本，就为该流开新 epoch。P3：新 epoch 首条记录带 `Gap{origin: Source, reason: schema_change}`。
- 旧 epoch 的记录保留旧标签。
- 核心按 `(schema_id, schema_version)` 精确匹配解释器。未注册的组合不触发解释器，载荷留作字节（字段不存在 → 不触发）。

### 成交与订单状态的契约语义 [设计]

公共 schema 里成交与订单状态两个种类的语义，及按执行计数的 fold 规则。它们决定同一笔执行经不同渠道到达时怎样被计数，集成怎样填、核心 `orders` 读模型与程序、下游怎样 fold 都依它，所以是契约，不留给实现。

**执行身份 `execution_id`。**

- 成交种类的每条记录带注册字段 `execution_id`：不透明，只比较相等；在来源 × 该成交所属 `WriteScope` 内唯一且稳定。
- 它只是上游那一笔执行的函数：同一笔执行经推送、一次性 `read`、`list_fills`、回执、`backfill` 到达，跨流 epoch、会话、重新握手与 UTA 重启，值都相同。
- 集成在记录映射里构造它（身份构造，见上文“记录映射”），只用上游文档证明能唯一识别一笔执行的原生键，或在上游声明的唯一范围比作用域窄时与界定该范围的字段组成的复合键（例如 instrument 与只在 instrument 内唯一的成交号）。消息顺序、时间 / 价格 / 数量的相同或相近、venue seq、`LogPosition`、`venue_order_id`、`AttemptRef`、请求或回执编号、非成交事件也带的回报编号（如 Binance 的 `I`，F12）都不是这种证明。
- 它是效应侧注册字段，不是载荷字段：读它的是核心的 `orders` 读模型（§2.1 推论 1：核心读的字段 = 锚点 ∪ 处理器读的字段；`attribution` 同样被读模型读，§5.3）。
- 它不是归因：不产生 `FromAttempt`，不构成 `Found`。成交与腿的关联只由 `attribution` 承载（§5.3）。

**修订 `execution_revision`。**

- 可选注册字段。上游明确给出某笔执行的修正关联、修正先后与完整替代内容时，集成在同一 `execution_id` 下以更大的 `execution_revision` 输出修正后的内容；不按到达先后编造修订号。没有修订号按 0 计，只表示没有可用于取舍的修订次序，不证明上游从未修正。
- 作废只在上游明确给出时出现：它是一个修订，其内容把该执行标为作废（公共成交 schema 的字段）；不是负成交，也不是反向交易。
- 修订记录自带完整替代内容，所以只看同一身份的记录就能求值，不依赖被修正的那条是否还在保留边界内。
- [证据：F12] IBKR 的修正是另一条执行回报，除 execId 最后一个点之后的数字外参数全同；文档证明了修正关联，没有说明这些数字的先后。集成以去掉该后缀的部分构造 `execution_id`；没有别的上游证据给出先后时，不填修订号，同一身份下内容不同即按下述冲突处理，不取最大后缀。

**按执行计数的 fold 规则。** 同一笔执行无论经哪个渠道、到达几次、以什么顺序到达，在任何按执行计数的 fold 里至多贡献一次；同一身份下内容不同只按修订取值，不相加，也不按到达先后任取。对每个 `(来源, WriteScope, execution_id)`：

1. 取已见记录中最大的修订号。
2. 该修订号上只有一种执行内容，它就是这笔执行；内容为作废则该执行保留身份、没有贡献。比较的只是公共成交 schema 定义的执行字段，不比较出处、原始负载、`received_at`、质量标记与 venue seq。
3. 该修订号上有不止一种内容，该执行为**冲突**：不贡献数量或价格，全部候选内容照实给出；之后到达更大修订号的记录取代它。
4. 缺 `execution_id` 的成交记录是集成违约：不计入，照实标为**无身份**。

- 结果与渠道、质量标记（`one_shot`/`backfilled`/`replayed`/`out_of_order`）、流 epoch、`LogPosition` 及到达顺序无关。
- 去重只作用于执行的贡献：观察记录照常 append、不删除，`Evidence` 不合并，归因处理器对每条记录照常触发（同一执行经另一渠道到达，可能带来新的归因证据）。
- 事件时间窗口在归并之后应用：先按身份与修订选出每笔执行的内容，再按所选内容的执行时间判断是否落在窗口内。
- 按执行计数的去重与投递去重（按 `LogPosition`，§4.2）、回填与实时的边界（按范围，§8.4）是三件事，互不替代。

**增量与累计。**

- 成交记录的数量与价格是**这一笔**执行的量与成交价。
- 订单状态的 `cumulative_filled_quantity` 与平均成交价（载荷）是该订单到这次观察为止的累计值：快照，不是增量。
- 核心 `orders` 读模型的订单累计成交量取自订单状态观察，不以成交之和替代；逐笔成交按上述规则列出；二者不一致时照实并列，不修账（§8.5）。程序或下游对去重后的成交求和是它们自己的派生计算（§4.3），不是上游原值。
- `positions` 只 fold 持仓观察，不消费成交（§8.5），不在本规则内。

**一个回应里的多笔执行。**

- 一次操作结果或一条推送的上游回应含若干笔执行时（例如立即成交的 `submit` 回执、`list_fills` 的结果），每笔可识别的执行各落一条成交记录到该作用域的成交流，与该回应的其余观察记录同一事务、同一出处（§6.5）。
- 各条的 `attribution` 只按该笔执行自己的关联证据填写，不因同在一个回应里而继承（§5.3）。
- 上游回应里不是执行的行（例如 Binance 执行回报的 trade id 为 -1 的那些，F12）由记录映射丢弃，不产生成交记录。
- 回执里的执行给不出合格身份时，只落订单状态与 `Evidence`，不产生成交记录。

**没有执行身份的来源。**

- 声明成交种类的流与渠道（推送、`read`、`backfill`、`list_fills`），即断言它送达的每笔执行都带上游证明的身份。作用域的成交推送流送达该作用域的全部执行，不是其子集。
- 一次读或回填不能为结果里的某笔执行给出身份时，该操作返回 `Unavailable`，不返回删过项的集合（见上文“操作的粒度”）。
- 任何渠道都给不出合格身份的来源，对该作用域不声明成交种类：对成交的读得 typed `Unsupported`（Q16），不是空结果；执行只以订单状态的累计值可见，原始负载只作审计证据。
- 不选：
  - 以时间、价格、数量的散列合成身份：这是猜测。同一毫秒两笔相同的成交会被并成一笔，不同渠道的时间精度会把同一笔拆成两笔；
  - 以扩展 schema 输出不带身份的成交：公共 schema 已有的种类必须以它输出（见上文“schema 属于契约”），且各 fold 只能各自猜怎样去重。

**精确重建的前提。**

- 无条件成立：对 `as_of` 以内的同一记录集，上述 fold 的结果确定（验收 §10.5 #18）。
- 某作用域在事件时间窗口 W 内 fold 出的有效执行集合，只在下列条件全部成立时才承诺等于上游在 W 内的执行：
  1. 该作用域有成交推送流，且其完备进度已越过 W 的末端。完整的证明只来自推送流及其回填：只经一次性读（它不推进完备进度，§8.2 `read`）、回执或取证得到的成交，即使恰好齐全，UTA 也没有证明，不承诺完整。
  2. 该流上可能承载 W 内执行的位置，直到 W 末端的完备位置（§5.2）为止：每个 `Gap{origin: Source}` 都已由覆盖该缺口的回填闭合（§8.4），没有 `backfill_incomplete`；这些位置，以及归并 W 内各执行的修订所需的记录，都不低于保留边界（§2.4）。
  3. 没有可能落入 W 的冲突或无身份执行；无法排除其落入 W 的，按落入计。
  4. 订阅者自行 fold 时，它实际收到了这些记录。确认过的 `Gap{origin: Delivery}` 是损失，不是覆盖（§4.2）。
- “完整”指执行集合完整，不承诺之后不再有修订。条件不全时，结果仍是已有记录上的确定性去重，但不是完整集合；`orders` 读模型怎样标出这一点见 §8.5。

## 8.2 核心→集成操作集

> 图：D6.5 W1 时序、D6.6 W2 时序、D9.2 操作落点、D4.1/D4.3 读处理器、D3.3 回填窗口（`design/diagrams/06-io-shell-attempt.md`、`09-alice-session.md`、`04-program-host.md`、`03-observation-ingest.md`）。

操作集小且闭合。

- IO 壳是核心中唯一调用集成写接口的地方。
- 对账取证是读副作用（可重试）；`submit`/`cancel` 是写副作用（永不重试）。
- 每个操作的**动作轴**（对外部世界是读 / 写 / 非动作）与**核心内部结果**（append / 持久化）分开写，避免“读 / 写副作用”一词混两义。
- `NoResponse` 与 `Unavailable` 是一等返回值而非异常。

### `handshake(session_epoch) → Projection | Refused | Unavailable`

- **语义**：声明作用域 / 流 / 能力。集成为构造声明可以先询问上游（`design/integration/design.md` 2.3）。
- **动作轴**：非动作（声明交换）。
- **返回**：
  - `Projection`（含契约版本）；
  - `Refused(reason)`：上游明确拒绝了该登记所列的身份或配置（凭据被拒、账户不存在或未开通、配置被上游拒绝）。只在上游给出明确拒绝时返回；
  - `Unavailable`：上游此刻不可达或未在时限内回答，没有得到明确结论。
- **核心内部结果**（会话状态见 §7.2 第 3 步）：
  - 合法 `Projection` → 会话 `Established`；更新路由表；append 该握手的声明版本（§7.5）；`required_inputs` 比对；既有订阅按新声明重算路由（见下）；
  - 该集成的每条观察流按 P3 决定是否开新 `StreamId.epoch`：集成能以 venue 游标证明续接，则续用原 epoch、`Seq` 接续；否则新 epoch 首条为 `Gap{origin: Source}`。会话 epoch 与流 epoch 独立；
  - `Refused(reason)` → `Halted{Refused(reason)}`：记 P14 原因，核心终止该集成进程，不自动重握手，等 `rotate_credential` 或 `restart_integration`；
  - `Unavailable` → 仍 `Connecting`，按 pacing 重连（新 `session_seq`）。
- **错误**：
  - 传输失败 → 重连（新 `session_seq`）；
  - 投影不合法 → `Halted{ProjectionInvalid}`，记 P14 原因；
  - 契约版本不兼容 → `Halted{ContractIncompatible}`，记 P14 原因，不降级运行；
  - `Halted` 只影响该集成：核心终止它的进程、不自动重试；投影与契约版本的拒绝只经 `restart_integration` 解除（§7.2 第 3 步）；
  - 不属当前在途握手的结果丢弃，不改变会话状态（§7.2 第 3 步）；
  - 能力比对缺失 → 引用该字段的树 fail-closed（§2.5）；
  - `session_epoch` 形状与接受条件见 §7.2 第 3 步。
- **重试**：传输失败与 `Unavailable` 时幂等、可重发；`Halted` 不自动重发。

**既有订阅随新声明** [设计]：

- 订阅的 selector 按逻辑流 `(source, stream)` 匹配，不随流 epoch 变。
- 新声明不再声明某条已选流时，该订阅对这条流**挂起**（P4），原因 `StreamUndeclared`：核心不再向集成路由这条流的需求；已 append 的记录仍按 cursor 投递，cursor 不变。挂起与恢复是由订阅状态派生的状态通知（P3），不是损失，不需确认。
- 同一订阅的其他已选流照常。订阅的整体状态为“活”，并逐流列出挂起的流；全部已选流都挂起时整体为“挂起”。
- 这条流再次被声明时恢复路由，续接与否按上一条的流 epoch 规则：能以游标证明续接则同 epoch、`Seq` 接续；否则新 epoch 首条是 `Gap{origin: Source}`，断代显式。cursor 不重置。
- 集成断连或 `Halted` 不使订阅挂起：流仍在最近一次声明里，没有新记录只是没有会话，由 readiness `Disconnected` 表达（§8.4）。
- 理由：订阅的 owner 是核心、归属 principal（C5），集成换了声明不能删除它；挂起保留需求，流回来时下游不必重新订阅。只停路由、不停投递，是因为已 append 的记录与声明无关。
- 不选：删除订阅或把它转为“被拒”（需求丢失，下游须自行发现后重订）；照旧“活”却不再有供给（与正常安静的流不可区分）。

### `submit(attempt) → Ack | Reject | NoResponse`

- **语义**：投放一次写（按腿）。
- **动作轴**：**写**。
- **返回**：`Ack(venue_id, receipt)`（业务回执，`receipt` 是订单状态的契约载荷及其原始负载）/ `Reject(reason)` / `NoResponse`。
- **核心内部结果**（记录模型，§6.5）：
  - `Ack` → 同一事务 append `VenueAccepted{venue_order_id, receipt: Evidence, observation}`（执行 J，永存，§6.5）+ 回执观察记录（订单状态；回执含成交时每笔可识别执行另一条成交记录，§8.1；`provenance: Receipt{AttemptRef}`，`attribution` 按各条自己的关联证据填写）；
  - `Reject` → `VenueRejected`；
  - `NoResponse` → `Undetermined`。
- **错误**：超时 / 集成崩溃 / 传输 ACK / HTTP 5xx 全部 `NoResponse` → `Undetermined`。venue 单方面决定，无 commit ack。
- **重试**：**永不重试**；`SendBarrier` 保证至多首执一次。

### `query_by_key(key) → Found | Absent | Unavailable`

- **语义**：按调用方键回读状态。**动作轴**：**读**。
- **返回**：`Found(state)` / `Absent` / `Unavailable`。
- **核心内部结果**：
  - `Found` → 同事务 观察记录 + `ResolutionEvidence{ByKey, Found{observation, evidence}}`；
  - `Absent` → `ResolutionEvidence{ByKey, Absent}`。这是唯一有明确否定语义的渠道。
- **错误**：`Unavailable` → `Gap{origin: Channel}`，同渠道再发，不算取证；能力不支持则该渠道跳过。
- **重试**：可重试。

### `list_open(scope) → Listing | Unavailable`

- **语义**：列 open orders。**动作轴**：**读**。
- **返回**：`Listing(items)` / `Unavailable`。
- **核心内部结果**：
  - 命中带归因身份的订单 → 同事务 观察记录 + `ResolutionEvidence{Listing, Found}`；
  - 未命中 → `ResolutionEvidence{Listing, Inconclusive}`。F10：listing 未见不证明未递；本渠道没有 `Absent`。
- **错误**：`Unavailable`（超时 / 断连 / 配额拒绝）→ `Gap{origin: Channel}`，同渠道再发；能力不支持则该渠道跳过；空 `Listing` ≠ `Absent`。
- **重试**：可重试、可换渠道。

### `list_fills(scope, since) → Fills | Unavailable`

- **语义**：列成交。**动作轴**：**读**。
- **返回**：`Fills(items, next_cursor?)` / `Unavailable`。`items` 是成交记录，各带 `execution_id`（§8.1）。
- **核心内部结果**：
  - 命中带归因身份的成交 → 同事务 观察记录 + `ResolutionEvidence{Fills, Found}`；
  - 未命中 → `ResolutionEvidence{Fills, Inconclusive}`（本渠道没有 `Absent`）。
- **错误**：
  - `Unavailable`（超时 / 断连 / 配额拒绝）→ `Gap{origin: Channel}`，同渠道再发；
  - 缺 `since` 游标 → 范围按声明保守取，仍只作 advisory；
  - 能力不支持则跳过；
  - 结果中某笔执行给不出身份时整体返回 `Unavailable`，不交出删掉它的缺项集合；上游那些不是执行的行（如 Binance `t = -1`）由记录映射丢弃，不是错误（§8.1）。
- **重试**：可重试；分页按 `next_cursor` 续。

### `cancel(venue_id | key)`

- **语义**：撤单。**动作轴**：**写**。
- **返回**：同 `submit` 的回执形态。
- **核心内部结果**：进 `Prepared` 链；也是 `Replace` 的 cancel 腿。
- **错误**：
  - 无回执 → `Undetermined`；
  - 无 cancel-by-key 能力 → 能力项 `Diverged`，不放行。能力项恒为必要项；构造期只查目标存在（§6.2）。
- **重试**：永不重试。

### `replay_by_key(key) → Original | Unavailable`

- **语义**：保留期内重放取原响应。**动作轴**：**读**（形式似写、语义是读）。
- **返回**：`Original(response)` / `Unavailable`。
- **核心内部结果**：
  - `Original` → 同事务 观察记录 + `ResolutionEvidence{Replay, Found}`；
  - 保留期内无此键 → `ResolutionEvidence{Replay, Inconclusive}`。
- **错误**：保留期声明错误即重复下单。默认关闭、按 venue 显式开启、只在声明保留期内调用、渠道顺序排最后（§6.6）。
- **重试**：保留期内幂等；期外不可用。

### `backfill(stream, window) → Covered | Unavailable | Refused`

- **语义**：历史回填（P5，§8.4）：取该流在 `window` 内的历史记录。`window = [from, to)` 用该流 `live_from` 的坐标（有 venue 序号的流用 venue 序号，否则用事件时间），`to` 不超过 `live_from`。**动作轴**：**读**。
- **返回**：`Covered{records, covered_to}` / `Unavailable` / `Refused(reason)`。
  - `covered_to` 表示本次实际取得的是从 `from` 起到 `covered_to` 的连续前缀；只在上游历史已穷尽时小于 `to`。
  - 上游分页、pacing 与重试都在适配器内（§8.1 操作的粒度）：一次调用一个封闭结果，任一页失败即整体 `Unavailable`，不交出缺项结果。
- **核心内部结果**：记录打 `backfilled` 标记 append 观察 `Journal`，同一事务再 append 一条读结论记录（同 `read`，见下），带窗口与 `covered_to`；`Refused` 只 append 结论记录。覆盖到 `live_from` 即边界闭合（§8.4）。
- **错误**：
  - `Unavailable` → `Gap{origin: Channel}`，可再发；
  - `Refused` → 上游明确拒绝这次回填（如未开通该历史数据）；不记 `Gap{origin: Channel}`（渠道没有失败），该窗口按穷尽处理；
  - `covered_to < to` 或 `Refused` 使边界无法闭合 → `Gap{origin: Source, reason: backfill_incomplete}` 标出未覆盖的区间；
  - 该流 `backfill` 能力为 `Unsupported` 或 `Unknown` → 核心不调用，该流无回填，断代只能标 gap（§8.4）；
  - 窗口内某笔成交给不出上游证明的执行身份时返回 `Unavailable`，不返回删过项的集合；上游回应里不是成交的行由记录映射丢弃，不算缺项（§8.1“成交与订单状态的契约语义”）。
- **重试**：可重试。核心可以把一个窗口切成若干连续的小窗口依次请求，续点是已覆盖的边界，不需要集成保存游标；有多少次上游调用、按什么节奏发，是适配器的事。

[设计] 回填与一次性读一样，分页留在适配器内：核心只需要“这一段有没有取全”，续点由已 append 记录的覆盖边界给出，崩溃后从这里重发即可。不选：由集成交出不透明的 `next_cursor` 让核心逐页驱动：核心要持久化一个自己不懂的游标才能续，且 pacing 本是上游的限制，放在核心就要把它声明成值再由核心执行。

### `read(stream, request, range?) → Answered | Unavailable | Refused`

- **语义**：一次性读。发起者是读处理器、钩子 `InputMissing` 取证、IO 壳按目标身份的读（§6.5）与消费方 `read`（§3.4、§8.5）。**动作轴**：**读**。
  - `request` 是按该流 `request_schema` 写成的参数值（§2.2 读侧声明）：查询主体与领域过滤条件都在里面。核心在调用前按 schema 校验形状，不解释它的含义，原样交给集成。
  - `range` 是返回记录事件时间 `occurred_at` 的区间 `[start, end)`，只对声明有事件时间的流合法；它不承载任何领域过滤（到期日、行权价等在 `request` 里）。
- **返回**：`Answered(items)` / `Unavailable` / `Refused(reason)`。
  - 上游分页在适配器内，`Answered` 是这次请求的完整回答，可以为空；任一次上游调用失败即 `Unavailable`（§8.1）。
  - `Refused` 只表示上游对这次请求给出了明确拒绝（未开通、主体不受支持、参数被上游拒绝），不改变该流声明的能力（§8.3 集成义务）。
- **核心内部结果**：
  - `Answered` → 同一事务在该流上 append：每个 item 一条观察记录，与该流的推送观察同形（同一 `payload_schema`，每条自带信封字段），打质量标记 `one_shot`；再 append 一条**读结论记录**：`{request 身份, origins, 本次 item 记录的位置}`。结论记录是流上的控制记录，与 `Gap` 同类，不是载荷记录，fold 与程序的载荷解释器不把它当作该种类的数据。
  - `Refused` → 该流上 append 一条读结论记录，结论为上游拒绝及其原因，不带 item。
  - item 记录与结论记录都带 `provenance: OneShot{origins, request}`。`origins` 是这次调用服务的全部发起方，取值 `Request(LogPosition) | Ticket(TicketId) | Attempt(AttemptRef) | Session(Principal)`，依次对应读处理器、钩子取证、IO 壳、消费方 `read`；`request = (流, 请求 schema 身份, 规范化参数, range)`，即合并 identity（§2.2 只读批处理条件）。
  - 一次性读不参与实时边界、不推进 frontier；`LogPosition` 由核心分配（§8.3）。
- **错误**：
  - `Unavailable`（超时 / 断连 / 限流）→ 该流上 `Gap{origin: Channel}`，可再发；
  - 结果中某笔成交给不出上游证明的执行身份时返回 `Unavailable`，不返回删过项的集合；上游回应里不是成交的行由记录映射丢弃，不算缺项（§8.1“成交与订单状态的契约语义”）；
  - 核心不调用集成的情形，按顺序判定：来源未登记；来源从未有过声明版本；流不在最近的声明版本里；该流 `read` 为 `Unsupported`；为 `Unknown`；请求不合 `request_schema` 或所依据的 schema 身份与当前声明不一致，`range` 用在无事件时间的流上；来源此刻没有已建立的会话。这些情形不 append 任何观察记录（没有渠道被调用，也就没有渠道失败），结果交还发起方，逐项名称见 §8.5 一次性读。
- **重试**：可重试、可批处理（只读批处理条件，§2.2）。

[设计] 读结论记录的理由：空回答不产生 item 记录，没有它，另一个订阅者、钩子乃至发起方都无法在观察侧区分“答了空”与“还没答”；每个 item 仍是独立记录，按种类 fold（如成交按执行身份）不受影响。不选：一次读 append 一条载荷为 item 集合的记录：同一流上出现两种载荷形状，推送记录与读记录不再可互换。不选：只在返回值或 `EffectResponse` 里表示完成：发起方之外的消费者看不到。

### 演进

- 新增 IDL 操作 = 改所有集成（IDL 轴）。它与轴 B 的 `OperationKind` 不同轴：后者只改实现该协议的集成（§8.3）。
- 操作集、返回值与错误语义已定。消息 schema 的文本形式（IDL 文件）随 release 发布，其中构造子的序列化直接取值树的 enum（§2.5）。

## 8.3 集成→核心推送、错误映射、集成义务、扩展三轴

### 集成→核心的推送

> 图：D3.1 推送入库时序、D1.3 边界接受（`design/diagrams/03-observation-ingest.md`、`01-process-topology.md`）。

推送不是对外部世界的读 / 写动作，而是集成把观察与状态送进核心；核心内部结果是 append 或进度推进。

| 推送 | 语义 | 核心内部结果 |
|---|---|---|
| 观察记录 | 集成推送观察记录（含 `session_epoch`、venue seq/cursor 证据、`attribution`、契约载荷 + `payload_schema`、原始负载）；`LogPosition` 由核心按到达顺序分配 | append 观察 `Journal`、推进 cursor/frontier、触发处理器与 DAG |
| `Gap{origin: Source}` | 集成负责的观察流断代 | 记来源 gap（新 epoch 首条记录，含前一范围与最后 `Seq`、原因） |
| 能力变更 | 握手后能力/配额变化，含某流一次性读与回填能力（§2.2 `StreamDecl`） | IO 壳 append `CapabilityObserved`（执行 J，§7.5）、重算受影响单据的 `alignment` |
| readiness（P16） | 按集成 × 流：`Starting` / `Backfilling` / `Live`（含 `Degraded` 子态）；回填 readiness 的 `live_from`（§8.4） | 派生健康观察；订阅状态派生（非损失，不需确认） |

错误与 undesired events：

- **观察记录**：
  - 畸形记录（缺锚点）在边界拒绝。
  - `session_epoch ≠ 当前 epoch` 的推送在边界拒绝、不 append（§7.2 第 3 步）。
  - 同一 `StreamId` 内 venue seq 倒退或重复的记录**照常 append**，并打质量标记（P2：`replayed` / `out_of_order`）；不去重、不重排。核心不伪造流顺序；frontier 不因倒退记录后退。重复与乱序由派生侧 fold 按记录种类的契约语义处理：成交按执行身份与修订计数（§8.1“成交与订单状态的契约语义”），不按 venue seq；其余种类按 venue seq 等证据，由各自的 fold 规定（`orders` 的订单状态见 §8.5）。
- **`Gap{origin: Source}`**：只波及其流；核心不受影响。
- **能力变更**：能力收紧使待决单据 `Diverged`。
- **readiness**：断线 → `Gap{origin: Source}`。集成离开 `Established` 后自己推不了任何东西，所以由核心为它的各流 append `Disconnected{since}`（§8.4）。健康面经读模型对解释层可见，再由它翻成下游的连接状态（§8.5）。

### 领域返回值 → 线缆错误的映射（同一失败三层）

领域状态转换由效应宇宙（§6）拥有。本表只给线缆表示到领域值的映射，不新增第三套业务状态。

| 领域值（§6 拥有） | 线缆/来源 | 三层中的层 |
|---|---|---|
| `Undetermined` | `submit` 返回 `NoResponse`（超时/集成崩溃/传输 ACK/5xx） | `submit` 的领域返回值，唯一映射到写边界 in-doubt |
| `Gap{origin: Channel}` | 对账读取渠道返回 `Unavailable` | 读证渠道的领域返回值，可重试 |
| `Gap{origin: Source}` | 集成上报观察流断代 | 流内记录，可续/可标 gap |
| `Gap{origin: Delivery}` | 慢消费者/conflated（§4.2） | 投递侧损失，需显式确认 |
| `VenueRejected(reason)` | `submit` 返回 `Reject` | 写已发出、venue 拒了——终态 |
| `Unmapped(raw)` | 记录映射的枚举映射表无对应词表值（§8.1） | 保留原始，不伪造穷尽映射（C13） |
| `Refused(reason)` | `read`/`backfill` 返回 `Refused` | 上游对该次读请求的明确拒绝；无记录缺失，不是 gap |
| `Halted{Refused(reason)}`（§7.2） | `handshake` 返回 `Refused` | 上游明确拒绝集成的身份或配置；集成停止自动重连，等运维动作 |

### 集成义务清单

集成由 IDL 固定的职责，按契约的三部分（§8.1）分列。

**声明义务**（握手时交给核心的值）：

- 声明投影：作用域、流及其 `payload_schema`、能力与取证渠道、扩展载荷 schema（§2.2）。
- 交互契约的对齐：把上游账户结构、市场地址、身份体系对齐到锚点契约（`WriteLaneKey`、`basis` 可引用的流、`target` 用的身份）。这是判断性设计动作，每个集成自己负责，做错只影响它自己的流。
- 为每个作用域给出 `account_ref`（§2.2）：在本集成内唯一；同一上游账户跨握手、重启、换凭据不变；一旦用过不再给另一个账户。核心只能查出声明之间的矛盾，这条义务由一致性测试验证。

**记录映射义务**（值，集成侧求值）：

- 为每条流与每种操作结果给出记录映射（§8.1），通过握手时的静态校验。
- 有公共 schema 的种类以公共 schema 输出；上游状态按枚举映射表落到契约词表，表外值为 `Unmapped(raw)`（§2.2）。
- 按映射保留原始负载：写路径与可带 `attribution` 的流必须保留（§8.1）。
- 成交记录带按 §8.1 构造的 `execution_id`（只用上游证明能唯一识别一笔执行的键）；上游明确给出修正次序与完整替代内容时带 `execution_revision`。同一执行在各渠道、各会话给出同一身份；对齐 `WriteScope` 的方式跨握手不变。

**行为义务**（适配器代码，每次调用与推送）：

- 把每个契约操作解释为对上游的调用编排；一个操作只返回一个封闭结果，任一次上游调用失败不以缺项结果冒充完整结果，写意图至多一次上游写（§8.1 操作的粒度）。
- 按流位置推进观察记录。
- 填锚点与已注册字段（含 `attribution`，见下），附上映射产出的契约载荷与 `payload_schema`，以及应保留的原始负载。
- 把当前 `session_epoch` 回填到每条推送与回执（§7.2 第 3 步）。
- 响应投放、对账查询、一次性读与回填（仅限核心调用），返回值只取 §8.2 的封闭集合，且每个值的含义严格成立：
  - `Ack` 只在上游给出业务回执时返回；`Reject` 只在上游明确拒绝时返回；其余一律 `NoResponse`（§8.2）。
  - 集成在 `submit` / `cancel` 内自行决定不发写（例如先读到上游没有交易权限、发现上游连接已断、载荷含它发不出的选项），同样返回 `NoResponse`，核心记 `Undetermined` [设计]。`SendBarrier` 此时已在，核心只凭证据区分“没发出”与“发出了没回音”。
    - 理由：设一个“未发送”返回值，要集成证明这笔写从未交给任何能把它送出去的东西（SDK 缓冲、断线排队后补发都算送出）。这与 `Absent` 同类，原则上可证；但发出前门已把最常见的来源（没有会话）挡在 `SendBarrier` 之前（§6.5），能力与参数 schema 也在 `Prepared` 之前检查（§6.2）。剩下的本地拒绝有多少是推断 [推断]，不足以抵消新终态给转移表、恢复与对外翻译带来的分支。保持现状的代价只是 fail-closed：lane 等取证收敛或人工决议。会推翻它的观测见 §10.4 #16。
  - `handshake` 的 `Refused` 只在上游对身份或配置给出明确拒绝时返回；上游不可达、超时或回答不明确一律 `Unavailable`（§8.2）。单笔操作被拒（某一单的权限、某一次读的开通）不是握手的 `Refused`，不使集成 `Halted`。
  - 会话中上游拒绝了集成的身份（凭据被吊销、会话令牌不能续期）时，集成立即结束当前会话；核心随即重握手，由那次握手返回 `Refused`（§7.2 第 3 步）。不另设推送：会话结束是核心本来就观察得到的事件。
  - `Absent` 只在上游对该键给出明确否定、且这个否定足以证明该键对应的写未发生时返回。上游的“查不到”不足以证明时（例如键已超出上游保证唯一或可查的期限，§1.6.1），返回 `Unavailable`，不返回 `Absent`。
  - `read`/`backfill` 的 `Refused` 只在上游对本次请求给出明确拒绝（未开通、主体不受支持、参数被拒）时返回；超时、断连、限流一律 `Unavailable`。`Refused` 不改变该流的声明能力。
  - `backfill` 的 `covered_to` 只表示从请求窗口起点起、本次实际取得的连续前缀，且只在上游历史已穷尽时短于窗口；任一次上游调用失败即整体 `Unavailable`（§8.1），不交出缺项结果。
  - 一次 `submit` / `cancel` 在上游至多产生一次写调用：集成内部不重发写，上游 SDK 自带的写重试必须关闭。写的重试由核心决定，而核心永不重试写（§8.2）。
- **不以拷贝回答读**：`query_by_key`、`list_open`、`list_fills`、`replay_by_key`、`backfill`、`read` 的回答必须来自本次对上游的询问，不来自集成自己保存的状态；问不到上游即 `Unavailable`。集成为消费推送流而维持的状态（如由增量重建的盘口）只用于产出推送记录，不用于回答读。理由：读的回答被核心当作证据（`Absent` 终结一条腿，§6.6），而集成保存的状态是上游原值在过去某刻的拷贝（§0.1）。
- 上报 `Gap{origin: Source}` 与 readiness（§8.4）。

集成**不持有**规则状态、不做决策、不接触 SQLite。

声明与记录映射义务由握手时的静态校验保证；行为义务无法由构造保证，以一致性测试验证：以 fixture 上游驱动集成，逐条义务观测其返回值与推送（验收 §10.5 #22；测试构成见 `design/integration/design.md`）。

**时间权威。** `LogPosition` 与完备进度只由核心裁定；集成只提供证据（venue seq/cursor/事件时间）（§2.3）。

**归因由谁填。** 规则见 §5.3：集成填 `attribution`；IO 壳在回执与取证观察记录上填 `FromAttempt(AttemptRef)`；集成填不出的记 `Unattributed`，IO 壳按键回读补。集成 IDL 因此含 `attribution` 字段。

### 扩展代价三轴

| 轴 | 加什么 | 代价 |
|---|---|---|
| 轴 A（provider） | 新集成 / 新协议字段 | 加处理器，不改锚点、不改核心 crate（additive） |
| 轴 B（`OperationKind`） | 新操作种类 | 改**实现该协议的所有集成**（`OperationKind` 是锚点），只波及该协议 |
| IDL 轴 | 新 IDL 操作 | 改**全部集成**（跨协议契约） |

## 8.4 回填、实时边界、readiness、健康面

> 图：D3.2 readiness 与流 epoch、D3.3 回填与实时边界（`design/diagrams/03-observation-ingest.md`）。

**回填**（P5）是 IDL 读操作 `backfill(stream, window) → Covered{records, covered_to} | Unavailable | Refused`（§8.2）。

- 由核心按订阅需求发起：核心决定要哪一段、切成几个窗口；上游分页与 pacing 在适配器内。
- 返回的记录与实时推送同形、同 epoch，打质量标记 `backfilled` 后 append。`LogPosition` 仍由核心按到达顺序分配。
- `Unavailable` → `Gap{origin: Channel}`，可再发。该流 `backfill` 能力不是 `Supported` 则不回填，断代只能标 gap。
- 续点是已取得的覆盖边界：每次 `Covered` 与记录同一事务 append 一条读结论记录，带窗口与 `covered_to`（§8.2）。窗口之间、崩溃重启之后都从最近的结论接着请求，不需要任何一方保存游标。

**实时边界。**

- 集成在 readiness 进入 `Live` 时声明 `live_from`：首条实时记录在该流坐标上的位置（有 venue 序号的流用 venue 序号，否则用事件时间）。
- 核心只请求 `< live_from` 的回填窗口，回填与实时记录因此按范围不重叠。Q14：同 epoch 无重复 bar，不靠逐条去重。
- 各窗口的 `covered_to` 连成一段、到达 `live_from` 即边界闭合，frontier 才允许越过它。
- 上游历史穷尽（`covered_to` 小于窗口末端）或上游拒绝回填（`Refused`）而未达 `live_from`，则 append `Gap{origin: Source, reason: backfill_incomplete}` 标出未覆盖的区间，frontier 跳过该区间（C6：不伪造连续）。

**readiness 状态机**（按集成 × 流）：

- `Starting → Backfilling{through: Seq} → Live`。
- 该流无回填可做时 `Starting → Live`：握手以 venue 游标证明续接原 epoch（§8.2 `handshake`），或能力不支持回填（断代只能标 gap）。
- 任一状态可进 `Disconnected{since}`；重连回 `Starting`（新 `session_seq`）。集成离开 `Established` 后自己推不了记录，所以它各流的 `Disconnected` 由核心 append，旧的 `Live` 不会继续显得在线。
- `Degraded{reason}` 是 `Live` 的子态（能力收紧、配额受限），不改变记录接受条件，也不阻断写：发出前门只看会话与能力（§6.5）。
- readiness 变化是派生健康观察，不需确认。

**健康面**（P16）[设计]：每个登记的集成一份 `IntegrationHealth`，只由健康观察 fold 出，经读模型对解释层可见（§8.5）。

| 字段 | 含义 | 由谁 append |
|---|---|---|
| `session` | `Connecting{since}` / `Established{since}` / `Halted{cause, since}`（§7.2 第 3 步） | 核心，在会话状态改变时 |
| `readiness` | 每条流的 readiness（上文） | 集成推送；`Disconnected` 由核心 |
| 按调用目标的 `consecutive_failures`、`last_success_at` | 见下 | 核心，在每个调用结果被接受时 |

调用结果的计数：

- **目标**是调用显式寻址的对象：`submit`、`cancel`、`query_by_key`、`list_open`、`list_fills`、`replay_by_key` 的目标是 `WriteScope`（按作用域寻址，或经 `AttemptRef` 所在 lane）；`read`、`backfill` 的目标是逻辑流 `(source, stream)`。不从 `WriteScope.streams` 推定某条流属于哪个作用域。
- **计入**：核心向集成实际发出、结果属当前会话而被接受的调用；每个最终结果恰推进一次它的目标，并 append 一条健康观察。握手不计（由 `session` 表达）；推送、`Attributed`、`Manual` 与 `CrashWindow` 都不是调用结果；无会话时没有发出的调用不计。
- **失败** = `Unavailable`、`NoResponse`：`consecutive_failures` 加一。
- **成功** = 其余每个封闭结果，含空 `Records`、`Reject`、`Refused`、`Absent`、`Inconclusive`：上游给出了回答。`consecutive_failures` 归零，`last_success_at` 取该结果被接受的时刻。它回答“这个目标最近一次得到上游回答是什么时候”，不表示业务成功。
- 从未成功过的目标没有 `last_success_at`。计数跨会话、跨核心重启延续：健康观察是记录，fold 即得。

由此“公共可用、私有失败”可以观察：公共流这一目标最近成功，某个作用域这一目标连续失败；逐流 readiness 另给流一级。它只说各个被调用目标自己的近况，不推断整个账户或鉴权域是否可用。

- **删去 `reach` 与 `tier`**：`reach`（旧 UTA 的 down / connected / readable 阶梯）由 `session`、逐流 readiness 与按目标的调用结果分别表达；合成单一可达度会丢掉“哪一面不通”。`tier`（旧 UTA 由 keyless / readOnly 推出的 data / account / trading，O4）说的是作用域能做什么，已由投影里的能力判定表达（§2.2）；在健康里再存一份会与能力证据分叉。
- **降级 / 离线的阈值**（旧 UTA 按连续失败 3 次、6 次）是呈现策略，不在核心：解释层或下游按上述字段自定。
- **健康不进写路径**：它只供运维与下游观察。放行门读能力证据（§6.3），发出前门读核心自己的会话状态与能力证据（§6.5）；集成失效时它的健康观察恰好停止更新，拿它作门的输入会在最需要时过时。

## 8.5 核心↔解释层

> 图：D9.1 会话与重连、D9.2 操作集落点、D9.3 人工决议、D9.4 控制动作、D9.5 声明与执行事实推送、D3.4 订阅（`design/diagrams/09-alice-session.md`、`03-observation-ingest.md`）。

本节契约的消费者只有解释层。下游（Alice、CLI 使用者、外部客户程序）是独立生命周期的消费方与控制方，只经解释层接触核心；它们看到的是解释层的对外概念，不是本节的操作、记录与状态（§0.1；`design/downstream/design.md`）。所以本节可以用核心的抽象写。

- 解释层为每个下游连接（一次性命令或一条长连接）开一个核心会话，以该下游自报的 actor 握手。
- 下文的“消费方”“会话”都指解释层代下游开的会话。

### 会话与 principal

会话以 `handshake(contract_version, actor) → Session{principal, instance_id, contract_version}` 建立。

- 传输给出 OS 对端凭据（UDS peer credential / 命名管道 ACL），这是 H7 信任边界。下游连到解释层的对外端点时同样取 OS 对端凭据，非本用户拒绝（§7.1）。
- `actor` 是下游自报的会话内身份（哪个 AI / 哪个人），由解释层带入握手；`principal = (os_user, actor)`。
- 同用户进程视为用户本人（H7），所以 `actor` 无需第二重认证：它是审计与 scope 的键，不是信任来源。
- 契约版本不兼容 → 拒绝会话并记 P14。
- 启动第 5 步之前（§7.2），会话可建立，但除 `handshake`/`health` 外的操作一律返回 `Starting`，不给部分状态。

**授权。** 写类操作按 `(principal, WriteLaneKey, OperationKind)` 授权（C11）；控制动作与人工决议按 `(principal, 动作种类)` 授权。三者同一规则族（授权步，§6.3）。

**订阅归属 principal。** 同一 principal 的新会话自动重新挂接其持久订阅，投递从已确认 cursor 续（§4.2）；不需要重新 `subscribe`。

**读不另授权。** 一次性读、订阅与读模型不按 principal 授权：信任单位是 OS 用户（H7），同用户进程视为用户本人；principal 只是订阅的归属与读记录出处里的发起方。

### 操作集

同一 JSON-RPC；动作轴与核心内部结果分开写，同 §8.2。

**订阅组**：`subscribe(selector, mode, from?) → Subscription`；`ack(subscription, cursor)`；`unsubscribe`。

`selector` 有两种，互斥：

- **观察流**：`(来源, 流, 主体集?)`。主体是该流种类的订阅主体（如 instrument 身份）；不带主体集即订整条流。
- **执行事实**：`(来源, WriteScope?)`：该来源（或其一个作用域）的执行事实，包括单据记录、Decision/`Outcome`/`Rejection`、`Prepared` 起的链与腿记录、`ResolutionEvidence`、`ReconciliationReopened`、取证 `Gap{origin: Channel}`、`CapabilityObserved`，以及该来源的声明版本（§7.5）。执行事实按 `WriteLaneKey` 各成一条流、每个来源的声明版本成一条流，都有 `LogPosition`，所以 cursor 与确认与观察订阅同一套（§4.2）。

- 动作轴：非动作。
- 核心内部结果：
  - 建 / 改订阅表；`ack` 推进 cursor（确认 = 已处理，§4.2）。
  - `from` 缺省 = 各选中流的当前流末：新订阅不补历史，要历史就显式给 `from`。执行事实订阅的 `from` 可以是起点：执行事实不压缩（§2.4），从起点订阅即可重放全部历史。
  - 订阅持久、归属 principal，重连自动挂接。
  - 订阅状态（P4）：观察流订阅在来源有声明时即为“活”，与来源此刻有没有会话无关：“活”只表示需求已接纳、有记录就投递，不表示实时数据在线（在线与否看 `health`）。来源已登记而从未有过声明版本时，订阅为“待接纳”，在该来源第一次握手成功时按其声明转为“活”或“被拒”。
  - 执行事实订阅只能用 `ordered`：按 §4.2 的 ordered 语义它无损（压缩与合并都不会发生，执行事实没有保留边界），慢消费者只对自己形成背压，不影响提交与其他订阅者；长期停住的订阅在 `subscriptions` 里可见。它不依赖任何集成的会话。
  - 投递按位置读出已提交的记录，原样交给订阅者。执行事实由效应侧的记录读取提供，订阅与投递只搬运位置与不透明内容，不解析单据或腿；观察侧的任何元素因此不依赖效应侧类型（§3.2）。
- 配额 [设计]：配额池属于来源（§2.2）。一个池的用量 = 其各流上当前被路由的不同订阅主体数：同一流上同一主体被多个订阅（含不同 principal）订阅只算一次，因为核心只向集成路由一次。配额池里的流只接受带主体集的订阅，不接受整条流的通配订阅，否则一个请求就能绕过上限。重新握手使上限变小时，按订阅创建先后接纳主体，超出上限的订阅转“挂起”（原因 `QuotaExceeded`），有余量时按同一顺序恢复。
- 错误：
  - selector 引用该来源最近声明版本里没有的流 → 拒绝；来源未登记 → 拒绝；
  - 观察流订阅的 `from` < 保留边界 → `BeyondRetention`；
  - 执行事实订阅用 `ordered` 以外的消费方式 → 拒绝；
  - 配额池里的流上不带主体集的订阅 → 拒绝；超过配额 → `QuotaExceeded{quota, limit}`。核心在路由前判定，集成不收到超限订阅，既有订阅不受影响（Q13）；
  - 断连后从已确认 cursor 重投。

[设计] 执行事实可订阅，理由：读模型 `orders`、`lanes` 读执行事实，而“原始记录是消费契约、消费方可自行 fold”（见下）要求这些记录本身可取得；解释层对下游的待审事项、结果未知与已确认推送（`design/downstream/design.md` 第 3.2 节）也需要一个与观察订阅同样持久、可续传的来源。不选：提供读模型的变更推送：那是一套新的变更引擎，重连与续传语义要另定。不选：只准轮询读模型并收窄自行 fold 的承诺：推送变成轮询，续传语义与其余订阅不同。

单据的偏离（`basis_validity`、`alignment` 的重算）不是记录，不出现在这条订阅里：待审事项的出现、决定与关闭经订阅推送；某项此刻是否偏离，解释层在呈现时读 `tickets` 当前态，真正放行仍由核心当次的门判定（§6.3）。

**一次性读**：`read(targets, deadline) → Vec<TargetResult>`，每个 `target = (来源, 流, request_schema 身份, request, range?)`（§8.2 `read`）。

- 动作轴：读（经核心→集成 `read`，§8.2）。
- 核心内部结果：每个 target 至多一次集成 `read`（同 identity 的在途请求可合并，§2.2），`origins` 含 `Session(principal)`；作答时 item 记录与读结论记录按 §8.2 append。
- `TargetResult`，各 target 独立，逐项判定顺序即 §8.2 `read` 的“核心不调用集成的情形”：
  - `UnknownTarget`：来源未登记；
  - `Unavailable{source_state}`：来源从未有过声明版本，或此刻没有已建立的会话。`source_state` 是该来源的会话状态（§7.2），供解释层区分“重连中”与“需要处理”；核心不调用集成、不记 gap；
  - `Unsupported`：流不在最近的声明版本里，或最近的声明版本对该流的 `read` 为 `Unsupported`；
  - `Unconfirmed`：最近的声明版本对该流的 `read` 为 `Unknown`；
  - `InvalidRequest{reason}`：请求不合 `request_schema`、所依据的 schema 身份与当前声明不一致（解释层应重取 `sources`），或 `range` 用在无事件时间的流上；
  - 调用之后：`Answered{conclusion, items}`（`conclusion` 是读结论记录的位置，`items` 是本次 item 记录及其位置，可以为空）、`Refused{conclusion, reason}`、`Unavailable`（该流上已记 `Gap{origin: Channel}`）；`deadline` 内未返回的 target 为 `Unavailable`，迟到的回答照常 append。
- 一个来源不可用不影响其余 target（Q15）；“不支持”“能力未确认”“没有会话”“上游拒绝”与空回答彼此可区分（Q16）。

[设计] 判定顺序先看最近的声明，再看会话：声明给出的“不支持 / 未确认”比“此刻离线”更具体，且二者都不调用集成。这里说的是**最近的声明版本**，离线时并未向上游重新确认；`Supported` 从不越过会话检查，只有已建立会话才调用。没有会话时不记 `Gap{origin: Channel}`：没有渠道被调用，也就没有渠道失败；若来源确实断线，断代由生命周期路径记为 `Gap{origin: Source}`。

**读模型**：`read_model(kind, as_of?) → Snapshot{value, as_of?, gaps?}`。

- 动作轴：读（核心内 fold）。核心内部结果：无。
- 错误：`kind` 未定义 → 拒绝；`as_of` 未达 → `NotYetAvailable{frontier}`；对只给当前态的种类（`tickets`、`subscriptions`）带历史 `as_of` → 拒绝。读模型非权威（§4.4）；种类与各自的输入见下文“读模型集合”。

**单据组**：`draft(intent) → TicketId`；`revise(ticket, expected_version, diff)`；`submit_for_decision(ticket, expected_version)`；`decide(ticket, expected_version, Approve | Reject(reason))`；`send_back`；`withdraw`；`transfer(ticket, to: principal)`。

- 动作轴：写（append `TicketAction`）。
- 核心内部结果：单据 fold 转移（§6.2）；`Approve` 触发 STS 链放行（§6.3）。
- `draft` / `revise` 不因参数不合规被拒：参数合规是单据 fold 的状态，读模型 `tickets` 返回它，送审时由输入约束步否决并留下意图与否决记录（§6.2 参数合规、§6.3）。
- `decide` 由人或下游的自动决定者调用，二者同受一版一条 Decision 的约束；交易协议检查目录之外的 guard 只能以这种身份出现（§6.2）。
- 错误：
  - `draft` 的意图构造不出锚点（缺 `WriteLaneKey`、操作种类不在交易协议的封闭集合内、缺 `basis`；撤单 / 改单缺 `target`；平仓所指的持仓观察记录取不出该作用域的 `PositionRef`）→ `Rejected(Malformed)`，不开单、不 append；
  - `expected_version ≠ current_version` → `Conflict`，不执行；
  - `decide` 时该 `(ticket, current_version)` 已有 Decision → `Conflict(AlreadyDecided)`。单据可能仍停在 lane 步而版本未变（§6.3）；
  - 越权 → `Unauthorized`；
  - `Closed` 后任何动作 → `Rejected(Closed)`；
  - 无 `responsible` 的 `revise` → `Rejected(NotResponsible)`。

**控制组（P14）**：`load_program(manifest_ref, cold_start?)`；`unload_program(id)`；`reload_config(kind)`；`rotate_credential(integration)`；`restart_integration(id)`；`request_snapshot`（核心的重启加速快照，§7.4）；`advance_retention(to: Set<LogPosition>)`；`rewind_cursor(subscription, to)`；`bypass_lane(ticket)`。

- 动作轴：写（append 控制记录）。
- 核心内部结果：
  - 每个动作的结果 `Applied(position) | Rejected(reason)` 作为控制记录 append，带 principal、动作、所读配置版本 hash。
  - 生效动作再触发相应记录：新 epoch、`Gap`、绕过 Decision、保留边界推进。
  - `advance_retention` 的 `to` 为每条要推进的观察流一个新边界（§2.4）。
  - `load_program` 的 `cold_start` 缺省为假；为真时不携带已持久化的 `Checkpoint` 装载，记 `ProgramReset{Operator}`（§8.6）。
- 错误：
  - 越权 → `Unauthorized`；
  - 配置文件不合法 → `Rejected(reason)`，并保留上一有效版本（§7.6）；
  - `bypass_lane` 记为对协议的自觉违反（§6.4）。
- `advance_retention` 逐流判定，任一流不通过即整体拒绝：
  - 新边界不高于该流当前边界 → `Rejected(NotForward)`（边界只前进，§7.5）；
  - 越过该流已登记引用最早位置 → `Rejected(ReferencedBelow{min})`（§2.4）；
  - 越过该流配置留存窗口下界 → `Rejected(InsideWindow{bound})`（§2.4）。

**决议组**：`resolve(attempt: AttemptRef, outcome: Found{observation: LogPosition} | Absent, note)`；`retry_reconciliation(attempt: AttemptRef)`。

- 动作轴：写（append `ResolutionEvidence::Manual` / `ReconciliationReopened{Manual}`）。
- `resolve` 的核心内部结果：
  - append `ResolutionEvidence{attempt, Manual, round, outcome}`，带 principal 与 `note`。
  - `Found` 的 `evidence` 取被引用观察记录当时的载荷与该记录保留的原始负载（`Evidence`，§6.5）。被引用记录通常先经 `read` 造出；观察副本可压缩，`evidence` 永存。
  - 该腿终结后重算整条链：链 `Resolved` 才从 lane 阻塞头集合移出，集合为空才解除（§6.4、§6.5）。撤单腿的 `Manual Found` 使链进入 `AwaitingTargetTerminal`，仍占阻塞头。
- `retry_reconciliation` 的核心内部结果：重开一轮自动取证（新 `round`，旧轮在途响应不计入，§6.6）。
- 错误：
  - 越权 → `Unauthorized`；
  - 腿不处于 `Undetermined` 或已终结 → `Rejected(NotUndetermined)`；
  - `Found` 引用的观察记录不存在或不属该 `WriteScope` → `Rejected(reason)`；
  - `AwaitingTargetTerminal` 的链不接受 `resolve`：它有界，出口是目标终态或 `deadline`（§6.5）。
- 不要求渠道已穷尽：人工可在任一时刻决议；自动取证仍在进行时的决议同样记为 `Manual`。

**健康**：`health() → Vec<IntegrationHealth>`，每个登记的集成一份：会话状态、逐流 readiness、按调用目标的连续失败数与最近成功时间（§8.4）。它等于 `read_model(health)` 的当前态。动作轴：读。核心内部结果：无。启动期返回 `Starting`（§7.2 第 5 步）。

### 读模型集合与一致性

核心维护的读模型（封闭集合）及各自的输入 [设计]：

- `orders`：按 `WriteScope`，执行事实（单据记录、链与腿记录）+ 该作用域的订单状态 / 成交观察（带 `attribution`，含 `External`/`Unattributed`）的 fold。
  - 每笔订单的状态取其**最近观察**：该订单的订单状态记录中，流带 venue 序号时取最新 epoch 内 venue 序号最大者，否则取 `LogPosition` 最大者。它是“UTA 最近观察到的”，不是“上游此刻的状态”。
  - 订单的上游累计成交量取自其订单状态观察（选取规则同上），不以成交之和替代；逐笔成交按 §8.1 的执行计数规则列出（冲突、无身份照实标出），二者不一致时并列，不修账。
  - 每个作用域另给**完整界**：事件时间上的一个界，其前的成交满足 §8.1“成交与订单状态的契约语义”中“精确重建的前提”里核心可判定的条件；消费方是否确实收到全部记录由它自己的 cursor 判定，不在界内。
- `positions`：按 `WriteScope`，只 fold 该作用域的持仓观察：给出每条持仓流在 `as_of` 处最近的观察记录，契约载荷原样给出，不承诺它重建出完整的持仓集合。它不从本地成交或回执推算持仓（那是上游原值的本地拷贝，§0.1、F1），不跨作用域或来源合并，没观察到的不当作零。
- `lanes`：每 lane 的未终结 Attempt 与 `Undetermined` 列表，执行事实的 fold。
- `tickets`：单据 fold 的当前态（§6.2）：执行事实 + 其 `basis_validity` 与 `alignment` 的当前评估。逐版本给出 fold 到的理由（退回原因、否决原因、规则 `Rejection` 及其违反项），以及当前版本的参数有效性（§6.2）。评估还取决于完备位置、撤回、保留边界与当时生效的能力证据和策略（§6.2、§6.3），所以这一种只给当前态；`Snapshot` 仍带它实际消费的位置（含 `checked_as_of`）供追溯，但那不是可重建的切面。
- `subscriptions`：订阅表与 cursor 的当前态（§7.5）。订阅表不是 `Journal`，没有历史切面，所以这一种只给当前态：它的 `Snapshot` 不带 `as_of`、不带 `gaps`，每个订阅已确认的 cursor 在 `value` 里给出。
- `sources`：按已有声明版本的来源，其最近声明版本（作用域及 `account_ref`、`label`，流声明，写能力，配额，扩展 schema 身份）再 fold 其后的 `CapabilityObserved`，即写门与读路由此刻使用的能力；每个 `account_ref` 是否可解析及原因（§2.2）。它只 fold 执行事实，可按历史 `as_of` 读取；历史切面只含该切面上已有声明版本的来源。来源的会话状态不在其中，在 `health`。
- `health`：健康观察的 fold（§8.3、§8.4）：集成推送的 readiness，与核心 append 的会话状态、`Disconnected` 与调用结果计数。

理由：持仓、订单状态、健康的原值在上游或来自观察，读模型只能 fold 已观察到的记录；lane 与单据是 UTA 自己的执行事实。读模型读观察记录，是效应侧读观察侧的同一条单向边（§3.2）。

`orders`、`positions`、`lanes`、`health`、`sources` 的每个 `Snapshot` 带 `as_of: Set<LogPosition>`（fold 吃到的位置集，可跨观察与执行事实两个 `Journal`）与 `gaps`（该范围内生效、未被回填补齐的 `Gap` 记录），可按历史 `as_of` 读取。消费者把 `as_of` 与自己的 cursor 比对，即知快照含哪些记录。

原始记录是消费契约：观察记录与执行事实都可订阅（订阅组），消费方可以自行 fold。`orders`、`positions`、`lanes`、`health` 是对 `as_of` 以内原始记录的确定性 fold，与对同一记录集的独立 fold 相等（验收 §10.5 #18）；`sources` 同样是对执行事实的确定性 fold，验收见 §10.5 #25。`tickets` 与 `subscriptions` 只给当前态，不在此等式内。

**解释层怎样得到声明** [设计]。解释层读 `sources` 取得来源、账户（`account_ref`）、流与读写能力，订阅各来源的执行事实得知声明版本与 `CapabilityObserved` 的变化，再重读 `sources`。`WriteLaneKey` 与 `Verdict` 只在解释层内部使用，对外翻成“账户”“支持 / 不支持 / 未确认”（`design/downstream/design.md` 第 2、4 节）。不选：把声明放进会话 `handshake` 的返回值：重新握手或能力变化后它就过时，且无从得知变化。不选：把声明写成观察流：能力证据属效应侧，写门据以判定的与解释层看到的会成为两份。

### 授权与审批策略的表示

- principal → scope 与控制动作、人工决议授权，写在统一路径的策略 / 审批规则文件（§7.6）。
- 规则文件只写交易协议定义的词汇与取值：人工审批条件与名义阈值、instrument 允许集合、按 `(WriteLaneKey, OperationKind)` 的冷却间隔（§6.3），检查目录各项的必要 / advisory、参数与“先查后判”（§6.2）。判据由交易协议写定，同一规则文件在任何实现里给出同一个放行 / 否决。
- 规则版本 = 内容 hash，经 `reload_config` 生效。
- 规则不冻结进单据；待决单据在放行时按当时规则重过五步（§6.3）。收紧规则可使待决单据在放行时 `Rejection`，记录带 `rule_version`。
- 负责人失联由规则处理：过期步 `Close(Expired)`，或带 principal 的强制 `transfer`（§6.2）。
- 每条 `Outcome`/`Rejection`/控制记录都带所依据的规则版本 hash，审计由此回链。

### 已定事实

- **独立生命周期**：下游或解释层退出 / 崩溃 / 重启不改变核心的订阅、程序、lane、日志（H5/C5）。
- **重连语义**：解释层代下游重连后握手，按 `as_of` 读模型取当前状态，从已确认 cursor 之后订阅记录（观察流与执行事实同一套）。断连期间的投递损失按 `Gap{origin: Delivery}` 显式标记，不伪造连续性；解释层把它翻成下游的缺失通知。执行事实订阅没有投递损失。
- **读模型**：对记录的只读 fold（`tickets`、`subscriptions` 只给当前态），种类与输入见上文“读模型集合”；非权威、不被规则引用；消费方也可直接订阅原始记录自行 fold（S10）。
- **没有“同步”操作**：核心不提供把自己持有的状态对齐到上游的操作，因为它不持有上游原值（§0.1）。旧入口 A08 `sync`（及 A37 账户快照捕获前的 best-effort `sync`）在新边界上是一次性 `read`：产生新的观察记录，以观察为输入的读模型随之 fold；返回的是该次读的结论与记录位置，不是“更新了几条”。账户快照本身不由 UTA 提供（§10.6）。
- **控制面**是同一 RPC 的一组操作，由认证 principal 传入，不经进程信号或 flag 文件。
- 本节消息 schema 的文本形式随 IDL 文件留在本仓库内部，不对下游发布；对下游发布的是解释层的对外面（`design/downstream/design.md` 第 7 节）。本节定义的是操作、返回值与错误语义。

## 8.6 核心↔程序宿主

> 图：D4.1 一轮 `Advance`、D4.2 程序生命周期、D7.4 程序侧崩溃分支（`design/diagrams/04-program-host.md`、`07-crash-recovery.md`）。

**宿主 = 受监督子进程** [设计]。

- 每个程序一个宿主进程，由核心拉起并登记（进程表，§7.2 第 1 步）。
- 宿主二进制是随核心发布的值树解释器。程序值不编译：“编译单元”就是效应宇宙值代数的规范序列化形式。
- 理由：三 OS 无需额外运行时即可构建与运行（Q25）；预算由 OS 进程机制限制；状态经宿主协议显式序列化。
- Wasm（wasmtime）不选为初始宿主（§6.1、§10.1）：无 live `Store` 快照 / 恢复 API、fuel 不限制阻塞 host 调用、三 OS 开箱即用未证。它可作实现阶段的替代宿主，走同一宿主协议，不改本节。

**程序与宿主的约束：**

- 程序 = 值代数的规范序列化形式：按 schema 校验的 JSON 值树，与 `DerivationNode`/`DecisionStep` 一一对应。任何面向 AI 的文本糖必须编译到同一值，且不是核心的一部分；表达力扩展 = 加构造子（§2.5）。
- 装载期校验：`Id` 越界与环、`required_inputs` 与握手声明比对，缺失即 fail-closed。
- 预算靠宿主不靠类型：CPU / 内存 / 意图速率 / 状态大小的预算由宿主进程隔离（C4）。超预算被隔离并报告，其他程序、账户、核心不受影响。
- 状态显式可序列化：程序状态只经 `Checkpoint` 序列化，不依赖运行时快照。
- 隔离边界：程序无写能力（Intent 是值而非外部调用）；程序不接触 SQLite 与凭据（C7、H2）；宿主仅作为解释器的宿主，不进入设计中心。

### 宿主协议

核心 ↔ 宿主进程，同一 IDL 传输。

**`Load(program, checkpoint?, budget) → Loaded{state_version} | LoadRejected(reason)`**（核心→宿主）

- 装载程序值；`checkpoint` 是最近一次持久化的 `Checkpoint{bytes, state_version}`。
- 装载期校验失败 → `LoadRejected`，程序不运行。
- `checkpoint` 的 `state_version` 不在程序声明接受的版本内，不是装载失败，按状态迁移处理（见下）：核心在 `Load` 前比对，不携带该 `checkpoint` 装载，并 `Reset`。

**`Advance(records, to: cursor) → Output{effects, derivations, checkpoint}`**（核心→宿主）

- 推进一批记录（解释① / ② 纯语义）；每次返回都带新 `Checkpoint`。
- 核心把 `effects`（append 为 `EffectRequest` 记录）、`derivations`（append 派生记录）、`checkpoint` 与该程序的 cursor **同一事务**持久化。
- 事务未提交则这批推进视为未发生，重启后重放同一批记录，因此不会重复 `Emit`（§10.5 #16）。
- 宿主超时 / 崩溃 → 视同 trap；`Output` 超预算（意图速率 / 状态大小）→ 不持久化，按预算语义处理（见下）。

**`Reset(reason)`**（核心→宿主）

- 丢弃状态、冷启动；`reason ∈ {StateVersionMismatch, Replace, Operator}`，三者依次由装载时版本不符、替换程序时新程序不接受旧版本、控制面 `load_program(…, cold_start)` 触发。
- append 程序观察 `ProgramReset{reason}`；程序流开新 epoch（`Gap{origin: Source, reason: program_upgrade}`，§4.2），按 H9 回填。

**`Unload`**（核心→宿主）

- 终止宿主进程并清除登记；最近已持久化的 `Checkpoint` 保留供 `Load`。

### 预算、迁移与恢复

- **预算语义**：CPU 时间与内存由 OS 进程限制（rlimit / job object）；意图速率与状态大小由核心在 `Output` 上检查。超预算或 trap（宿主进程异常退出）时：
  - 核心终止宿主进程，append 失败观察 `ProgramFailed{reason: Budget(kind) | Trap}`；
  - 程序停在 `Failed`，直到控制面 `load_program` 重新装载；
  - 其他程序、账户、核心不受影响。
- **状态迁移**：`Checkpoint` 带 `state_version`；程序值声明它接受的 `state_version`，核心在 `Load` 前比对：
  - 接受 → 携带 `checkpoint` 装载，续跑；
  - 不接受 → `Reset(StateVersionMismatch)`；替换程序 = `Unload` 旧 + `Load` 新（携旧 checkpoint），新程序不接受旧版本则 `Reset(Replace)`；
  - 两种都显式记录（C14），不以 `LoadRejected` 拒绝运行。
- **运维冷启动**：`load_program(manifest_ref, cold_start = true)`（§8.5）不携带 `checkpoint` 装载，`Reset(Operator)`。这是 `Failed` 程序在已持久化状态本身导致反复 trap 时的出口；运行中的程序经 `unload_program` 再 `load_program(…, cold_start = true)` 冷启动。
- **崩溃恢复**：核心重启后，从与 cursor 同事务持久化的最近 `Checkpoint` `Load`（§7.2 第 5 步）；宿主崩溃同上。程序因此不会看到已折入状态的记录（§4.2）。

## 8.7 核心↔可选行情派生计算子系统

行情派生高性能计算子系统是可选项，独立于核心，不属于核心；核心在没有它时完整可运行。

核心与它之间只有一个组合子接口，本节完整拥有这个接口。实现细节在 `design/hpc-derivation/design.md`，其证据在 `design/hpc-derivation/research/`。

### 存在理由（核心层）

- **完整窗口**：黑盒闭包计算每次触发可见指定输入的完整最新窗口，而非 delta。核心不理解其算法，只把已注册的行情读数据与触发信号交给它，把它产生的值交给程序中已约定的消费者。
- **一次洗入**：进入这种内存要求高度对齐的布局，边界上的一次拷贝就是洗入（记录 → 对齐布局），有意为之。零拷贝指洗入之后每次调用不再搬窗口。
- **段池与 `Journal` 分离**：段池是为高性能计算设计的运行期快照，由 `Pooled` 洗入，不持久；`Journal` 是记录的载体，持久于 SQLite。两套存储互不派生，只共用 `LogPosition` 标定；持久化行情归 `Journal`。
- **扇出复用**：同一个指标被 N 个策略复用，并派生出一堆计算。共享只读映射让派生 DAG 里每条“一个段被多个消费者读”的边成本为零，每个段只物化一份。按消费者复制的方案成本是 O(窗口 × 消费者)。
- **独立故障域**：只读共享内存映射让计算在独立进程里，既零拷贝又有故障域。op 进程 panic/OOM 只死计算进程，核心记失败观察。独立进程与扇出共享同时成立，只有共享段这一条路。
- **无 sandbox / 安装即授权**：只读映射保证计算不能写核心内存，但仍可任意 syscall：是故障域，不是 sandbox。原生 artifact 由 principal 经控制面安装，安装即授权（信任边界在安装期）；值树程序仍不可信、受预算。

### 接口只有一个组合子

```rust
DerivationNode::Pooled { input: Id, window: Window }   // 输出不是逐条值流，而是可借用的完整窗口段视图
```

`Window(Id, W)` 与 `Pooled{input, window}` 是不同入口，不是同一物（§4.3）。

### 四条前置条件及装载期判定

由“输出类型” fold（§2.5）在装载期判定，不满足即拒绝该程序。`input` 的记录类型须：

1. **定长**：洗入后每字段元素字节数固定；
2. **位置线性**：`LogPosition` 单调、同列内相邻位置相邻；
3. **无指针**：无引用 / `Vec` / 字符串；
4. **可容忍 ring 回收**：旧位置被回收只产生 `BeyondRetention`，不产生错误结果。

quote/bar/tick 及其指标满足；余额 / 持仓 / 订单状态 / 新闻不满足。这是条件不满足，不是架构隔离。

### 失败语义

- 子系统未安装时，含 `Pooled` 的程序在装载期被拒绝，其余程序不受影响。
- 前置条件不满足时，该程序在装载期被拒绝（同一 fold 判定），错误指出违反了哪一条件。
- 两种都是装载期 fail-closed，不影响运行期其他程序。

### 对核心的零影响

- `Journal`、`LogPosition`、五个 fold（§2.5）、处理器注册表（§8.1）的含义不变。
- `required_inputs` 遇到 `Pooled` 交给子系统解析，其余解析到普通流。
- 效应侧、单据、读模型、保留语义（§2.4）不受影响。

### 原生 op 是注册表黑盒

- 原生计算不是新节点种类，而是注册表（§8.1）里由子系统提供的黑盒 op，要求输入是 `Pooled` 的。
- 输出是一条派生观察流，下游节点像读任何派生流一样读它。
- “程序不是黑盒函数”对决策（解释②）继续成立。
- 若产生外部写，走单据 → STS → IO 壳，不增设旁路。“唤醒对应订单的 AI”是程序里的 `On(pattern) → Emit(EffectRequest)`。

### 验收

- **核心层最小验收**：含 `Pooled` 的程序在无子系统时被拒，且其余程序不受影响；子系统 op 的输出对下游是普通派生流（下游节点无需知道它由原生 op 产出）。
- **实现细节与验收**：见 `design/hpc-derivation/design.md`。核心只保留一个“子系统是否安装”的外部依赖状态，不复制其验收标准。
