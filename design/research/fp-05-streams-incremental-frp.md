# FP-05：流、增量计算与 FRP 的设计中心

## 摘要

本组案例中最常见的设计中心是可组合的“小代数/节点”（`Pull`、`Proxy`、`Fold`、`Behavior/Event`、依赖图节点或 `(data,time,diff)`），它们把业务值当作参数而不是巨型对象，因此由组合律、解释器和消费边界生成业务行为。

- 流库主要把设计中心放在逐步请求/响应与生命周期：它们解决运输、背压和资源，不自动解决依赖图增量重算。
- FRP 把设计中心放在时间语义：连续 `Behavior`、离散 `Event` 或因果 `SF`，实现可以 push、pull 或混合。
- 增量计算把设计中心放在可观察依赖图：只重新计算受影响节点；它不是消息队列，也不应直接承载外部写副作用。
- Dataflow/Differential 把设计中心放在事件时间、进度和可加/可撤回更新，因而能表达迟到数据与结果修订。
- Reactive Streams/Akka 将需求量和 typed ports 放在解释边界，保证异步互操作；具体拆分、合并、重试和领域能力仍在上层。
- 本报告不把任一案例升级为 UTA 方案；最后仅给出带条件的可迁移命题。

## 范围与筛选标准

1. 只读本次实际打开的一手来源：官方源码、官方 API/指南、作者论文/博客和项目仓库；代码仓库均以本次 shallow clone 的提交固定。
2. 本仓库只读取允许的 UTA 设计说明 §1 与附录 B；其内容不作为设计证据，也不在本文复述为方案。
3. 每个案例固定回答六问：系统与生产证据、设计中心、provider/能力、效果组合与消费、基础类型、问题与代价。
4. `[观察]` 表示来源直接写出；`[推断]` 表示由已打开类型/语义作出的工程解释；`[无一手证据]` 表示本次材料没有找到该主张，不能推成“不存在”。
5. “背压”只用于执行协议；纯 FRP 语义中的采样/丢失与队列背压分开记录。“gap”只有来源明确给出进度或丢失语义时才使用。
6. 能力筛选统一问三件事：能力在编译期、解释器构造期还是 runtime 才知道；增加 provider 是否无需改核心而增加 operation 是否也无需改核心；请求、资源、消费者、解释器和结果之间是否有显式关联。
7. 资料版本按来源标注：Streamly 0.11.0、pipes 4.3.16、Yampa 0.15、Reflex 0.5 文档、fs2 提交 `8aa47a…`、Incremental `v0.18~preview.130.106+341`、Salsa 0.28.2 等。网页若无发布日期不猜测。

## 逐案例分析

### 1. Typelevel fs2：`Pull`/`Stream` 与 Topic、Signal、Channel

#### ① 系统做什么、规模与生产使用

- [观察] fs2 README 将其定义为 “purely functional, effectful, and polymorphic stream processing library”，目标包括 compositionality、resource safety 和 speed；官方 cover 称其为可在 constant memory 做 I/O 的 “Functional, effectful, concurrent streams”。
- [观察] 官方 adopters 页自称是 “a (non-exhaustive) list of companies that use FS2 in production”，列出 Disney Streaming、Deutsche Bank、Comcast、Ocado Technology、OVO Energy、Permutive、Teikametrics 等；cover 还称 fs2 powers http4s、skunk 和 doobie。
- [观察] 这提供生产采用证据，但本次 clone 没有当前生产 QPS、fan-out、延迟或内存 SLA；JMH 只有 workload 参数，不能冒充生产规模（源码：`/tmp/fp05-fs2-main/site/adopters.md:1-24`、`benchmark/...`；来源[1][2]）。

#### ② 设计中心是什么

- [观察] 核心是 `sealed abstract class Pull[+F[_], +O, +R]`：`F` 是可执行 effect，`O` 是输出，`R` 是成功终止结果；Pull 是 “a purely functional data structure that describes a process”，可以成功、抛错、取消或不终止（`Pull.scala:36-44,111-122`）。
- [观察] Pull 是 pure、immutable、referentially transparent；输出以 immutable `Chunk[O]` 为基本边界，chunk 内构造失败时整个 chunk 不产生（`Pull.scala:43-63`）。
- [观察] `flatMap` 先消费前一个 Pull 输出再接后一个，`>>` 是懒 append；文档给出 Monad laws。源码定义 `final class Stream[+F[_], +O] private[fs2] (private[fs2] val underlying: Pull[F, O, Unit])`，所以 Stream 不是第二套 AST；`p.stream` 引入 resource scope，`streamNoScope` 明确有 memory leak 风险（`Pull.scala:302-323`、`Stream.scala:166`）。
- [推断] 因而设计中心是“effectful pull step + Chunk + scope”，不是 FRP 依赖图或领域对象；Stream 的增量粒度是 Pull step/Chunk，不是每个指标节点。

#### ③ 多 provider / 异构能力怎么表达

- [观察] `type Pipe[F[_], -I, +O] = Stream[F,I] => Stream[F,O]`；`through`、`translate(FunctionK)` 和 `Compiler[F,G]` 让同一数据管道接到不同 effect/interpreter（`fs2.scala:25-32`、`Stream.scala:2940-3046`）。
- [观察] 入口按 typeclass 要求能力：`fromIterator` 要 `Sync[F]`，publisher interop 要 `Async[F]`，Topic/Channel/SignallingRef 构造要 `Concurrent[F]`；这属于编译期能力门槛。
- [推断] provider 本身的限流、重连、幂等键和是否真的有推送，在 `F` action/runtime handshake 中才知道；fs2 没有领域 Read/Write capability row 或 provider 注册表。增加 connector 可以写新的 `Stream/Pipe`，但不能由类型静态证明某 provider 的业务操作矩阵。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] `Pull.eval`/`Acquire`/`Output` 节点描述 effect，`Compiler` 将 Pull fold 到目标 `F`；scope 在正常终止、错误和 cancellation 上执行 finalizer（`Pull.scala:629-647,1087-1116`）。
- [观察] `abstract class Topic[F[_], A]` 的关键签名是 `publish1(a: A): F[Either[Topic.Closed, Unit]]`、`subscribe(maxQueued: Int): Stream[F, A]`；它是多 publisher/multi-subscriber broadcast，发布动作要等待所有 subscriber 队列有空间，`publish1` 被中断时不保证 atomicity。
- [观察] `trait Signal[F[_], A]` 提供 `discrete: Stream[F, A]`、`continuous: Stream[F, A]`、`get: F[A]`；`sealed trait Channel[F[_], A]` 提供 `send(a: A): F[Either[Channel.Closed, Unit]]`、`trySend(a: A): F[Either[Channel.Closed, Boolean]]`、`stream: Stream[F, A]`。Channel 是 MPSC、single consumer；Signal 是 current value holder，不是逐事件 bus。组合因而是 `Stream` 的 `map/flatMap/through`、Topic 的 per-subscriber queue、Signal 的 latest-value view，而不是一个共享领域 effect graph。
- [推断] fs2 的 `F[A]` 返回值可关联一次 API 调用，但没有 request key、audit record、causal id 或 write replay identity；把外部写放进可重跑 Stream 需要应用自行保证幂等。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] 终止/错误被 `R`、`F` failure、cancellation 和 `Either[Closed,...]` 分层表达；Topic/Channel close 是 graceful drain，但没有 unknown-result 类型。
- [观察] `Topic` 默认慢消费者阻塞 publisher；若 close race，官方 issue #3644 记录 `publish1` 返回成功但 subscriber 未收到事件。`Signal.discrete` 明确是 “only the latest update since your last pull”；要每个 update 应使用 Queue/Channel。
- [观察] Channel bounded 满时 `send` 阻塞，`trySend` 返回 `false`；同一 stream 不能并发多 consumer。`awakeEvery`/`Temporal` 使用 runtime monotonic time，`dampen` 可合并 missed periods；这些是执行时间而非 event-time watermark。
- [推断] fs2 提供 active、未取消订阅者范围内的消费协议，却没有 sequence number、gap marker、durable replay；Topic broadcast 与 Signal latest-value 的 loss 语义不能互换。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 官方文档称 chunk-level Pull 更快但更复杂；migration guide 说早期 Segment operator fusion 在多数 stream use case 让整体性能更差、算法不直观，1.0 回到 Chunk。
- [观察] guide 将外部不可 throttle callback 的选择说成 possibly unbounded memory 或 bounded/blocking primitive；它还把 Reactive Streams 描述为 “complicated, mutable and unsafe”，只建议用于 interop。
- [观察] changelog/issue 记录过 `Signal#discrete` memory leak、`runFoldScope` leak、Topic unregister race/flaky test；这些是维护历史，不应断言当前版本必然复现。
- [结论] fs2 是很强的 transport/effect design center，但没有证据把它当成增量依赖图、业务 capability 或审计工作流中心。

### 2. Streamly：`Fold`/`Scanl` 与并发调度

#### ① 系统做什么、规模与生产使用

- [观察] Streamly 0.11.0/streamly-core 0.3.0 文档把 streams、arrays、并发、时间和资源管理统一在高性能 Haskell API 中；官方说明 streams 是 immutable、composable serial processing，arrays 面向 storage/mutability/random access。
- [观察] 文档提供 `parBuffered`、`parTee`、`parDistributeScan`、`parDemuxScan` 等并发组合与 benchmark 链接；本次官方页面没有客户名单、部署数量或生产 SLA。
- [无一手证据] 因此只报告库级设计能力；不能把文档的“orders of magnitude”或示例当生产吞吐。

#### ② 设计中心是什么

- [观察] `data Step s b = Partial !s | Done !b`；`Fold` 构造器形状为 `forall s. Fold (s -> a -> m (Step s b)) (m (Step s b)) (s -> m b) (s -> m b)`：初始化、逐值 step、extract、final。
- [观察] `Scanl m a b` 使用同一类状态推进但允许持续输出/重复 extract；非终止构造为 `mkScanlM :: Monad m => (b -> a -> m b) -> m b -> Scanl m a b`。
- [推断] `Fold` 是“消耗到终止再返回 b”，`Scanl` 是“每个输入更新状态并可采样”；这比直接把所有指标字段列在 object 中更小，但仍是逐输入状态机而非通用 patch 图。

#### ③ 多 provider / 异构能力怎么表达

