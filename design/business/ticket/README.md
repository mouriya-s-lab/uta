# 单据

单据是写意图形成期的抽象：一把由负责人持有的锁、一条线性版本链、绑定版本的裁决，以及把获准意图交给写边界的唯一交出点；不负责意图发出之后的任何事。

## 依据

待决意图在送审期间可能被修订，而另一个裁决者可能基于过期版本作出批准（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`TicketRecord.status`` 在 ``Ticket`Awaiting`` 与 ``Ticket`Approved`` 期间冻结版本，``Ticket`Revise`` 只在草稿中可用，``Ticket`SendBack`` 退回草稿后才能修订，已裁决的版本必须先修订才能再送审，``Ticket`Decide`` 在期望版本不符或同版本已决时返回 `<Conflict>`；``Ticket`Version`` 把意图、依据、期限与准入方式一起绑进版本，退回后任一项都可改。

从起单到放行可能相隔很久，放行时依据的观察可能已被删、被取代或已换代，而来源能力可能已变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Release`` 只对 ``Ticket`Approved`` 的单据开门，在放行时刻重新核对 ``Observation`CheckBasis``、调用方给出的 ``Ticket`Alignment`` 与 ``Source`SessionCapability``：依据失效不可逆，以 `<BasisLost>` 关单；来源明确不支持以 `<Unsupported>` 关单；能力未确认、离线或对账项未对齐时等待；购买力与可交易性只作为读入的 ``Ticket`Alignment``，核心不自行判断。

谁能作出 ``Ticket`Decision``、``Ticket`Alignment`` 里哪些 ``Ticket`CheckKey`` 必须对齐、是否自动批准，会随需求改变（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`）。所以 ``Ticket`Decision.decider`` 只记录是谁，``Ticket`Alignment`` 只记录未对齐的项；谁有权、哪些项必要，不进 ``Ticket``。

## 需求

- 一张单可以被退回修改后再批准，批准的一定是裁决者看过的那个版本，批准之后不能再被退回（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`），由 ``Ticket`Draft``、``Ticket`SubmitForDecision``、``Ticket`SendBack``、``Ticket`Revise`` 与 ``Ticket`Decide`` 走通。
- 同一张单据只放行一次；到期、来源不支持或依据失效时以如实的原因关单，不冒充裁决（来源：审批并发约束，证据见附录 `evidence/approval-facts.md`），由 ``Ticket`Release`` 走通。
- 程序发出的写请求必须先成为一张由装载人负责的单据（来源：可编程派生要求，证据见附录 `evidence/integration-facts.md`），由 ``Ticket`AdoptRequest`` 走通。

## 为什么

不选把 ``Ticket`Decision`` 放进 ``Write``：``Write`Prepare`` 与 ``Write`Barrier`` 只处理 ``Write`Intent`` 的准入与发出，不关心它在送交之前改过几版、由哪个 ``Request`Principal`` 批准；把形成期的锁与版本链留在 ``Ticket``，各种 ``Request`EffectKind`` 共用同一套审批与放行规则。

``Ticket`Version.admission`` 随版本受审：批准的是带准入方式的意图，放行时不再另给参数。不选把准入方式作为 ``Ticket`Release`` 的参数：审批者批准的普通单据会在放行时被改成绕过。

单据不持有通道键：通道由 ``Mapping`Encode`` 在放行时从当前版本的意图导出，单据在修订后不必重算它。

能力未确认或离线时 ``Ticket`Release`` 等待而不否决：来源没有给出否定，否决记录却会说"来源不提供"，一次瞬断就会关闭已批准的单据。能力已明确为 `<Unsupported>` 或依据已失效时关单：前者来源已给出否定，后者永远不会再变新鲜，等待没有意义。

## 语义

``Ticket`Release`` 调用 ``Write`Prepare`` 成功后在同一次提交中以 ``Ticket`Prepared`` 关闭单据并经 ``Request`Record`` 记事；``Write`Prepare`` 返回 `<LaneBlocked>` 或 `<Unmapped>` 时单据保持已批准状态，由实现按节奏重试 ``Ticket`Release``。写处理器将 ``Request`EffectRequest.payload`` 解为 ``Ticket`Proposal`` 后调用 ``Ticket`AdoptRequest``。对账项 ``Ticket`Alignment`` 由业务在放行时刻对当前观察或一次性读计算后传入，具体检查目录属实现，索引见附录 `reference/original-design-index.md`。
