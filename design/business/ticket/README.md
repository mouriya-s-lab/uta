# 单据

单据是写意图在进入写边界之前的形成期：只有 ``Ticket`TicketRecord.responsible`` 能改版本链，裁决绑定版本，放行时最后核对一次；不负责谁有权裁决、哪些条件必须满足、意图从哪里来，也不负责放行之后的任何事。

## 依据

待决意图在送审期间可能被修订，而另一个裁决者可能基于过期版本作出批准（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`TicketRecord.status`` 在 ``Ticket`Awaiting`` 与 ``Ticket`Approved`` 期间冻结版本，``Ticket`Decide`` 在期望版本不符或同版本已决时返回 `<Conflict>`；``Ticket`Version`` 把意图、依据、期限与准入方式一起绑进版本。

从起单到放行可能相隔很久，放行时依据的观察可能已被删、被取代或已换代，而来源能力可能已变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Release`` 在放行时刻重新经 ``Observation`CheckBasis`` 与 ``Source`SessionCapability`` 核对，``Ticket`Gate`` 给出 `<Lapsed>` 时关单。

谁能裁决、放行前哪些条件必须满足、是否自动批准，会随需求改变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Decision.decider`` 只记录是谁；条件写在 ``Ticket`Version.intent`` 里随版本受审，由业务在放行时刻求值后以对齐与否交给 ``Ticket`Release``。

## 需求

- 批准的一定是裁决者看过的那个版本（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`），由 ``Ticket`Decide`` 与 ``Ticket`Tickets`` 的不变式保证。
- 同一张单据只放行一次，依据失效、到期或来源不支持时不放行（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`），由 ``Ticket`Release`` 走通。

## 为什么

撤阻塞头会越过一笔结果未知的写，裁决者必须在批准时就看到它，所以 ``Write`Admission`` 放在 ``Ticket`Version`` 里随版本受审；放到放行时再给，批准的普通单据就能在放行时变成撤阻塞头。

单据不持有通道键：通道由 ``Mapping`Encode`` 在放行时从当前版本的意图导出，单据在修订后不必重算它。

``Ticket`Gate`` 给出 `<Hold>` 时单据保持已批准：能力未确认或离线只说明此刻不能确认，不说明来源不支持；一次瞬断不应关闭已审批的单据。

## 语义

``Ticket`Release`` 调用 ``Write`Prepare`` 成功后在同一次提交中以 ``Ticket`Prepared`` 关闭单据；``Write`Prepare`` 返回 `<LaneBlocked>` 或 `<Unmapped>` 时单据保持已批准，由业务按自己的节奏再次调用 ``Ticket`Release``。