- [观察] effect 在 `m` 中，并发要求 `MonadAsync m`；`Config -> Config` 修饰器暴露 `maxThreads`、`maxBuffer`、`ordered`、`interleaved`。
- [推断] `m` 是开放 provider/interpreter 轴，`MonadAsync` 是静态能力门槛；没有领域 provider 枚举、Read/Write handshake 或 capability evidence。
- [观察] `parDemuxScan` 可按动态 key 创建 Fold；没有对应 Fold 时输入会丢弃，这是一种运行中的 consumer availability 语义，而非类型级 provider matrix。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] `Tee`/`teeWith`/`distribute` 将一份输入复制给多个 Fold/Scanl，再用 `Applicative`、`Semigroup` 或 `Monoid` 合并结果；`parTee`/`parUnzip` 可并行多个 consumer。
- [观察] `parBuffered` 将输入放入 bounded buffer，buffer 满时停止生成更多并发任务；`maxBuffer < 0` 才是不设上限。默认并发结果按到达顺序，`ordered` 恢复输入顺序。
- [观察] 解释边界是 `Stream.fold` 交给 `m`，资源由 bracket/Acquire 管理；本次材料没有一个统一 interpreter 类型或 effect journal。
- [推断] 组合律依赖 Haskell `Applicative/Semigroup/Monoid`；副作用 `m` 仍让执行顺序和取消成为可观察行为。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] 基础状态是 `Partial s | Done b`；`m` 承载异常/取消，Rate 使用 `Double`/yield credit，定时窗口使用真实 clock；没有 money、venue id 或 unknown result 类型。
- [观察] `intervalsOf` 可在窗口结束强制 Fold 完成；`Rate` 文档说实际速率可能落后/超过目标，超过 `rateBuffer` 的追赶 gap 只恢复到 buffer 范围。
- [观察] bounded buffer 是等待/停止生成，不是满时静默丢弃；但 `parDemuxScan` 对没有 consumer 的 key 明确 discard。多消费者是复制给各 Fold，不是共享可靠广播。
- [推断] Rate gap 不是数据丢失证明；Streamly 没有序号、重放、审计关联或“不确定结果”类型。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 官方警告无界 `maxBuffer` 加无界 `maxThreads` 在 infinite/large streams 上是 “recipe for disaster”，可能无限并发生成。
- [观察] Streamly 的 dual representation（Scott/CPS 与 vector-like）和多种 scheduling mode 是性能优势，也增加理解、调试和顺序选择成本；文档没有给出统一语义 law 定理。
- [无一手证据] 本次页面没有作者 post-mortem、生产事故或可靠性 SLA；必须保留证据不对称。

### 3. pipes：四端口 `Proxy` 与同步背压

#### ① 系统做什么、规模与生产使用

- [观察] pipes 4.3.16 Hackage 页面将其定义为 “Compositional pipelines”；官方教程把 effects、streaming 和 composability 作为同时目标。
- [观察] Producer、Consumer、Pipe、Effect 都是同一底层 Proxy 的受限视图；本次 Hackage/API/源码没有生产部署数量或 SLA。
- [无一手证据] 文档示例不等于生产规模；报告只使用其 API 与作者维护者对性能/律的说明。

#### ② 设计中心是什么

- [观察] 源码定义：`data Proxy a' a b' b m r = Request a' (a -> Proxy a' a b' b m r) | Respond b (b' -> Proxy a' a b' b m r) | M (m (Proxy a' a b' b m r)) | Pure r`。
- [观察] `Producer b = Proxy X () () b`、`Pipe a b = Proxy () a () b`、`Consumer a = Proxy () a () X`、`Effect m r = Proxy X () () X m r`，其中 `X = Void`。
- [推断] Proxy 是带两个方向端口、基础 monad 和结果的协程/自由结构；类型别名通过端口把不可能的操作在编译期排除，而非业务对象字段对齐。

#### ③ 多 provider / 异构能力怎么表达

- [观察] provider 的基础效果由 `m` 和 `M` 节点承载；端口参数表达能否 `Request`/`Respond`，`Functor m`/`Monad m` 是组合要求。
- [观察] `reflect` 给出 request/respond、pull/push 对偶；但没有领域 capability registry、运行时 provider handshake 或 Read/Write operation 类型。
- [推断] 增加 producer/provider 通常是新建同样端口的 Proxy；增加领域 operation 只是 `M` 内部的 m action，因而 capability 失败只会在 m/interpreter 或用户协议层出现。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] `yield` 发出值后等待下游；教程原话是 “if nobody consumes, `yield` never returns”，形成同步逐元素背压。`await` 从上游请求值，`for` 逐个 yield 展开。
- [观察] `>->`、`>~`、`~>`、`cat` 等连接遵循 Category/for-loop laws；`runEffect :: Monad m => Effect m r -> m r` 是两端处理完后的解释边界。
- [推断] 同一 Proxy 可组合成 pipeline，但不是多消费者广播，也不记录 request→result audit/correlation；多消费者必须显式复制连接。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] `X=Void`、`Pure r` 和基础 `m` 是主要终止/错误表达；没有内置时间、money、quantity、unknown 或结果状态类型。
- [观察] 标准连接没有“buffer full drop”；无显式 discard 时，生产者通过 `yield` 等待消费者，默认握手可视作 demand-preserving。官方没有内建广播一致性或 durable replay。
- [观察] Proxy 本身是顺序协程；异步/并行必须由基础 monad 或另加库实现，因而 ordering 不是一个统一全局保证。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] `Pipes.Internal` 警告快速实现会弱化 monad-transformer laws，例如 `lift . return = M . return . Pure` 不一定等于 `return`；`observe` 可恢复 law，但付出小性能成本。
- [观察] 教程附录指出部分组合（例如重复 `lift await`）可能二次复杂度，建议 `lowerCodensity` 线性化。
- [推断] 逐元素协程解释、连接层分配和严格律/快速实现的取舍，是 pipes 的维护成本；没有材料把它描述成事件时间计算引擎。

### 4. Conal Elliott：push-pull FRP、`Behavior`/`Event` 与 Reactive Normal Form

#### ① 系统做什么、规模与生产使用

- [观察] Conal Elliott 的 Haskell Symposium 2009 论文 “Push-pull Functional Reactive Programming” 研究把 data-driven push 与 demand-driven pull 结合，避免 pull-only 重复计算并降低采样周期造成的延迟。
- [观察] 论文以连续 Behavior、离散 Event 和可组合语义为中心；未提供生产用户、部署数量或 SLA。
- [观察] 论文结尾要求 “Much more testing, measurement, and tuning”；这是作者对成熟度/工程证据的直接限制，不把近即时描述当生产测量。

#### ② 设计中心是什么

- [观察] 语义类型是 `type Ba = T -> a`（Behavior：时间到值）与 `type Ea = [(T^b, a)]`（Event：按非递减时间的发生列表）；`at`/`occs` 分别观察行为和发生。
- [观察] Behavior 的 pointwise Applicative 满足 `at (fmap f b) = f . at b`；Event 可 Functor、Monad，`merge` 按时间排序并对同刻发生采用左偏序。
- [观察] `switcher :: Behavior a -> Event (Behavior a) -> Behavior a`、`stepper` 把发生切换到新的 Behavior；Reactive Normal Form 给出 `data Reactive a = a Stepper Event a`，连续/离散部分分开。
- [推断] 设计中心是时间语义及其组合律，RNF 只是实现可增量化的分解；它不是带容量的消息 queue。

#### ③ 多 provider / 异构能力怎么表达

- [观察] `T` 只需可比较时间；`Fun t a = K a | Fun (t -> a)` 将常量和时间函数统一。论文没有 Read/Write provider 或 capability evidence。
- [推断] 增加新的时间函数/事件源通常不改 Behavior/Event 语义；但 provider 的 IO、速率、重连和能力需要外部 Sink/interpreter，不能由 `Functor/Applicative` 推出。
- [观察] 作者的 denotational design 原则是 “The instance's meaning is the meaning's instance”，强调先定义语义模型再转移 laws；这不是 provider 协商协议。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] Event 的 Monoid 合并发生；Applicative/Monad 组合可产生交叉发生，RNF 中 push 传播未来离散变化，pull 采样当前连续值；`newtype Event a = Ev (Future (Reactive a))` 一次剥离未来发生。
- [观察] 论文将消费边界显式写成 `type Sink a = a -> IO ()`，并给出 `sinkR`（初值加后续事件）与 `sinkE`（只消费事件）。
- [推断] 因此 effect 解释集中在 Sink，而非每个 Behavior 节点；多消费者可以各有 Sink，但没有内建广播、审计或 effect-result correlation。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] 时间是连续语义（历史上可为 `R`，只需 total order），Event occurrence time 非递减；同刻事件的左偏 merge 是确定 ordering。论文没有 money/quantity/id/error/unknown 类型。
- [无一手证据] 论文不规定网络背压、队列容量、gap 或丢包；如果 Sink/thread 跟不上，属于具体解释器问题。
- [推断] Reactive Applicative 缓存上次函数/参数并只组合发生变化的一侧，增量粒度是 Reactive/Future 触及的子图，而非每个 bar 全量重跑；Event Applicative 发生数可能乘法增长。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 论文批评纯 demand-driven 会把延迟绑定到采样周期，指出 Yampa/Arrow 输入组合可能使所有输入一起变化；GADT 优化会带来 runtime pattern matching 与代码复杂度。
- [观察] 作者讨论 `Fun` 的 Monad 是否保留想要的时间函数语义，指出命令式 callback 难以保持确定 merge；未来工作包含更多测试、测量、缓存和单调性研究。
- [结论] Elliott 的贡献是语义先行与 push/pull 分工；它不能直接替代执行层的背压/丢失/可靠消费协议。

### 5. Reflex：`Behavior`/`Event`/`Dynamic`/`Incremental`

#### ① 系统做什么、规模与生产使用

- [观察] Reflex 0.5 是独立于 DOM 的 Haskell FRP 基础；官方文档列出 Tenjin Reader、Gonimo、游戏和其他应用，同时注明部分示例过时或实验性。
- [观察] 文档/源码提供 frame-based event propagation、host trigger、DOM 之外的应用入口；本次材料无统一用户量、延迟或生产 SLA。
- [推断] 应用清单证明生态使用，不证明每个应用的当前生产状态；版本证据是官方 0.5 文档（PDF 日期 2019-08-23）与 develop branch 源码。

#### ② 设计中心是什么

- [观察] `Reflex t` class 关联 `Behavior t`、`Event t`、`Dynamic t`、`Incremental t`、`PushM t`、`PullM t`，并要求 `MonadHold`/`MonadSample` 等组合约束。
- [观察] 官方注释把 Behavior 描述为“可随时间变化、可随时 sample、但不能被通知”的容器；Event 在 frame 中发生或不发生并携带值；Dynamic 同时包含 current value 与 update Event。
- [观察] Incremental 用 `Patch`/`PatchTarget` 应用部分更新；`currentIncremental` 取当前值，`updatedIncremental` 消费 patch Event；官方建议它只用于大值、小 patch 和性能关键路径。
- [推断] 设计中心是 frame 内传播的时间图节点，`Dynamic` 是 full replacement + notification，`Incremental` 是 patch + notification；两者都不是普通 queue。

#### ③ 多 provider / 异构能力怎么表达

