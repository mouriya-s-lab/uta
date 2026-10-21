# 核心

核心位于集成与解释层之间：经集成收下来源信封并记成观察日志，持有效应请求与每一次外部写尝试的记录，向下游消费方交出订阅投递；不持有账户、订单、持仓与行情的原值，不解释来源原文，不持有下游的状态。

## 依据

账户、持仓、订单、成交与行情的原值由上游券商或交易所持有并裁决（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以核心只记意图及其推进记录，``Observation`Entry.raw`` 原样保存来源原文作证据，``Write`Outcome`` 的每一个已知值都来自来源经集成取回的副本。

提供商的类别、协议、是否提供 SDK 及 SDK 的语言无法预先确定，且上游协议何时改动无人知道（来源：多来源接入要求，证据见附录 `evidence/integration-facts.md`）。所以集成是外挂进程，只交出固定头加原文的 ``Mapping`Envelope``；核心按来源装载一份当前映射 ``Mapping`Mappings.current`` 抽出自己要用的字段，其余字段原样进出；映射不记版本，解不出记为 ``Observation`EntryKind`` 的 `<Unmapped>`，编不出返回 ``Mapping`EncodeOutcome`` 的 `<Unmapped>`，取证匹配不到一律为 ``Mapping`Evidence`` 的 `<Inconclusive>`。

上游推送流不存在通用断线续传游标，完备只能由来源给出的连续序号证明，且消费位置、完备进度与本地保留边界各自独立变化（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Observation`StreamId.epoch`` 在每次来源连续性中断时开新一代并以 `<SourceGap>` 记录起点，``Observation`Journal.coverage`` 只按来源序号区间推进且 ``Mapping`Ingest`` 只在来源声明 ``Source`StreamDecl.venueSeq`` 时推进它，``Observation`LogPosition.seqNo`` 由核心分配而不取来源序号，消费位置另由 ``Delivery`Subscription.cursor`` 持有。

外部写的结果可能未知：网络超时、进程崩溃或连接中断可能发生在写请求已到达上游之后、回执返回之前（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Write`Attempt`` 把等待轴 ``Write`Waiting`` 与结果轴 ``Write`Outcome`` 分开记录，``Write`Barrier`` 以持久屏障把崩溃窗口二分。

同一品种上的后续写的语义依赖前一笔的结果，而资金面能否成交只由上游裁决，本地没有判断的位置（来源：交易语义约束，证据见附录 `evidence/lane-facts.md`）。所以 ``Mapping`LaneKey`` 由映射从意图按品种导出，``Write`BlockingHeads`` 只在同一通道内阻塞，购买力等资金面核对只作为放行前的读。

部分能力在握手前未知，且只在会话在线时有效（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Mapping`CurrentCapability`` 由映射的静态声明 ``Mapping`Mappings.static`` 与会话有效声明 ``Source`Sources.live`` 合成，离线一律为 `<Unavailable>`。

下游有多个消费方，各自与核心生命周期独立，解释层不持有状态（来源：多来源接入要求，证据见附录 `evidence/integration-facts.md`）。所以 ``Request`Principal`` 只作为行动主体的不透明身份进入核心，持久订阅 ``Delivery`Subscriptions`` 归核心所有而不随下游连接断开消失。

## 为什么

观察与效应是两套独立的抽象：观察侧可撤回、可压缩，效应侧只追加；唯一的边是效应侧读观察侧（``Write`Intent.basis`` 引用 ``Observation`LogPosition``），观察侧不引用效应侧的任何类型。不选把两者合成一套记录：派生值的撤回不能当作外部写的补偿。

不让集成把上游协议清洗成核心契约值：契约从哪里来没有稳定答案，契约一变就要改每个集成进程；核心只认信封里抽出的字段，改映射比改集成便宜，而下游仍然拿得到原文。

映射住在核心而不住在集成：核心能够离线核验映射，集成进程不含任何 UTA 语义；不选在集成进程内求值映射，那样核心无法独立核验，集成作者要同时理解上游与 UTA。

``Mapping`` 依赖 ``Observation`` 而不是反过来：观察日志对外部流与派生流一视同仁，不读映射的任何状态；入账 ``Mapping`Ingest`` 是先解码再追加的场景，归映射持有。不选让 ``Observation`` 调用解码：那会让读侧底座依赖含写侧编码与取证分类的模块。

映射不记版本：无人知道上游何时改协议，为映射版本建立与会话或记录的绑定只换来多版本并存的复杂度；失配直接报错，在途的结果未知尝试只会得到 `<Inconclusive>` 而不会被误判。

``Delivery`` 单独成子模块：订阅归消费方、日志归核心，两者权威不同；不选把订阅游标放进 ``Observation`Journal``，那会让日志的不变量混入消费方的进度。

效应请求是值，不是调用：``Request`EffectRequest`` 追加为记录，答复作为流事件回投请求方；不选代数效应式的通用续接，续接函数随程序状态持久化会带来多次恢复时的资源泄漏，而流事件匹配不需要它。

## 语义

每个 ``Source`SourceId`` 对应一个独立的集成子进程与会话通道；``Observation`Journal``、``Write`Attempts``、``Request`Requests``、``Delivery`Subscriptions`` 都在核心进程内持久保存。核心进程重启后 ``Source`Sources.live`` 从空重建，直到各集成子进程重新握手；``Source`Sources.latest`` 保留。映射由核心在启动时与运维动作时装载，替换映射不开新会话。解释层在核心之外：它把下游连接上认证过的主体作为 ``Request`Principal`` 交进核心，把核心的记录与状态翻译成下游接口；核心不记下游连接，也不认证主体。
