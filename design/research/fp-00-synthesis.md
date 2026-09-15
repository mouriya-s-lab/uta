# FP 调查 00：综合索引（Haskell / Scala 在"多 provider 适配 + 组合式副作用消费"场景的真实案例）

> 本文只综合 `fp-01`–`fp-05` 五份调查报告的结论，不引入任何未在五份报告中出现的论断，不做 UTA 设计。每条结论后标注来源报告与小节；一手出处（URL、commit、行号）在各报告的来源清单里。调查约束：未读本仓库任何源码；只读 `problem-domain.md`（问题域 §1 与附录 B）理解域；其中设计细节一律不作依据。

## 0. 五份报告与读法

| 文件 | 范围 | 案例数 | 行数 |
|---|---|---|---|
| [fp-01-haskell-finance-cases.md](fp-01-haskell-finance-cases.md) | Haskell（及方言）金融/交易/多 provider **生产系统**：Haxl、SC Mu/Cortex、Barclays FPF、Composing Contracts/LexiFi、DAML、Marlowe、cardano-ledger STS、Tsuru、Mercury、Bitnomial | 10 | 609 |
| [fp-02-scala-jvm-cases.md](fp-02-scala-jvm-cases.md) | Scala/JVM：gvolpe/trading、Fetch、Stitch、ZIO+金融邻近、fs2/cats-effect provider 栈、Akka/Baker/squbs、Scala 3 + Iron/refined | 7 组 | 523 |
| [fp-03-effect-composition-and-open-providers.md](fp-03-effect-composition-and-open-providers.md) | **理论/库**：tagless final、Free/Freer/DTC、代数效应、Haskell 效应库、Haxl/Servant、Scala 3 capability、reify+event sourcing、类型级能力矩阵；含批评者原话 | 8 条目 | 408 |
| [fp-04-base-types-and-domain-primitives.md](fp-04-base-types-and-domain-primitives.md) | **基础类型**：safe-money、Squants、DMMF、indexed、Cardano、Bignum、Incremental、Cats Validated/Ior/Clock、Flink/Dataflow/Lamport、Stripe/PayPal 幂等、负例 CCXT/QuantLib | 12 | 589 |
| [fp-05-streams-incremental-frp.md](fp-05-streams-incremental-frp.md) | **观察簇**：fs2、Streamly、pipes、Elliott FRP、Reflex、Yampa、Incremental、Adapton、Salsa、Dataflow Model、Naiad、Differential、Materialize、Pine Script、Reactive Streams、Akka Streams | 16 | 753 |

统一结构：摘要 → 范围 → 逐案例六问（做什么/设计中心签名/provider 异构/副作用组合与解释/基础类型/作者原话代价）→ 横向表 → **对 UTA 的可迁移命题**（"案例 X 在条件 Y 下用机制 Z 解决 W；UTA 满足 Y 才可迁移"）→ 未覆盖 → 来源清单。fp-03 §"分析框架" 定义了贯穿五份报告的三把尺子：统一 litmus、五种关联关系、能力三阶段（下文 §2–§3）。

读法建议：先 fp-03 §分析框架 + 横向矩阵，再 fp-01 横向表与 M1–M11，再按簇读 fp-05（观察）/ fp-04（基础类型）/ fp-02（JVM 工程细节）。

## 1. 跨报告一致的结论：设计中心是什么

五份报告 53 个案例/条目，**无一以"业务对齐、把字段列全的大 object"为设计中心**（fp-01 横向表"读表要点①"；fp-02 命题 12"负面命题"；fp-03 摘要；fp-04 摘要与范围标准 2；fp-05 摘要）。它们的设计中心分成五族，每族都是"一个小代数 + 外置解释/消费者"：

