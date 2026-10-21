# 单据

单据是写意图形成期的抽象：一把由负责人持有的锁、一条线性版本链、绑定版本的裁决，以及把获准意图交给写边界的唯一交出点；不负责意图发出之后的任何事。

## 依据

待决意图在送审期间可能被修订，而并发决定者可能基于过期版本作出批准（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`TicketRecord.status`` 在 ``Ticket`Awaiting`` 期间冻结版本，``Ticket`Revise`` 在送审中被拒，裁决只能由 ``Ticket`SendBack`` 退回草稿后再修订，``Ticket`Decide`` 在期望版本不符或同版本已决时返回 `<Conflict>`。

从起单到放行可能相隔很久，放行时依据的观察可能已被撤回、压缩或落后于流末，而来源能力可能已变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Release`` 在放行时刻重新核对 ``Observation`CheckBasis`` 与调用方给出的 ``Ticket`Alignment``，任一必要项不对齐即不放行；购买力、可交易性等资金面项只作为读入的对账项，核心不自行判断。

谁有权裁决、哪些对账项必须对齐、是否自动批准，会随需求改变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Decision.decider`` 只记录是谁，``Ticket`Alignment`` 只记录对齐与否；权限与必要项留在代码里。

## 需求

- 一张单可以被退回修改后再批准，批准的一定是裁决者看过的那个版本（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`），由 ``Ticket`SendBack`` 与 ``Ticket`Decide`` 走通。
- 同一张单据只放行一次，依据过期或到期之后不放行（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`），由 ``Ticket`Release`` 走通。
- 程序发出的写请求必须先成为一张由装载人负责的单据（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`），由 ``Ticket`AdoptRequest`` 走通。

## 为什么

不选让 ``Trading`` 与 ``Program`` 各自记录 ``Ticket`Decision``：一张单据的裁决会散在两处。也不选把 ``Ticket`Decision`` 放进 ``Write``：``Write`Intent`` 只需要一个获准的意图。

单据不持有通道键：通道由 ``Mapping`Encode`` 在放行时从当前版本的意图导出，单据在修订后不必重算它。

## 语义

``Ticket`Release`` 调用 ``Write`Prepare`` 成功后在同一次提交中以 ``Ticket`Prepared`` 关闭单据；``Write`Prepare`` 返回 `<LaneBlocked>` 或 `<Unmapped>` 时单据保持送审状态，由实现按节奏重试 ``Ticket`Release``。对账项 ``Ticket`Alignment`` 由业务在放行时刻对当前观察或一次性读计算后传入，计算发生在核心进程内。