- [观察] `PushM`/`PullM` 表示 push/pull calculation，`TriggerEvent/newTriggerEvent` 从外部 IO 注入 Event，`performEvent/performEventAsync` 把 action 交给 host。
- [推断] timeline 参数 `t` 与 engine 分离，host/trigger/interpreter 是 provider 轴；但没有领域 Read/Write capability handshake、audit log 或 replay identity。
- [观察] `MonadHold` 规定 frame 后才更新 held value；同 frame sample 看到旧值，说明能力/时序部分由 engine 语义而非业务 object 承载。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] Event 有 `mergeWith`、`mergeList`、`leftmost`、`merge`；`fan/fanG` 提供高效多路 fanout；`current`/`updated` 是 Dynamic 的两种消费投影。
- [观察] `switchDyn` 与 `switchPromptlyDyn` 处理动态 Event 网络切换；Promptly 版本在旧/新同 frame 时偏新事件，但官方警告性能/递归风险。
- [观察] `performEvent` 是 effect interpreter 边界；核心图主要传播纯值和 patch，不为任意用户函数建立可审计执行记录。
- [推断] `Incremental` 的组合对象是 patch algebra/target；与 Streamly `Scanl` 的“每项更新状态”不同。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] frame 是原子传播时间单位，`NominalDiffTime`/`UTCTime` 出现在 `tickLossy :: NominalDiffTime -> UTCTime -> m (Event t TickInfo)`；`tickLossy` 明确“不保证每个周期都有 tick”。
- [观察] `holdUniqDyn` 通过 equality 抑制相等更新；官方性能文档提示 pointer equality/WHNF 判断会产生成本。核心没有 money/id/unknown write-result 类型。
- [推断] loss 主要发生在 timer/host 边界，不应泛化为所有 Event 都丢失；frame propagation 也不等价于阻塞队列。`fan` 复制事件通道，但下游状态仍独立。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 官方性能材料说 Incremental 不应普遍替代 Dynamic；只有大值小 patch、关键性能路径才值得付出复杂度。
- [观察] Promptly API 在 RecursiveDo feedback loop 下可能 hang 或 stack overflow，官方建议优先非-Promptly 版本；`switchPromptlyDyn` 也可能降低性能。
- [无一手证据] 本次材料没有统一 production replay、audit linkage 或 provider failure recovery；这些不能从类型名推断。

### 6. Yampa：因果 `SF a b` 与混合系统

#### ① 系统做什么、规模与生产使用

- [观察] Yampa 0.15 Hackage 描述为 “Elegant Functional Reactive Programming Language for Hybrid Systems”；官方模块称其为混合离散/连续系统的领域语言。
- [观察] 官方文档写明用于专业 Haskell 跨平台游戏，目标含 iOS、Android、桌面和 Web，并列出 SDL/OpenGL/Gloss/HTML5 Canvas 等后端/设备适配；Hackage 页面显示 16 个直接 reverse dependencies、约 33,930 downloads 和 2025-02-28 构建信息。
- [推断] 这些是应用/生态证据，不是单一生产集群规模；Hackage 还把 `-fexpose-core` 标作 unsupported，Basic 模块头写 stability provisional、non-portable GHC。

#### ② 设计中心是什么

- [观察] 概念上 `Signal a -> Signal b`；信号是 `Time -> value`，Event 是信号中的离散发生。内部源码为 `type Time = Double`、`type DTime = Double`、`data SF a b = SF { sfTF :: a -> Transition a b }`。
- [观察] `SF` 有 Arrow 实例；内部 `FunDesc` 提供 `fdComp :: FunDesc a b -> FunDesc b c -> FunDesc a c` 与 `fdPar :: FunDesc a b -> FunDesc c d -> FunDesc (a,c) (b,d)`。
- [推断] 设计中心是带状态/transition 的因果信号变换，不是裸 stream；Arrow 串联/并行和 signal semantics 避免把业务字段拼成单一记录。

#### ③ 多 provider / 异构能力怎么表达

- [观察] `reactimate` 将 sensing/actuation 回调接到网络，`react` 单步推进，`embed` 用给定输入序列运行；polling/queue 和输出采样由 backend 处理。
- [推断] backend 是开放 provider/interpreter 轴，可换设备、网络或测试输入；`SF` 没有 Read/Write tags、runtime handshake、审计日志或结果关联。
- [观察] `hold`、`dHold` 把离散 Event 接到持续值；它们改变的是时间信号语义，不是 provider capability。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] Arrow combinators、`par`/`parZ`/`parC`、`pSwitch`/`dpSwitch` 负责串联、并行、集合路由与网络切换；`dpSwitch` 可恢复、丢弃、保存或组合旧 continuation。
- [观察] `reactimate` 是外部执行边界，`react`/`embed` 是确定输入下的计算消费；核心 SF 保持 backend-neutral。
- [推断] 组合关系是 Arrow 图和 transition state；副作用真正发生在 sensing/actuation callback，核心不提供 workflow result unknown 或 effect audit。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] `Time`/`DTime` 当前实现均为 `Double`；`DTime` 表示相邻反应间隔，时间概念是连续值的采样近似。没有 money、quantity、venue id、unknown result 类型。
- [观察] `parZ/parC` 要求子 SF 数量由首个输入决定且稳定；多余输入忽略，不足输入抛异常。`repeatedly` 警告周期短于采样频率时 occurrence 可能错过。
- [推断] Yampa 没有统一 queue backpressure、sequence gap 或 replay；采样不足是“不可观察/漏 occurrence”，与生产者阻塞不同。多消费者是结构性并行复制。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] provisional/non-portable 标记和 unsupported expose-core 是维护边界；`pSwitch/dpSwitch` 的 continuation 管理和并行 cardinality 是额外心智负担。
- [观察] 高频 transition、集合路由和切换 continuation 带来状态/采样成本；官方没有声明核心具备消息队列背压或审计保证。
- [无一手证据] 本次来源没有作者 post-mortem 或生产事故数据；不把应用列表扩写成稳定性结论。

### 7. Jane Street Incremental：OCaml 依赖图、`stabilize` 与 Observer

#### ① 系统做什么、规模与生产使用

- [观察] `incremental_intf.ml` 开头定义它用于“computation that depends on variables and that can automatically incrementally recompute after the values of some variables change”；依赖图可按需传播。
- [观察] Yaron Minsky 的 “Introducing Incremental” 将它用于 GUI construction 和 risk calculations：portfolio risk 是依赖 live market data 与 user configuration 的复杂模型，配置既可改系数也可改 factor list；Jane Street 已在 “a number of our UIs” 使用该方法。
- [观察] 本次仓库提交为 `98b5750ec3c006641351bfd858a89136a5dbc52c`（`v0.18~preview.130.106+341`，2026-07-10）；这是源码版本，不是交易生产 QPS。

#### ② 设计中心是什么

- [观察] 核心抽象是 `'a t`（`Incr.t`）：`val map : 'a t -> f:('a -> 'b) -> 'b t`、`val bind : 'a t -> f:('a -> 'b t) -> 'b t`；`Var` 提供 mutable input，`Observer` 把需要的数据拉到消费边界，`stabilize` 推进图。
- [观察] `module Var`: `create`, `set`, `watch`, `value`；`module Observer`: `observing`, `value : 'a Or_error.t`, `value_exn`, `on_update_exn`，更新为 `Initialized | Changed | Invalidated`；`val stabilize : unit -> unit`（`incremental_intf.ml:712-849`）。
- [观察] 接口称节点形成 DAG，只有 observed/necessary nodes 被计算；`bind` 能按当前值动态创建/删除子图。
- [推断] 设计中心是可观察、可 cutoff 的 dependency graph node，不是 stream transport；所有 UI/risk 业务值都是节点参数和组合结果。

#### ③ 多 provider / 异构能力怎么表达

- [观察] generative functor `Incremental.Make()` 为每个世界 mint fresh types，防止不同 graph world 混用；能力不是 Read/Write 枚举，而是 `Var`/node 的依赖关系与 Cutoff。
- [观察] 用户函数被当作纯计算；接口明确说 Incremental 不知道函数是否有副作用，若节点不 necessary 就不会调用。
- [推断] 增加 provider 等价于把外部值以 Var/输入桥接进图，增加 operation 等价于新增纯 node combinator；真正 IO、重试、不可判定 Write 必须在图外解释，不能依赖 `stabilize` 语义。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] `map/map2/array_fold/reduce_balanced` 组合静态依赖；`bind` 的签名允许 `f` 返回任意新子图，变更时旧节点 invalidate。`observe` 建立消费点，`stabilize` 按 height heap 拓扑重算。
- [观察] `Observer.on_update_exn` 在 stabilization 后仅当值初始化/变化时回调；setting Var 并不会立刻更新结果，必须显式 `stabilize`。
- [观察] `set_cutoff` 可按物理相等、比较或阈值停止传播；这是一种派生观察的消费策略，不是消息丢弃。
- [推断] 谁关联谁由 DAG edge、necessary path 与 observer 明确承载；没有外部 effect 的执行/audit 关系。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] `Observer.value` 用 `Or_error.t` 表达读取失败，Update ADT 有 `Invalidated`；`stabilize` 按 height 保证 children before parents，节点最多计算一次。
- [观察] `max_height_allowed` 默认 128；异常导致 Incremental system unusable，后续 stabilize 立即 raise；接口没有时间/背压/queue/gap 类型，也没有 unknown external result。
- [观察] 多 observer 可共享 DAG；只有从 necessary node 到 observed node 的子图才维持/计算。数组聚合支持 inverse/update 结构，作者文章给出 binary tree 更新 `log(n)`、有 inverse 时可做到 constant time 的思路。
- [推断] 增量粒度是受影响 graph node，ordering 是拓扑 height，不是 event-time order；外部多消费者的慢/断线必须另建 transport。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 作者明确说单节点 firing 约 30ns，远高于简单求和；Incremental 适用于单节点工作足够大或 graph 很大但受影响子图很小的情况。
- [观察] `bind` 动态图很强，但会引入 height constraints、invalidate 和 cycle detection；`stabilize` 异常后全局不可用是强失败语义。
- [观察] generative worlds、显式 observe/stabilize、cutoff 和纯函数约束都是可维护性收益，也要求使用者理解图生命周期。
- [结论] Incremental 是本组最清楚的“派生计算 design center”，但不是 effectful stream；将外部写直接塞进节点会违反其明确的纯性/必要性语义。

### 8. Adapton：命名 `Art<T>` 与 demanded computation graph

#### ① 系统做什么、规模与生产使用

- [观察] Adapton Rust README 将自身定义为 general-purpose Incremental Computation library；背景是 demanded computation graph（DCG）、demand-driven change propagation 和 first-class names。
- [观察] README 称 Rust 实现相对 Python/OCaml 有更好性能，并以经验结果宣称 predictable/scalable；本次材料是研究库/benchmark 证据，没有生产客户或运行时 SLA。
- [观察] 本次 clone 提交 `ea903dd961bee3568c483a18a72983d9f4ef5ed2`（2019-12-21），GitHub 元数据显示 archived；这本身是维护状态事实，不能从 archive 推出算法不可用。

