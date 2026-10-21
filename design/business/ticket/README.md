# 单据

单据是写意图形成期的抽象：一把由负责人持有的锁、一条线性版本链、绑定版本的裁决，以及把获准意图交给写边界的唯一交出点；不负责意图发出之后的任何事。

## 依据

待决意图在送审期间可能被修订，而并发决定者可能基于过期版本作出批准（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`TicketRecord.status`` 在 ``Ticket`Awaiting`` 期间冻结版本，``Ticket`Revise`` 在送审中被拒，裁决只能由 ``Ticket`SendBack`` 退回草稿后再修订，``Ticket`Decide`` 在期望版本不符或同版本已决时返回 `<Conflict>`。

从起单到放行可能相隔很久，放行时依据的观察可能已被撤回、压缩或落后于流末，而来源能力可能已变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Release`` 在放行时刻重新核对 ``Observation`CheckBasis`` 与调用方给出的 ``Ticket`Alignment``，任一必要项不对齐即不放行；购买力、可交易性等资金面项只作为读入的对账项，核心不自行判断。

## 需求

- 负责人起草、修订、送审并可撤回单据，送审中的版本不被修改：``Ticket`Draft``、``Ticket`Revise``、``Ticket`SubmitForDecision``、``Ticket`Withdraw``。
- 裁决者对指定版本作出裁决或退回修改，裁决者身份随裁决记录：``Ticket`Decide``、``Ticket`SendBack``。
- 放行前确认当前版本已批准、依据未失效、必要项对齐、未过期且通道可准入，然后一次性交给写边界并关闭单据：``Ticket`Release``。
- 写处理器接手程序或下游以效应请求发出的写意图，以请求发出者为负责人起草并送审，再以单据身份答复请求：``Ticket`AdoptRequest``。

## 为什么

锁与版本链放在同一张单据上：负责人是唯一能改版本链的人，裁决绑定版本号，三者共享"裁决版本不超过版本链长度"这条不变量。不选让每个业务各自维护审批状态：审批、放行门与通道准入是所有外部写共有的形成期规律。

是否允许起单人自批、哪些对账项是必要项、自动审批的阈值，都是随需求变动的策略，留在代码里；单据只提供裁决者身份与对账项的槽位。

单据不持有通道键：通道由 ``Mapping`Encode`` 在放行时从当前版本的意图导出，单据在修订后不必重算它。

## 语义

``Ticket`Release`` 调用 ``Write`Prepare`` 成功后在同一次提交中以 ``Ticket`Prepared`` 关闭单据；``Write`Prepare`` 返回 `<LaneBlocked>` 或 `<Unmapped>` 时单据保持送审状态，由实现按节奏重试 ``Ticket`Release``。对账项 ``Ticket`Alignment`` 由业务在放行时刻对当前观察或一次性读计算后传入，计算发生在核心进程内。
