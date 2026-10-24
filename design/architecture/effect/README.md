# 效应

负责把副作用请求记成 ``Request`EffectRequest`` 并交给经 ``Request`Register`` 登记的种类处理，以及把效应侧的全部进展记成只追加的 ``Request`Requests.events``（``Request``）；负责核心内唯一的外部写发出点：从获准的 ``Write`Intent`` 到 ``Write`Barrier``、发出、结果未知与证据收敛（``Write``）；不负责意图怎样形成与审批，不负责解释来源对写的回应意味着什么。

## 依据

核心内与下游的请求方需要以 ``Request`EffectRequest`` 请求核心预先不认识的种类（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以 ``Request`EffectKind`` 由 ``Request`Register`` 登记而不由核心穷举，没有登记的种类的请求停在 ``Request`RequestStatus`` 的 `<Unhandled>` 而不是报错。

请求先持久化、处理在其后，不存在同步返回值；请求方与重启恢复都只能读持久记录，而效应侧的事实不能被压缩或撤回（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以答复与写尝试的每次迁移都经 ``Request`Record`` 追加到 ``Request`Requests.events``，并以 ``Write`Intent.about`` 关联回它所响应的请求，请求方按位置 ``Request`ReadEvents`` 读回；事件序列不进观察日志。

外部写不可逆，且写请求可能在到达上游之后、回执返回之前丢失回执（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Write`Barrier`` 在调用集成之前持久落下屏障，``Write`Attempt.barriered`` 为假的尝试确知未发出，为真而无后继的尝试只能进入 ``Write`Outcome`` 的 `<Unknown>`，由 ``Write`Resolve`` 与 ``Write`Attribute`` 以证据收敛，核心从不自动重发。

同一品种上的后续写依赖前一笔的结果（来源：交易语义约束，证据见附录 `evidence/lane-facts.md`）。所以 ``Write`BlockingHeads`` 在前一笔等待仍为 `<Active>` 时阻塞同通道的普通准入，不论它是否已过屏障；撤阻塞头的撤单以 ``Write`CancelHead`` 准入，``Write`Admit`` 那一刻它必须指向结果未知的阻塞头的调用方键，且同一通道同时至多一笔未决的撤阻塞头尝试；之后两次尝试各自收敛，原尝试先结束不影响撤单尝试。

## 为什么

等待轴与结果轴分开记录：``Write`Waiting`` 回答核心还要不要等，源头是核心自己；``Write`Outcome`` 回答写在来源发生没有，源头是来源。不选合成一个阶段枚举：合成后"放弃等待"与"确认未发生"无法区分，而前者只解除本地阻塞，后者是上游事实。

``Write`Abandon`` 不产生任何结果：它把尝试移出阻塞头集合，结果仍可由迟到的证据补上。不选把放弃记为 `<NotSent>`：核心无权替上游断言。

调用方键是尝试身份的单射 ``Write`KeyOf``，在 ``Write`Prepare`` 里先于编码铸出并交给 ``Mapping`Encode``：同一意图重新准入是新的尝试，必须拿到新键，否则来源的幂等去重会把两次尝试混为一次。不选先编码再铸键：请求原文里就没有键。

未过屏障的尝试留在 ``Write`BlockingHeads``：从 ``Write`Prepare`` 提交到 ``Write`Barrier`` 落盘之间有窗口，不占住通道，同通道第二笔意图会在窗口内准入。它的出口是 ``Write`Expire``：到期、来源能力已明确为 `<Unsupported>` 或人工丢弃都走它，因为它确知未发出。

``Request`Requests.events`` 不进 ``Observation`Journal``：观察日志可经 ``Observation`AdvanceRetention`` 删、可经 ``Observation`Entry.supersedes`` 撤、有 ``Observation`LogPosition.epoch`` 分代，事件序列三者都没有；不选让 ``Delivery`` 同时订阅两类流，那会让只为 ``Delivery`DeliveryGap`` 存在的投递逻辑背上永远不触发的分支。

``Write`Admission`` 随 ``Write`Intent`` 一起进入写边界而不是 ``Write`Prepare`` 的参数：越过阻塞的决定和意图本身一起形成、一起被记录，发出点不另收指令。

## 语义

``Write`Barrier`` 的屏障记录先于对集成的调用持久写下；核心进程崩溃重启后，``Write`Attempt.barriered`` 为真且结果仍为 `<Pending>` 的尝试经 ``Write`CompleteSend`` 以 `<NoResponse>` 记为 `<Unknown>`，为假的尝试在对账启动之后重新过 ``Write`Barrier``。处理器在核心进程内运行；写处理器把请求转成 ``Write`Intent`` 的路径由业务决定。尝试进入 `<Unknown>` 时以及带调用方键的观察入账时对 `<Unknown>` 尝试调用 ``Write`Attribute``；仍未查清的尝试，核心按来源可用的回读渠道周期调用 ``Write`ProbeAttempt``，渠道种类、顺序与调用间隔由实现决定，索引见附录 `reference/original-design-index.md`。

每个 ``Write`Attempt.request`` 至多发往来源一次，崩溃重启之后或久无回复时也不再发；同一个 ``Write`Intent`` 要再发，只能再经 ``Write`Prepare`` 成为新的 ``Write`AttemptRef``。