#### ② 设计中心是什么

- [观察] `Art<T>` 是统一 eager ref cell/lazy thunk 的 articulation：`pub struct Art<T> { art: EnumArt<T> }`，`EnumArt` 可为 `Rc(Rc<T>)`、`Loc(Rc<Loc>)` 或 `Force(Rc<dyn Force<T>>)`（`engine.rs:2090-2121`）。
- [观察] 每个 Art 有唯一 `Name`，可 O(1) hashing/equality；`cell(Name,T) -> Art<T>`、`thunk(...) -> Art<Res>`、`force(&Art<T>) -> T` 将需求与缓存统一在一个操作。
- [推断] 设计中心不是“所有字段的缓存 object”，而是有 identity 的可 force articulation；thunk 消费 incremental input 并产生 incremental output。

#### ③ 多 provider / 异构能力怎么表达

- [观察] `NameChoice` 可指定 nominal Name、structural identity 或不缓存；thunk 还带 `ProgPt` 和 ordinary/spurious arguments，精确命名用于 memo matching。
- [推断] 能力主要是值级/命名级缓存和 engine choice（Naive/DCG），不是 compile-time Read/Write capability；新增 provider 需要把输入作为 Art/外部 editor role 接入。
- [无一手证据] 本次源码没有领域 provider handshake、能力矩阵或运行时协商协议。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] README 区分 Editor（创建/修改 input、demand 输出）与 Archivist（cached thunks）；`get!/force` 是观察/demand，`thunk!`/`memo!` 创建计算。
- [观察] `force` 同时消费 eager cell 和 lazy thunk；`force_map` 适合先 compose 再 projection；DCG engine 记录依赖，变更传播只触及被 demand 的部分。
- [推断] 这是按需纯/缓存计算的解释器，不是副作用执行器；IO 写、审计和未知结果不应通过 `force` 推断。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] 基础类型是 `Name`、`ProgPt`、`Art<T>`，要求 `Hash + Eq + Debug + Clone + 'static`；`force_cycle` 可在 force edge cycle 时给 fallback。
- [无一手证据] 本次核心没有时间、money、quantity、ordering、backpressure、gap 或 unknown provider result；多 consumer 共享命名 Art/缓存，但各自 force 仍受 demand/lifetime 影响。
- [推断] 命名 identity 解决的是 memo/cache 关联，不等于外部 request id 或交易审计 id；把 Name 直接当业务幂等键会越界。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] README 直接指出 traversal-based garbage collection 对 incremental computation 有 performance challenges；Rust 选择部分是为了避开该代价。
- [观察] thunk 的 spurious higher-order arguments 不能比较，源码称之为 “necessary hack”；nominal/structural identity 让缓存命中与依赖正确性变得复杂。
- [观察] archived 状态、需要初始化 DCG engine、命名纪律和 force cycle 处理都是维护成本；本次材料未找到作者生产事故 post-mortem。

### 9. Rust Salsa：tracked query、revision 与 on-demand graph

#### ① 系统做什么、规模与生产使用

- [观察] Salsa README 定义为 “A generic framework for on-demand, incrementalized computation”；query 是 `K -> V`，inputs 可变，纯 functions 结果 memoized。
- [观察] 官方 overview 明确：“Salsa is used in rust-analyzer ... to help it recompile your program quickly as you type.” 这是真实生产/大规模工具链使用证据，但没有把 rust-analyzer 性能数字归因给某单个 query。
- [观察] 本次 clone 提交 `22d6cc724f063a91974d73b0f0dbd7d3aafdfdad`（Cargo 0.28.2）；源码与 book 均以当前提交为准。

#### ② 设计中心是什么

- [观察] `#[salsa::input]` 生成的新typed `ProgramFile(salsa::Id)`；tracked function 形状为 `#[salsa::tracked] fn parse_file<'db>(db: &'db dyn Db, file: ProgramFile) -> Ast<'db>`。
- [观察] 调用 tracked query 会记录其读取的 input、memoize 返回值；若 inputs 未变可复用，red-green algorithm 决定重执行。`Database`/`Storage` 保存 revisions、memo 与 cancellation。
- [推断] 设计中心是可追踪 query graph + database revision，而不是 Rust struct 字段本身；input/AST/Id 只是 graph key/value 的载体。

#### ③ 多 provider / 异构能力怎么表达

- [观察] query 必须以 `&dyn Database` 为首参，输入 key 要实现 Eq/Hash；`Database` trait 可由 `#[salsa::db]` 子 trait 扩展，durability 分级用于判断变化传播。
- [推断] provider/interpreter 通过 Database 实现与 input/query definitions 扩展；新增 provider 不要求修改核心 storage，但新增 operation 必须是符合 tracked/query 约束的函数。
- [观察] query 假设纯 deterministic；外部未知读取只能 `report_untracked_read`，下个 revision 重新执行。没有领域 Read/Write capability 或网络 handshake。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] tracked query 内只能读 database（不能修改 input）；setter 在外层 master loop 修改 input 并产生新 revision。query 间通过调用关系组合，memo 负责复用。
- [观察] cancellation、events、LRU eviction、durability 由 `Storage/Database` 解释；没有把 arbitrary IO 当作 tracked query 的可靠副作用。
- [推断] 多消费者是多个 query/observer 共享 memo database；消费关联由 query key、revision 和 dependency read 承载，未包含外部写回执/审计链。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] 核心基础类型是 `salsa::Id`、revision、durability、query key/value；`Database::report_untracked_read` 表达“依赖 Salsa 不知道的状态”。
- [观察] query cycle 默认 panic；fixed-point 仅适用于 monotone、partial order、fixed height，`cycle_result` 可用 execution-order-independent fallback。取消涉及跨 worker coordination。
- [无一手证据] Salsa 没有 stream backpressure、event-time watermark、gap marker 或 unknown network result；revision ordering 是内部一致性顺序，不是 provider 事件顺序。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] book 明确 cycle 超过 200 次会 panic；cycle recovery 中再次进入同 cycle 可能有 unpredictable results。
- [观察] `Database` 文档警告 snapshot 存在时 write/LRU eviction 会 block，当前线程持有 snapshot 可能 deadlock；storage cancellation 也警告一个 worker 持有两个 handle 会 deadlock。
- [观察] red-green reuse、LRU eviction、untracked read 与 cancellation 让性能好但生命周期/线程约束复杂；没有把 query memoization 当消息可靠性证明。

### 10. Google Cloud Dataflow Dataflow Model：window、watermark、trigger、accumulation

#### ① 系统做什么、规模与生产使用

- [观察] Akidau 等 VLDB 2015 论文把 Dataflow Model 定义为 Google Cloud Dataflow 模型，基于 FlumeJava 与 MillWheel；目标是 unbounded、unordered data 上可调 correctness/latency/cost 的 event-time computation。
- [观察] 论文说实现覆盖 MillWheel streaming engine、FlumeJava batch engine 和 runtime-agnostic open-source SDK；这是生产系统/工程化证据，论文没有在本文给出一个统一 QPS 数字。
- [观察] 作者明确批评许多旧系统缺 exactly-once、event-time window 或 trigger；这不是把所有流都当“最终会完整”的模型。

#### ② 设计中心是什么

- [观察] 论文没有单一 OO 核心类型，而是四维模型：计算什么结果、在 event time 哪里计算、在 processing time 何时 materialize、早期结果如何与后续 refinement 关联。
- [观察] window API 为 `Set<Window> AssignWindows(T datum)` 与 `Set<Window> MergeWindows(Set<Window> windows)`；输入从 `(key,value)` 扩展为 `(key,value,event time,window)`。
- [推断] 设计中心是 event-time window + trigger/refinement algebra；不能强行写成某个单一 `Stream<T>`，否则丢掉迟到数据与修订语义。

#### ③ 多 provider / 异构能力怎么表达

- [观察] 抽象模型可落到 batch/micro-batch/streaming engine；触发器可利用 watermark timer、processing-time timer、data arrival、RPC completion 或其他 external signal。
- [推断] execution engine 是开放 interpreter 轴，window/trigger/refinement 是跨 engine 的逻辑语义；没有领域 provider operation/capability matrix。
- [观察] 论文强调将 logical processing 与 physical implementation 分离，便于把 engine 选择还原为 latency/resource/correctness trade-off。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] windowing 决定 event-time 分组，triggering 决定 processing-time pane 发出；trigger 支持 AND/OR/loop/sequence/custom signal 组合。
- [观察] 三种 refinement：Discarding（后 pane 与前 pane 无关）、Accumulating（后结果 refinement 前结果）、Accumulating & Retracting（先撤回旧值再发新值）。后者是多个 serial GroupByKey/Window 正确传递修订所需。
- [推断] 这让“早期观察→后续修订”有明确解释层，但论文没有外部资金 effect 或 unknown write receipt；sink 是否支持 overwrite/retraction 仍是下游契约。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] Event Time 是事件发生时钟，Processing Time 是 pipeline 观察时钟；论文不假定分布式 clock synchronization。watermark 是已处理 event-time 的 lower bound，通常 heuristic。
- [观察] watermark 太快会放过 late data，太慢会被一个 straggler 拖住；watermark alone insufficient。Discarding/Accumulating/Retracting 明确表达 pane ordering 与修订。
- [无一手证据] 模型本身没有 money/id/unknown external result、网络 backpressure 或 sequence gap marker；“late”是事件时间相对 progress 的事实，不等于传输断线。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 论文指出 watermark 100% 正确通常不可知；Lambda 方案通过 batch overwrite 获得 eventual correctness，但要维护两套系统，运维复杂度高。
- [观察] Accumulating & Retracting 要求 deterministic operations；支持 nondeterminism 需要额外 complexity/cost。早期结果可被后续数据 invalidated，这是用户可见的语义负担。
- [结论] Dataflow Model 的价值在于把时间与修订作为一等语义；它不回答领域 Write 的不可判定回执或 provider capability。

### 11. Naiad：Differential Dataflow 的部分序 logical time

#### ① 系统做什么、规模与生产使用

- [观察] Naiad 论文引入 differential dataflow，支持 incremental input、arbitrarily nested fixed-point iteration；数据分析例子包括 shortest paths、SCC、PageRank。
- [观察] 论文用 Amazon co-purchasing network（约 400,000 nodes、3.4 million edges）测量 connected components：最终迭代只处理 18 difference records，约 5ms；单边更新平均约 0.49ms。这里是论文 prototype benchmark，不是现行生产 SLA。
- [观察] 论文说明原型在 single multi-core computer，很多 motivating applications 需要 cluster；不能把 benchmark 规模误报为云生产规模。

#### ② 设计中心是什么

- [观察] Collection 是从 records 到 integer counts 的 multiset；difference 可加/减。logical time 是 partial-order lattice，嵌套 fixed point 通过增加 time coordinate 表示。
- [观察] 论文形式化 `A : T -> (R -> Z)` 的 collection trace 与 `δA` difference trace；dataflow graph 的 edge 传 trace，operator 对 collection 做 typed transformation。
- [推断] Naiad 的设计中心是“lattice-time trace + difference”，不是单个流元素或业务 record；增量、迭代和多版本由同一代数组合。

