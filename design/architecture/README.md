# 核心

核心位于集成与解释层之间：经集成收下来源信封并记成观察日志，持有效应请求与每一次外部写尝试的记录，向下游消费方交出订阅投递；不持有账户、订单、持仓与行情的原值，不解释来源原文，不持有下游的状态。

## 依据

账户、持仓、订单、成交与行情的原值由上游券商或交易所持有并裁决（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以核心只记意图及其推进记录，``Observation`Entry.raw`` 原样保存来源原文作证据，``Write`Outcome`` 的每一个已知值都来自来源经集成取回的副本。

上游的类别与协议无法预先确定，也无法预先知道它何时改动（来源：多来源接入要求，证据见附录 `evidence/integration-facts.md`）。所以核心只接受 ``Mapping`Envelope``，不接受任何按上游协议定义的结构；核心按来源装载一份 ``Mapping`Mappings.current`` 抽出自己要用的 ``Observation`Decoded``，其余留在 ``Observation`Entry.raw`` 原样进出；``Mapping`Mappings`` 不记版本，解不出记为 ``Observation`EntryKind`` 的 `<Unmapped>`，编不出返回 ``Mapping`EncodeOutcome`` 的 `<Unmapped>`，``Mapping`ClassifyEvidence`` 匹配不到一律为 `<Inconclusive>`。

上游推送流不存在通用断线续传游标，完备只能由来源给出的连续序号证明，且消费位置、完备进度与本地保留边界各自独立变化（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Observation`LogPosition.epoch`` 在每次来源连续性中断时开新一代并以 `<SourceGap>` 记录起点，``Observation`LogPosition.seqNo`` 由核心分配而不取来源序号，消费位置另由 ``Delivery`Subscription.cursor`` 持有；按来源序号判完备与回填属实现，索引见附录 `reference/original-design-index.md`。

外部写的结果可能未知：网络超时、进程崩溃或连接中断可能发生在写请求已到达上游之后、回执返回之前（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Write`Attempt`` 把等待轴 ``Write`Waiting`` 与结果轴 ``Write`Outcome`` 分开记录，``Write`Barrier`` 以持久屏障把崩溃窗口二分。

同一品种上的后续写的语义依赖前一笔的结果，而资金面能否成交只由上游裁决，本地没有判断的位置（来源：交易语义约束，证据见附录 `evidence/lane-facts.md`）。所以 ``Mapping`LaneKey`` 由映射从意图按品种导出，``Write`BlockingHeads`` 只在同一通道内阻塞，购买力等资金面核对只作为放行前的读。

部分能力在握手前未知，且只在会话在线时有效（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Source`SessionCapability`` 只读 ``Source`Sources.live``，离线一律为 `<Unavailable>`；``Source`Sources.latest`` 只供离线查阅最近声明，从不代替此刻的能力。

下游有多个消费方，各自与核心生命周期独立，解释层不持有状态（来源：多来源接入要求，证据见附录 `evidence/integration-facts.md`）。所以 ``Request`Principal`` 只作为行动主体的不透明身份进入核心，持久订阅 ``Delivery`Subscriptions`` 归核心所有而不随下游连接断开消失。

## 为什么

观察与效应是两套独立的抽象：观察侧可撤回、可压缩，效应侧只追加；唯一的边是效应侧读观察侧（``Write`Attribute`` 读 ``Observation`ByClientKey``，``Request`EffectRequest.basis`` 引用 ``Observation`LogPosition``），观察侧不引用效应侧的任何类型。不选把两者合成一套记录：派生值的撤回不能当作外部写的补偿。

不让与来源对接的进程把来源协议翻译成核心定义的结构：那套结构从哪里来没有稳定答案，它一变就要改每个对接进程；核心只认 ``Observation`Decoded`` 里抽出的字段，改 ``Mapping`Mappings.current`` 比改对接进程便宜，而下游仍然拿得到 ``Observation`Entry.raw``。

``Mapping`Decode`` 与 ``Mapping`Encode`` 在核心进程内求值而不在对接进程内：核心能够离线核验它们，对接进程不含任何核心的语义；不选在对接进程内求值，那样核心无法独立核验，对接的作者要同时理解来源与核心。

``Mapping`` 依赖 ``Observation`` 而不是反过来：观察日志对外部流与派生流一视同仁，不读映射的任何状态；入账 ``Mapping`Ingest`` 是先解码再追加的场景，归映射持有。不选让 ``Observation`` 调用解码：那会让读侧底座依赖含写侧编码与取证分类的模块。

``Mapping`Mappings.current`` 每个来源只有一份：若保留多份，每条 ``Observation`Entry`` 与每个 ``Write`Attempt`` 都得记下它产生时用的是哪一份，替换之后两套解释并存。替换由运维在核心运行中完成，不重开 ``Source`` 会话、不改在途的 ``Write`Attempt``；旧映射下发出、结果未知的尝试改用新映射取证，匹配不到得到 `<Inconclusive>` 而不是被判为未发生。

``Delivery`` 单独成子模块：订阅归消费方、日志归核心，两者权威不同；不选把订阅游标放进 ``Observation`Journal``，那会让日志的不变量混入消费方的进度。

效应请求是值，不是调用：``Request`EffectRequest`` 追加为记录，答复与效应侧的后续进展经 ``Request`Record`` 追加到 ``Request`Requests.events``，请求方按位置读回；不选代数效应式的通用续接，续接函数随程序状态持久化会带来多次恢复时的资源泄漏，而按位置读事件不需要它。

``Observation`StreamId`` 不含分代，分代只在 ``Observation`LogPosition.epoch`` 与 ``Observation`Cursor.epoch`` 上：订阅与程序检查点绑定逻辑流，来源中断开新一代后 ``Observation`ReadFrom`` 跨代按先后交出记录，订阅方先收到新一代的 `<SourceGap>` 再收到数据。不选让订阅绑定带分代的流身份：开新代后订阅方无从得知新代，停在旧代末尾。

## 语义

每个 ``Source`SourceId`` 对应一个独立的集成子进程与会话通道；``Observation`Journal``、``Write`Attempts``、``Request`Requests``、``Delivery`Subscriptions`` 都在核心进程内持久保存。核心进程重启后 ``Source`Sources.live`` 从空重建，直到各集成子进程重新握手；``Source`Sources.latest`` 保留。映射由核心在启动时与运维动作时装载，替换映射不开新会话。解释层在核心之外：它把下游连接上认证过的主体作为 ``Request`Principal`` 交进核心，把核心的记录与状态翻译成下游接口；核心不记下游连接，也不认证主体。