| 族 | 真实签名（出处） | 业务如何"组合出来"而非"列出来" |
|---|---|---|
| **描述/解释分层的小代数** | `Contract`（10 原语）+ `Obs a`（Composing Contracts）；Marlowe 闭合 6 构造子 `Contract`；FPF payout 深嵌 DSL；`Work_ = WorkPure\|WorkApp\|WorkIO`（SC Mu）— fp-01 案例 2/3/4/6 | 每种产品 = 一个组合表达式，"each different kind of financial product … is modelled by a different Mu datatype"（fp-01 案例 2 ⑤）；一份 AST 被定价/生命周期/文档/代码生成多解释器消费（fp-01 案例 3 ②） |
| **开放 provider 的调度 monad** | `GenHaxl u w a` + `class DataSource u req`，`BlockedFetch`/`ResultVar`，`Map TypeRep (BlockedFetches u)` 动态分桶（fp-01 案例 1 ②③，commit `b33c1c1`）；Scala `Fetch[F,A]` + `DataSource[F,I,A]`、`Stitch[T]` + `Group`（fp-02 案例 2/3） | provider 以 type class instance / Group 接入，核心 monad 对 provider 一无所知；`<*>` = 可批处理，`>>=` = 依赖顺序（fp-03 条目 5） |
| **reify 后的 log + fold** | Equinox/Nubank/Wlaschin：intent/decision/event 为不可变 append-only log，state = `fold`（fp-03 条目 7）；gvolpe/trading `FSM[F,S,I,O]` = `(S,I) => F[(S,O)]`（fp-02 案例 1）；cardano `STS`：`(Environment,State,Signal) → Either [PredicateFailure] State`（fp-01 案例 7，fp-04 案例 5）；DAML `Update` monad + `signatory/controller`（fp-01 案例 5）；Mercury `Workflow`（纯、可重放）/`Activity`（唯一 IO 壳）（fp-01 案例 9） | 关联靠 log record + correlation key，不靠对象字段互指（fp-03 命题 2）；"you never do an update in place and lose that history"（Nubank，fp-03 条目 7） |
| **effect row / capability 成员资格** | tagless 约束、`Member`/`:>`/`Has`、capture set、`ZIO[R,E,A]`+`ZLayer`（fp-03 条目 1/3/4/6；fp-02 案例 4） | 程序声明需要哪些操作，instance/handler/layer 选择实现；加 provider = 加 instance（fp-03 命题 8） |
| **流 / 增量节点 / 时间-差分代数** | fs2 `Pull[F,O,R]`、Streamly `Fold`/`Scanl`、pipes `Proxy`、`Behavior/Event`、`Incr.t` DAG + `stabilize`、`(data,time,diff)`、window/watermark/trigger（fp-05 横向表；fp-04 案例 7/9） | 派生指标 = 图节点/差分，只重算受影响子图；时间与顺序是独立原语（fp-05 专题对比） |

**避免大对象的三条共同手法**（fp-01 摘要；fp-03 摘要）：deep embedding（先 reify 成小代数值）；provider 异构不进核心类型（type class 实例 / 值级 key / 外置后端）；描述与执行分层（解释器族可换）。

## 2. "谁和谁能关联"不是一种关系

fp-03 §分析框架把维护者的"谁和谁能关联"拆成五种关系，并逐条目写明承载物与验证阶段（fp-03 条目 2/3/5/6/7 一致；fp-02 §统一 litmus"Write 与结果/审计如何关联"）：

| 关系 | 真实承载物（案例） |
|---|---|
| operation ↔ capability | 类型证据：type class instance（Haxl `DataSource`）、row 成员（`Member`）、capability 值 + capture set（Scala 3） |
| request/resource ↔ provider 身份 | 值级 key：Haxl request 的 `Eq/Hashable/Typeable`（`DataSource.hs:130-136`）、Stitch `Group`、Kafka partition/offset、gvolpe/trading Pulsar key |
| 多 effect ↔ 同一因果/并发作用域 | Haxl round、fs2 scope、Akka materialization、DAML 单个 `Update` transaction |
| program ↔ handler/interpreter | 自然变换 `f ~> g`、handler、`ZLayer`、Servant 一契约多 instance |
| intent ↔ 执行结果/审计/replay 身份 | **只有 log record + correlation/causation id 承载**（fp-03 条目 7；fp-02 案例 1 CorrelationId + Pulsar txn + MsgId；Mercury idempotency key） |

结论（fp-03 命题 2）：把这五种关系塞进一个对象的字段互指，五组案例均判为反模式；capability 与类型证据不承载审计（fp-03 命题 7："capabilities … by themselves don't perform an effect"）。

## 3. 能力"何时已知"：运行期未知不能伪装成静态保证

三阶段（fp-03 §分析框架）：C-compile（type class instance / row 成员）、C-construct（interpreter/layer/resource 装配期）、C-runtime（握手探测后才知道）。

跨报告一致结论：**没有任何机制原生表达"运行期才知道、部分未文档化"的能力**——全部退化为 existential/refinement 或运行期错误（fp-03 横向矩阵结论 (i)、命题 3；fp-02 §统一 litmus"能力在哪个阶段确定"；fp-01 读表要点④"本组系统能力多在 compile-time/构造期已知"）。最接近的一手先例：Muse 的 `satisfies?` 运行期反射（fp-03 条目 5）；CCXT `has`/`features` + 运行期 `NotSupported`（fp-04 案例 11，作为负例但机制真实）；DAML 承认 contract-keys "Canton 3.x 不支持"、时间不单调（fp-01 M11）；safe-money 的 `Some*` existential 边界（fp-04 案例 1 ③）。