#### ③ 多 provider / 异构能力怎么表达

- [观察] 程序在 strongly typed collections 上使用 Select、GroupBy、Join、FixedPoint；operator key function 决定可分区方式，跨线程/进程/机器只需相同 key 保持一起。
- [推断] provider/执行 engine 可以扩展，但 capability 主要是 operator/key/lattice 构造，没有 Read/Write provider handshake 或外部 effect type。
- [观察] times 对程序员大多隐藏，由 `OnNext` 推进 epoch；这强调“内部 progress”而非业务 ID。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] operator 组合产生 output collection；`Subscribe` 注册每 epoch event handler，`Sync` 等待前序 epoch quiet。FixedPoint 只处理变化 difference，收敛后不需重跑全量。
- [观察] `Distinct`/`Min` 等 operator 通过正负差异维护内部状态，sink 收到 additions/subtractions；这属于纯数据流解释，不是外部写 effect。
- [推断] collection→operator→Subscribe 是计算与消费链；request/resource identity、audit、unknown result 不在 Naiad 核心中。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] multiset record frequency 是 `Z`；time 可视作整数 tuple/product lattice，collection value 是所有 `s <= t` difference 的和；ordering 是 partial order 而非全局序列。
- [观察] Naiad 明确放弃 collection 的 relative order/set semantics，靠 `Distinct`/Min/Max 等 operator 恢复部分行为；这是一个可见的基础类型取舍。
- [无一手证据] 没有消息 backpressure、gap marker、money/id/unknown result。多消费者/epoch handlers 需由运行时/程序显式管理。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 论文说 multiset 让 addition/subtraction/commutativity 好推理，但放弃 ordering；加入 sequence type 会增加实现复杂度。
- [观察] nested fixed-point graph 可很复杂：SCC 示例实际图有 58 vertices；lattice differences 有时保留不直观的工作，但总体少于 total-order incrementalization。
- [结论] Naiad 是 event-time-like differential algebra 的真实系统证据；它不等于通用推送队列，也不自动表达外部 provider 失败。

### 12. Differential Dataflow Rust：`Collection` 与 `(data,time,diff)`

#### ① 系统做什么、规模与生产使用

- [观察] 官方 README 定义为运行在 timely dataflow 之上的 data-parallel framework，支持大数据、arbitrary input changes、`map/filter/join/reduce/iterate`。
- [观察] README 示例在 10,000,000 nodes/50,000,000 edges 上初次加载约 15.47s，单变更约 228.451µs，称约一万 updates/s；这是作者 laptop benchmark，README 自己也说 “it isn’t the right answer yet”，不能当 production claim。
- [观察] 当前 clone 提交 `aa8745f93ea8abe131104fc7885ba4fd47e63902`；源码当前 `Collection` 使用 abstract container，不能擅自简化成旧版具体 `Vec`。

#### ② 设计中心是什么

- [观察] `pub struct Collection<'scope, T: Timestamp, C: 'static> { pub inner: timely::dataflow::Stream<'scope,T,C> }`；文档称其为 “core abstraction for an updatable pile of data”。
- [观察] README 的 `inspect` 输出形式为 `((degree,count), time, delta)`，`delta < 0` 表示 record departure；`Collection::concat` 对应 collection addition，collections unordered。
- [观察] difference trait 由 `IsZero`、`Semigroup`、`Monoid`、`Abelian` 构成；`IsZero` 允许删除无影响 update，`Abelian` 提供 negation（`difference.rs:1-63`）。
- [推断] `(data,time,diff)` 加 collection operators 是核心代数；小 tuple 取代 giant object，但它只适合有可加/可撤回更新的派生数据。

#### ③ 多 provider / 异构能力怎么表达

- [观察] source/input 通过 timely worker；operator 对 `Collection<T,C>` 组合，`T: Timestamp` 和 container/difference traits 是静态要求。
- [推断] 可接入新的 input/worker/provider 而不改变 collection algebra；新增非-Abelian operation 可能要求新的 operator/trait，不会由类型自动获得撤回正确性。
- [无一手证据] README/源码没有领域 provider capability、runtime handshake、幂等 Write 或审计关联。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] `map/filter/join/reduce/iterate` 组合 collection；`probe/inspect` 是完成观察和输出变化的边界，input handle 负责注入，timely worker 负责推进。
- [观察] 差异可以合并/压缩；累计到 zero 可删除 update；负 diff 让下游撤销旧输出而非全量重算。
- [推断] 这是纯 differential computation→probe/sink 的解释分层；若 sink 需要把多个 diff 变成外部副作用，关联/重试/unknown 要由 sink 自行定义。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] `T: Timestamp` 是逻辑时间，`C` 是 abstract container，difference 是 monoid/abelian group-like value；time frontier/probe 由 timely 承载。
- [观察] collection unordered；同一 record 可有正、负多个 diff；没有内建 latest-value drop。progress/gap 不由 Differential `Collection` 本身标记，而由 timely progress/sink 观察。
- [无一手证据] 没有 money/venue id/unknown external outcome 或网络 reconnect/replay contract。多消费者可共享 dataflow graph，但各 sink 的读取/积压策略不是 collection algebra。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] README 展示“小输入变化可产生多个 output changes”、有些 rounds 不输出，因为变化抵消；使用者必须理解负 diff、retraction 和多输出。
- [观察] 非交换 semigroup 应谨慎，因为很多 timestamp 没有 total order；container implementation 还必须维护 timestamp invariant，否则 operators may behave unexpectedly。
- [推断] 主要代价是 Abelian/difference 设计、compaction/frontier、retraction-aware sink 与调试；benchmark 数字不能外推生产。

### 13. Materialize：Persist pTVC、Differential Compute 与 `SUBSCRIBE`

#### ① 系统做什么、规模与生产使用

- [观察] 官方技术文章定义 Materialize 为 “a fast, distributed SQL database built on streaming internals”，供 data/software teams 构建高速度服务；逻辑组件是 Storage/Persist、Adapter、Compute，`clusterd` 可扩展到多个 processes/replicas。
- [观察] Compute 将 SQL/IR 编译为 Differential Dataflow，结果可返回 one-off query、留在 index 或写回 Persist materialized view；Sources/Sinks 连接 Kafka/Redpanda 等外部系统。
- [观察] 这是官方产品架构与企业使用定位，不是本报告独立测得的生产规模；文章明确 time-travel 仍未普遍实现。

#### ② 设计中心是什么

- [观察] Materialize 的关键抽象是 durable named partial time-varying collection（pTVC）：collection rows + counts（可负），读 frontier 与写 frontier 限制可保留时间区间；物理上以 timestamped diffs 表示。
- [观察] `SUBSCRIBE` 输出 `mz_timestamp`、`mz_progressed`、`mz_diff` 与 relation columns；`mz_diff` 正数插入、负数删除，timestamp 对同一 subscription 非递减。
- [推断] design center 是“durable pTVC + frontier/diff + SQL dataflow”，不是一个巨型 row object；SQL schema 是 payload，版本/变化由外层代数承载。

#### ③ 多 provider / 异构能力怎么表达

- [观察] Adapter 把 PostgreSQL/SQL 请求翻译成 Compute IR；Storage sources/sinks 翻译外部系统与 Persist representation；Persist/Compute/Adapter 可独立部署伸缩。
- [推断] 增加 source/sink/SQL operator 是开放扩展轴，内部用统一 pTVC/diff；provider 的 schema、CDC、connection failure 和 delivery capability 在 Adapter/Source/Sink/runtime 层。
- [观察] 官方架构没有给 SQL 用户一个 Read/Write capability type 或 provider handshake ADT；能力由可用对象、source/sink connector 与运行时错误体现。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] SQL operator 组合成 Differential program；Persist 把写 frontier/read frontier 与数据持久化，Compute 将 pTVC 转成下游 pTVC/index。
- [观察] `SUBSCRIBE` 是 relation changes over time 的消费边界；默认先发 snapshot，再发后续 updates。`PROGRESS` 行表达 timestamp 前不会再有 update；文档说无 progress 无法区分 stall 与 legitimate no-update。
- [观察] durable subscriptions 通过 history retention + `AS OF` 在 connection drop 后从断点继续，避免丢数据和重复 snapshot；这比一般 volatile stream 明确。
- [推断] 关联由 timestamp、diff、frontier、AS OF 和持久对象 identity 承载；它仍不是资金 Write 的审批/回执/audit workflow。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] 关键基础类型是 `mz_timestamp: numeric`、`mz_progressed: boolean`、`mz_diff: bigint`、`AS OF/UP TO` logical timestamp。timestamp non-decreasing，progress true 的 row 只推进时间。
- [观察] `SUBSCRIBE` 支持 `ENVELOPE UPSERT`、`ENVELOPE DEBEZIUM`、`WITHIN TIMESTAMP ORDER BY`；按 key 输出 before/after 或更新顺序。durable AS OF 依赖历史 retention，过度 compaction/retention 会限制恢复窗口。
- [无一手证据] 这些类型不提供 external unknown outcome、venue capability 或 money scale semantics；多个 subscribers/clients 的连接与 cursor 是各自消费会话，不等于一个共享 Topic。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 官方文章说大多数 TVC 无法完整物理表示，需 pTVC/read/write frontier；time-travel 的概念收益尚未普遍实现。
- [观察] 逻辑 timestamp selection 受各 collection bounds 与 strict serializability 约束；`SUBSCRIBE` 可能永不完成，普通 drivers 会一直 buffer，文档建议 cursor/`FETCH`。
- [推断] frontier、compaction、snapshot、diff、progress 和 reconnect 都增加运维/消费者复杂度，但换来可解释修订与恢复；不能把 `SUBSCRIBE` 当无条件 exactly-once external sink。

### 14. TradingView Pine Script：bar-by-bar execution、series 与 alert

#### ① 系统做什么、规模与生产使用

- [观察] TradingView 官方执行模型称 Pine 是 event-driven、sequential；脚本在 historical bars 与 realtime ticks 上反复执行，每个 bar 单独计算，closed-bar 数据进入内部 time series。
- [观察] alerts 在 TradingView servers 24x7 运行，不要求用户登录；创建时保存 script/input/symbol/timeframe mirror。平台本身是生产服务，但官方页面没有给单个脚本吞吐 SLA。
- [观察] Pine limits 页面说明云资源共享导致 data/execution/memory/script-size limits：20/40s 全数据集执行（按账户）、单 bar loop 500ms、64 plot counts、40/64 unique `request.*` calls。

#### ② 设计中心是什么

