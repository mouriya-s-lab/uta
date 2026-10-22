# 程序

负责确定性程序的装载、按输入订阅推进检查点、产出派生观察与效应请求；不负责程序内部怎样计算，也不负责请求的处理。

## 依据

策略与指标程序由外部或 AI 编写，可能死循环、超内存或崩溃重启，重启后需要从最近确认的位置继续而不丢失状态（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以 ``Program`Checkpoint`` 把程序状态绑定到各输入流的已消费游标与效应事件的已消费位置，``Program`AdvanceOn`` 在同一次提交中保存检查点、确认订阅游标并追加派生记录；程序不是行动主体，``Request`EffectRequest.member`` 取装载人 ``Program`Loaded.loadedBy``。

程序需要请求核心预先不认识的副作用，并要看到请求之后单据与写尝试的结局才能组合多步写（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以程序的唯一出口是 ``Request`Emit``，答复与后续进展经 ``Program`Events`` 从 ``Request`Requests.events`` 按位置读回。

## 需求

- 程序不是行动主体，它发出的每条请求都要能追溯到装载它的人（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`），由 ``Program`Load`` 走通。
- 程序崩溃重启后从最近一次确认的位置继续，不重复消费、不漏消费，也不重复发出请求（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`），由 ``Program`Poll``、``Program`Events``、``Program`AdvanceOn`` 与 ``Program`AdvanceOnEvents`` 走通。
- 程序被卸载后，核心里不再有任何东西替它等待记录（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`），由 ``Program`Unload`` 走通。

## 为什么

``Program`Keep`` 与 ``Delivery`Ack`` 在 ``Program`AdvanceOn`` 的同一次提交里生效：两者分开提交时，崩溃恢复会出现已确认而未保存或已保存而未确认，程序要么重复消费要么漏消费。

程序与下游消费方用同一套机制面对压缩：都经 ``Delivery`DeliveryGap`` 得知自己被越过，核心不为程序保护观察位置。不选让程序登记引用阻止 ``Observation`AdvanceRetention``：慢程序会锁死整条流的压缩，而程序本来就要确定性地处理缺口。

``Program`AdvanceOn`` 一次只确认一条输入流：程序一步只对一条流上到达的记录作出反应，多条流在同一步里确认会让崩溃恢复时分不清哪条流的记录已经进入过程序状态。``Program`AdvanceOnEvents`` 不确认任何流，因为效应事件的消费位置在 ``Program`Checkpoint.reqCursor`` 里，不在订阅上。

## 语义

程序在受监督的子进程中运行，核心按 ``Program`Loaded.subscription`` 投递并收回 ``Program`Step``；子进程崩溃后核心从 ``Program`Loaded.checkpoint`` 重新装载。
