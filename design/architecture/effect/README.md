# 效应

负责把副作用请求记成值并交给注册的处理器（``Request``），以及核心内唯一的外部写发出点：从获准意图到持久屏障、发出、结果未知与证据收敛（``Write``）；不负责意图怎样形成与审批，不负责解释来源对写的回应意味着什么。

## 依据

程序与下游需要请求核心预先不认识的副作用：下单、发通知、拉一次历史、调外部模型（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以 ``Request`EffectKind`` 由处理器注册而不由核心穷举，处理器注册时声明 ``Request`HandlerClass``，没有处理器的请求停在 ``Request`RequestStatus`` 的 `<Unhandled>` 而不是报错。

外部写不可逆，且写请求可能在到达上游之后、回执返回之前丢失回执（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Write`Barrier`` 在调用集成之前持久落下屏障，``Write`Attempt.barriered`` 为假的尝试确知未发出，为真而无后继的尝试只能进入 ``Write`Outcome`` 的 `<Unknown>`，由 ``Write`Resolve`` 与 ``Write`Attribute`` 以证据收敛，核心从不自动重发。

同一品种上的后续写依赖前一笔的结果（来源：交易语义约束，证据见附录 `evidence/lane-facts.md`）。所以 ``Write`BlockingHeads`` 在前一笔等待仍为 `<Active>` 时阻塞同通道的普通准入；撤阻塞头的撤单以 ``Write`Admission`` 的 `<CancelHead>` 准入，并必须指向已过屏障的阻塞头的 ``Write`Attempt.key``。

## 为什么

等待轴与结果轴分开记录：``Write`Waiting`` 回答核心还要不要等，源头是核心自己；``Write`Outcome`` 回答写在来源发生没有，源头是来源。不选合成一个阶段枚举：合成后"放弃等待"与"确认未发生"无法区分，而前者只解除本地阻塞，后者是上游事实。

``Write`Abandon`` 不产生任何结果：它把尝试移出阻塞头集合，结果仍可由迟到的证据补上。不选把放弃记为 `<NotSent>`：核心无权替上游断言。

``Write`Attempt.key`` 由核心按 ``Write`AttemptRef`` 铸造而不由意图自带：同一意图重新准入是新的尝试，必须拿到新键，否则来源的幂等去重会把两次尝试混为一次。

`<Bypass>` 准入作为显式记录存在：运维确需越过阻塞时留下可审计的痕迹，而不是让调用方自证已知阻塞集合。

## 语义

``Write`Barrier`` 的屏障记录先于对集成的调用持久写下；核心进程崩溃重启后，``Write`Attempt.barriered`` 为真且结果仍为 `<Pending>` 的尝试记为 `<Unknown>`，为假的尝试在对账启动之后重新过 ``Write`Barrier``。处理器在核心进程内运行；写处理器把请求转成 ``Write`Intent`` 的路径由业务决定。对结果未知的尝试，核心按来源声明的取证渠道周期调用 ``Write`Resolve``，调用间隔由实现决定。