Expression problem 两轴（fp-03 §分析框架；fp-02 §统一 litmus"开放世界哪一轴开放"）：加 provider 在 Haxl/Fetch/Free/effect/row/ZLayer 都是 additive；**加一种新 operation 都要改各 provider 的解释器**——这是所有机制共有、不可回避的代价（fp-03 命题 8）。

## 4. 读与写不对称：自动组合只对只读安全

- Haxl 论文 §5.4 "all our requests must be read-only"；`<*>` 故意违反 `ap` 以批处理，仅在只读幂等下成立（fp-03 条目 5、命题 4）。
- 三个生产系统用**类型分级**区分"可重跑的读"与"有后果的写"：SC Mu `SafeIO` vs `IO`（`perform :: SafeIO a -> a`）；DAML `Update` vs 纯计算；Mercury `Workflow`（`MonadIO` 被 `TypeError` 拒）vs `Activity`（fp-01 M4）。
- Replay 不等于安全：Fowler "external systems don't know the difference between real processing and replays"；Stitch 重跑可能 undefined，Fetch 每次 run 新 cache；本次打开的 Fetch/Stitch/ZIO/FS2/Akka/Baker 通用 API **无一**保证 replay 不再次执行 write（fp-03 命题 5；fp-02 §统一 litmus）。
- 多效应组合无默认可交换语义：Koka amb/state 两序两义、Plotkin/Pretnar 结合律不可表达、Nubank "ordering matters"（fp-03 命题 6）。

## 5. 效应簇最大的证据缺口：不可判定的写结果

五份报告独立得出同一缺口：**没有一个成熟案例直接建模"已发出、既非成功也非失败、不得盲重试"的写结果**（fp-01 未覆盖第 1 条；fp-02 未覆盖第 1 条；fp-04 未覆盖第 4 条；fp-05 未覆盖第 2 条）。

最接近的一手材料：
- Haxl `AsyncFetch` 异常时"inner io 是否执行不可判定"——作者在源码承认未解决（fp-01 M2，`Fetch.hs:508-510`）。
- Stripe/PayPal 幂等协议：key ↔ attempt ↔ latest status，unknown 与 reconciliation 分开，"重试不能保证未发生"（fp-04 案例 10、命题 12）。
- Mercury：durable execution + 重放 + 幂等键，但靠平台记录 event history（fp-01 M7）；Marlowe/DAML 靠链共识消解 unknown（fp-01 M5/M6）。
- 反面参照：DAML `lookupByKey → Optional`、`abort :: Text`——成熟 ledger 也把 unknown 压成 Optional/文本（fp-01 M11）；Marlowe `TransactionError` 枚举失败、无 catch-all（fp-01 M5）；cardano `Either (NonEmpty PredicateFailure)`（fp-01 M8）。

## 6. 观察簇：三种不同的"增量"，不能互相冒充

fp-05 专题对比（§"Pine、Incremental、Differential 与 FRP"）：
- **Pine Script**：每 historical bar 一次、realtime bar 每 tick 一次并 rollback，`var` 跨 bar 状态，但表达式不是可观察 DAG；alert 靠 bar-close confirmation 换可靠性。
- **Jane Street Incremental**：`Var.set → stabilize → Observer`，只重算 necessary path，`cutoff` 截断，`bind` 动态换子图（fp-04 案例 7；fp-05 案例 7）。SC Mu 的 `Work_` DAG + `Key` 共享同属此族（fp-01 M3）。
- **Differential / Dataflow Model / Materialize**：`(data,time,diff)` 带负 diff 撤回；window/watermark/trigger/accumulation 处理迟到与修订；pTVC/`AS OF` 断线续读（fp-05 案例 10–13）。
- **流 transport**（fs2 `Pull`、Reactive Streams `request(n)`、Akka port）：只表达慢消费者与 demand，不承诺 event-time completeness 或最小重算；fs2 `Signal` latest-wins、`Topic` per-subscriber queue、Reflex `tickLossy` 明确可丢（fp-05 横向表"loss / gap"列）。

fp-05 §"能力与关联的横向判读"：不存在一个案例同时给出 provider capability 矩阵、断线 gap、纯派生增量、不可判定 Write、审批/回执/对账、多消费者可靠重放的统一核心；把这些维度压成一个 giant object 只会掩盖缺失语义。时间与顺序是独立原语：event vs processing time、watermark、`Clock[F]` 单调/墙钟分离、Lamport/向量时钟 `Concurrent`（fp-04 案例 8/9、命题 9–11）。

## 7. 基础类型：精确量、身份、阶段、错误各是独立原语