- [观察] 设计中心是“compiled script state + bar series”；默认变量每次 execution 重声明，`var` 只在 first bar 初始化并跨 bar 持久化，`close[1]`/`close[100]` 访问历史 series。
- [观察] realtime bar 每个 tick 重新计算；rollback 在每次 tick 前恢复 bar-open confirmed state，只有 closing tick 的最终值进入 internal time series；`varip` 跨 tick 保持，但官方警告可能 repaint。
- [推断] 这是一种按 bar/tick 重跑的状态/历史模型，不是 Incremental DAG：即使某表达式未受影响，脚本主体仍按执行模型重新走。

#### ③ 多 provider / 异构能力怎么表达

- [观察] `request.security`、`request.currency_rate`、`request.*` 将 symbol/timeframe/data context 作为脚本语言 API；脚本本身没有 provider capability row。
- [推断] provider/context 在脚本运行时由 TradingView data service 决定；增加数据源 operation 依赖平台允许的 builtin/signature，不能由 `series` 类型证明 venue Read/Write 能力。
- [观察] alert 创建后的 script/input/chart mirror 与原脚本解耦；后续修改不影响已运行 alert，需 delete/recreate。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] 计算表达式和 `ta.*` 形成顺序脚本组合；`alert(message,freq)` 只创建 alert event，不能由代码直接创建 UI running alert；用户在 UI 配置运行。
- [观察] `alert.freq_once_per_bar`、`once_per_bar_close`、`all` 分别选择首个 tick、收盘 tick 或每次 call；策略默认 bar close 重算，order-fill alert 另有 broker emulator 事件。
- [推断] interpreter 是 TradingView chart/alert server，consumer 是图表、strategy broker emulator 和 webhook alert；脚本没有 effect result unknown/audit chain。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] `series`、历史引用和 barstate（`ishistory/isrealtime/isconfirmed`）是基本时间/ordering 类型；realtime current high/low/close 未确认，rollback 丢弃前 tick 临时状态。
- [观察] alert 只在 realtime bar 触发；`freq_once_per_bar_close` 通过等待 confirmed close 减少 repaint，但牺牲即时性。官方原话：可靠性需要 “sacrifice immediacy”。
- [观察] 平台限制是硬资源边界而非 backpressure；脚本超时/超限是错误/拒绝，不产生 gap marker 或 unknown result。多个 alerts 是独立 server snapshots，不是共享多 consumer queue。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 官方 repaint 文档指出使用 high/low/close 的条件会在 realtime bar 变化；非收盘频率可能提前触发，最简单的避免方式是 close-only，但必须等待 confirmed information。
- [观察] `varip` 可逃避 rollback，却因历史重载拿不到 pre-close ticks 而 repaint；`request.*`、execution time、loop、plot 限制迫使开发者拆解脚本/做 proof of concept。
- [结论] Pine 是一个真实的逐 bar alert 消费案例，说明“显式确认 vs 即时性”必须成为用户可见语义；它不是受影响节点增量重算引擎。

### 15. Reactive Streams：`Publisher`/`Subscriber`/`Subscription` 协议

#### ① 系统做什么、规模与生产使用

- [观察] Reactive Streams JVM 1.0.4 README 定义目标为 asynchronous stream processing with non-blocking backpressure，处理 potentially unbounded、in-sequence、异步元素。
- [观察] 规范由 API、TCK 与多实现互操作组成；它是标准层，不能用规范本身声称某具体生产部署规模。
- [推断] Akka、Reactor 等实现生产采用不等于每个实现的行为都由规范定义；本案例只取规范约束。

#### ② 设计中心是什么

- [观察] `Publisher<T>.subscribe(Subscriber)`、`Subscriber.onSubscribe/onNext/onError/onComplete`、`Subscription.request(long)/cancel()`；`Processor<T,R>` 同时 extends Subscriber 与 Publisher。
- [观察] protocol 是 `onSubscribe onNext* (onError | onComplete)?`；`Publisher` 是按 Subscriber demand 发布 sequenced elements 的 provider。
- [推断] design center 是一对一 Subscription-mediated demand protocol，不是数据 schema、派生 algebra 或业务对象。

#### ③ 多 provider / 异构能力怎么表达

- [观察] 规范要求 implementations 可互操作，但明确说 transformation、splitting、merging 不在 scope；Processor 可在上下游之间形成 stage。
- [推断] 增加 provider/operation 主要是实现新的 Publisher/Processor，核心协议不变；但 capability、ordering、replay、domain error 都在实现/上层。
- [观察] Publisher 可支持多 Subscriber，并决定 unicast/multicast；规范不统一规定广播分发。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] Subscriber 通过 request 表达愿意消费多少；Publisher 不得超过 demand。Subscriber 可在 `onNext` 中 request，协议要求 request/cancel serial 与 bounded recursion。
- [观察] `onError/onComplete` 是 terminal；Subscriber 怀疑会影响 Publisher responsivity 时应异步 dispatch。`cancel` 要 eventually stop signaling，但不保证立即清理。
- [推断] 解释/消费边界是 Subscriber/Subscription，组合算子属于实现；规范没有外部 effect interpreter、request key 或 audit result。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] demand 是 additive `long`，可达到 `Long.MAX_VALUE`；非正 request 必须 signal `IllegalArgumentException`；failure 用 `onError(Throwable)`，成功终止用 `onComplete()`。
- [观察] Publisher MAY signal fewer `onNext` than requested and terminate；这不是“丢包”证明，而是 provider 可能无法满足全部 demand。规范不提供 sequence number、gap marker 或 event-time。
- [观察] Subscription 恰好关联一个 Publisher 与一个 Subscriber；多消费者 unicast/multicast 由实现决定，不能从接口推断一致广播。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 规范要求 onNext/termination 不阻塞 Publisher，同时 request/cancel 要及时返回；同步 reentrant request 还要限制 recursion depth，给实现和用户造成严格规则负担。
- [观察] spec intentionally 不覆盖 transformation/splitting/merging；因此仅采用接口不能解决慢消费者策略、drop/latest、replay、provider quota 或业务 Write unknown。
- [结论] Reactive Streams 是可靠的 transport contract（需求量和 terminal protocol），不是完整的组合式副作用模型。

### 16. Akka Streams：GraphStage typed ports 与 materialization

#### ① 系统做什么、规模与生产使用

- [观察] Akka 官方设计文档称 Stream 是 immutable blueprint，`Source`/`Sink`/`Flow`/`BidiFlow` 可组合；Akka HTTP 是该项目内 built-on-Akka-Streams 的例子。
- [观察] 当前源码 `GraphStage` 定义为 reusable graph stream processing operator；`Shape` 描述 input/output ports，factory 创建 `GraphStageLogic`。本次源码 clone 提交为 `e0f2eff299576fbfc06052e5c6a28b51ef160aee`。
- [无一手证据] 官方材料没有给 GraphStage 单独的生产 QPS/SLA；这里只报告成熟的设计/互操作证据。

#### ② 设计中心是什么

- [观察] `abstract class GraphStageWithMaterializedValue[+S <: Shape,+M] extends Graph[S,M]`；Stage 是 shape + logic，运行时 materialization 产生 materialized value。
- [观察] `InHandler.onPush()` 在输入有值时回调，`OutHandler.onPull()` 在 output ready 时回调；`pull(in)` 每个 inlet 只能有一个 outstanding request，`push(out,elem)` 必须在 downstream pull 后调用（`GraphStage.scala:32-44,493-523,623-656,1881-1917`）。
- [推断] design center 是 typed port graph + materialized interpreter，而不是 `Publisher` object 或巨型业务 record。

#### ③ 多 provider / 异构能力怎么表达

- [观察] Source/Sink/Flow/BidiFlow 的 Shape 通过端口数量/方向表达静态连接；Reactive Streams 仅作为 interop SPI，用户层使用 GraphStage/DSL。
- [观察] 官方说 materialization 时才分配 live resources；可在 materialization 做 fusion、dispatcher configuration；dynamic networks 要显式使用 Reactive Streams plugging。
- [推断] 增加 provider/connector 是新 Graph/Stage/Shape；增加 operation 是新 operator；端口类型无法表达 venue Read/Write quota、idempotency 或 runtime handshake。

#### ④ 副作用如何描述、组合、解释与消费

- [观察] `GraphStageLogic` 回调把 `pull`/`push` 组成执行；`Graph` blueprint 与 materialization 分离，materialized value 用于 shutdown/metrics/运行交互。
- [观察] Akka 文档明确 default materialized Processor single Subscriber；需要广播必须显式 `Broadcast` 或 `Sink.asPublisher(WITH_FANOUT)`。这避免把隐式多消费者语义藏在 Publisher。
- [观察] stream failure 会 tear down stream，不等 processing finished；in-flight elements discarded，cancellation 向 upstream 传播；资源清理需用户在 timeout/finalizer/result observation 处理。
- [推断] effect 解释由 materializer/dispatcher 承担，graph 只描述可组合拓扑；外部 write audit/unknown 不在 Stage contract。

#### ⑤ 基础类型、时间、ordering、error、unknown、loss 与多消费者

- [观察] Shape/Inlet/Outlet、materialized `M`、`Throwable` failure、cancel 是基础类型；每个端口单 outstanding pull/push 形成局部 ordering/backpressure。
- [观察] 背压沿 pull/push 传播；但 `take/drop/filter/conflate/buffer` 等 operator 可主动 drop，failure/cancel 会丢弃 in-flight。没有统一 gap/replay/unknown result 类型。
- [推断] 多消费者必须显式 fan-out topology；端口 cardinality 关系比自由 object 字段更可审计，但多个 sink 的慢策略仍是各 operator/dispatcher 选择。

#### ⑥ 已知问题、后悔、性能或人因代价

- [观察] 官方设计文档直说 Reactive Streams SPI “hard to get right”，因此不鼓励终端用户直接实现 Subscriber 等底层件；Akka 用更 type-safe GraphStage 隐藏它们。
- [观察] materialization 一次性约束：如果库直接替用户 materialize，组合性会被破坏；可复用 blueprint 不能绑定 live resource，必须延迟到 materialization。
- [推断] graph shape、materialized value、dispatcher/fusion、explicit fan-out 和 resource lifecycle 提供强边界，但增加运行时拓扑/生命周期心智负担。

## 横向对比表

