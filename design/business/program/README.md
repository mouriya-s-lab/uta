# 程序运行

负责确定性程序按输入观察流游标推进检查点、产出派生观察并提交写意图单据的接入规律（``Program``）；不负责程序内部的指标算法、语法树结构与子进程宿主隔离机制。

## 依据

策略与指标程序由外部或 AI 编写，可能死循环、超内存或崩溃重启，且重启后需要从最近确认的位置继续计算而不丢失指标状态（来源：威胁模型与连续性要求，证据见附录 `evidence/venue-facts.md`）。所以 ``Program`Checkpoint`` 将程序状态绑定到输入观察流位置 ``Program`Checkpoint.cursorSeq``，且程序不直接调用外部写，只能经 ``Ticket`Draft`` 以装载人 ``Ticket`Principal`` 的身份提交单据草稿。

## 需求

- 程序完成一轮输入消费时，推进输入游标与持久检查点、将派生结果追加到输出观察流，并以装载人身份创建待审单据（来源：可编程派生与受控决策要求），由 ``Program`Advance`` 走通。

## 为什么

``Program`Advance`` 通过 ``Observation`AcquireRef`` 登记 ``Program`Checkpoint.cursorSeq`` 对应的观察位置，因为程序从检查点恢复时需要从该位置之后续读，保留边界不得在检查点推进前删去该位置。

## 语义

每个 ``Program`ProgramId`` 在独立的受监督宿主子进程中求值，不持有上游凭据且不直接访问存储；子进程崩溃或超预算退出时，未完成 ``Program`SaveCheckpoint`` 的推进不生效，重启后按 ``Program`Programs.checkpoints`` 中最近已提交的检查点恢复。