fp-04 横向表与命题 1–16 的要点：
- **金额/数量**：`Dense currency`（`Rational`，拒绝 `Fractional`）、`Discrete' currency scale`、`ExchangeRate src dst` 是 `Category`；`discreteFromDense` 返回余数不静默丢钱（safe-money，`Money/Internal.hs`）；Squants `Quantity[A]`/`MoneyContext` 把 FX 放到作用域 context（隐式 context 是代价）；Bignum 要求调用点显式选择舍入语义。
- **身份**：newtype/opaque 只是名字——"names are not type safety"，需要 smart constructor + 隐藏构造器（fp-04 命题 16）；"parse, don't validate" 在入口把证明保留成结构（命题 15）；gvolpe/trading 用 opaque UUID/Instant/BigDecimal，无 Money/Currency（fp-02 案例 1 ⑤）。
- **阶段/状态机**：DMMF 阶段 union；`indexed` 的 `m i j a` 只在"合法转移构造时可知"下成立（fp-04 命题 3/4）；typestate 不能替代运行期 capability。
- **错误**：`Validated`（独立项累积）vs `Ior`（值与诊断共存，解释器可能丢左值）；`PredicateFailure` 封闭 sum 而非字符串（fp-04 案例 5/8）。
- **负例原话**：CCXT 维护者 "a single human and even a small team can't possibly have enough time to do it all"；QuantLib QuEP5 "In short, the problem is still not solved"（fp-04 案例 11/12 ⑥）。

## 8. 人因代价（作者原话，跨报告一致）

- FPF 放弃 point-free 组合子（量化分析师读不懂）；Mercury "has not been painless"；Bitnomial "not always clear if cognitive load is net-reduced"，建议把类型级复杂度封装在边界（fp-01 M10）。
- De Goes "effect parametric reasoning is a lie"/"Without lawful operations, there is no abstraction"；Karpov/Kiselyov 对 Free 性能；Hillerström/King 对 multi-shot handler（fp-03 横向矩阵、来源 3）。
- Iron issue #281 详细编译错误 "quickly becomes unusable"；refined 宏 pitfalls 与编译时间数字（fp-02 案例 7 ⑥）。
- Akka 官方："Reactive Streams SPI hard to get right"（fp-05 案例 16 ⑥）；Baker listener at-most-once/无序、`awaitCompleted` 文档 Ready-for-review（fp-02 案例 6）。

## 9. 证据缺口汇总（不推导、留待后续 spike）

| 缺口 | 报告 |
|---|---|
| UTA 式 unknown 写结果的成熟类型化建模 | fp-01/02/04/05 未覆盖 |
| C-runtime 能力探测后"抬进类型"的 existential/refinement 构造模式，无成熟一手实现 | fp-03 未覆盖 |
| 跨机制同口径性能基准（Free vs tagless vs 代数效应 vs event sourcing） | fp-03 未覆盖 |
| Scala 金融交易系统一手源码同时含多 venue R/W capability + unknown + audit + 增量 | fp-02 未覆盖 |
| Tsuru/Bitnomial/Mercury 交易或账本核心类型未公开 | fp-01 未覆盖 |
| "There is no Fork" PDF 正文、LexiFi MLFi 论文、Caprese talk 原件未逐页打开 | fp-01/03 未覆盖 |
| Reflex 无稳定 commit 行号；Streamly/pipes/Elliott 无生产规模数字 | fp-05 未覆盖 |

## 10. 可迁移命题去向

五份报告共 63 条条件式命题（fp-01 M1–M11、fp-02 1–12、fp-03 1–8、fp-04 1–16、fp-05 1–16），全部按"案例 X 在条件 Y 下用机制 Z 解决 W"书写并挂案例出处。最集中的跨报告交叉点：
- 效应簇设计中心候选 = reify 后的 log + fold（fp-03 命题 1）× Marlowe 纯 `computeTransaction` 出 intent（fp-01 M5）× DAML 授权随 action 携带（fp-01 M6）× Mercury Workflow/Activity 分离（fp-01 M7）× cardano STS 规则 + `PredicateFailure`（fp-01 M8，fp-04 命题 5）。
- 观察簇设计中心候选 = 只读调度（Haxl/Fetch/Stitch，fp-01 M1、fp-02 命题 1/2、fp-03 命题 4）× 增量 DAG（fp-01 M3、fp-04 命题 7、fp-05 命题 8）× 时间/进度原语（fp-04 命题 9–11、fp-05 命题 11–13）× transport 的丢失语义显式化（fp-05 命题 1/2/15）。
- 两簇之间的关联只由 log record + key 承载（fp-03 命题 2），能力只能是值级、运行期可获得、可为 unknown 的证据（fp-03 命题 3）。

验证记录：本文所有 URL 级引用抽样 12 条（fp-01 S1–S5、fp-03 来源 1/3、fp-02/04/05 的 pinned commit 链接）以 HTTP HEAD 全部返回 200；`Haxl/Core/DataSource.hs:130-136` 的 `type Request req a` 定义按 commit `b33c1c1` 原文核对一致。