| 案例 | design center | composition | backpressure | loss / gap | time | incremental granularity | multi-consumer | production evidence |
|---|---|---|---|---|---|---|---|---|
| fs2 | `Pull[F,O,R]`；`Stream` wrapper | Monad/Chunk/`Pipe`/scope | Topic/Channel bounded wait；pull demand | Signal latest-wins；Topic close race；无 gap | effect monotonic clock | Pull step/Chunk | Topic per-subscriber queue；Channel 单 consumer | adopters、http4s/skunk/doobie |
| Streamly | `Fold`/`Scanl` + `Step` | Applicative/Semigroup/Monoid、tee/demux | maxBuffer/maxThreads 节流 | 无 consumer 的 demux key discard；无序缓冲无自动 drop | wall clock interval/rate | 每 input `step` | tee/distribute/parTee | 无生产部署数字 |
| pipes | `Proxy a' a b' b m r` | Category/`for`/`reflect` | `yield` 等待 consumer | 默认握手不静默丢；无广播 gap | 无内建时间 | 每次 Request/Respond | 无内建广播 | 无生产部署数字 |
| Elliott FRP | `Behavior=T→a`、`Event` occurrence list、RNF | pointwise Applicative、merge/switch | 纯语义无队列背压 | Sink 策略未规定 | 连续/有序 event time | Reactive/Future touched subgraph | Sink 外置 | 无生产证据，作者要求更多测量 |
| Reflex | `Behavior/Event/Dynamic/Incremental` + frame | merge/fan/switch/patch | host-specific | `tickLossy` 明确可丢 tick | frame/timeline `t` | frame graph / patch | fan/fanG | 官方应用清单，规模未知 |
| Yampa | causal `SF a b` | Arrow/`par`/switch | 无统一 queue | sampling may miss occurrence | `Time/DTime=Double` | transition/state per sample | structured par | 专业游戏/后端清单 |
| Incremental | `Incr.t` DAG + `Var`/`Observer` | map/bind/cutoff/stabilize | 非 transport | cutoff suppresses propagation；无 gap | 无内建 event time | affected node | shared DAG/observers | Jane Street GUI/risk |
| Adapton | named `Art<T>` / DCG | memo/thunk/force | 无 | demand/cache semantics，无 gap | 无 | demanded thunk/art | shared named Art | research benchmarks |
| Salsa | query `K→V` + revision DB | tracked query/memo/red-green | 无 | cancellation/untracked read，不是 stream loss | revision order | affected query | shared DB/memo | rust-analyzer |
| Dataflow Model | window+watermark+trigger+refinement | trigger AND/OR/loop；pane modes | engine-specific | late data；retraction/accumulation | event vs processing | pane/window refinement | downstream panes/sinks | Google Cloud Dataflow model |
| Naiad | multiset differences + lattice trace | typed operators/fixed point | engine-specific | negative diff；无序 multiset | partial-order lattice time | changed difference | Subscribe/handlers | Amazon graph benchmark |
| Differential | `Collection<T,C>` + `(data,time,diff)` | map/filter/join/reduce/iterate | timely-specific | signed diff；frontier separate | `Timestamp` | changed collection records | graph/sinks | laptop benchmark only |
| Materialize | durable pTVC + frontier/diff | SQL→Differential IR | storage/compute/runtime | `mz_progressed`、durable AS OF | logical timestamp/frontier | diff/pTVC update | independent subscriptions/cursors | official distributed product |
| Pine | script state + series/bar/tick rollback | sequential code/`ta.*`/alert freq | platform limits, not queue BP | realtime pre-close values rollback/repaint | bar/tick, confirmed close | full script per bar/tick | independent alerts | TradingView servers |
| Reactive Streams | Publisher/Subscriber/Subscription | Processor stage；operators out of scope | demand `request(n)` | fewer onNext allowed；无 gap | no event-time | element signal | unicast/multicast implementation choice | JVM standard/TCK |
| Akka GraphStage | Shape + GraphStageLogic + materialized M | immutable graph blueprint | pull/push ports | explicit drop/failure/cancel may discard | scheduler/runtime time | port event/logic step | explicit Broadcast/fanout | Akka HTTP/production ecosystem |

## 专题对比：Pine、Incremental、Differential 与 FRP

### Pine 与 Incremental：重跑脚本 vs 受影响节点

- Pine 的官方模型是每个 historical bar 一次、realtime bar 每 tick 一次；rollback 只保留最终 close tick。`var` 提供跨 bar 状态，但并不把表达式变成可观察 DAG。
- Incremental 用 `Var.set → stabilize → Observer`；只有 necessary path 上受影响的 node 重新计算，cutoff 可截断传播，`bind` 可动态换子图。
- 因而 Pine 适合“一个脚本沿序列解释、alert 依赖确认/频率”的消费模型；Incremental 适合“多个派生值共享输入、只重算受影响图”的模型。二者都没有自动的外部 Write audit/unknown。

### Incremental 与 Differential：节点缓存 vs signed revision

- Incremental 的 `Incr.t` 值是 DAG 节点，变化传播依赖 physical equality/cutoff；结果通常是当前值或 `Observer.Update`。
- Differential 的 `Collection` 是随 `time` 演化的多重集，输出是 `diff`，负值可撤回旧结果；Naiad/Dataflow Model 进一步把 partial order、watermark/window/refinement 放进时间与修订语义。
- 若问题是纯配置/输入改变导致少量派生节点变更，Incremental 的 node graph 更直接；若问题是迟到、重排、历史版本或多次修订，differential/pTVC 的时间-diff 代数更直接。两者都不应被改写成普通 append-only stream。

### FRP 与流 transport：语义时间 vs 执行需求量

- Elliott/Reflex/Yampa 先定义 Behavior/Event/SF 的时间、发生、frame 或 transition 语义，Sink/host/backend 再解释；它们没有统一 bounded queue。
- fs2/Reactive Streams/Akka 把 Pull、`request(n)`、`pull/push` 和 cancellation 放在执行边界，能表达慢消费者，但不承诺 event-time completeness 或派生图最小重算。
- Reflex 的 `Dynamic/Incremental` 是两者的近邻：它有 frame propagation 与 patch，但 `tickLossy` 的 loss 仍是 host timer 语义；不可把名称“Incremental”直接等同 Jane Street/Differential。
- Pine 的 close-only alert 与 Dataflow 的 watermark/refinement 都揭示一个共同事实：确认越强，观察越晚；但 Pine 用脚本 rollback/频率，Dataflow 用 pane/watermark/retraction，机制和可迁移条件不同。

### 能力与关联的横向判读

- 流 transport 案例通常只在 compile/interpreter/runtime 约束 `F`、`m`、`Subscription` 或 port shape；它们没有 operation↔capability、resource↔provider identity、intent↔audit result 的领域关系。
- Incremental/Adapton/Salsa 把 input↔derived query/node、observer↔necessary subgraph、name↔memo identity 做得最清楚，但把 external side effect 明确留在图外或纯性边界。
- Dataflow/Differential/Materialize 把 record↔time↔diff/frontier、pane↔retraction、subscription↔AS OF 关联得最清楚；其主要代价是时间/修订/压缩与 sink 复杂度。
- 不存在一个本组案例同时给出：异构 provider capability matrix、网络断线 gap、纯派生增量、不可判定外部 Write、审批/回执/对账和多消费者可靠重放的统一核心；将这些维度压成一个 giant object 会掩盖缺失语义。

## 对 UTA 的可迁移命题

以下不是 UTA 设计，只是“案例 X 在条件 Y 下用机制 Z 解决问题 W；UTA 若满足 Y 可迁移，否则不可”的证据命题。

1. **fs2**：在消费者愿意用 bounded queue 施加等待、且 active subscription 的资源生命周期由 effect scope 管理时，用 `Pull`/Topic 的按需执行和 per-subscriber backpressure 解决慢消费者占满内存；UTA 若必须保留每条更新且不能阻塞任何 provider，则不可直接迁移 Topic 的默认语义。
2. **fs2 Signal**：在只关心当前值、允许中间更新丢失时，用 latest-update-wins Signal 降低广播成本；UTA 若要求 gap 可审计或每个成交都不可丢，则不可迁移。
3. **Streamly Scanl**：在派生值是逐输入状态更新、状态可用 `step` 高效维护时，用 `Scanl`/`Fold` 避免每个窗口全量重跑；UTA 若需要跨乱序 event-time retraction，则不可只迁移 `Scanl`。
4. **pipes**：在单一上游/下游需要同步 demand-preserving 协议时，用四端口 `Proxy` 和 `yield` 等待解决 producer overrun；UTA 若需多消费者广播、断线 replay 或 provider capability，则不可把 Proxy 当完整核心。
5. **Elliott FRP**：在行为可先给出时间语义、离散事件有确定 ordering、实现可分 push/pull 时，用 Behavior/Event/RNF 减少不变部分重算；UTA 若需要网络背压/持久 gap，则只能迁移语义先行方法，不能迁移其纯语义为 transport。
6. **Reflex**：在更新按 frame 聚合、值需要 current 与 updated 两种消费、且大值只发生小 patch 时，用 `Dynamic/Incremental` 与 `fan` 组合减少复制；UTA 若不能接受 `tickLossy` 式 host loss，必须另有无损 transport。
7. **Yampa**：在问题是因果、连续/离散混合信号，采样周期可作为明确系统参数时，用 `SF a b` Arrow composition 组织网络；UTA 若要求 provider 写结果审计和 event replay，则不可迁移 `SF` 的 backend-neutral 假设。
8. **Jane Street Incremental**：在派生函数纯、输入可识别、消费点有限且需要动态依赖时，用 `Incr.t` DAG、`observe/stabilize/bind/cutoff` 只重算 necessary subgraph；UTA 若把外部不可判定 Write 放入节点，则不可迁移其纯函数语义。
9. **Adapton**：在缓存命中依赖稳定 identity、且 demand 决定需要哪些计算时，用 named `Art<T>`/DCG 统一 eager/lazy producer；UTA 若把 Name 当资金操作幂等/审计 id 而没有相同命名与缓存不变量，则不可迁移。
10. **Salsa**：在输入改变发生于 revision 边界、query 纯且可 memoize 时，用 tracked query、red-green reuse、durability 和数据库存储减少重算；UTA 若 provider 返回未知状态或副作用不可纯化，则只能迁移 query/invalidation 思路，不可迁移其 query-as-effect。
11. **Dataflow Model**：在数据无界、无序、event-time 与 processing-time 分离且迟到会修订结果时，用 window/watermark/trigger/Accumulating&Retracting 解决“何时发、如何修订”；UTA 若只有即时 transport、没有 late-data/revision 需求，则引入 watermark 会是无谓成本。
12. **Naiad/Differential**：在派生结果可表示为 multiset/difference、更新可正负撤回、计算需要部分序 time 或 nested iteration 时，用 `(data,time,diff)` 与 Abelian/difference algebra 限制重算范围；UTA 若 effect 不是可撤回纯更新，不能把负 diff 当外部 Write compensating action。
13. **Materialize**：在需要 durable time-varying relations、progress、按 `AS OF` 断线续读和下游 diff 消费时，用 pTVC/frontier/`SUBSCRIBE` 解决持久订阅不丢历史；UTA 若没有 retention/compaction 与 timestamp authority，则不可只复制 `mz_progressed` 名称。
14. **Pine**：在消费者可接受 realtime 值回滚、并能以 bar-close confirmation 换取可靠性时，用 `series`/history/`var` 与 alert frequency 明确“即时 vs 确认”；UTA 若必须在未知回执下立即安全重试，则不可把 rollback/close alert 当结果确认。
15. **Reactive Streams**：在每个 consumer 能声明 demand、实现只需解决异步边界而不规定领域 operator 时，用 Publisher/Subscriber/Subscription/TCK 约束非阻塞背压；UTA 若要表达 quota、gap、provider operation matrix 或 audit correlation，必须在其上增加明确领域语义而非扩展 `request(n)`。
16. **Akka GraphStage**：在可把拓扑做成 immutable blueprint、资源延迟到 materialization、端口 cardinality 能静态描述时，用 Shape + `onPush/onPull` + materialized value 组织可复用运行图；UTA 若要求动态 provider/consumer 在线改图且不接受显式 fan-out/materialization 边界，则不可直接迁移。

