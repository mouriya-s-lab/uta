# 程序

负责确定性程序的装载、按输入订阅推进检查点、产出派生观察与效应请求；不负责程序内部怎样计算，也不负责请求的处理。

## 依据

策略与指标程序由外部或 AI 编写，可能死循环、超内存或崩溃重启，重启后需要从最近确认的位置继续而不丢失状态（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以 ``Program`Checkpoint`` 把程序状态绑定到各输入流的已消费位置，``Program`Advance`` 在同一次提交中保存检查点、确认订阅游标并追加派生记录；程序不是行动主体，``Request`EffectRequest.member`` 取装载人 ``Program`Loaded.loadedBy``。

程序需要请求核心预先不认识的副作用（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`）。所以程序的唯一出口是 ``Request`Emit``，答复与执行事实作为流事件经 ``Delivery`Deliver`` 回到程序的下一步推进。

## 需求

- 以装载人身份装载程序并建立输入订阅：``Program`Load``。
- 消费一批投递项、推进检查点、追加派生记录并发出请求：``Program`Advance``。
- 卸载程序并释放其检查点持有的保留引用：``Program`Unload``。

## 为什么

检查点与订阅游标同一次提交：两者分开提交时，崩溃恢复会出现"游标已确认但状态未保存"或相反，程序要么重复消费要么漏消费。不选在检查点内完整快照输入窗口：输入窗口的大小由程序决定，而保留引用只需保护游标位置之后的记录。

## 语义

程序在受监督的子进程中运行，核心按 ``Program`Loaded.subscription`` 投递并收回 ``Program`Step``；子进程崩溃后核心从 ``Program`Loaded.checkpoint`` 重新装载。``Observation`AcquireRef`` 的持有者是程序身份，检查点替换时引用随之替换，卸载时释放。
