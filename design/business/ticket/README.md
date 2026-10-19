# 单据与审批

负责外部写意图在进入 ``Effect`` 写边界之前的责任持有、线性版本修订、绑定版本的审批裁决与依据新鲜度核对（``Ticket``）；不负责具体风控阈值与允许集合的业务规则配置，也不负责越过写边界之后的外部调用。

## 依据

待决意图在送审期间可能被修订，而并发决定者可能基于过期版本作出批准（来源：既有系统审计与版本冲突调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Ticket`TicketRecord`` 维护负责人 ``Ticket`TicketRecord.responsible`` 与当前版本 ``Ticket`TicketRecord.version``，``Ticket`Revise`` 推进版本时自动清空旧裁决，``Ticket`Decide`` 在期望版本不符或同版本已决时返回 `<Conflict>`。

## 需求

- 发起人创建单据草稿并由唯一负责人修订，非负责人或已关闭单据的修订被拒绝（来源：协作与责任隔离要求），由 ``Ticket`Draft`` 与 ``Ticket`Revise`` 走通。
- 审批决定必须绑定单据的当前版本，对同一版本的第二次决定或旧版本决定返回冲突且不改变单据状态（来源：审计可追溯与并发冲突要求），由 ``Ticket`Decide`` 走通。
- 获批单据在放行进入写边界前核对观察依据仍然新鲜且来源当前会话支持该能力，交出后单据关闭且不可再次放行（来源：放行门安全要求），由 ``Ticket`Release`` 走通。

## 为什么

单据的编辑锁直接表示为 ``Ticket`TicketRecord.responsible`` 字段而非互斥原语，因为意图形成期可以跨小时或跨会话持续，按责任主体校验 ``Ticket`Revise`` 不会产生死锁或超时释放竞态。

放行操作 ``Ticket`Release`` 同时调用 ``Observation`CheckBasis`` 与 ``Source`CurrentCapability``，因为审批等待期间观察流可能撤回旧派生值、推进保留边界或断开来源会话，不能凭起单时的旧状态直接进入 ``Effect`Prepare``。