## 未覆盖与开放问题

- [无一手证据] Streamly、pipes、Elliott 论文未提供可复核生产部署数量；Reflex 的应用清单也没有统一规模，Yampa 的下载量不是生产用户数。
- [无一手证据] 本组多数库没有领域 Read/Write capability、provider handshake、幂等键、审计记录或 external unknown result；报告把这些明确标为缺口，而不从 `Monad`/`Arrow`/`F` 名称推导。
- [无一手证据] fs2 Topic/Channel、Reactive Streams/Akka 的 transport 语义不提供通用 event-time watermark；Dataflow/Differential/Materialize 的 progress 也不是资金副作用回执。
- Reflex GitHub 文件在本次读取中只能获得官方 `develop` 分支 URL，未获得稳定 commit 行号；不伪造行号，后续应以指定 release commit 复核。
- Hackage/官方站点页面的 HTML 抽取有些定义集中在单行或链接锚点；报告同时保留 canonical URL、符号锚点和本地打开文件，未把搜索摘要作为主证据。
- 版本会漂移：fs2、Akka、Salsa、Differential 的当前 API/性能可能改变；本文结论锚定来源清单中的提交/页面日期。
- 未覆盖完整的 Durable Streams、Kafka transactional sink、Temporal workflow、Apache Beam runner 实现比较；这些可作为后续专门调查，但不应填补本组“无一手证据”的空白。
- 开放问题仍是语义组合而非字段清单：如何同时保持 provider capability 可检查、派生增量可重算、断线 gap 可定位、外部 Write unknown 不盲重试，以及多消费者的 causal/audit 关联；本组没有单一现成系统回答全部问题。

## 来源清单（正文使用的来源均已在本次打开；URL 状态逐项注明）

1. **fs2 源码提交**：<https://github.com/typelevel/fs2/commit/8aa47aba38454789ce206ad282ed30d45dcbae26>；源码路径 `core/shared/src/main/scala/fs2/Pull.scala`、`Stream.scala`、`fs2.scala`、`concurrent/{Topic,Signal,Channel}.scala`、tests/benchmarks；本地 clone `/tmp/fp05-fs2-main`；已打开。
2. **fs2 官方 README/site/adopters/guide**：<https://github.com/typelevel/fs2/blob/8aa47aba38454789ce206ad282ed30d45dcbae26/README.md>、<https://github.com/typelevel/fs2/blob/8aa47aba38454789ce206ad282ed30d45dcbae26/site/adopters.md>、<https://github.com/typelevel/fs2/blob/8aa47aba38454789ce206ad282ed30d45dcbae26/site/guide.md>、issues <https://github.com/typelevel/fs2/issues/3644>、<https://github.com/typelevel/fs2/issues/799>；本地同一 clone；已打开。
3. **Streamly 官方 Haddock 0.11.0/streamly-core 0.3.0**：<https://streamly.composewell.com/haddocks/streamly-0.11.0/Streamly-Data-Stream-Prelude.html>、<https://streamly.composewell.com/haddocks/streamly-core-0.3.0/Streamly-Internal-Data-Fold.html>、<https://streamly.composewell.com/haddocks/streamly-core-0.3.0/Streamly-Data-Scanl.html>、<https://streamly.composewell.com/streamly-0.11.0/Explanatory/unified-abstractions.html>；本地 opened captures `/tmp/fp05-streamly-1.html`–`4.html`；已打开。
4. **pipes 4.3.16 官方 Hackage**：<https://hackage.haskell.org/package/pipes-4.3.16/docs/Pipes.html>、<https://hackage.haskell.org/package/pipes-4.3.16/docs/Pipes-Tutorial.html>、<https://hackage.haskell.org/package/pipes-4.3.16/docs/src/Pipes.Internal.html>、<https://hackage.haskell.org/package/pipes-4.3.16/docs/Pipes-Core.html>；本地 captures `/tmp/fp05-pipes-0.html`–`2.html`；已打开。
5. **Elliott push-pull FRP**：<https://conal.net/papers/push-pull-frp/push-pull-frp.pdf>（Haskell Symposium 2009）；本地 PDF `/tmp/fp05-push-pull-frp.pdf` 与文本抽取 `/tmp/fp05-push-pull-frp.txt`；已打开，正文引用 §2.1、§2.2.1、§5、§6、§8。
6. **Elliott denotational/type-class morphisms**：<https://conal.net/papers/type-class-morphisms/>、<https://conal.net/papers/type-class-morphisms/type-class-morphisms.pdf>；已打开作者页面/摘要。
7. **Reflex 官方源码/文档**：<https://github.com/reflex-frp/reflex/blob/develop/src/Reflex/Class.hs>、<https://github.com/reflex-frp/reflex/blob/develop/src/Reflex/Dynamic.hs>、<https://github.com/reflex-frp/reflex/blob/develop/src/Reflex/Incremental.hs>、<https://docs.reflex-frp.org/en/latest/overview.html>、<https://docs.reflex-frp.org/en/latest/reflex_docs.html>；本地 captures `/tmp/fp05-reflex-*.txt`/PDF；已打开；develop URL 未伪造 commit 行号。
8. **Yampa 0.15 Hackage/源码文档**：<https://hackage.haskell.org/package/Yampa-0.15>、<https://hackage.haskell.org/package/Yampa-0.15/docs/FRP-Yampa.html>、<https://hackage.haskell.org/package/Yampa-0.15/docs/src/FRP.Yampa.InternalCore.html>、<https://hackage.haskell.org/package/Yampa-0.15/docs/src/FRP.Yampa.Basic.html>；本地 `/tmp/fp05-yampa-0`–`4`；已打开。
9. **Jane Street Incremental 源码**：<https://github.com/janestreet/incremental/tree/98b5750ec3c006641351bfd858a89136a5dbc52c>、<https://github.com/janestreet/incremental/blob/98b5750ec3c006641351bfd858a89136a5dbc52c/src/incremental_intf.ml>；本地 clone `/tmp/fp05-incremental-main`，metadata `v0.18~preview.130.106+341`；已打开。
10. **Yaron Minsky 官方文章**：<https://blog.janestreet.com/introducing-incremental/>、<https://blog.janestreet.com/self-adjusting-dom/>；本地 opened text `/tmp/fp05-introducing-incremental.txt`、`/tmp/fp05-self-adjusting-dom.txt`；已打开。
11. **Adapton Rust**：<https://github.com/Adapton/adapton.rust/tree/ea903dd961bee3568c483a18a72983d9f4ef5ed2>、<https://github.com/Adapton/adapton.rust/blob/ea903dd961bee3568c483a18a72983d9f4ef5ed2/src/engine.rs>、<https://github.com/Adapton/adapton.rust/blob/ea903dd961bee3568c483a18a72983d9f4ef5ed2/src/lib.rs>；本地 clone `/tmp/adapton-rust`；已打开；仓库 README 还链接 PLDI 2014/OOPSLA 2015 论文。
12. **Salsa**：<https://github.com/salsa-rs/salsa/tree/22d6cc724f063a91974d73b0f0dbd7d3aafdfdad>、<https://github.com/salsa-rs/salsa/blob/22d6cc724f063a91974d73b0f0dbd7d3aafdfdad/book/src/overview.md>、`src/database.rs`、`src/storage.rs`、`book/src/cycles.md`；本地 clone `/tmp/fp05-salsa`；已打开。
13. **Dataflow Model**：<https://www.vldb.org/pvldb/vol8/p1792-Akidau.pdf>（Akidau et al., VLDB 2015）；本地 `/tmp/fp05-dataflow.pdf`，文本抽取 `/tmp/fp05-dataflow.txt`；已打开；正文引用 §1.2、§1.3、§2.2、§2.3。
14. **Naiad/Differential Dataflow 论文**：<https://www.microsoft.com/en-us/research/publication/naiad-a-timely-dataflow-system/>（出版页 URL 本次 reader 未能打开；正文不依赖该页面）；本地官方论文 PDF/text `/tmp/fp05-dataflow/naiad.pdf`、`naiad.txt` 已打开并作为正文依据；正文引用论文 §2–§4。
15. **Differential Dataflow Rust**：<https://github.com/TimelyDataflow/differential-dataflow/tree/aa8745f93ea8abe131104fc7885ba4fd47e63902>、`README.md`、`differential-dataflow/src/collection.rs`、`difference.rs`；本地 clone `/tmp/fp05-differential`；已打开。
16. **Materialize 官方架构文章**：<https://materialize.com/blog/materialize-architecture/>；本地 `/tmp/fp05-dataflow/mz-materialize_com_blog_materialize_architecture_.txt`；已打开。
17. **Materialize 官方 `SUBSCRIBE` 文档**：<https://materialize.com/docs/sql/subscribe/>；本地 `/tmp/fp05-dataflow/mz-materialize_com_docs_sql_subscribe_.txt`；已打开；正文引用 output/progress/durable subscription/AS OF 段落。
18. **TradingView Pine 官方执行模型**：<https://www.tradingview.com/pine-script-docs/language/execution-model/>；本地 `/tmp/fp05-pine-execution.html`、`/tmp/fp05-pine-execution.txt`；已打开。
19. **TradingView Pine 官方 alerts/limitations**：<https://www.tradingview.com/pine-script-docs/concepts/alerts/>、<https://www.tradingview.com/pine-script-docs/writing/limitations/>；本地 `/tmp/fp05-pine-alerts.html`、`/tmp/fp05-pine-alerts.txt`、`/tmp/fp05docs/pine-limits.txt`；已打开。
20. **Reactive Streams JVM 1.0.4**：<https://github.com/reactive-streams/reactive-streams-jvm/tree/a625d3aba756e9842ad1291a5b73f5db280b6168>、`README.md`、`api/src/main/java/org/reactivestreams/{Publisher,Subscriber,Subscription,Processor}.java`；本地 clone `/tmp/fp05-reactive-streams`；已打开。
21. **Akka Streams**：<https://github.com/akka/akka/tree/e0f2eff299576fbfc06052e5c6a28b51ef160aee>、`akka-stream/src/main/scala/akka/stream/stage/GraphStage.scala`；官方文档 <https://doc.akka.io/libraries/akka-core/current/stream/stream-design.html>、`stream-composition.html`；本地 clone `/tmp/fp05-akka` 与 captures `/tmp/fp05docs/akka-design.txt`、`akka-composition.txt`；已打开。
